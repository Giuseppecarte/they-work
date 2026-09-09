use std::fs::File;
use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use theywork_core::{Agent, SourceId, ThreadIdentity, WorkerRole};

use crate::model::*;
use crate::rpc::{Rpc, RpcError};
use crate::security;

#[cfg(test)]
type StateWriter = Box<dyn Fn(&Path, &[u8]) -> Result<()> + Send + Sync>;

struct Host {
    config: ControlConfig,
    state: Arc<Mutex<ControlSnapshot>>,
    rpc: Mutex<Option<Arc<Rpc>>>,
    /// Serializes explicit mutations; snapshots and provider events stay live.
    mutations: Mutex<()>,
    #[cfg(test)]
    state_writer: Option<StateWriter>,
    _lock: File,
}

/// Call before normal CLI parsing. The generated config path is the sole
/// argument; this path contains no credentials, which are never process args.
pub fn maybe_run_supervisor() -> Result<bool> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--control-host")) {
        return Ok(false);
    }
    let path = PathBuf::from(
        args.next()
            .context("--control-host requires its private config path")?,
    );
    anyhow::ensure!(args.next().is_none(), "Unexpected control host arguments");
    let config: ControlConfig = serde_json::from_slice(&security::read_private(&path)?)?;
    run_supervisor(config)?;
    Ok(true)
}

/// Blocking dedicated-process entrypoint. Startup opens no provider connection
/// and executes no user turn. An explicit command lazily starts the provider.
pub fn run_supervisor(config: ControlConfig) -> Result<()> {
    let config = config.normalize()?;
    security::private_dir(&config.state_dir)?;
    let lock = security::private_open(&config.state_dir.join("host.lock"), true)?;
    lock.try_lock()
        .context("A supervisor already owns this control directory")?;
    let host = Arc::new(Host::load(config, lock)?);
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let endpoint = Endpoint {
        address: listener.local_addr()?,
        token: security::random_token()?,
        generation: host.state.lock().unwrap().generation.clone(),
        codex_home: host.config.codex_home.clone(),
    };
    security::write_private(
        &host.config.state_dir.join("endpoint.json"),
        &serde_json::to_vec(&endpoint)?,
    )?;
    let active = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let stream = stream?;
        if active.fetch_add(1, Ordering::Relaxed) >= 32 {
            active.fetch_sub(1, Ordering::Relaxed);
            continue;
        }
        let host = host.clone();
        let active = active.clone();
        let token = endpoint.token.clone();
        std::thread::spawn(move || {
            let _ = serve(stream, &host, &token);
            active.fetch_sub(1, Ordering::Relaxed);
        });
    }
    Ok(())
}

fn serve(mut stream: TcpStream, host: &Host, token: &str) -> Result<()> {
    anyhow::ensure!(stream.peer_addr()?.ip().is_loopback(), "Local clients only");
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let line = security::read_line_limited(&mut BufReader::new(stream.try_clone()?))?
        .context("Empty client request")?;
    let envelope: Envelope = serde_json::from_str(&line)?;
    anyhow::ensure!(
        security::token_matches(&envelope.token, token),
        "Unauthenticated local client"
    );
    let response = match host.handle(envelope.request) {
        Ok(value) => Response {
            value: Some(value),
            error: None,
        },
        Err(error) => Response {
            value: None,
            error: Some(error.to_string()),
        },
    };
    serde_json::to_writer(&mut stream, &response)?;
    stream.write_all(b"\n")?;
    Ok(())
}

