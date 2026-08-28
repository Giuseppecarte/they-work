use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use theywork_collect::{
    normalize_office_path, sources as build_sources, ClaudeSource, CodexSource, Config,
    DEFAULT_ACTIVE_WITHIN,
};
use theywork_core::{Activity, Agent, Event, EventKind, Source, World};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "theywork-collect-{stamp}-{}-{sequence}",
            std::process::id()
        ));
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

fn append_jsonl(path: &Path, value: Value) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    serde_json::to_writer(&mut file, &value).unwrap();
    file.write_all(b"\n").unwrap();
}

fn set_modified_millis(path: &Path, millis: i64) {
    let modified = UNIX_EPOCH + Duration::from_millis(millis as u64);
    fs::File::open(path)
        .unwrap()
        .set_modified(modified)
        .unwrap();
}

fn has_activity(events: &[Event], expected: &Activity) -> bool {
    events.iter().any(|event| match &event.kind {
        EventKind::Acted(activity) => activity == expected,
        _ => false,
    })
}

fn has_worker(events: &[Event], worker: &str) -> bool {
    events.iter().any(|event| event.worker.0 == worker)
}

#[test]
fn claude_maps_tools_text_turns_and_names() {
    let temp = TempDir::new();
    let session_dir = temp.path().join("projects/demo");
    fs::create_dir_all(session_dir.join("session-123/subagents")).unwrap();
    let transcript = session_dir.join("session-123.jsonl");

    append_jsonl(
        &transcript,
        json!({
            "type": "system",
            "timestamp": "2026-08-27T17:38:22.306Z",
            "sessionId": "session-123",
            "cwd": "/workspace/app",
            "gitBranch": "main",
            "customTitle": "Custom title",
            "aiTitle": "AI title"
        }),
    );
    append_jsonl(
        &transcript,
        json!({
            "type": "user",
            "timestamp": "2026-08-27T17:38:23Z",
            "sessionId": "session-123",
            "cwd": "/workspace/app",
            "gitBranch": "main",
            "message": {"content": [{"type": "text", "text": "go"}]}
        }),
    );

    let tools = [
        (
            "Bash",
            json!({"command": "cargo test"}),
            Activity::Typing {
                detail: "cargo test".into(),
            },
        ),
        (
            "Read",
            json!({"file_path": "src/lib.rs"}),
            Activity::Reading {
                detail: "src/lib.rs".into(),
            },
        ),
        (
            "Edit",
            json!({"file_path": "src/lib.rs"}),
            Activity::Editing {
                detail: "src/lib.rs".into(),
            },
        ),
        (
            "Write",
            json!({"file_path": "src/main.rs"}),
            Activity::Editing {
                detail: "src/main.rs".into(),
            },
        ),
        (
            "NotebookEdit",
            json!({"file_path": "notebook.ipynb"}),
            Activity::Editing {
                detail: "notebook.ipynb".into(),
            },
        ),
        (
            "Grep",
            json!({"pattern": "EventKind"}),
            Activity::Searching {
                detail: "EventKind".into(),
            },
        ),
        (
            "Glob",
            json!({"pattern": "**/*.rs"}),
            Activity::Searching {
                detail: "**/*.rs".into(),
            },
        ),
        (
            "WebSearch",
            json!({"query": "rust sqlite"}),
            Activity::Searching {
                detail: "rust sqlite".into(),
            },
        ),
        (
            "WebFetch",
            json!({"url": "https://example.test"}),
            Activity::Searching {
                detail: "https://example.test".into(),
            },
        ),
        ("Task", json!({}), Activity::Thinking),
        ("Agent", json!({}), Activity::Thinking),
        (
            "AskUserQuestion",
            json!({"questions": [{"question": "Deploy now?"}]}),
            Activity::Waiting {
                detail: "Deploy now?".into(),
            },
        ),
        ("UnknownTool", json!({}), Activity::Thinking),
    ];
    for (index, (name, input, _)) in tools.iter().enumerate() {
        append_jsonl(
            &transcript,
            json!({
                "type": "assistant",
                "timestamp": format!("2026-08-27T17:38:{:02}Z", 24 + index),
                "sessionId": "session-123",
                "cwd": "/workspace/app",
                "gitBranch": "main",
                "message": {"content": [{"type": "tool_use", "name": name, "input": input}]}
            }),
        );
    }
    append_jsonl(
        &transcript,
        json!({
            "type": "assistant",
            "timestamp": "2026-08-27T17:39:00Z",
            "sessionId": "session-123",
            "cwd": "/workspace/app",
            "message": {"content": [{"type": "text", "text": "finished\nwith details"}]}
        }),
    );

    let subagent = session_dir.join("session-123/subagents/agent-abc.jsonl");
    append_jsonl(
        &subagent,
        json!({
            "type": "assistant",
            "timestamp": "2026-08-27T17:39:01Z",
            "sessionId": "session-123",
            "agentId": "agent-abc",
            "agentName": "lint worker",
            "cwd": "/workspace/app",
            "message": {"content": [{"type": "text", "text": "linting"}]}
        }),
    );

    let mut source = ClaudeSource::new(temp.path());
    let events = source.poll(1_000).unwrap();

    for (_, _, expected) in tools {
        assert!(
            has_activity(&events, &expected),
            "missing mapped activity: {expected:?}"
        );
    }
    assert!(has_activity(
        &events,
        &Activity::Talking {
            detail: "finished with details".into()
        }
    ));
    assert!(events
        .iter()
        .any(|event| matches!(&event.kind, EventKind::Turn { in_flight: true })));
    assert!(events
        .iter()
        .any(|event| matches!(&event.kind, EventKind::Turn { in_flight: false })));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Seen { name, git_branch: Some(branch) }
            if name == "Custom title" && branch == "main"
    )));
    assert!(events.iter().any(|event| {
        event.worker.0 == "agent-abc"
            && matches!(&event.kind, EventKind::Seen { name, .. } if name == "sub:lint worker")
    }));
}

