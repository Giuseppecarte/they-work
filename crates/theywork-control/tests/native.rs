#![cfg(unix)]
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use theywork_control::native::NativeProvider;
use theywork_core::{Agent, SourceId, ThreadIdentity};

struct Fixture {
    root: PathBuf,
    program: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/control-tests")
            .join(theywork_control::operation_id().unwrap());
        fs::create_dir_all(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let program = root.join("native fixture");
        fs::copy(
            format!("{}/tests/fake_native.py", env!("CARGO_MANIFEST_DIR")),
            &program,
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        Self { root, program }
    }
    fn provider(&self) -> NativeProvider {
        NativeProvider::from_program(Agent::Claude, &self.root, &self.program).unwrap()
    }
    fn identity(&self) -> ThreadIdentity {
        ThreadIdentity::new(
            Agent::Claude,
            SourceId(self.root.to_string_lossy().into_owned()),
            "full-session",
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn native_login_and_prompt_use_literal_arguments_and_official_config_home() {
    let fixture = Fixture::new();
    let provider = fixture.provider();
    assert!(provider.background_supported);
    assert_eq!(
        provider.login(&fixture.root).unwrap().args,
        ["auth", "login"]
    );
    let prompt = "--dangerously-skip-permissions; $(touch marker) `echo nope` 'quoted'";
    let command = provider.new_conversation(&fixture.root, prompt).unwrap();
    assert_eq!(command.args, ["--bg", "--", prompt]);
    assert!(command.run().unwrap().success());
    let received: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("received.json")).unwrap()).unwrap();
    assert_eq!(received["args"], json!(["--bg", "--", prompt]));
    assert!(!fixture.root.join("marker").exists());
    fs::write(fixture.root.join("legacy"), "").unwrap();
    let legacy = fixture.provider();
    assert!(!legacy.background_supported);
    assert_eq!(
        legacy.new_conversation(&fixture.root, prompt).unwrap().args,
        ["--", prompt]
    );
}

#[test]
fn attach_requires_fresh_same_source_live_background_and_never_resumes_a_stopped_job() {
    let fixture = Fixture::new();
    let provider = fixture.provider();
    assert!(provider.attach(&fixture.identity()).is_err());
    let row = json!({"kind":"background","cwd":fixture.root,"id":"a123","sessionId":"full-session","state":"working","pid":123,"status":"working"});
    fs::write(fixture.root.join("roster.json"), json!([row]).to_string()).unwrap();
    let command = provider.attach(&fixture.identity()).unwrap();
    assert_eq!(command.args, ["attach", "a123"]);
    let mut wrong = fixture.identity();
    wrong.source.0.push_str("-other");
    assert!(provider.attach(&wrong).is_err());
    let mut child = fixture.identity();
    child.session_id = Some("parent".into());
    assert!(provider.attach(&child).is_err());
    for state in ["failed", "stopped"] {
        let mut row = row.clone();
        row["state"] = json!(state);
        fs::write(fixture.root.join("roster.json"), json!([row]).to_string()).unwrap();
        assert!(provider.attach(&fixture.identity()).is_err());
    }
    let mut row = row;
    row.as_object_mut().unwrap().remove("pid");
    fs::write(fixture.root.join("roster.json"), json!([row]).to_string()).unwrap();
    assert!(provider.attach(&fixture.identity()).is_err());
}

#[test]
fn native_console_process_entry() {
    if let Ok(spec) = std::env::var("THEYWORK_NATIVE_TEST_SPEC") {
        let command: theywork_control::native::NativeCommand = serde_json::from_str(&spec).unwrap();
        // SAFETY: read the process disposition into valid sigaction values.
        let mut before: libc::sigaction = unsafe { std::mem::zeroed() };
        let mut after: libc::sigaction = unsafe { std::mem::zeroed() };
        unsafe {
            libc::sigaction(libc::SIGINT, std::ptr::null(), &mut before);
        }
        assert!(!command.run().unwrap().success());
        unsafe {
            libc::sigaction(libc::SIGINT, std::ptr::null(), &mut after);
        }
        assert_eq!(before.sa_sigaction, after.sa_sigaction);
        fs::write(command.cwd.join("console.returned"), "restored").unwrap();
    }
}

#[test]
fn ctrl_c_interrupts_native_child_and_returns_to_parent_in_a_real_pty() {
    let fixture = Fixture::new();
    let provider = fixture.provider();
    let mut command = provider
        .new_conversation(&fixture.root, "placeholder")
        .unwrap();
    command.args = vec!["wait".into()];
    let result =
        std::process::Command::new(theywork_control::native::find_executable("python3").unwrap())
            .arg(format!(
                "{}/tests/pty_console.py",
                env!("CARGO_MANIFEST_DIR")
            ))
            .arg(std::env::current_exe().unwrap())
            .arg(serde_json::to_string(&command).unwrap())
            .arg(&fixture.root)
            .output()
            .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
