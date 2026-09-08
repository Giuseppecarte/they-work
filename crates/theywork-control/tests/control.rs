#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use theywork_control::{
    run_supervisor, ControlClient, ControlConfig, ControlSnapshot, OperationStatus,
};

struct Fixture {
    root: PathBuf,
    config: ControlConfig,
    child: Option<Child>,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/control-tests")
            .join(theywork_control::operation_id().unwrap());
        fs::create_dir_all(root.join("home")).unwrap();
        fs::create_dir_all(root.join("project with spaces")).unwrap();
        let mut config = ControlConfig::new(root.join("control"), root.join("home"));
        config.codex_program = theywork_control::native::find_executable("python3")
            .expect("Python 3 is needed for offline process fixtures");
        config.codex_args = vec![
            format!("{}/tests/fake_provider.py", env!("CARGO_MANIFEST_DIR")),
            root.join("requests.jsonl").to_string_lossy().into_owned(),
        ];
        config.rpc_timeout_ms = 500;
        Self {
            root,
            config,
            child: None,
        }
    }
    fn start(&mut self) -> ControlClient {
        self.child = Some(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "supervisor_process_entry", "--nocapture"])
                .env(
                    "THEYWORK_CONTROL_TEST_CONFIG",
                    serde_json::to_string(&self.config).unwrap(),
                )
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(fs::File::create(self.root.join("host.log")).unwrap())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(client) = ControlClient::connect(self.config.clone()) {
                return client;
            }
            if let Some(status) = self.child.as_mut().unwrap().try_wait().unwrap() {
                panic!(
                    "fixture host failed {status}: {}",
                    fs::read_to_string(self.root.join("host.log")).unwrap()
                );
            }
            assert!(Instant::now() < deadline, "fixture host startup timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Ok(pid) = fs::read_to_string(self.root.join("launcher.pid")) {
            if let Ok(pid) = pid.trim().parse::<i32>() {
                // SAFETY: PID was written by this test's own detached fixture.
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                }
            }
            let _ = fs::remove_file(self.root.join("launcher.pid"));
        }
    }
    fn requests(&self) -> Vec<Value> {
        fs::read_to_string(self.root.join("requests.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    fn project(&self) -> PathBuf {
        self.root.join("project with spaces")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn supervisor_process_entry() {
    if let Ok(config) = std::env::var("THEYWORK_CONTROL_TEST_CONFIG") {
        run_supervisor(serde_json::from_str(&config).unwrap()).unwrap();
    }
}

fn wait_for(
    client: &ControlClient,
    predicate: impl Fn(&ControlSnapshot) -> bool,
) -> ControlSnapshot {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let snapshot = client.snapshot().unwrap();
        if predicate(&snapshot) {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "state did not converge: {snapshot:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn closing_clients_preserves_work_and_bound_identity_with_explicit_steer_interrupt() {
    let mut fixture = Fixture::new();
    let client = fixture.start();
    assert!(!client.snapshot().unwrap().connected);
    assert!(fixture.requests().is_empty());
    let receipt = client
        .start_codex(fixture.project(), "working", "first")
        .unwrap();
    assert_eq!(receipt.status, OperationStatus::Confirmed);
    assert_eq!(receipt.thread_id.as_deref(), Some("managed-1"));
    drop(client);
    let client = ControlClient::connect(fixture.config.clone()).unwrap();
    let state = wait_for(&client, |state| {
        state
            .threads
            .get("managed-1")
            .is_some_and(|t| t.active_turn_id.is_some())
    });
    assert_eq!(
        state.threads["managed-1"].identity.source.0,
        fs::canonicalize(&fixture.config.codex_home)
            .unwrap()
            .to_string_lossy()
    );
    assert!(state.threads["managed-1"].capabilities.steer);
    assert_eq!(
        client
            .reconnect_codex("managed-1", "busy-reconnect")
            .unwrap()
            .status,
        OperationStatus::Rejected
    );
    assert!(!fixture
        .requests()
        .iter()
        .any(|request| request["method"] == "thread/resume"));
    let poll_time = state.observed_at;
    assert!(poll_time > 0);
    assert!(theywork_control::snapshot_events(&state, 0)
        .iter()
        .filter_map(|event| match &event.kind {
            theywork_core::EventKind::Coverage(coverage) => Some(coverage.observed_at),
            _ => None,
        })
        .all(|timestamp| timestamp == poll_time));
    assert_eq!(
        client
            .send_codex("managed-1", "wrong", Some("old-turn".into()), "bad-steer")
            .unwrap()
            .status,
        OperationStatus::Rejected
    );
    assert_eq!(
        client
            .send_codex("managed-1", "correct", Some("turn-1".into()), "steer")
            .unwrap()
            .status,
        OperationStatus::Confirmed
    );
    assert_eq!(
        client
            .interrupt_codex("managed-1", "turn-1", "interrupt")
            .unwrap()
            .status,
        OperationStatus::Confirmed
    );
    wait_for(&client, |state| {
        state.threads["managed-1"].status == "interrupted"
    });
    assert!(!fixture.requests().iter().any(|request| request
        .pointer("/params/expectedTurnId")
        .and_then(Value::as_str)
        == Some("old-turn")));
}

#[test]
fn uncertain_send_is_durable_and_restart_never_replays_or_reconnects() {
    let mut fixture = Fixture::new();
    let client = fixture.start();
    let receipt = client
        .start_codex(fixture.project(), "uncertain", "once")
        .unwrap();
    assert_eq!(receipt.status, OperationStatus::Uncertain);
    let request_count = fixture.requests().len();
    let duplicate = client
        .start_codex(fixture.project(), "uncertain", "once")
        .unwrap();
    assert_eq!(duplicate.status, OperationStatus::Uncertain);
    assert!(client
        .start_codex(fixture.project(), "different", "once")
        .is_err());
    assert_eq!(fixture.requests().len(), request_count);
    fixture.stop();
    let saved = ControlClient::saved_snapshot(fixture.config.clone()).unwrap();
    assert!(!saved.connected);
    assert!(saved.threads["managed-1"].managed);
    assert!(!saved.threads["managed-1"].capabilities.send);
    assert_eq!(fixture.requests().len(), request_count);
    let client = fixture.start();
    assert!(!client.snapshot().unwrap().connected);
    assert_eq!(fixture.requests().len(), request_count);
    assert!(
        !client.snapshot().unwrap().threads["managed-1"]
            .capabilities
            .send
    );
    assert_eq!(
        client
            .start_codex(fixture.project(), "uncertain", "once")
            .unwrap()
            .status,
        OperationStatus::Uncertain
    );
    assert_eq!(fixture.requests().len(), request_count);
    assert_eq!(
        client
            .reconnect_codex("managed-1", "reconnect")
            .unwrap()
            .status,
        OperationStatus::Confirmed
    );
    let after = fixture.requests();
    assert_eq!(
        after
            .iter()
            .filter(|request| request["method"] == "turn/start")
            .count(),
        1
    );
    assert_eq!(
        after
            .iter()
            .filter(|request| request["method"] == "thread/resume")
            .count(),
        1
    );
}

#[test]
fn approvals_require_current_request_and_single_action_decision() {
    let mut fixture = Fixture::new();
    let client = fixture.start();
    client
        .start_codex(fixture.project(), "approval", "approval-start")
        .unwrap();
    let state = wait_for(&client, |state| !state.pending_requests.is_empty());
    let request = state.pending_requests[0].clone();
    assert_eq!(request.thread_id, "managed-1");
    assert!(!request.reply_sent);
    assert_eq!(
        client
            .respond(&request.id, json!({"decision":"acceptForSession"}), "wide")
            .unwrap()
            .status,
        OperationStatus::Rejected
    );
    assert_eq!(
        client
            .respond(&request.id, json!({"decision":"decline"}), "decline")
            .unwrap()
            .status,
        OperationStatus::Confirmed
    );
    wait_for(&client, |state| state.pending_requests.is_empty());
    assert_eq!(
        client
            .respond(&request.id, json!({"decision":"accept"}), "stale")
            .unwrap()
            .status,
        OperationStatus::Rejected
    );
    assert_eq!(
        fixture
            .requests()
            .iter()
            .filter(|request| request.get("result").is_some())
            .count(),
        1
    );
}

#[test]
fn cancelled_turn_clears_approvals_and_external_ids_have_no_authority() {
    let mut fixture = Fixture::new();
    let client = fixture.start();
    assert_eq!(
        client
            .send_codex("external", "must not send", None, "outside")
            .unwrap()
            .status,
        OperationStatus::Rejected
    );
    assert!(fixture.requests().is_empty());
    client
        .start_codex(fixture.project(), "approval", "start")
        .unwrap();
    let state = wait_for(&client, |state| !state.pending_requests.is_empty());
    client
        .interrupt_codex("managed-1", "turn-1", "stop")
        .unwrap();
    wait_for(&client, |state| state.pending_requests.is_empty());
    assert_eq!(
        client
            .respond(
                &state.pending_requests[0].id,
                json!({"decision":"accept"}),
                "stale"
            )
            .unwrap()
            .status,
        OperationStatus::Rejected
    );
}

#[test]
fn owner_permissions_token_authentication_and_source_binding_are_enforced() {
    use std::io::{Read, Write};
    use std::os::unix::fs::PermissionsExt;
    let mut fixture = Fixture::new();
    let client = fixture.start();
    assert_eq!(
        fs::metadata(&fixture.config.state_dir)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let endpoint_path = fixture.config.state_dir.join("endpoint.json");
    assert_eq!(
        fs::metadata(&endpoint_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let endpoint: Value = serde_json::from_slice(&fs::read(endpoint_path).unwrap()).unwrap();
    let mut stream = std::net::TcpStream::connect(endpoint["address"].as_str().unwrap()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    writeln!(
        stream,
        "{}",
        json!({"token":"0".repeat(64),"request":"Snapshot"})
    )
    .unwrap();
    let mut bytes = String::new();
    let _ = stream.read_to_string(&mut bytes);
    assert!(bytes.is_empty());
    let mut other = fixture.config.clone();
    other.codex_home = fixture.project();
    assert!(ControlClient::connect(other).is_err());
    assert!(!client.snapshot().unwrap().connected);
}

#[test]
fn explicit_delegations_create_three_people_and_results_without_granting_child_control() {
    use theywork_core::{CollaborationKind, WorkerRole, World};
    let mut fixture = Fixture::new();
    let client = fixture.start();
    client
        .start_codex(fixture.project(), "team", "team")
        .unwrap();
    let state = wait_for(&client, |state| {
        state.threads.len() == 3
            && state
                .threads
                .get("child-1")
                .is_some_and(|thread| thread.status == "completed")
    });
    assert_eq!(state.threads["child-0"].role, WorkerRole::Subagent);
    assert!(!state.threads["child-0"].capabilities.send);
    assert_eq!(
        client
            .reconnect_codex("child-0", "child-reconnect")
            .unwrap()
            .status,
        OperationStatus::Rejected
    );
    let mut world = World::new();
    for event in theywork_control::snapshot_events(&state, 0) {
        world.apply(event);
    }
    assert_eq!(world.worker_count(), 3);
    assert_eq!(
        world
            .children(&state.threads["managed-1"].identity.worker_id())
            .len(),
        2
    );
    assert_eq!(
        world
            .collaboration()
            .filter(|event| event.kind == CollaborationKind::Result)
            .count(),
        2
    );
    let after = state.events.last().unwrap().sequence;
    for event in theywork_control::snapshot_events(&state, after) {
        world.apply(event);
    }
    assert_eq!(world.worker_count(), 3);
    assert_eq!(
        world
            .collaboration()
            .filter(|event| event.kind == CollaborationKind::Result)
            .count(),
        2
    );
}

#[test]
fn complete_and_disconnected_turns_do_not_stay_controllable_as_working() {
    let mut fixture = Fixture::new();
    let client = fixture.start();
    client
        .start_codex(fixture.project(), "complete", "complete")
        .unwrap();
    wait_for(&client, |state| {
        state
            .threads
            .get("managed-1")
            .is_some_and(|thread| thread.status == "completed" && thread.active_turn_id.is_none())
    });
    client
        .start_codex(fixture.project(), "disconnect", "disconnect")
        .unwrap();
    let state = wait_for(&client, |state| !state.connected);
    assert!(state
        .threads
        .values()
        .all(|thread| !thread.capabilities.send));
    assert!(state.pending_requests.is_empty());
    let generation = state.connection_generation;
    let resumed = client
        .start_codex(fixture.project(), "working", "explicit-new")
        .unwrap();
    assert_eq!(resumed.status, OperationStatus::Confirmed);
    let state = client.snapshot().unwrap();
    assert_ne!(generation, state.connection_generation);
    assert!(
        !state.threads["managed-1"].capabilities.send,
        "other threads were not resumed"
    );
    let coverage: std::collections::BTreeMap<_, _> = theywork_control::snapshot_events(&state, 0)
        .into_iter()
        .filter_map(|event| match event.kind {
            theywork_core::EventKind::Coverage(coverage) => {
                Some((event.worker, coverage.available))
            }
            _ => None,
        })
        .collect();
    assert!(!coverage[&state.threads["managed-1"].identity.worker_id()]);
    assert!(
        coverage[&state.threads[resumed.thread_id.as_ref().unwrap()]
            .identity
            .worker_id()]
    );
    assert_eq!(
        fixture
            .requests()
            .iter()
            .filter(|request| request["method"] == "thread/resume")
            .count(),
        0
    );
}

#[test]
fn connect_or_spawn_detaches_one_host_and_reuses_it_without_replaying_start() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let launcher = fixture.root.join("launch fixture.py");
    let executable =
        serde_json::to_string(&std::env::current_exe().unwrap().to_string_lossy()).unwrap();
    let pid_path =
        serde_json::to_string(&fixture.root.join("launcher.pid").to_string_lossy()).unwrap();
    fs::write(&launcher, format!("#!/usr/bin/env python3\nimport os,pathlib,sys\npathlib.Path({pid_path}).write_text(str(os.getpid()))\nos.environ['THEYWORK_CONTROL_TEST_CONFIG']=pathlib.Path(sys.argv[2]).read_text()\nos.execl({executable},{executable},'--exact','supervisor_process_entry','--nocapture')\n")).unwrap();
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
    let client = ControlClient::connect_or_spawn(fixture.config.clone(), &launcher).unwrap();
    let before = client.snapshot().unwrap().generation;
    assert!(fixture.requests().is_empty());
    client
        .start_codex(fixture.project(), "working", "once")
        .unwrap();
    drop(client);
    let client = ControlClient::connect_or_spawn(fixture.config.clone(), &launcher).unwrap();
    assert_eq!(client.snapshot().unwrap().generation, before);
    assert_eq!(
        fixture
            .requests()
            .iter()
            .filter(|request| request["method"] == "turn/start")
            .count(),
        1
    );
}