#[test]
fn claude_tails_incrementally_rotates_and_discovers_sessions() {
    let temp = TempDir::new();
    let project_dir = temp.path().join("projects/demo");
    fs::create_dir_all(&project_dir).unwrap();
    let transcript = project_dir.join("session-rotate.jsonl");
    let initial = json!({
        "type": "user",
        "timestamp": "2026-08-27T17:40:00Z",
        "sessionId": "session-rotate",
        "cwd": "/workspace/app",
        "message": {"content": [{"type": "text", "text": "x".repeat(600)}]}
    });
    append_jsonl(&transcript, initial);

    let mut source = ClaudeSource::new(temp.path());
    assert!(!source.poll(1_000).unwrap().is_empty());
    assert!(source.poll(1_001).unwrap().is_empty());

    let new_session = project_dir.join("session-new.jsonl");
    append_jsonl(
        &new_session,
        json!({
            "type": "assistant",
            "timestamp": "2026-08-27T17:40:01Z",
            "sessionId": "session-new",
            "cwd": "/workspace/new",
            "message": {"content": [{"type": "text", "text": "new session"}]}
        }),
    );
    assert!(has_worker(&source.poll(1_002).unwrap(), "session-new"));

    let rotated = json!({
        "type": "assistant",
        "timestamp": "2026-08-27T17:40:02Z",
        "sessionId": "session-rotate",
        "cwd": "/workspace/app",
        "message": {"content": [{"type": "text", "text": "after rotation"}]}
    });
    let mut rotated_bytes = serde_json::to_vec(&rotated).unwrap();
    rotated_bytes.push(b'\n');
    fs::write(&transcript, rotated_bytes).unwrap();

    let rotated_events = source.poll(1_003).unwrap();
    assert!(has_activity(
        &rotated_events,
        &Activity::Talking {
            detail: "after rotation".into()
        }
    ));
}