impl Host {
    fn load(config: ControlConfig, lock: File) -> Result<Self> {
        let path = config.state_dir.join("state.json");
        let mut state: ControlSnapshot = if path.exists() {
            serde_json::from_slice(&security::read_private(&path)?)?
        } else {
            ControlSnapshot::default()
        };
        anyhow::ensure!(
            state.codex_home.as_os_str().is_empty() || state.codex_home == config.codex_home,
            "This control directory belongs to a different Codex home"
        );
        state.generation = security::random_token()?;
        state.codex_home = config.codex_home.clone();
        state.connected = false;
        state.pending_requests.clear();
        for thread in state.threads.values_mut() {
            thread.capabilities = Capabilities::default();
            thread.active_turn_id = None;
            thread.status = "disconnected".into();
        }
        for receipt in state.operations.values_mut() {
            if receipt.status == OperationStatus::Sending {
                receipt.status = OperationStatus::Uncertain;
                receipt.detail = "Host restarted before acknowledgement. Inspect the conversation; nothing was resent.".into();
            }
        }
        let host = Self {
            config,
            state: Arc::new(Mutex::new(state)),
            rpc: Mutex::new(None),
            mutations: Mutex::new(()),
            #[cfg(test)]
            state_writer: None,
            _lock: lock,
        };
        host.persist()?;
        Ok(host)
    }

    fn persist(&self) -> Result<()> {
        let state = self.state.lock().unwrap();
        self.persist_state(&state)
    }

    fn persist_state(&self, state: &ControlSnapshot) -> Result<()> {
        let bytes = bounded_state(state)?;
        #[cfg(test)]
        if let Some(writer) = &self.state_writer {
            return writer(&self.config.state_dir.join("state.json"), &bytes);
        }
        security::write_private(&self.config.state_dir.join("state.json"), &bytes)
    }

