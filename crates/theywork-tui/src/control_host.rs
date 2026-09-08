//! Off-thread provider I/O. The rendering loop only copies the latest snapshot.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde_json::{json, Value};
use theywork_control::native::{NativeCommand, NativeProvider};
use theywork_control::{
    ControlClient, ControlConfig, ControlSnapshot, OperationReceipt, OperationStatus,
    PendingRequest,
};
use theywork_core::{Agent, ThreadIdentity, World};
use theywork_render::views::control::{
    Choice, Command, ControlStatus, Question, Request, TaskAccess,
};

use crate::connections::Connections;

#[derive(Clone, Default)]
pub struct Latest {
    pub status: ControlStatus,
    pub snapshot: Option<Arc<ControlSnapshot>>,
    pub project_aliases: BTreeMap<String, String>,
}

pub enum Outcome {
    Receipt {
        detail: String,
        clear_draft: bool,
    },
    Console {
        command: NativeCommand,
        clear_draft: bool,
    },
}

struct Job {
    command: Command,
    identity: Option<ThreadIdentity>,
    expected_turn: Option<String>,
}

pub struct Host {
    jobs: mpsc::Sender<Job>,
    outcomes: mpsc::Receiver<Outcome>,
    latest: Arc<Mutex<Latest>>,
}