#[test]
fn claude_carries_real_timestamps_and_accumulates_usage() {
    let temp = TempDir::new();
    let project_dir = temp.path().join("projects/demo");
    fs::create_dir_all(&project_dir).unwrap();
    let transcript = project_dir.join("session-timestamps.jsonl");

    append_jsonl(
        &transcript,
        json!({
            "type": "system",
            "sessionId": "session-timestamps",
            "cwd": "/workspace/app",
            "customTitle": "Timestamp worker"
        }),
    );
    append_jsonl(
        &transcript,
        json!({
            "type": "user",
            "timestamp": 7_000_000,
            "sessionId": "session-timestamps",
            "cwd": "/workspace/app",
            "message": {"content": [{"type": "text", "text": "go"}]}
        }),
    );
    append_jsonl(
        &transcript,
        json!({
            "type": "assistant",
            "sessionId": "session-timestamps",
            "cwd": "/workspace/app",
            "message": {
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 20,
                    "cache_creation_input_tokens": 30,
                    "cache_read_input_tokens": 40
                },
                "content": [{"type": "text", "text": "first"}]
            }
        }),
    );
    append_jsonl(
        &transcript,
        json!({
            "type": "assistant",
            "timestamp": 8_000_000,
            "sessionId": "session-timestamps",
            "cwd": "/workspace/app",
            "message": {
                "usage": {
                    "input_tokens": 1,
                    "output_tokens": 1,
                    "cache_creation_input_tokens": 1,
                    "cache_read_input_tokens": 1
                },
                "content": [{"type": "text", "text": "second"}]
            }
        }),
    );
    set_modified_millis(&transcript, 6_000_000);

    let mut source = ClaudeSource::with_paths_and_active_within(
        temp.path(),
        Vec::new(),
        Duration::from_secs(60 * 60),
    );
    let now = 9_000_000;
    let events = source.poll(now).unwrap();
    assert!(events
        .iter()
        .any(|event| { event.worker.0 == "session-timestamps" && event.at == 6_000_000 }));
    assert!(events.iter().any(|event| {
        event.worker.0 == "session-timestamps"
            && event.at == 7_000_000
            && matches!(&event.kind, EventKind::Tokens(100))
    }));
    assert!(events.iter().any(|event| {
        event.worker.0 == "session-timestamps"
            && event.at == 8_000_000
            && matches!(&event.kind, EventKind::Tokens(104))
    }));
    assert!(!events.iter().any(|event| event.at == now));
}

#[test]
fn claude_uses_file_mtime_for_all_untimestamped_lines_and_honors_recency() {
    let temp = TempDir::new();
    let project_dir = temp.path().join("projects/demo");
    fs::create_dir_all(&project_dir).unwrap();

    let recent = project_dir.join("session-mtime.jsonl");
    append_jsonl(
        &recent,
        json!({
            "type": "system",
            "sessionId": "session-mtime",
            "cwd": "/workspace/app"
        }),
    );
    set_modified_millis(&recent, 4_000_000);

    let mut source = ClaudeSource::with_paths_and_active_within(
        temp.path(),
        Vec::new(),
        Duration::from_secs(60),
    );
    let recent_events = source.poll(4_030_000).unwrap();
    assert!(recent_events
        .iter()
        .any(|event| event.worker.0 == "session-mtime" && event.at == 4_000_000));

    let ancient = project_dir.join("session-ancient.jsonl");
    append_jsonl(
        &ancient,
        json!({
            "type": "system",
            "sessionId": "session-ancient",
            "cwd": "/workspace/old"
        }),
    );
    set_modified_millis(&ancient, 1_000);
    let mut bounded = ClaudeSource::with_paths_and_active_within(
        temp.path(),
        vec![PathBuf::from("/workspace/old")],
        Duration::from_secs(60),
    );
    assert!(bounded.poll(1_000_000).unwrap().is_empty());
}