    fn provider(&self) -> Result<Arc<Rpc>> {
        let mut slot = self.rpc.lock().unwrap();
        if let Some(rpc) = &*slot {
            if self.state.lock().unwrap().connected {
                return Ok(rpc.clone());
            }
        }
        // Reached only by an explicit operation. Replace a dead transport,
        // keeping every old task disconnected until separately reconnected.
        slot.take();
        let (sender, receiver) = mpsc::channel();
        let rpc = Rpc::launch(&self.config, sender).context("Could not launch Codex app-server")?;
        let info = rpc.initialize().map_err(rpc_anyhow)?;
        {
            let mut state = self.state.lock().unwrap();
            state.connected = true;
            state.connection_generation = security::random_token()?;
            state.provider_info = info;
            state.last_error = None;
        }
        *slot = Some(rpc.clone());
        let state = self.state.clone();
        let path = self.config.state_dir.join("state.json");
        std::thread::spawn(move || {
            let mut last_save = Instant::now();
            let mut dirty = false;
            loop {
                let mut disconnected = false;
                match receiver.recv_timeout(Duration::from_millis(250)) {
                    Ok(message) => {
                        disconnected = message.is_none();
                        apply_event(&mut state.lock().unwrap(), message);
                        dirty = true;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                if dirty && (disconnected || last_save.elapsed() >= Duration::from_millis(250)) {
                    let mut snapshot = state.lock().unwrap();
                    let result = bounded_state(&snapshot)
                        .and_then(|bytes| security::write_private(&path, &bytes));
                    if let Err(error) = result {
                        snapshot.last_error =
                            Some(format!("Could not save control events: {error}"));
                    }
                    last_save = Instant::now();
                    dirty = false;
                }
                if disconnected {
                    break;
                }
            }
        });
        Ok(rpc)
    }

    fn handle(&self, request: Request) -> Result<Value> {
        if matches!(request, Request::Snapshot) {
            let mut state = self.state.lock().unwrap();
            state.observed_at = now();
            let bytes = bounded_state(&state)?;
            return Ok(serde_json::from_slice(&bytes)?);
        }
        let _mutation = self.mutations.lock().unwrap();
        let (id, thread_id) = match &request {
            Request::Start { operation_id, .. } => (operation_id, None),
            Request::Send {
                operation_id,
                thread_id,
                ..
            }
            | Request::Interrupt {
                operation_id,
                thread_id,
                ..
            }
            | Request::Reconnect {
                operation_id,
                thread_id,
            } => (operation_id, Some(thread_id.clone())),
            Request::Reply { operation_id, .. } => (operation_id, None),
            Request::Snapshot => unreachable!(),
        };
        anyhow::ensure!(
            !id.is_empty() && id.len() <= 128,
            "A stable operation ID is required"
        );
        let key = serde_json::to_string(&request)?;
        if let Some(receipt) = self.state.lock().unwrap().operations.get(id) {
            anyhow::ensure!(
                receipt.request_key == key,
                "Operation ID was reused for a different action"
            );
            return Ok(serde_json::to_value(receipt)?);
        }
        anyhow::ensure!(
            self.state.lock().unwrap().operations.len() < 10_000,
            "Operation ledger is full; keep the existing state and choose a new control directory"
        );
        let receipt = OperationReceipt {
            id: id.clone(),
            status: OperationStatus::Sending,
            thread_id,
            turn_id: None,
            detail: "Awaiting provider acknowledgement".into(),
            request_key: key,
        };
        self.state
            .lock()
            .unwrap()
            .operations
            .insert(id.clone(), receipt);
        // Durable intent is written BEFORE any external command. Recovery will
        // mark it uncertain and never execute it again.
        if let Err(error) = self.persist() {
            // This handler has not called the provider. A failed save can still
            // leave Sending on disk (for example after rename), so preserve its
            // ID and the conservative restart path rather than deleting intent.
            let mut state = self.state.lock().unwrap();
            let receipt = state.operations.get_mut(id).unwrap();
            receipt.status = OperationStatus::Rejected;
            receipt.detail = "Not sent: local state could not be saved. Restore storage access and submit again.".into();
            let result = serde_json::to_value(&*receipt)?;
            state.last_error = Some(format!("Could not save instruction intent: {error:#}"));
            if let Err(rejection_error) = self.persist_state(&state) {
                state.last_error = Some(format!(
                    "Could not save instruction intent: {error:#}. Rejection is recorded only in this running host: {rejection_error:#}"
                ));
            }
            return Ok(result);
        }
        let result = self.execute(&request);
        let mut state = self.state.lock().unwrap();
        let receipt = state.operations.get_mut(id).unwrap();
        match result {
            Ok((thread_id, turn_id, detail)) => {
                receipt.status = OperationStatus::Confirmed;
                receipt.thread_id = thread_id;
                receipt.turn_id = turn_id;
                receipt.detail = detail;
            }
            Err(RpcError::Rejected(error)) => {
                receipt.status = OperationStatus::Rejected;
                receipt.detail = error;
            }
            Err(RpcError::Uncertain(error)) => {
                receipt.status = OperationStatus::Uncertain;
                receipt.detail = error;
            }
        }
        let result = serde_json::to_value(&*receipt)?;
        self.persist_state(&state)?;
        Ok(result)
    }

    fn execute(
        &self,
        request: &Request,
    ) -> Result<(Option<String>, Option<String>, String), RpcError> {
        let reject = |message: &str| RpcError::Rejected(message.into());
        match request {
            Request::Start {
                project,
                prompt,
                operation_id,
            } => {
                validate_prompt(prompt).map_err(|e| reject(&e.to_string()))?;
                let project = std::fs::canonicalize(project).map_err(|e| reject(&e.to_string()))?;
                if !project.is_dir() {
                    return Err(reject("Project is not a directory"));
                }
                let rpc = self.provider().map_err(|e| reject(&e.to_string()))?;
                let result = rpc.call("thread/start", json!({"cwd":project,"approvalPolicy":"on-request","sandbox":"workspace-write"}))?;
                let thread = result.get("thread").ok_or_else(|| {
                    RpcError::Uncertain(
                        "thread/start returned no thread; inspect provider before retrying".into(),
                    )
                })?;
                let id = thread
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        RpcError::Uncertain("thread/start returned no native ID".into())
                    })?
                    .to_owned();
                self.register_thread(&id, &project, prompt, true);
                if let Some(receipt) = self.state.lock().unwrap().operations.get_mut(operation_id) {
                    receipt.thread_id = Some(id.clone());
                }
                self.persist()
                    .map_err(|e| RpcError::Uncertain(e.to_string()))?;
                let result = rpc.call(
                    "turn/start",
                    json!({"threadId":id,"input":[{"type":"text","text":prompt}]}),
                )?;
                let turn = Some(result.pointer("/turn/id").and_then(Value::as_str).filter(|id| !id.is_empty()).ok_or_else(|| RpcError::Uncertain("Turn acknowledgement omitted its ID; inspect the conversation before sending again".into()))?.to_owned());
                self.set_turn(&id, turn.clone());
                Ok((Some(id), turn, "Instruction accepted".into()))
            }
            Request::Send {
                thread_id,
                prompt,
                expected_turn_id,
                ..
            } => {
                validate_prompt(prompt).map_err(|e| reject(&e.to_string()))?;
                let managed = self.controlled_thread(thread_id)?;
                let rpc = self.provider().map_err(|e| reject(&e.to_string()))?;
                let (method, params) = if let Some(turn) = expected_turn_id {
                    if managed.active_turn_id.as_ref() != Some(turn) {
                        return Err(reject("Active turn changed; inspect before steering"));
                    }
                    (
                        "turn/steer",
                        json!({"threadId":thread_id,"expectedTurnId":turn,"input":[{"type":"text","text":prompt}]}),
                    )
                } else {
                    if managed.active_turn_id.is_some() {
                        return Err(reject(
                            "Conversation is working; supply its expected turn ID to steer",
                        ));
                    }
                    (
                        "turn/start",
                        json!({"threadId":thread_id,"input":[{"type":"text","text":prompt}]}),
                    )
                };
                let result = rpc.call(method, params)?;
                let turn = Some(
                    result
                        .get("turnId")
                        .or_else(|| result.pointer("/turn/id"))
                        .and_then(Value::as_str)
                        .filter(|id| !id.is_empty())
                        .ok_or_else(|| {
                            RpcError::Uncertain(
                                "Turn acknowledgement omitted its ID; inspect before sending again"
                                    .into(),
                            )
                        })?
                        .to_owned(),
                );
                if expected_turn_id.is_some() && &turn != expected_turn_id {
                    return Err(RpcError::Uncertain(
                        "Steering acknowledgement referred to a different turn".into(),
                    ));
                }
                self.set_turn(thread_id, turn.clone());
                Ok((Some(thread_id.clone()), turn, "Instruction accepted".into()))
            }
            Request::Interrupt {
                thread_id, turn_id, ..
            } => {
                let managed = self.controlled_thread(thread_id)?;
                if managed.active_turn_id.as_ref() != Some(turn_id) {
                    return Err(reject("Active turn changed; interruption was not sent"));
                }
                self.provider().map_err(|e| reject(&e.to_string()))?.call(
                    "turn/interrupt",
                    json!({"threadId":thread_id,"turnId":turn_id}),
                )?;
                Ok((
                    Some(thread_id.clone()),
                    Some(turn_id.clone()),
                    "Interrupt requested; waiting for completion".into(),
                ))
            }
            Request::Reply {
                request_id,
                response,
                ..
            } => {
                let pending = self
                    .state
                    .lock()
                    .unwrap()
                    .pending_requests
                    .iter()
                    .find(|p| &p.id == request_id)
                    .cloned()
                    .ok_or_else(|| reject("Request is no longer pending"))?;
                if pending.reply_sent {
                    return Err(reject(
                        "A reply was already sent; waiting for provider resolution",
                    ));
                }
                let thread = self.controlled_thread(&pending.thread_id)?;
                if pending.turn_id.is_some() && pending.turn_id != thread.active_turn_id {
                    return Err(reject("Request belongs to an expired turn"));
                }
                validate_reply(&pending, response).map_err(|e| reject(&e.to_string()))?;
                if let Some(current) = self
                    .state
                    .lock()
                    .unwrap()
                    .pending_requests
                    .iter_mut()
                    .find(|p| &p.id == request_id)
                {
                    current.reply_sent = true;
                }
                self.persist().map_err(|e| {
                    RpcError::Rejected(format!("Reply not sent because durable state failed: {e}"))
                })?;
                self.provider()
                    .map_err(|e| reject(&e.to_string()))?
                    .reply(&pending.native_id, response.clone())?;
                Ok((
                    Some(pending.thread_id),
                    pending.turn_id,
                    "Reply written; awaiting provider resolution".into(),
                ))
            }
            Request::Reconnect { thread_id, .. } => {
                if !self
                    .state
                    .lock()
                    .unwrap()
                    .threads
                    .get(thread_id)
                    .is_some_and(|thread| thread.managed)
                {
                    return Err(reject("This host does not own the conversation"));
                }
                if self
                    .state
                    .lock()
                    .unwrap()
                    .threads
                    .get(thread_id)
                    .is_some_and(|thread| thread.active_turn_id.is_some())
                {
                    return Err(reject(
                        "Conversation still has a live turn; reconnect was not sent",
                    ));
                }
                let rpc = self.provider().map_err(|e| reject(&e.to_string()))?;
                // Explicit resume loads history only, never starts a user turn.
                let result = rpc.call("thread/resume", json!({"threadId":thread_id}))?;
                let returned = result.pointer("/thread/id").and_then(Value::as_str);
                if returned != Some(thread_id) {
                    return Err(RpcError::Uncertain(
                        "Provider resumed a different identity".into(),
                    ));
                }
                if let Some(thread) = self.state.lock().unwrap().threads.get_mut(thread_id) {
                    thread.capabilities = managed_capabilities();
                    thread.status = "idle".into();
                }
                Ok((
                    Some(thread_id.clone()),
                    None,
                    "Conversation connected; no turn started".into(),
                ))
            }
            Request::Snapshot => unreachable!(),
        }
    }

