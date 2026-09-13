//! Native process fixture: the test executable also supplies its offline
//! provider. Windows x64/ARM64 do not depend on an emulated Python runtime.
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use theywork_control::{ControlClient, ControlConfig, OperationStatus};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).and_then(|arg| arg.to_str()) == Some("--control-host") {
        theywork_control::maybe_run_supervisor()?;
        return Ok(());
    }
    match args.get(1).and_then(|arg| arg.to_str()) {
        Some("--fixture-provider") => return provider(PathBuf::from(&args[2])),
        Some("--fixture-host") => {
            let config = serde_json::from_slice(&fs::read(&args[2])?)?;
            return theywork_control::run_supervisor(config);
        }
        _ => {}
    }
    let mut results = Vec::new();
    for (name, test) in [
        (
            "durable_receipt_and_fingerprint",
            durable_receipt as fn() -> Result<()>,
        ),
        ("interrupted_send_never_replays", interrupted_send),
        ("missing_established_state_blocks_startup", missing_state),
    ] {
        test().with_context(|| format!("storage process case {name}"))?;
        results.push(json!({"name":name,"status":"pass"}));
    }
    #[cfg(windows)]
    {
        sharing_violation()?;
        results.push(
            json!({"name":"sharing_violation_preserves_state_and_rejects_unsent","status":"pass"}),
        );
    }
    let report = json!({"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,
        "fixture_provider":"native Rust test executable; no account or model", "cases":results});
    println!("{}", serde_json::to_string_pretty(&report)?);
    if let Some(path) = std::env::var_os("THEYWORK_STORAGE_REPORT") {
        fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    }
    Ok(())
}

fn provider(log: PathBuf) -> Result<()> {
    for line in std::io::stdin().lock().lines() {
        let value: Value = serde_json::from_str(&line?)?;
        let mut file = OpenOptions::new().append(true).create(true).open(&log)?;
        writeln!(file, "{value}")?;
        file.sync_all()?;
        let id = value.get("id");
        let params = &value["params"];
        let result = match value["method"].as_str().unwrap_or("") {
            "initialize" => json!({"userAgent":"native-storage-fixture"}),
            "initialized" => continue,
            "thread/start" => json!({"thread":{"id":"native-managed","cwd":params["cwd"]}}),
            "thread/resume" => json!({"thread":{"id":params["threadId"]}}),
            "turn/start" if params["input"][0]["text"] == "hold acknowledgement" => continue,
            "turn/start" => json!({"turn":{"id":"native-turn","status":"inProgress"}}),
            "turn/steer" => json!({"turnId":params["expectedTurnId"]}),
            "turn/interrupt" => json!({}),
            _ => bail!("Unexpected fixture method: {value}"),
        };
        let mut output = std::io::stdout().lock();
        writeln!(output, "{}", json!({"id":id,"result":result}))?;
        output.flush()?;
    }
    Ok(())
}

struct Fixture {
    root: PathBuf,
    config: ControlConfig,
    host: Option<Child>,
}