#[test]
fn claude_collapses_nested_workdirs_to_the_nearest_git_root() {
    let temp = TempDir::new();
    let repo = temp.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    let project_dir = temp.path().join("projects/demo");
    fs::create_dir_all(&project_dir).unwrap();
    let transcript = project_dir.join("session-repo.jsonl");

    append_jsonl(
        &transcript,
        json!({
            "type": "system",
            "timestamp": 1_000_000,
            "sessionId": "session-repo",
            "cwd": repo.join("apps/web").to_string_lossy(),
            "customTitle": "repo worker"
        }),
    );
    append_jsonl(
        &transcript,
        json!({
            "type": "assistant",
            "timestamp": 2_000_000,
            "sessionId": "session-repo",
            "cwd": repo.join("docs/design/mock").to_string_lossy(),
            "message": {"content": [{"type": "text", "text": "design"}]}
        }),
    );

    let root = normalize_office_path(&repo.to_string_lossy());
    let mut source = ClaudeSource::new(temp.path());
    let events = source.poll(1_000).unwrap();
    assert!(!events.is_empty());
    assert!(events.iter().all(|event| event.office_path == root));
    assert!(events.iter().all(|event| event.office.0 == root));
}
fn create_codex_fixture(home: &Path) {
    let sqlite_dir = home.join("sqlite");
    fs::create_dir_all(&sqlite_dir).unwrap();

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
                "thread-active",
                "/not/on/this/machine",
                1,
                2,
                "/workspace/app",
                "Dev 3",
                42,
                "feature/demo",
                0
            ],
        )
        .unwrap();
    state
        .execute(
            "INSERT INTO threads VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                "thread-archived",
                "/not/on/this/machine",
                1,
                2,
                "/workspace/old",
                "old",
                99,
                "main",
                1
            ],
        )
        .unwrap();
    state
        .execute(
            "INSERT INTO threads VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                "thread-other",
                "/not/on/this/machine",
                1,
                2,
                "/workspace/other",
                "Other",
                7,
                "main",
                0
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

    let items = [
        (
            100,
            "commandExecution",
            json!({"command": "cargo test", "cwd": "/workspace/app"}),
        ),
        (200, "reasoning", json!({"summary": "considering"})),
        (300, "agentMessage", json!({"message": "done"})),
        (400, "mcpToolCall", json!({"name": "search"})),
        (500, "fileChange", json!({"path": "src/main.rs"})),
        (600, "userMessage", json!({"message": "continue"})),
        (700, "contextCompaction", json!({})),
    ];
    for (at, item_type, item_json) in items {
        insert_item(&history, "thread-active", at, item_type, item_json);
    }
    history
        .execute(
            "INSERT INTO thread_turns VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "thread-active",
                "turn-a",
                "completed",
                100,
                150,
                50,
                Option::<String>::None
            ],
        )
        .unwrap();
    history
        .execute(
            "INSERT INTO thread_turns VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "thread-active",
                "turn-b",
                "inProgress",
                200,
                Option::<i64>::None,
                Option::<i64>::None,
                Some(r#"{"message":"permission denied"}"#)
            ],
        )
        .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn insert_codex_m3_thread(
    state: &Connection,
    id: &str,
    cwd: &str,
    title: Option<&str>,
    tokens_used: i64,
    git_branch: &str,
    archived: i64,
    name: Option<&str>,
    agent_nickname: Option<&str>,
    updated_at_ms: i64,
) {
    state
        .execute(
            "INSERT INTO threads (
                id, rollout_path, created_at, updated_at, cwd, title,
                tokens_used, git_branch, archived, name, agent_nickname, updated_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id,
                "/not/on/this/machine",
                0,
                0,
                cwd,
                title,
                tokens_used,
                git_branch,
                archived,
                name,
                agent_nickname,
                updated_at_ms
            ],
        )
        .unwrap();
}

