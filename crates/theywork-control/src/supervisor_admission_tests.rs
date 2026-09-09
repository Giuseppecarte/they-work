use super::*;
use std::fs;

struct Fixture {
    root: PathBuf,
    config: ControlConfig,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/control-tests")
            .join(security::random_token().unwrap());
        fs::create_dir_all(root.join("home")).unwrap();
        fs::create_dir_all(root.join("project")).unwrap();
        let mut config = ControlConfig::new(root.join("control"), root.join("home"));
        config.codex_program = root.join("provider-must-not-start");
        let config = config.normalize().unwrap();
        security::private_dir(&config.state_dir).unwrap();
        Self { root, config }
    }

    fn host(&self) -> Host {
        let lock = security::private_open(&self.config.state_dir.join("host.lock"), true).unwrap();
        lock.try_lock().unwrap();
        Host::load(self.config.clone(), lock).unwrap()
    }

    fn request(&self) -> Request {
        Request::Start {
            project: self.root.join("project"),
            prompt: "Check the fixture".into(),
            operation_id: "not-sent".into(),
        }
    }

    fn saved(&self) -> ControlSnapshot {
        serde_json::from_slice(
            &security::read_private(&self.config.state_dir.join("state.json")).unwrap(),
        )
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn receipt(host: &Host, request: Request) -> OperationReceipt {
    serde_json::from_value(host.handle(request).unwrap()).unwrap()
}

fn assert_not_sent(receipt: &OperationReceipt) {
    assert_eq!(receipt.status, OperationStatus::Rejected);
    assert_eq!(
        receipt.detail,
        "Not sent: local state could not be saved. Restore storage access and submit again."
    );
}

#[test]
fn storage_fault_disables_live_authority_without_discarding_provider_state() {
    let fixture = Fixture::new();
    let host = fixture.host();
    host.register_thread("thread", &fixture.root.join("project"), "Fixture", true);
    {
        let mut state = host.state.lock().unwrap();
        state.connected = true;
        let thread = state.threads.get_mut("thread").unwrap();
        thread.capabilities = Capabilities {
            send: true,
            steer: true,
            interrupt: true,
            reply: true,
            attach: false,
        };
        thread.active_turn_id = Some("turn".into());
        state.pending_requests.push(PendingRequest {
            id: "request".into(),
            native_id: json!(1),
            thread_id: "thread".into(),
            turn_id: Some("turn".into()),
            method: "item/commandExecution/requestApproval".into(),
            params: json!({}),
            received_at: 0,
            reply_sent: false,
            supported: true,
        });
    }
    host.persist().unwrap();
    let saved = fs::read(fixture.config.state_dir.join("state.json")).unwrap();
    assert!(serde_json::from_slice::<Value>(&saved)
        .unwrap()
        .get("storage_recovery_required")
        .is_none());
    fs::remove_file(fixture.config.state_dir.join("state.json")).unwrap();
    assert!(host
        .provider()
        .err()
        .unwrap()
        .to_string()
        .contains("recovery"));
    assert!(host.rpc.lock().unwrap().is_none());
    let visible: ControlSnapshot =
        serde_json::from_value(host.handle(Request::Snapshot).unwrap()).unwrap();
    assert!(!visible.connected);
    assert!(visible.storage_recovery_required);
    assert!(visible.pending_requests.is_empty());
    assert_eq!(
        visible.threads["thread"].capabilities,
        Capabilities::default()
    );
    assert!(visible.threads["thread"].active_turn_id.is_none());
    assert!(visible.last_error.unwrap().contains("recovery"));
    {
        let state = host.state.lock().unwrap();
        assert!(state.connected);
        assert!(!state.storage_recovery_required);
        assert_eq!(state.pending_requests.len(), 1);
        assert!(state.threads["thread"].capabilities.send);
    }
    security::write_private(&fixture.config.state_dir.join("state.json"), &saved).unwrap();
    assert!(host.provider().is_err());
    drop(host);
    let restarted = fixture.host();
    restarted.storage.check_health().unwrap();
    assert!(restarted.rpc.lock().unwrap().is_none());
}

#[test]
fn permission_and_file_size_failures_reject_without_losing_fingerprint() {
    for errno in [libc::EACCES, libc::EFBIG] {
        let fixture = Fixture::new();
        let mut host = fixture.host();
        host.state_writer = Some(Box::new(move |_, _| {
            Err(std::io::Error::from_raw_os_error(errno)).context("Injected pre-write failure")
        }));
        let request = fixture.request();
        let original = receipt(&host, request.clone());
        assert_not_sent(&original);
        assert_eq!(
            original.request_key,
            serde_json::to_string(&request).unwrap()
        );
        let snapshot: ControlSnapshot =
            serde_json::from_value(host.handle(Request::Snapshot).unwrap()).unwrap();
        assert_not_sent(&snapshot.operations["not-sent"]);
        let error = snapshot.last_error.unwrap();
        assert!(error.contains("Injected pre-write failure"));
        assert!(error.contains("only in this running host"));
        assert!(fixture.saved().operations.is_empty());
        assert!(host.rpc.lock().unwrap().is_none());

        host.state_writer = None;
        assert_not_sent(&receipt(&host, request.clone()));
        let Request::Start { project, .. } = &request else {
            unreachable!();
        };
        let different = Request::Start {
            project: project.clone(),
            prompt: "A different action".into(),
            operation_id: "not-sent".into(),
        };
        assert!(host
            .handle(different)
            .unwrap_err()
            .to_string()
            .contains("reused for a different action"));
        assert_eq!(
            host.state.lock().unwrap().operations["not-sent"].request_key,
            original.request_key
        );
        host.persist().unwrap();
        drop(host);
        let restarted = fixture.host();
        assert_not_sent(&receipt(&restarted, request));
        assert!(restarted.rpc.lock().unwrap().is_none());
    }
}

#[test]
fn rejected_intent_is_saved_if_the_second_write_succeeds() {
    let fixture = Fixture::new();
    let mut host = fixture.host();
    let writes = AtomicUsize::new(0);
    host.state_writer = Some(Box::new(move |path, bytes| {
        if writes.fetch_add(1, Ordering::SeqCst) == 0 {
            bail!("Injected first-write failure");
        }
        security::write_private(path, bytes)
    }));
    assert_not_sent(&receipt(&host, fixture.request()));
    let saved = fixture.saved();
    assert_not_sent(&saved.operations["not-sent"]);
    assert!(saved
        .last_error
        .unwrap()
        .contains("Injected first-write failure"));
    assert!(host.rpc.lock().unwrap().is_none());
    drop(host);
    let restarted = fixture.host();
    assert_not_sent(&receipt(&restarted, fixture.request()));
    assert!(restarted.rpc.lock().unwrap().is_none());
}

#[test]
fn failure_after_rename_keeps_restart_uncertain_when_rejection_cannot_be_saved() {
    let fixture = Fixture::new();
    let mut host = fixture.host();
    let writes = AtomicUsize::new(0);
    host.state_writer = Some(Box::new(move |path, bytes| {
        if writes.fetch_add(1, Ordering::SeqCst) == 0 {
            return security::write_private_before_sync(path, bytes, || {
                bail!("Injected failure after rename before directory sync")
            });
        }
        bail!("Injected rejection-write failure")
    }));
    assert_not_sent(&receipt(&host, fixture.request()));
    assert_eq!(
        fixture.saved().operations["not-sent"].status,
        OperationStatus::Sending
    );
    let error = host.state.lock().unwrap().last_error.clone().unwrap();
    assert!(error.contains("after rename before directory sync"));
    assert!(error.contains("Injected rejection-write failure"));
    assert!(host.rpc.lock().unwrap().is_none());
    drop(host);

    let restarted = fixture.host();
    let recovered = receipt(&restarted, fixture.request());
    assert_eq!(recovered.status, OperationStatus::Uncertain);
    assert!(recovered.detail.contains("nothing was resent"));
    assert!(restarted.rpc.lock().unwrap().is_none());
}