    fn controlled_thread(&self, id: &str) -> Result<ManagedThread, RpcError> {
        self.state
            .lock()
            .unwrap()
            .threads
            .get(id)
            .filter(|t| t.capabilities.send)
            .cloned()
            .ok_or_else(|| {
                RpcError::Rejected(
                    "Conversation is not bound to this live host; observation grants no control"
                        .into(),
                )
            })
    }

    fn register_thread(&self, id: &str, project: &Path, title: &str, controlled: bool) {
        self.state.lock().unwrap().threads.insert(
            id.into(),
            ManagedThread {
                identity: ThreadIdentity::new(
                    Agent::Codex,
                    SourceId(self.config.codex_home.to_string_lossy().into_owned()),
                    id,
                ),
                role: WorkerRole::Main,
                managed: controlled,
                project: project.into(),
                title: title.chars().take(160).collect(),
                active_turn_id: None,
                status: "idle".into(),
                latest_text: String::new(),
                capabilities: if controlled {
                    managed_capabilities()
                } else {
                    Capabilities::default()
                },
                updated_at: now(),
            },
        );
    }

    fn set_turn(&self, id: &str, turn: Option<String>) {
        let mut state = self.state.lock().unwrap();
        let completed = state
            .events
            .iter()
            .rev()
            .find(|event| {
                event.thread_id.as_deref() == Some(id)
                    && event.turn_id == turn
                    && event.method == "turn/completed"
            })
            .map(|event| {
                event
                    .params
                    .pointer("/turn/status")
                    .and_then(Value::as_str)
                    .unwrap_or("completed")
                    .to_owned()
            });
        if let Some(thread) = state.threads.get_mut(id) {
            if let Some(status) = completed {
                thread.active_turn_id = None;
                thread.status = status;
            } else {
                thread.active_turn_id = turn;
                thread.status = "working".into();
            }
            thread.updated_at = now();
        }
    }
}