fn create_codex_m3_fixture(home: &Path) {
    let sqlite_dir = home.join("sqlite");
    fs::create_dir_all(&sqlite_dir).unwrap();

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
                archived INTEGER,
                name TEXT,
                agent_nickname TEXT,
                updated_at_ms INTEGER
            );
            CREATE TABLE thread_spawn_edges (
                parent_thread_id TEXT,
                child_thread_id TEXT,
                status TEXT
            );",
        )
        .unwrap();

    let assessor_title =
        "The following is the Codex agent history whose request action you are assessing";
    insert_codex_m3_thread(
        &state,
        "developer-edge",
        "/workspace/repo/apps/web",
        Some("/goal edge"),
        42,
        "main",
        0,
        Some("Dev edge"),
        None,
        1_900_000,
    );
    insert_codex_m3_thread(
        &state,
        "assessor-edge",
        "/workspace/repo/apps/web",
        Some(assessor_title),
        0,
        "main",
        0,
        Some("reviewer"),
        None,
        2_000_000,
    );
    insert_codex_m3_thread(
        &state,
        "developer-fallback",
        "/workspace/repo/apps/web",
        Some("/goal fallback"),
        7,
        "main",
        0,
        Some("Dev fallback"),
        None,
        1_700_000,
    );
    insert_codex_m3_thread(
        &state,
        "assessor-fallback",
        "/workspace/repo/apps/web",
        Some(assessor_title),
        0,
        "main",
        0,
        None,
        Some("reviewer fallback"),
        2_000_000,
    );
    insert_codex_m3_thread(
        &state,
        "nickname-worker",
        "/workspace/repo/apps/web",
        Some("/goal nickname"),
        3,
        "main",
        0,
        None,
        Some("Dev nickname"),
        1_900_000,
    );
    insert_codex_m3_thread(
        &state,
        "short-id-worker",
        "/workspace/repo/apps/web",
        Some("/goal fallback-name"),
        4,
        "main",
        0,
        Some("/path-as-name"),
        Some(""),
        1_900_000,
    );
    state
        .execute(
            "INSERT INTO thread_spawn_edges (parent_thread_id, child_thread_id, status)
             VALUES (?1, ?2, ?3)",
            params!["developer-edge", "assessor-edge", "pending"],
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
        "developer-edge",
        1_800_000,
        "commandExecution",
        json!({"command": "old command", "status": "completed"}),
    );
    insert_item(
        &history,
        "developer-edge",
        1_950_000,
        "commandExecution",
        json!({"command": "cargo deploy", "status": "awaitingApproval"}),
    );
    insert_item(
        &history,
        "developer-fallback",
        1_650_000,
        "commandExecution",
        json!({"command": "rm -rf target", "status": "running"}),
    );
    history
        .execute(
            "INSERT INTO thread_turns VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "developer-edge",
                "edge-turn",
                "inProgress",
                1_900,
                Option::<i64>::None,
                Option::<i64>::None,
                Option::<String>::None
            ],
        )
        .unwrap();
    history
        .execute(
            "INSERT INTO thread_turns VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "developer-fallback",
                "fallback-turn",
                "inProgress",
                1_700,
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

#[test]
fn codex_reads_roster_items_and_turn_state_incrementally() {
    let temp = TempDir::new();
    create_codex_fixture(temp.path());
    let mut source = CodexSource::new(temp.path());

    let events = source.poll(10_000).unwrap();
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Seen { name, git_branch: Some(branch) }
            if event.worker.0 == "thread-active" && name == "Dev 3" && branch == "feature/demo"
    )));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Tokens(42) if event.worker.0 == "thread-active"
    )));
    assert!(events.iter().any(|event| {
        event.at == 2_000
            && matches!(&event.kind, EventKind::Tokens(42) if event.worker.0 == "thread-active")
    }));
    assert!(events.iter().any(|event| {
        event.at == 2_000
            && matches!(
                &event.kind,
                EventKind::Seen { name, .. } if name == "Dev 3"
            )
    }));
    assert!(!has_worker(&events, "thread-archived"));
    assert!(has_worker(&events, "thread-other"));

    assert!(has_activity(
        &events,
        &Activity::Typing {
            detail: "cargo test".into()
        }
    ));
    assert!(has_activity(&events, &Activity::Thinking));
    assert!(has_activity(
        &events,
        &Activity::Talking {
            detail: "done".into()
        }
    ));
    assert!(has_activity(
        &events,
        &Activity::Searching {
            detail: "search".into()
        }
    ));
    assert!(has_activity(
        &events,
        &Activity::Editing {
            detail: "src/main.rs".into()
        }
    ));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Turn { in_flight: true } if event.worker.0 == "thread-active"
    )));
    assert!(!events.iter().any(|event| matches!(&event.kind, EventKind::Turn { in_flight: false } if event.worker.0 == "thread-active")));
    assert!(has_activity(
        &events,
        &Activity::Error {
            detail: "permission denied".into()
        }
    ));
    assert!(events.iter().any(|event| event.at == 200_000 && matches!(&event.kind, EventKind::Acted(Activity::Error { detail }) if detail == "permission denied")));
    assert!(!events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Acted(Activity::Talking { detail }) if detail == "continue"
    )));

    let quiet = source.poll(10_001).unwrap();
    assert!(!quiet.iter().any(|event| matches!(
        &event.kind,
        EventKind::Acted(_)
            | EventKind::Turn { .. }
            | EventKind::Seen { .. }
            | EventKind::Tokens(_)
    )));

    let history_path = temp.path().join("sqlite/thread_history_1.sqlite");
    let history = Connection::open(history_path).unwrap();
    insert_item(
        &history,
        "thread-active",
        800,
        "agentMessage",
        json!({"message": "next"}),
    );
    insert_item(
        &history,
        "thread-other",
        650,
        "agentMessage",
        json!({"message": "late"}),
    );
    history
        .execute(
            "UPDATE thread_turns SET status = 'completed', completed_at = 800, error_json = NULL WHERE thread_id = 'thread-active' AND turn_id = 'turn-b'",
            [],
        )
        .unwrap();
    drop(history);

    let next = source.poll(10_002).unwrap();
    assert!(has_activity(
        &next,
        &Activity::Talking {
            detail: "next".into()
        }
    ));
    assert!(next.iter().any(|event| {
        event.worker.0 == "thread-other"
            && event.at == 650
            && matches!(&event.kind, EventKind::Acted(Activity::Talking { detail }) if detail == "late")
    }));
    assert!(next.iter().any(|event| matches!(
        &event.kind,
        EventKind::Turn { in_flight: false } if event.worker.0 == "thread-active"
    )));

    let mut filtered = CodexSource::with_paths(temp.path(), vec![PathBuf::from("/workspace/app")]);
    let filtered_events = filtered.poll(10_003).unwrap();
    assert!(has_worker(&filtered_events, "thread-active"));
    assert!(!has_worker(&filtered_events, "thread-other"));
}

