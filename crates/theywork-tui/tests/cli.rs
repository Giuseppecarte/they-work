use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use rusqlite::{params, Connection};
use serde_json::{json, Value};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("they-work-tui-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct Fixture {
    temp: TempDir,
    claude_home: PathBuf,
    codex_home: PathBuf,
    project_a: PathBuf,
    project_b: PathBuf,
    config_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new();
        let project_a = temp.path().join("project-a");
        let project_b = temp.path().join("project-b");
        fs::create_dir_all(project_a.join(".git")).unwrap();
        fs::create_dir_all(project_b.join(".git")).unwrap();

        let claude_home = temp.path().join("claude");
        let codex_home = temp.path().join("codex");
        create_claude_fixture(&claude_home, &project_a, &project_b);
        create_codex_fixture(&codex_home, &project_a);

        let config_dir = temp.path().join("config");
        fs::create_dir_all(&config_dir).unwrap();

        Self {
            temp,
            claude_home,
            codex_home,
            project_a,
            project_b,
            config_dir,
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn append_jsonl(path: &Path, value: Value) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    serde_json::to_writer(&mut file, &value).unwrap();
    file.write_all(b"\n").unwrap();
}

fn create_claude_fixture(home: &Path, project_a: &Path, project_b: &Path) {
    let projects = home.join("projects");
    fs::create_dir_all(&projects).unwrap();

    let old = now_ms() - 300_000;
    let session_a = projects.join("fixture-a").join("session-a.jsonl");
    let session_b = projects.join("fixture-b").join("session-b.jsonl");
    fs::create_dir_all(session_a.parent().unwrap()).unwrap();
    fs::create_dir_all(session_b.parent().unwrap()).unwrap();

    append_jsonl(
        &session_a,
        json!({
            "type": "system",
            "timestamp": old,
            "sessionId": "claude-a",
            "cwd": project_a.to_string_lossy(),
            "customTitle": "Claude blocked"
        }),
    );
    append_jsonl(
        &session_a,
        json!({
            "type": "user",
            "timestamp": old + 1_000,
            "sessionId": "claude-a",
            "cwd": project_a.to_string_lossy(),
            "message": {"content": [{"type": "text", "text": "please fix the build"}]}
        }),
    );

    append_jsonl(
        &session_b,
        json!({
            "type": "system",
            "timestamp": old,
            "sessionId": "claude-b",
            "cwd": project_b.to_string_lossy(),
            "customTitle": "Claude idle"
        }),
    );
    append_jsonl(
        &session_b,
        json!({
            "type": "assistant",
            "timestamp": now_ms(),
            "sessionId": "claude-b",
            "cwd": project_b.to_string_lossy(),
            "message": {"content": [{"type": "text", "text": "done"}]}
        }),
    );
}

fn create_codex_fixture(home: &Path, project: &Path) {
    let sqlite_dir = home.join("sqlite");
    fs::create_dir_all(&sqlite_dir).unwrap();
    let old_ms = now_ms() - 300_000;

    let state = Connection::open(sqlite_dir.join("state_5.sqlite")).unwrap();
    state
        .execute_batch(
            "CREATE TABLE threads (
                id TEXT,
                rollout_path TEXT,
                created_at INTEGER,
                updated_at INTEGER,
                cwd TEXT,
                title TEXT,
                tokens_used INTEGER,
                git_branch TEXT,
                archived INTEGER
            );",
        )
        .unwrap();
    state
        .execute(
            "INSERT INTO threads VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                "codex-a",
                "/not-on-this-machine",
                old_ms / 1_000,
                old_ms / 1_000,
                project.to_string_lossy().to_string(),
                "Codex blocked",
                42_i64,
                "main",
                0_i64
            ],
        )
        .unwrap();
    drop(state);

    let history = Connection::open(sqlite_dir.join("thread_history_1.sqlite")).unwrap();
    history
        .execute_batch(
            "CREATE TABLE thread_items (
                thread_id TEXT,
                turn_id TEXT,
                item_id TEXT,
                created_at_ms INTEGER,
                item_type TEXT,
                item_json TEXT
            );
            CREATE TABLE thread_turns (
                thread_id TEXT,
                turn_id TEXT,
                status TEXT,
                started_at INTEGER,
                completed_at INTEGER,
                duration_ms INTEGER,
                error_json TEXT
            );",
        )
        .unwrap();
    insert_item(
        &history,
        "codex-a",
        old_ms + 1_000,
        "commandExecution",
        json!({"command": "cargo test"}),
    );
    history
        .execute(
            "INSERT INTO thread_turns VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "codex-a",
                "turn-a",
                "inProgress",
                old_ms / 1_000,
                Option::<i64>::None,
                Option::<i64>::None,
                Option::<String>::None
            ],
        )
        .unwrap();
}