fn bounded_state(state: &ControlSnapshot) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(state)?;
    anyhow::ensure!(
        bytes.len() <= 16 * 1024 * 1024 - 128,
        "Control state limit reached; inspect current tasks before further instructions"
    );
    Ok(bytes)
}

fn managed_capabilities() -> Capabilities {
    Capabilities {
        send: true,
        steer: true,
        interrupt: true,
        reply: true,
        attach: false,
    }
}
fn rpc_anyhow(error: RpcError) -> anyhow::Error {
    match error {
        RpcError::Rejected(s) | RpcError::Uncertain(s) => anyhow::anyhow!(s),
    }
}

fn validate_prompt(prompt: &str) -> Result<()> {
    anyhow::ensure!(
        !prompt.trim().is_empty() && prompt.len() <= 64 * 1024,
        "Instruction must contain 1–65536 bytes"
    );
    Ok(())
}

fn validate_reply(pending: &PendingRequest, response: &Value) -> Result<()> {
    anyhow::ensure!(
        pending.supported,
        "This request type is not supported; use the original provider"
    );
    match pending.method.as_str() {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            let decision = response.get("decision").context("A decision is required")?;
            let simple = decision
                .as_str()
                .context("Only a single-request decision is supported")?;
            anyhow::ensure!(
                matches!(simple, "accept" | "decline" | "cancel"),
                "Session-wide grants are not supported"
            );
            if let Some(available) = pending
                .params
                .get("availableDecisions")
                .and_then(Value::as_array)
            {
                anyhow::ensure!(
                    available.contains(decision),
                    "Decision was not offered by the provider"
                );
            }
        }
        "item/tool/requestUserInput" => {
            let answers = response
                .get("answers")
                .and_then(Value::as_object)
                .context("Answers object is required")?;
            let questions = pending
                .params
                .get("questions")
                .and_then(Value::as_array)
                .context("No structured questions supplied")?;
            for (id, answer) in answers {
                anyhow::ensure!(
                    questions
                        .iter()
                        .any(|q| q.get("id").and_then(Value::as_str) == Some(id)),
                    "Unknown question ID"
                );
                let values = answer
                    .get("answers")
                    .and_then(Value::as_array)
                    .context("Question answer must contain answers array")?;
                anyhow::ensure!(values.iter().all(Value::is_string), "Answers must be text");
            }
            anyhow::ensure!(!answers.is_empty(), "An answer is required");
        }
        "mcpServer/elicitation/request" => {
            anyhow::ensure!(
                matches!(
                    response.get("action").and_then(Value::as_str),
                    Some("accept" | "decline" | "cancel")
                ),
                "Invalid elicitation action"
            );
        }
        "item/permissions/requestApproval" => {
            anyhow::ensure!(
                response
                    .get("scope")
                    .and_then(Value::as_str)
                    .is_none_or(|scope| scope == "turn"),
                "Only turn-scoped permissions are supported"
            );
            let grants = response
                .get("permissions")
                .context("Granted permissions are required")?;
            let requested = pending
                .params
                .get("permissions")
                .context("Requested permissions are missing")?;
            anyhow::ensure!(
                json_subset(grants, requested),
                "Permissions must be a subset of the current request"
            );
        }
        _ => bail!("Unsupported request type"),
    }
    Ok(())
}