#[test]
fn codex_applies_configured_recency_bound() {
    let temp = TempDir::new();
    create_codex_fixture(temp.path());
    let mut source = CodexSource::with_paths_and_active_within(
        temp.path(),
        Vec::new(),
        Duration::from_millis(1),
    );
    assert!(source.poll(10_000).unwrap().is_empty());
}

#[test]
fn codex_hides_assessors_and_correlates_waiting_developers() {
    let temp = TempDir::new();
    create_codex_m3_fixture(temp.path());
    let mut source = CodexSource::with_paths_and_active_within(
        temp.path(),
        Vec::new(),
        Duration::from_secs(60 * 60),
    );

    let events = source.poll(2_000_000).unwrap();
    assert!(!has_worker(&events, "assessor-edge"));
    assert!(!has_worker(&events, "assessor-fallback"));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Acted(Activity::Waiting { detail })
            if event.worker.0 == "developer-edge" && detail.contains("cargo deploy")
    )));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Acted(Activity::Waiting { detail })
            if event.worker.0 == "developer-fallback"
                && detail.contains("rm -rf target")
    )));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Seen { name, .. }
            if event.worker.0 == "nickname-worker" && name == "Dev nickname"
    )));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Seen { name, .. }
            if event.worker.0 == "short-id-worker" && name == "short-id"
    )));

    let mut world = World::new();
    for event in events {
        world.apply(event);
    }
    let waiting_workers: Vec<_> = world
        .offices()
        .flat_map(|office| office.workers.iter())
        .filter(|worker| matches!(worker.activity, Activity::Waiting { .. }))
        .collect();
    assert!(waiting_workers
        .iter()
        .any(|worker| worker.id.0 == "developer-edge"));
    assert!(waiting_workers
        .iter()
        .any(|worker| worker.id.0 == "developer-fallback"));
    assert!(world
        .offices()
        .flat_map(|office| office.workers.iter())
        .all(|worker| !worker.name.starts_with('/')));
}
#[test]
fn sources_wires_existing_homes_and_skips_missing_ones() {
    let temp = TempDir::new();
    let claude_home = temp.path().join("claude");
    fs::create_dir_all(&claude_home).unwrap();
    let codex_home = temp.path().join("codex");
    create_codex_fixture(&codex_home);

    let configured = Config {
        claude_home: Some(claude_home),
        codex_home: Some(codex_home),
        active_within: DEFAULT_ACTIVE_WITHIN,
        only_paths: Vec::new(),
    };
    let sources = build_sources(&configured);
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].name(), "claude");
    assert_eq!(sources[1].name(), "codex");

    let missing = Config {
        claude_home: Some(temp.path().join("missing-claude")),
        codex_home: Some(temp.path().join("missing-codex")),
        active_within: DEFAULT_ACTIVE_WITHIN,
        only_paths: Vec::new(),
    };
    assert!(build_sources(&missing).is_empty());
}