fn insert_item(
    connection: &Connection,
    thread_id: &str,
    at: i64,
    item_type: &str,
    item_json: Value,
) {
    connection
        .execute(
            "INSERT INTO thread_items VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                thread_id,
                "turn-a",
                format!("item-{at}"),
                at,
                item_type,
                serde_json::to_string(&item_json).unwrap()
            ],
        )
        .unwrap();
}

fn binary() -> PathBuf {
    for name in ["CARGO_BIN_EXE_they-work", "CARGO_BIN_EXE_they_work"] {
        if let Some(path) = std::env::var_os(name) {
            return PathBuf::from(path);
        }
    }
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("they-work")
}

fn run(fixture: &Fixture, args: &[&str]) -> Output {
    run_with_homes(fixture, args, &fixture.claude_home, &fixture.codex_home)
}

fn run_with_homes(
    fixture: &Fixture,
    args: &[&str],
    claude_home: &Path,
    codex_home: &Path,
) -> Output {
    Command::new(binary())
        .current_dir(fixture.temp.path())
        .env("THEYWORK_CLAUDE_HOME", claude_home)
        .env("THEYWORK_CODEX_HOME", codex_home)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected success\nstdout:\n{}\nstderr:\n{}",
        stdout(output),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn first_run_non_tty_prints_discovery_and_picker() {
    let fixture = Fixture::new();
    let output = run(&fixture, &[]);
    assert_success(&output);

    let text = stdout(&output);
    assert!(text.contains("THEY WORK — first run"));
    assert!(text.contains("WHAT WAS FOUND"));
    assert!(text.contains("WHAT THIS READS"));
    assert!(text.contains("PICK AN OFFICE"));
    assert!(text.contains("↑↓ choose   Enter open office   Tab guard office   q quit"));
    assert!(text.contains(fixture.project_a.to_str().unwrap()));
    assert!(text.contains(fixture.project_b.to_str().unwrap()));
}

#[test]
fn doctor_reports_fixture_homes() {
    let fixture = Fixture::new();
    let output = run(&fixture, &["--doctor"]);
    assert_success(&output);

    let text = stdout(&output);
    assert!(text.contains("claude_home=found"));
    assert!(text.contains("claude_store=readable projects=2 threads=2 active=2"));
    assert!(text.contains("codex_home=found"));
    assert!(text.contains("codex_store=readable projects=1 threads=1 active=1"));
}

#[test]
fn once_never_emits_transcript_terminal_controls() {
    let fixture = Fixture::new();
    let transcript = fixture
        .claude_home
        .join("projects/fixture-b/session-b.jsonl");
    let osc_title = "\u{1b}]0;AUDIT_TITLE\u{7}";
    append_jsonl(
        &transcript,
        json!({
            "type": "assistant",
            "timestamp": now_ms() + 1,
            "sessionId": "claude-b",
            "cwd": fixture.project_b.to_string_lossy(),
            "customTitle": osc_title,
            "message": {"content": [{
                "type": "tool_use",
                "id": "malicious",
                "name": "Bash",
                "input": {"command": "\u{1b}]52;c;ZGFuZ2Vy\u{7}"}
            }]}
        }),
    );

    let output = run(&fixture, &["--once"]);
    assert_success(&output);
    assert!(!output.stdout.contains(&0x1b), "{:?}", output.stdout);
    let text = stdout(&output);
    assert!(text.contains("␛]0;AUDIT_TITLE␇"), "{text}");
    assert!(text.contains("␛]52;c;ZGFuZ2Vy␇"), "{text}");
}

#[test]
fn doctor_explains_the_published_container_terminal_fallback() {
    let fixture = Fixture::new();
    let output = Command::new(binary())
        .current_dir(fixture.temp.path())
        .env("THEYWORK_CLAUDE_HOME", &fixture.claude_home)
        .env("THEYWORK_CODEX_HOME", &fixture.codex_home)
        .env("TERM", "xterm-256color")
        .env_remove("COLORTERM")
        .env_remove("TERM_PROGRAM")
        .env_remove("LANG")
        .env_remove("LC_ALL")
        .env_remove("LC_CTYPE")
        .env_remove("NO_COLOR")
        .env_remove("THEYWORK_COLOR")
        .env_remove("THEYWORK_ENCODING")
        .env_remove("THEYWORK_SEXTANTS")
        .env_remove("THEYWORK_QUADRANTS")
        .arg("--doctor")
        .output()
        .unwrap();
    assert_success(&output);

    let text = stdout(&output);
    assert!(text.contains(
        "terminal_env TERM=\"xterm-256color\" COLORTERM=unset TERM_PROGRAM=unset LANG=unset LC_ALL=unset LC_CTYPE=unset"
    ));
    assert!(text.contains("terminal_color depth=palette256"));
    assert!(text.contains("terminal_encoding encoding=quadrants"));
    assert!(text.contains("sextant glyph coverage cannot be queried"));
    assert!(text.contains("terminal_graphics protocol=none probe=\"skipped:not_a_tty\""));
    assert!(text.contains("terminal_frame mode=cells covered_cells=unknown source_pixels=unknown"));
    assert!(text.contains("terminal_action=set_THEYWORK_ENCODING=sextants"));
}

#[test]
fn doctor_explains_an_explicit_color_override() {
    let fixture = Fixture::new();
    let output = run(&fixture, &["--doctor", "--color", "true"]);
    assert_success(&output);

    let text = stdout(&output);
    assert!(text.contains("terminal_color depth=truecolor"));
    assert!(text.contains("reason=\"colour selected by THEYWORK_COLOR=true\""));
}

#[test]
fn no_color_remains_authoritative_over_explicit_truecolor() {
    let fixture = Fixture::new();
    let output = Command::new(binary())
        .current_dir(fixture.temp.path())
        .env("THEYWORK_CLAUDE_HOME", &fixture.claude_home)
        .env("THEYWORK_CODEX_HOME", &fixture.codex_home)
        .env("NO_COLOR", "1")
        .args(["--doctor", "--color", "true"])
        .output()
        .unwrap();
    assert_success(&output);

    let text = stdout(&output);
    assert!(text.contains("terminal_color depth=none"), "{text}");
    assert!(text.contains("NO_COLOR"), "{text}");
}

#[test]
fn doctor_retains_recorded_codex_projects_when_the_worktree_is_not_mounted() {
    let fixture = Fixture::new();
    let state = Connection::open(fixture.codex_home.join("sqlite/state_5.sqlite")).unwrap();
    state
        .execute(
            "UPDATE threads SET cwd = ?1",
            [fixture
                .temp
                .path()
                .join("not-mounted/project")
                .to_string_lossy()],
        )
        .unwrap();

    let output = run(&fixture, &["--doctor"]);
    assert_success(&output);
    let text = stdout(&output);
    assert!(text.contains("codex_store=readable projects=1 threads=1 active=1"));
    assert!(!text.contains("projects=unresolved"));
}

#[test]
fn doctor_fails_with_no_homes_and_explains_both_paths() {
    let fixture = Fixture::new();
    let claude = fixture.temp.path().join("missing-claude");
    let codex = fixture.temp.path().join("missing-codex");
    let output = run_with_homes(&fixture, &["--doctor"], &claude, &codex);
    assert!(!output.status.success());

    let text = stdout(&output);
    assert!(text.contains("claude_home=missing"));
    assert!(text.contains("codex_home=missing"));
    assert_eq!(text.matches("reason=home is not a directory").count(), 2);
}

#[test]
fn doctor_reports_an_installed_but_empty_home() {
    let fixture = Fixture::new();
    let empty_claude = fixture.temp.path().join("empty-claude");
    let empty_codex = fixture.temp.path().join("empty-codex");
    fs::create_dir_all(&empty_claude).unwrap();
    fs::create_dir_all(&empty_codex).unwrap();

    let output = run_with_homes(&fixture, &["--doctor"], &empty_claude, &empty_codex);
    assert_success(&output);

    let text = stdout(&output);
    assert!(text.contains(
        "claude_store=readable projects=0 threads=0 active=0 status=empty action=installed_never_run_here note=\"installed, never run here\""
    ));
    assert!(text.contains(
        "codex_store=readable projects=0 threads=0 active=0 status=empty action=installed_never_run_here note=\"installed, never run here\""
    ));

    let output = run_with_homes(&fixture, &[], &empty_claude, &empty_codex);
    assert_success(&output);
    let text = stdout(&output);
    assert!(text.contains("claude_store=readable projects=0 threads=0 active=0"));
    assert!(text.contains("codex_store=readable projects=0 threads=0 active=0"));
    assert!(text.contains("installed, never run here"));
}

#[test]
fn doctor_reports_one_agent_without_treating_it_as_a_failure() {
    let fixture = Fixture::new();
    let missing_codex = fixture.temp.path().join("no-codex");
    let output = run_with_homes(
        &fixture,
        &["--doctor"],
        &fixture.claude_home,
        &missing_codex,
    );
    assert_success(&output);

    let text = stdout(&output);
    assert!(text.contains("claude_store=readable projects=2 threads=2 active=2"));
    assert!(text.contains("codex_home=missing"));
    assert!(text.contains("override=THEYWORK_CODEX_HOME action=set_override"));
}

#[cfg(unix)]
#[test]
fn doctor_reports_owner_and_permissions_for_an_unreadable_home() {
    let fixture = Fixture::new();
    let unreadable = fixture.temp.path().join("unreadable-claude");
    fs::create_dir_all(&unreadable).unwrap();
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();

    let output = run_with_homes(&fixture, &["--doctor"], &unreadable, &fixture.codex_home);
    assert!(!output.status.success());

    let text = stdout(&output);
    assert!(text.contains("claude_home=found"));
    assert!(text.contains("status=unreadable action=check_permissions_or_set_THEYWORK_CLAUDE_HOME"));
    assert!(text.contains("owner="));
    assert!(text.contains("permissions=0o000"));

    fs::set_permissions(
        fixture.codex_home.join("sqlite/state_5.sqlite"),
        fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    let output = run_with_homes(
        &fixture,
        &["--doctor"],
        &fixture.claude_home,
        &fixture.codex_home,
    );
    assert!(!output.status.success());
    let text = stdout(&output);
    assert!(text.contains("codex_store=unavailable"));
    assert!(text.contains("Codex state database cannot be read"));
    assert!(text.contains("permissions=0o000"));
}

#[test]
fn doctor_marks_a_windows_shaped_path_as_unusual_and_requests_confirmation() {
    let fixture = Fixture::new();
    let unusual = PathBuf::from("/mnt/c/Users/Example/.codex");
    let missing_claude = fixture.temp.path().join("missing-claude");
    let output = run_with_homes(&fixture, &["--doctor"], &missing_claude, &unusual);
    assert!(!output.status.success());

    let text = stdout(&output);
    assert!(text.contains("codex_home=missing"));
    assert!(text.contains("source=unusual"));
    assert!(text.contains("confirm_path=true"));
}

#[test]
fn once_lists_blocked_project_first_and_reports_unknown_waiting_state() {
    let fixture = Fixture::new();
    let output = run(&fixture, &["--once"]);
    assert_success(&output);

    let text = stdout(&output);
    assert!(text.contains("projects=2 workers=3"));
    assert!(text.contains("workers=2"));
    assert!(text.contains("workers=1"));
    assert!(text.contains("waiting_on=\"waiting, no pending command identified\""));
    assert!(text.contains("status=blocked"));
    assert!(!text.contains("status=blocked activity=idle"));
    assert!(text.contains("status=idle"));
    assert!(
        text.find("office=").unwrap() < text.find("status=idle").unwrap(),
        "blocked office should be emitted before idle office:\n{text}"
    );
    assert!(
        text.find("status=blocked").unwrap() < text.find("status=idle").unwrap(),
        "blocked worker should be emitted before idle worker:\n{text}"
    );
}

#[test]
fn headless_exit_after_runs_the_full_polling_loop() {
    let fixture = Fixture::new();
    let output = run(&fixture, &["--all", "--headless", "--exit-after", "250ms"]);
    assert_success(&output);

    let text = stdout(&output);
    assert!(text.contains("they-work --headless"));
    assert!(text.contains("target_fps=10"));
    assert!(text.contains("frames="));
    assert!(text.contains("roster initial_offices="));
    assert!(text.contains("rss_before_bytes="));
    assert!(text.contains("rss_after_bytes="));
    assert!(text.contains("cpu_seconds="));
    assert!(text.contains("cpu_average_percent="));
    assert!(!text.contains("THEY WORK — first run"));
}

#[test]
fn project_scopes_once_and_persists_only_with_config_dir() {
    let fixture = Fixture::new();
    let project_a = fixture.project_a.to_str().unwrap();
    let project_b = fixture.project_b.to_str().unwrap();

    let output = run(&fixture, &["--once", "--project", project_a]);
    assert_success(&output);
    let text = stdout(&output);
    assert!(text.contains("projects=1"));
    assert!(text.contains(project_a));
    assert!(!text.contains(project_b));
    assert!(!fixture.config_dir.join("project").exists());

    let output = run(
        &fixture,
        &[
            "--once",
            "--project",
            project_b,
            "--config-dir",
            fixture.config_dir.to_str().unwrap(),
        ],
    );
    assert_success(&output);
    assert_eq!(
        fs::read_to_string(fixture.config_dir.join("project")).unwrap(),
        format!("{project_b}\n")
    );
}

#[test]
fn first_run_without_homes_explains_overrides_and_stops() {
    let fixture = Fixture::new();
    let claude = fixture.temp.path().join("missing-claude");
    let codex = fixture.temp.path().join("missing-codex");
    let output = run_with_homes(&fixture, &[], &claude, &codex);
    assert_success(&output);

    let text = stdout(&output);
    assert!(text.contains("No agent home was found; no empty office will be opened."));
    assert!(text.contains("Set THEYWORK_CLAUDE_HOME or THEYWORK_CODEX_HOME"));
    assert!(text.contains("PICK AN OFFICE"));
    assert!(text.contains("No active offices found yet."));
    assert!(!text.contains("office="));
}

#[test]
fn missing_config_directory_is_a_clear_error() {
    let fixture = Fixture::new();
    let missing = fixture.temp.path().join("missing-config");
    let project = fixture.project_a.to_str().unwrap();
    let output = run(
        &fixture,
        &[
            "--once",
            "--project",
            project,
            "--config-dir",
            missing.to_str().unwrap(),
        ],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("config directory"));
    assert!(!missing.exists());
}

#[test]
fn demo_once_does_not_open_agent_homes() {
    let fixture = Fixture::new();
    let trap = fixture.temp.path().join("not-a-home");
    File::create(&trap).unwrap();
    let output = run_with_homes(&fixture, &["--demo", "--once"], &trap, &trap);
    assert_success(&output);

    let text = stdout(&output);
    assert!(text.contains("they-work --once"));
    assert!(text.contains("projects="));
    assert!(!text.contains("collector_error="));
}

#[test]
fn invalid_args_report_clear_failure() {
    let fixture = Fixture::new();
    let output = run(&fixture, &["--color", "purple"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("invalid --color value \"purple\"; use auto, true, 256, or none"));
}