fn json_subset(grant: &Value, requested: &Value) -> bool {
    match (grant, requested) {
        (Value::Object(grants), Value::Object(requests)) => grants.iter().all(|(key, value)| {
            requests
                .get(key)
                .is_some_and(|request| json_subset(value, request))
        }),
        (Value::Array(grants), Value::Array(requests)) => {
            grants.iter().all(|grant| requests.contains(grant))
        }
        (Value::Bool(false), Value::Bool(_)) | (Value::Null, _) => true,
        _ => grant == requested,
    }
}

fn apply_event(state: &mut ControlSnapshot, message: Option<Value>) {
    let Some(message) = message else {
        state.connected = false;
        state.pending_requests.clear();
        state.last_error =
            Some("Provider disconnected. No turn will be restarted automatically.".into());
        for thread in state.threads.values_mut() {
            thread.capabilities = Capabilities::default();
            thread.active_turn_id = None;
            thread.status = "disconnected".into();
        }
        return;
    };
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let thread_id = params
        .get("threadId")
        .or_else(|| params.pointer("/thread/id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let turn_id = params
        .get("turnId")
        .or_else(|| params.pointer("/turn/id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let item_id = params
        .get("itemId")
        .or_else(|| params.pointer("/item/id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let timestamp = now();
    // Only a spawn tool item emitted on an already owned connection creates
    // child people. Generic internal/reviewer thread notifications do not.
    if matches!(method, "item/started" | "item/completed") {
        if let Some(parent) = thread_id
            .as_ref()
            .and_then(|id| state.threads.get(id))
            .cloned()
        {
            let item = params.get("item").unwrap_or(&Value::Null);
            let tool = item
                .get("tool")
                .and_then(Value::as_str)
                .unwrap_or("")
                .replace(['_', '-'], "")
                .to_ascii_lowercase();
            let actor_matches = item
                .get("senderThreadId")
                .and_then(Value::as_str)
                .is_none_or(|id| id == parent.identity.native_id);
            if actor_matches
                && matches!(
                    item.get("type").and_then(Value::as_str),
                    Some("collabToolCall" | "collabAgentToolCall")
                )
                && matches!(tool.as_str(), "spawn" | "spawnagent")
            {
                let mut children: Vec<&str> = ["newThreadId", "receiverThreadId"]
                    .into_iter()
                    .filter_map(|key| item.get(key).and_then(Value::as_str))
                    .collect();
                if let Some(ids) = item.get("receiverThreadIds").and_then(Value::as_array) {
                    children.extend(ids.iter().filter_map(Value::as_str));
                }
                for child in children {
                    if child == parent.identity.native_id || state.threads.contains_key(child) {
                        continue;
                    }
                    state.threads.insert(
                        child.into(),
                        ManagedThread {
                            identity: ThreadIdentity::new(
                                Agent::Codex,
                                parent.identity.source.clone(),
                                child,
                            ),
                            role: WorkerRole::Subagent,
                            managed: false,
                            project: parent.project.clone(),
                            title: item
                                .get("agentNickname")
                                .and_then(Value::as_str)
                                .map(str::to_owned)
                                .unwrap_or_else(|| {
                                    format!("Agent {}", child.chars().take(8).collect::<String>())
                                }),
                            active_turn_id: None,
                            status: "unknown".into(),
                            latest_text: String::new(),
                            capabilities: Capabilities::default(),
                            updated_at: timestamp,
                        },
                    );
                }
            }
        }
    }
    if let (Some(native), Some(thread)) = (message.get("id"), thread_id.as_ref()) {
        let supported = matches!(
            method,
            "item/commandExecution/requestApproval"
                | "item/fileChange/requestApproval"
                | "item/tool/requestUserInput"
                | "mcpServer/elicitation/request"
                | "item/permissions/requestApproval"
        );
        if state.pending_requests.len() < 64 {
            state.pending_requests.push(PendingRequest {
                id: format!(
                    "{}:{}:{}",
                    state.generation, state.connection_generation, native
                ),
                native_id: native.clone(),
                thread_id: thread.clone(),
                turn_id: turn_id.clone(),
                method: method.into(),
                params: params.clone(),
                received_at: timestamp,
                reply_sent: false,
                supported,
            });
        } else {
            state.last_error = Some("Too many pending requests; provider remains blocked".into());
        }
    }
    if method == "serverRequest/resolved" {
        state
            .pending_requests
            .retain(|pending| Some(&pending.native_id) != params.get("requestId"));
    }
    if let Some(id) = &thread_id {
        if let Some(thread) = state.threads.get_mut(id) {
            thread.updated_at = timestamp;
            match method {
                "turn/started" => {
                    thread.active_turn_id = turn_id.clone();
                    thread.status = "working".into();
                    state
                        .pending_requests
                        .retain(|p| &p.thread_id != id || p.turn_id == turn_id);
                }
                "turn/completed" => {
                    if thread.active_turn_id == turn_id {
                        thread.active_turn_id = None;
                        thread.status = params
                            .pointer("/turn/status")
                            .and_then(Value::as_str)
                            .unwrap_or("completed")
                            .into();
                    }
                    state
                        .pending_requests
                        .retain(|p| &p.thread_id != id || p.turn_id != turn_id);
                }
                "item/agentMessage/delta" => {
                    if let Some(delta) = params.get("delta").and_then(Value::as_str) {
                        thread.latest_text.push_str(delta);
                        if thread.latest_text.len() > 32_768 {
                            thread.latest_text = thread
                                .latest_text
                                .chars()
                                .rev()
                                .take(8_192)
                                .collect::<String>()
                                .chars()
                                .rev()
                                .collect();
                        }
                    }
                }
                "item/completed" => {
                    if params.pointer("/item/type").and_then(Value::as_str) == Some("agentMessage")
                    {
                        if let Some(text) = params.pointer("/item/text").and_then(Value::as_str) {
                            thread.latest_text = text.chars().take(32_768).collect();
                        }
                    }
                }
                "thread/name/updated" => {
                    if let Some(name) = params
                        .get("threadName")
                        .or_else(|| params.get("name"))
                        .and_then(Value::as_str)
                    {
                        thread.title = name.into();
                    }
                }
                _ => {}
            }
        }
    }
    let sequence = state.events.last().map_or(1, |event| event.sequence + 1);
    let event_params = if params.to_string().len() > 32_768 {
        json!({"truncated":true,"detail":"Large event; inspect native transcript"})
    } else {
        params
    };
    state.events.push(ControlEvent {
        sequence,
        at: timestamp,
        method: method.into(),
        thread_id,
        turn_id,
        item_id,
        params: event_params,
    });
    if state.events.len() > 256 {
        state.events.remove(0);
    }
}

#[cfg(test)]
#[path = "supervisor_admission_tests.rs"]
mod admission_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_replies_cannot_expand_paths_network_or_lifetime() {
        let pending = PendingRequest {
            id: "request".into(),
            native_id: json!(1),
            thread_id: "thread".into(),
            turn_id: Some("turn".into()),
            method: "item/permissions/requestApproval".into(),
            params: json!({"permissions":{"fileSystem":{"write":["/project"]},"network":{"enabled":false}}}),
            received_at: 0,
            reply_sent: false,
            supported: true,
        };
        assert!(validate_reply(&pending, &json!({"permissions":{},"scope":"turn"})).is_ok());
        assert!(validate_reply(
            &pending,
            &json!({"permissions":{"fileSystem":{"write":["/"]}},"scope":"turn"})
        )
        .is_err());
        assert!(validate_reply(
            &pending,
            &json!({"permissions":{"network":{"enabled":true}}})
        )
        .is_err());
        assert!(validate_reply(&pending, &json!({"permissions":{},"scope":"session"})).is_err());
    }

    #[test]
    fn structured_reply_rejects_other_question_ids_and_nontext_answers() {
        let pending = PendingRequest {
            id: "request".into(),
            native_id: json!(1),
            thread_id: "thread".into(),
            turn_id: Some("turn".into()),
            method: "item/tool/requestUserInput".into(),
            params: json!({"questions":[{"id":"language"}]}),
            received_at: 0,
            reply_sent: false,
            supported: true,
        };
        assert!(validate_reply(
            &pending,
            &json!({"answers":{"language":{"answers":["Rust"]}}})
        )
        .is_ok());
        assert!(
            validate_reply(&pending, &json!({"answers":{"other":{"answers":["Rust"]}}})).is_err()
        );
        assert!(validate_reply(
            &pending,
            &json!({"answers":{"language":{"answers":[{"run":"command"}]}}})
        )
        .is_err());
    }
}