#[test]
fn real_machine_smoke_when_homes_exist() {
    let config = Config::discover();
    if config.claude_home.is_none() && config.codex_home.is_none() {
        eprintln!("real-machine smoke skipped: no agent homes");
        return;
    }
    let mut sources = build_sources(&config);
    if sources.is_empty() {
        eprintln!("real-machine smoke skipped: no readable agent stores");
        return;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_millis() as i64;
    let mut events = Vec::new();
    for source in &mut sources {
        events.extend(source.poll(now).unwrap_or_else(|error| {
            panic!("real-machine smoke poll failed: {error}");
        }));
    }
    if events.is_empty() {
        eprintln!("real-machine smoke skipped: no events in active horizon");
        return;
    }
    events.sort_by_key(|event| event.at);
    for event in &events {
        assert_eq!(
            event.office.0,
            normalize_office_path(&event.office_path),
            "collector emitted a non-normalized office id",
        );
    }
    let mut world = World::new();
    for event in events {
        world.apply(event);
    }
    let assessor_prefix =
        "The following is the Codex agent history whose request action you are assessing";
    assert!(world
        .offices()
        .flat_map(|office| office.workers.iter())
        .all(|worker| {
            !worker.name.starts_with(assessor_prefix) && !worker.name.starts_with('/')
        }));
    let claude_workers: Vec<_> = world
        .offices()
        .flat_map(|office| office.workers.iter())
        .filter(|worker| worker.agent == Agent::Claude)
        .collect();
    if config.claude_home.is_some() {
        assert!(
            !claude_workers.is_empty(),
            "Claude home exists but no active workers were collected"
        );
        assert!(claude_workers.iter().any(|worker| worker.last_seen != now));
        assert!(claude_workers.iter().any(|worker| worker.tokens_used > 0));
    }
    let waiting_developers = world
        .offices()
        .flat_map(|office| office.workers.iter())
        .filter(|worker| {
            worker.agent == Agent::Codex && matches!(worker.activity, Activity::Waiting { .. })
        })
        .count();
    println!(
        "real-machine smoke: any developer Waiting = {}",
        waiting_developers > 0
    );
    for office in world.offices() {
        println!(
            "real-machine smoke: office={} workers={}",
            office.path,
            office.workers.len()
        );
    }
    let hugo_path = normalize_office_path("/home/gc/AIStudio/projects/hugo-ai");
    let hugo_count = world
        .offices()
        .find(|office| office.path == hugo_path)
        .map(|office| office.workers.len())
        .unwrap_or(0);
    println!("real-machine smoke: Hugo worker count after M3 = {hugo_count}");
    assert!(
        (1..=100).contains(&hugo_count),
        "Hugo worker count should be plausible (tens, not hundreds), got {hugo_count}",
    );
    let hugo_nested_path = normalize_office_path("/home/gc/AIStudio/projects/hugo-ai/apps/web");
    if Path::new("/home/gc/AIStudio/projects/hugo-ai/.git").exists() {
        assert!(!world
            .offices()
            .any(|office| office.path == hugo_nested_path));
    }
    assert!(world
        .offices()
        .flat_map(|office| office.workers.iter())
        .any(|worker| worker.last_seen != now));
    let normalized_offices: HashSet<String> = world
        .offices()
        .map(|office| normalize_office_path(&office.path))
        .collect();
    assert_eq!(normalized_offices.len(), world.office_count());
}