impl Fixture {
    fn new() -> Result<Self> {
        let root = std::env::temp_dir().join(format!(
            "theywork native 窓 {}",
            theywork_control::operation_id()?
        ));
        fs::create_dir_all(root.join("home"))?;
        fs::create_dir_all(root.join("project space"))?;
        let mut config = ControlConfig::new(root.join("control"), root.join("home"));
        config.codex_program = std::env::current_exe()?;
        config.codex_args = vec![
            "--fixture-provider".into(),
            root.join("calls.jsonl").to_string_lossy().into_owned(),
        ];
        config.rpc_timeout_ms = 2000;
        fs::write(root.join("fixture.json"), serde_json::to_vec(&config)?)?;
        Ok(Self {
            root,
            config,
            host: None,
        })
    }
    fn start(&mut self) -> Result<ControlClient> {
        self.host = Some(
            Command::new(std::env::current_exe()?)
                .arg("--fixture-host")
                .arg(self.root.join("fixture.json"))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(fs::File::create(self.root.join("host.log"))?)
                .spawn()?,
        );
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Ok(client) = ControlClient::connect(self.config.clone()) {
                return Ok(client);
            }
            if let Some(status) = self.host.as_mut().unwrap().try_wait()? {
                bail!(
                    "Fixture host exited {status}: {}",
                    fs::read_to_string(self.root.join("host.log"))?
                );
            }
            anyhow::ensure!(Instant::now() < deadline, "Fixture startup timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn stop(&mut self) {
        if let Some(mut host) = self.host.take() {
            let _ = host.kill();
            let _ = host.wait();
        }
    }
    fn calls(&self, method: &str) -> usize {
        fs::read_to_string(self.root.join("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|value| value["method"] == method)
            .count()
    }
    fn project(&self) -> PathBuf {
        self.root.join("project space")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn durable_receipt() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let client = fixture.start()?;
    let first = client.start_codex(fixture.project(), "first", "stable-id")?;
    assert_eq!(first.status, OperationStatus::Confirmed);
    assert_eq!(
        client
            .start_codex(fixture.project(), "first", "stable-id")?
            .status,
        first.status
    );
    assert!(client
        .start_codex(fixture.project(), "different", "stable-id")
        .is_err());
    fixture.stop();
    let client = fixture.start()?;
    assert_eq!(
        client
            .start_codex(fixture.project(), "first", "stable-id")?
            .status,
        OperationStatus::Confirmed
    );
    assert_eq!(fixture.calls("turn/start"), 1);
    assert_eq!(
        client
            .start_codex(fixture.project(), "explicit next", "new-id")?
            .status,
        OperationStatus::Confirmed
    );
    assert_eq!(fixture.calls("turn/start"), 2);
    Ok(())
}

fn interrupted_send() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let client = fixture.start()?;
    let project = fixture.project();
    let waiter = std::thread::spawn(move || {
        client.start_codex(project, "hold acknowledgement", "interrupted-id")
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while fixture.calls("turn/start") == 0 {
        anyhow::ensure!(
            Instant::now() < deadline,
            "Provider did not receive fixture instruction"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    fixture.stop();
    let _ = waiter.join().unwrap();
    let client = fixture.start()?;
    let receipt =
        client.start_codex(fixture.project(), "hold acknowledgement", "interrupted-id")?;
    assert_eq!(receipt.status, OperationStatus::Uncertain);
    assert_eq!(fixture.calls("turn/start"), 1);
    assert!(client
        .start_codex(fixture.project(), "different", "interrupted-id")
        .is_err());
    Ok(())
}

fn missing_state() -> Result<()> {
    let mut fixture = Fixture::new()?;
    assert!(ControlClient::saved_snapshot_optional(fixture.config.clone())?.is_none());
    let client = fixture.start()?;
    client.start_codex(fixture.project(), "first", "missing-id")?;
    fixture.stop();
    fs::remove_file(fixture.config.state_dir.join("state.json"))?;
    let error = ControlClient::connect_or_spawn(fixture.config.clone(), std::env::current_exe()?)
        .unwrap_err();
    assert!(format!("{error:#}").contains("storage needs recovery"));
    assert!(ControlClient::saved_snapshot_optional(fixture.config.clone()).is_err());
    assert!(!fixture.config.state_dir.join("state.json").exists());
    assert_eq!(fixture.calls("turn/start"), 1);
    Ok(())
}

#[cfg(windows)]
fn sharing_violation() -> Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
    let mut fixture = Fixture::new()?;
    let client = fixture.start()?;
    let path = fixture.config.state_dir.join("state.json");
    let before = fs::read(&path)?;
    let held = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&path)?;
    let receipt = client.start_codex(fixture.project(), "blocked", "blocked-id")?;
    assert_eq!(receipt.status, OperationStatus::Rejected);
    assert!(receipt.detail.starts_with("Not sent:"));
    assert_eq!(fs::read(&path)?, before);
    assert_eq!(fixture.calls("turn/start"), 0);
    drop(held);
    assert_eq!(
        client
            .start_codex(fixture.project(), "blocked", "blocked-id")?
            .status,
        OperationStatus::Rejected
    );
    assert_eq!(
        client
            .start_codex(fixture.project(), "explicit new", "new-id")?
            .status,
        OperationStatus::Confirmed
    );
    assert_eq!(fixture.calls("turn/start"), 1);
    Ok(())
}