impl Host {
    pub fn start(connections: Connections, directory: Option<PathBuf>, enabled: bool) -> Self {
        let (sender, jobs) = mpsc::channel();
        let (outcomes, receiver) = mpsc::channel();
        let latest = Arc::new(Mutex::new(Latest::default()));
        let shared = latest.clone();
        std::thread::spawn(move || {
            let mut backend = Backend::new(connections, directory, enabled);
            let mut next_probe = Instant::now();
            loop {
                if Instant::now() >= next_probe {
                    backend.refresh();
                    if let Ok(mut value) = shared.lock() {
                        *value = backend.latest();
                    }
                    next_probe = Instant::now() + Duration::from_secs(2);
                }
                match jobs.recv_timeout(Duration::from_millis(150)) {
                    Ok(job) => {
                        let result = backend.execute(job);
                        let outcome = result.unwrap_or_else(|error| Outcome::Receipt {
                            detail: format!("{error:#}"),
                            clear_draft: false,
                        });
                        if outcomes.send(outcome).is_err() {
                            break;
                        }
                        next_probe = Instant::now();
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Self {
            jobs: sender,
            outcomes: receiver,
            latest,
        }
    }

    pub fn latest(&self) -> Latest {
        self.latest
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }
    pub fn drain(&self) -> Vec<Outcome> {
        self.outcomes.try_iter().collect()
    }
    pub fn submit(&self, command: Command, world: &World) -> Result<()> {
        let worker = match &command {
            Command::Send { worker, .. }
            | Command::OpenNative { worker }
            | Command::Interrupt { worker }
            | Command::Reconnect { worker } => Some(worker),
            _ => None,
        };
        let identity = worker
            .and_then(|id| world.worker(id))
            .and_then(|worker| worker.identity.clone());
        if worker.is_some() {
            anyhow::ensure!(
                identity.is_some(),
                "This historical task has no verified native identity"
            );
        }
        let latest = self.latest();
        let expected_turn = identity.as_ref().and_then(|id| {
            latest
                .snapshot
                .as_ref()?
                .threads
                .get(&id.native_id)?
                .active_turn_id
                .clone()
        });
        self.jobs
            .send(Job {
                command,
                identity,
                expected_turn,
            })
            .context("Control worker stopped")?;
        Ok(())
    }
}

struct Backend {
    connections: Connections,
    config: Option<ControlConfig>,
    enabled: bool,
    codex: Option<NativeProvider>,
    claude: Option<NativeProvider>,
    codex_error: String,
    claude_error: String,
    client: Option<ControlClient>,
    snapshot: Option<ControlSnapshot>,
    native_tasks: BTreeMap<String, TaskAccess>,
    last_native_probe: Option<Instant>,
}

impl Backend {
    fn new(connections: Connections, directory: Option<PathBuf>, enabled: bool) -> Self {
        let config = directory.map(|directory| {
            // Independent source homes cannot share an owning app-server.
            let home = std::fs::canonicalize(&connections.codex_home)
                .unwrap_or_else(|_| connections.codex_home.clone());
            let hash = home
                .to_string_lossy()
                .bytes()
                .fold(0xcbf29ce484222325u64, |n, b| {
                    (n ^ b as u64).wrapping_mul(0x100000001b3)
                });
            ControlConfig::new(directory.join(format!("control/{hash:016x}")), home)
        });
        Self {
            connections,
            config,
            enabled,
            codex: None,
            claude: None,
            codex_error: String::new(),
            claude_error: String::new(),
            client: None,
            snapshot: None,
            native_tasks: BTreeMap::new(),
            last_native_probe: None,
        }
    }

    fn refresh(&mut self) {
        if !self.enabled {
            return;
        }
        if self
            .last_native_probe
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(15))
        {
            self.last_native_probe = Some(Instant::now());
            if self.connections.codex {
                match NativeProvider::detect(Agent::Codex, &self.connections.codex_home) {
                    Ok(provider) => {
                        self.codex = Some(provider);
                        self.codex_error.clear();
                    }
                    Err(error) => {
                        self.codex = None;
                        self.codex_error = error.to_string();
                    }
                }
            }
            if self.connections.claude {
                match NativeProvider::detect(Agent::Claude, &self.connections.claude_home) {
                    Ok(provider) => {
                        self.claude = Some(provider);
                        self.claude_error.clear();
                    }
                    Err(error) => {
                        self.claude = None;
                        self.claude_error = error.to_string();
                    }
                }
            }
        }
        if self.connections.codex && self.client.is_none() {
            self.client = self
                .config
                .clone()
                .and_then(|config| ControlClient::connect(config).ok());
            if self.client.is_none() && self.snapshot.is_none() {
                self.snapshot = self
                    .config
                    .clone()
                    .and_then(|config| ControlClient::saved_snapshot(config).ok());
            }
        }
        if let Some(client) = &self.client {
            match client.snapshot() {
                Ok(snapshot) => self.snapshot = Some(snapshot),
                Err(error) => {
                    self.codex_error = format!("Control disconnected: {error}");
                    self.client = None;
                    if let Some(snapshot) = &mut self.snapshot {
                        snapshot.connected = false;
                        snapshot.pending_requests.clear();
                        for thread in snapshot.threads.values_mut() {
                            thread.capabilities = Default::default();
                            thread.active_turn_id = None;
                            thread.status = "connection unavailable".into();
                        }
                    }
                }
            }
        }
        self.native_tasks.clear();
        if let Some(provider) = self
            .claude
            .as_ref()
            .filter(|provider| provider.background_supported)
        {
            match provider.claude_sessions() {
                Ok(sessions) => {
                    for session in sessions {
                        if session.kind != "background"
                            || session.pid.is_none_or(|pid| pid == 0)
                            || matches!(session.state.as_deref(), Some("failed" | "stopped"))
                        {
                            continue;
                        }
                        if let Some(id) = session.session_id {
                            let identity = ThreadIdentity::new(
                                Agent::Claude,
                                theywork_core::SourceId(
                                    provider.home.to_string_lossy().into_owned(),
                                ),
                                id,
                            );
                            self.native_tasks.insert(identity.worker_id().0, TaskAccess { native: true, description: "Live Claude background task · Enter opens its official console".into(), ..Default::default() });
                        }
                    }
                }
                Err(error) => self.claude_error = format!("Attach unavailable: {error}"),
            }
        }
    }

    fn latest(&self) -> Latest {
        let mut status = ControlStatus {
            tasks: self.native_tasks.clone(),
            ..Default::default()
        };
        status.can_start_codex =
            self.enabled && self.connections.codex && self.codex.is_some() && self.config.is_some();
        status.can_start_claude = self.enabled && self.connections.claude && self.claude.is_some();
        status.codex = if !self.enabled {
            "Demo · controls disabled".into()
        } else if !self.connections.codex {
            "Not selected · choose access with c".into()
        } else {
            format!(
                "{} · {}",
                self.codex
                    .as_ref()
                    .map_or(self.codex_error.as_str(), |p| p.version.as_str()),
                self.connections.codex_home.display()
            )
        };
        status.claude = if !self.enabled {
            "Demo · controls disabled".into()
        } else if !self.connections.claude {
            "Not selected · choose access with c".into()
        } else {
            format!(
                "{} · {}{}",
                self.claude
                    .as_ref()
                    .map_or(self.claude_error.as_str(), |p| p.version.as_str()),
                self.connections.claude_home.display(),
                if self.claude.as_ref().is_some_and(|p| p.background_supported) {
                    " · background attach"
                } else {
                    " · native foreground console"
                }
            )
        };
        if self.enabled && self.config.is_none() {
            status
                .codex
                .push_str(" · temporary view: enable Remember in c to create managed tasks");
        }
        if let Some(snapshot) = &self.snapshot {
            for thread in snapshot.threads.values() {
                status.tasks.insert(
                    thread.identity.worker_id().0,
                    TaskAccess {
                        send: snapshot.connected && thread.capabilities.send,
                        interrupt: snapshot.connected && thread.capabilities.interrupt,
                        native: false,
                        reconnect: thread.managed
                            && (!snapshot.connected || !thread.capabilities.send)
                            && thread.active_turn_id.is_none(),
                        description: format!(
                            "Managed Codex · {}{}",
                            thread.status,
                            if snapshot.connected {
                                ""
                            } else {
                                " · control disconnected"
                            }
                        ),
                    },
                );
            }
            if snapshot.connected {
                status.requests = snapshot
                    .pending_requests
                    .iter()
                    .filter(|request| !request.reply_sent)
                    .filter_map(|request| {
                        let thread = snapshot.threads.get(&request.thread_id)?;
                        let mut value =
                            request_view(request, thread.identity.worker_id(), &snapshot.events);
                        value.origin = format!(
                            "{} / {} · {}",
                            thread.project.display(),
                            thread.title,
                            thread.identity.native_id
                        );
                        Some(value)
                    })
                    .collect();
            }
        }
        let project_aliases = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .threads
                    .values()
                    .map(|thread| {
                        let raw = thread.project.to_string_lossy().into_owned();
                        let normalized = crate::normalize_cli_path(&thread.project)
                            .unwrap_or_else(|_| raw.clone());
                        (raw, normalized)
                    })
                    .collect()
            })
            .unwrap_or_default();
        Latest {
            status,
            snapshot: self
                .snapshot
                .as_ref()
                .map(|snapshot| Arc::new(snapshot.clone())),
            project_aliases,
        }
    }

    fn provider(&self, agent: Agent) -> Result<&NativeProvider> {
        anyhow::ensure!(self.enabled, "Demo tasks cannot start provider work");
        match agent {
            Agent::Codex => {
                anyhow::ensure!(
                    self.connections.codex,
                    "Select Codex access in Connections first (c)"
                );
                self.codex
                    .as_ref()
                    .context("Official Codex CLI unavailable; install it, then reopen Connections")
            }
            Agent::Claude => {
                anyhow::ensure!(
                    self.connections.claude,
                    "Select Claude access in Connections first (c)"
                );
                self.claude
                    .as_ref()
                    .context("Official Claude CLI unavailable; install it, then reopen Connections")
            }
        }
    }

    fn execute(&mut self, job: Job) -> Result<Outcome> {
        let clear_draft = matches!(job.command, Command::Start { .. } | Command::Send { .. });
        match job.command {
            Command::Sources => anyhow::bail!("Source selection belongs to the office interface"),
            Command::Login { provider } => {
                anyhow::ensure!(self.enabled, "Demo mode cannot launch provider login");
                let cwd = std::env::current_dir()?;
                let home = if provider == Agent::Codex {
                    &self.connections.codex_home
                } else {
                    &self.connections.claude_home
                };
                std::fs::create_dir_all(home)?;
                Ok(Outcome::Console {
                    command: NativeProvider::detect(provider, home)?.login(cwd)?,
                    clear_draft: false,
                })
            }
            Command::Start {
                provider: Agent::Claude,
                project,
                prompt,
            } => {
                return Ok(Outcome::Console {
                    command: self
                        .provider(Agent::Claude)?
                        .new_conversation(project, &prompt)?,
                    clear_draft: true,
                })
            }
            Command::OpenNative { .. } => {
                return Ok(Outcome::Console {
                    command: self
                        .provider(Agent::Claude)?
                        .attach(job.identity.as_ref().context("Native identity missing")?)?,
                    clear_draft: false,
                })
            }
            command => {
                self.provider(Agent::Codex)?;
                let op = theywork_control::operation_id()?;
                if matches!(command, Command::Start { .. } | Command::Reconnect { .. })
                    && self.client.is_none()
                {
                    let mut config = self.config.clone().context("This is a temporary view. Enable Remember in Connections to keep managed Codex tasks alive after the office closes")?;
                    config.codex_program = self.provider(Agent::Codex)?.program.clone();
                    self.client = Some(ControlClient::connect_or_spawn(
                        config,
                        std::env::current_exe()?,
                    )?);
                }
                let client = self.client.as_ref().context("The owning Codex runtime is not connected; existing history alone cannot be controlled")?;
                let native_id = job.identity.as_ref().map(|id| id.native_id.clone());
                if let Some(identity) = &job.identity {
                    let snapshot = self
                        .snapshot
                        .as_ref()
                        .context("No verified runtime snapshot")?;
                    anyhow::ensure!(
                        snapshot
                            .threads
                            .get(&identity.native_id)
                            .is_some_and(|thread| thread.identity == *identity),
                        "Task identity does not match this runtime"
                    );
                }
                let receipt: Result<OperationReceipt> = match command {
                    Command::Start {
                        project, prompt, ..
                    } => client.start_codex(Path::new(&project), prompt, &op),
                    Command::Send { prompt, .. } => client.send_codex(
                        native_id.context("Task identity missing")?,
                        prompt,
                        job.expected_turn,
                        &op,
                    ),
                    Command::Interrupt { .. } => client.interrupt_codex(
                        native_id.context("Task identity missing")?,
                        job.expected_turn.context("No active turn was selected")?,
                        &op,
                    ),
                    Command::Reconnect { .. } => {
                        client.reconnect_codex(native_id.context("Task identity missing")?, &op)
                    }
                    Command::Reply { request, response } => client.respond(request, response, &op),
                    _ => unreachable!(),
                };
                let receipt = receipt
                    .with_context(|| format!("Operation {op}; inspect before sending again"))?;
                let confirmed = receipt.status == OperationStatus::Confirmed;
                Ok(Outcome::Receipt {
                    detail: format!(
                        "{:?}: {} · operation {}",
                        receipt.status, receipt.detail, receipt.id
                    ),
                    clear_draft: clear_draft && confirmed,
                })
            }
        }
    }
}

fn request_view(
    request: &PendingRequest,
    worker: theywork_core::WorkerId,
    events: &[theywork_control::ControlEvent],
) -> Request {
    let mut choices = Vec::new();
    let mut questions = Vec::new();
    let mut detail = String::new();
    let title = match request.method.as_str() {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            let item_id = request.params.get("itemId").and_then(Value::as_str);
            let item = events
                .iter()
                .rev()
                .filter(|event| {
                    event.thread_id.as_deref() == Some(&request.thread_id)
                        && event.turn_id == request.turn_id
                })
                .find_map(|event| {
                    let item = event.params.get("item")?;
                    (item_id.is_some() && item.get("id").and_then(Value::as_str) == item_id)
                        .then_some(item)
                });
            for key in ["reason", "command", "cwd", "grantRoot"] {
                if let Some(value) = request
                    .params
                    .get(key)
                    .or_else(|| item.and_then(|item| item.get(key)))
                    .and_then(Value::as_str)
                {
                    detail.push_str(&format!("{key}: {value}\n"));
                }
            }
            let mut reviewable = if request.method == "item/commandExecution/requestApproval" {
                request
                    .params
                    .get("command")
                    .or_else(|| item.and_then(|item| item.get("command")))
                    .and_then(Value::as_str)
                    .is_some_and(|command| !command.trim().is_empty())
            } else {
                false
            };
            if request.method == "item/fileChange/requestApproval" {
                if let Some(changes) = item
                    .and_then(|item| item.get("changes"))
                    .and_then(Value::as_array)
                    .filter(|changes| !changes.is_empty())
                {
                    reviewable = changes.iter().all(|change| {
                        change.get("path").and_then(Value::as_str).is_some()
                            && change.get("diff").and_then(Value::as_str).is_some()
                    });
                    for change in changes {
                        detail.push_str(&format!(
                            "\nFile: {}\n{}\n",
                            change
                                .get("path")
                                .and_then(Value::as_str)
                                .unwrap_or("unavailable"),
                            change
                                .get("diff")
                                .and_then(Value::as_str)
                                .unwrap_or("Diff unavailable")
                        ));
                    }
                }
            }
            if detail.chars().count() > 65_536 {
                detail = detail.chars().take(65_536).collect();
                reviewable = false;
                detail.push_str("\n[Review limit reached; approval disabled because the complete change is not shown.]");
            }
            if !reviewable {
                detail.push_str("\nThe exact command or file changes are unavailable in this connection's retained history. Allow is disabled; inspect the source before approving.");
            }
            for (decision, label) in [
                ("accept", "Allow this request"),
                ("decline", "Decline"),
                ("cancel", "Cancel"),
            ] {
                if decision == "accept" && !reviewable {
                    continue;
                }
                if request
                    .params
                    .get("availableDecisions")
                    .and_then(Value::as_array)
                    .is_none_or(|values| values.contains(&json!(decision)))
                {
                    choices.push(Choice {
                        label: label.into(),
                        response: json!({"decision":decision}),
                    });
                }
            }
            "Approval requested"
        }
        "item/tool/requestUserInput" => {
            if let Some(items) = request.params.get("questions").and_then(Value::as_array) {
                questions = items
                    .iter()
                    .filter_map(|question| {
                        Some(Question {
                            id: question.get("id")?.as_str()?.into(),
                            prompt: question
                                .get("question")
                                .or_else(|| question.get("header"))?
                                .as_str()?
                                .into(),
                            options: question
                                .get("options")
                                .and_then(Value::as_array)
                                .map(|items| {
                                    items
                                        .iter()
                                        .filter_map(|option| {
                                            option.get("label")?.as_str().map(String::from)
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                            secret: question
                                .get("isSecret")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        })
                    })
                    .collect();
            }
            detail = "Tab moves between answers. Type freely or F6–F12 selects a listed option. F5 submits to this request.".into();
            "Your answer is needed"
        }
        "item/permissions/requestApproval" => {
            if let Some(permissions) = request.params.get("permissions") {
                detail = format!(
                    "Permissions requested for this turn:\n{}",
                    serde_json::to_string_pretty(permissions).unwrap_or_default()
                );
                choices.push(Choice {
                    label: "Grant requested permissions for this turn".into(),
                    response: json!({"permissions":permissions,"scope":"turn"}),
                });
                choices.push(Choice {
                    label: "Grant none".into(),
                    response: json!({"permissions":{},"scope":"turn"}),
                });
            }
            "Turn permissions requested"
        }
        "mcpServer/elicitation/request" => {
            detail = request
                .params
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("This service needs input in its own interface.")
                .into();
            for action in ["decline", "cancel"] {
                choices.push(Choice {
                    label: action.into(),
                    response: json!({"action":action}),
                });
            }
            "Service request · inspect in its source"
        }
        _ => {
            detail = "This provider request format is not supported. Inspect the original client; no answer will be fabricated.".into();
            "Unknown request"
        }
    };
    if !request.supported {
        choices.clear();
        questions.clear();
    }
    Request {
        id: request.id.clone(),
        origin: format!("Codex / {}", request.thread_id),
        worker,
        title: title.into(),
        detail,
        choices,
        questions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_controls_only_offer_native_single_request_decisions() {
        let pending = PendingRequest {
            id: "generation:7".into(),
            native_id: json!(7),
            thread_id: "thread".into(),
            turn_id: Some("turn".into()),
            method: "item/commandExecution/requestApproval".into(),
            params: json!({"availableDecisions":["decline","acceptForSession"],"command":"echo hello"}),
            received_at: 0,
            reply_sent: false,
            supported: true,
        };
        let request = request_view(&pending, theywork_core::WorkerId("worker".into()), &[]);
        assert_eq!(request.choices.len(), 1);
        assert_eq!(request.choices[0].response, json!({"decision":"decline"}));
        assert_eq!(request.id, "generation:7");
    }

    #[test]
    fn approvals_require_the_matching_recorded_item_not_an_unrelated_diff() {
        let pending = PendingRequest {
            id: "g:1".into(),
            native_id: json!(1),
            thread_id: "a".into(),
            turn_id: Some("turn".into()),
            method: "item/fileChange/requestApproval".into(),
            params: json!({"itemId":"patch"}),
            received_at: 1,
            reply_sent: false,
            supported: true,
        };
        let event = theywork_control::ControlEvent {
            sequence: 1,
            at: 1,
            method: "item/started".into(),
            thread_id: Some("wrong-task".into()),
            turn_id: Some("turn".into()),
            item_id: Some("patch".into()),
            params: json!({"item":{"id":"patch","type":"fileChange","changes":[{"path":"src/main.rs","diff":"-before\n+after"}]}}),
        };
        let wrong = request_view(
            &pending,
            theywork_core::WorkerId("a".into()),
            std::slice::from_ref(&event),
        );
        assert!(!wrong
            .choices
            .iter()
            .any(|choice| choice.response == json!({"decision":"accept"})));
        let correct = request_view(
            &pending,
            theywork_core::WorkerId("a".into()),
            &[theywork_control::ControlEvent {
                thread_id: Some("a".into()),
                ..event
            }],
        );
        assert!(correct.detail.contains("src/main.rs\n-before\n+after"));
        assert!(correct
            .choices
            .iter()
            .any(|choice| choice.response == json!({"decision":"accept"})));
    }
}
