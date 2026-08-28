use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OpenFlags};
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

fn snapshot_live_codex_timestamps(home: &Path) -> Option<HashMap<String, i64>> {
    let state_path = home.join("sqlite/state_5.sqlite");
    let Ok(state) = Connection::open_with_flags(state_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    else {
        eprintln!("real-machine smoke: timestamp audit skipped: state database unavailable");
        return None;
    };
    let Ok(mut statement) = state.prepare("SELECT id, updated_at_ms FROM threads") else {
        eprintln!("real-machine smoke: timestamp audit skipped: updated_at_ms column unavailable");
        return None;
    };
    let mut rows = statement.query([]).ok()?;
    let mut timestamps = HashMap::new();
    loop {
        let row = match rows.next() {
            Ok(Some(row)) => row,
            Ok(None) => break,
            Err(error) => {
                eprintln!("real-machine smoke: timestamp audit query failed: {error}");
                return None;
            }
        };
        let id: String = row.get(0).ok()?;
        let Some(updated_at_ms) = row.get::<_, Option<i64>>(1).ok()? else {
            continue;
        };
        timestamps.insert(id, updated_at_ms);
    }
    Some(timestamps)
}

fn assert_live_codex_timestamps(world: &World, timestamps: &HashMap<String, i64>, now: i64) {
    let codex_workers: Vec<_> = world
        .offices()
        .flat_map(|office| office.workers.iter())
        .filter(|worker| worker.agent == Agent::Codex)
        .collect();
    let target = codex_workers
        .iter()
        .filter(|worker| timestamps.contains_key(&worker.id.0))
        .count()
        .min(3);
    let mut checked = 0;
    for worker in codex_workers {
        let Some(&updated_at_ms) = timestamps.get(&worker.id.0) else {
            continue;
        };
        assert!(
            worker.last_seen >= updated_at_ms,
            "last_seen moved before updated_at_ms for {}: {} < {}",
            worker.id.0,
            worker.last_seen,
            updated_at_ms
        );
        println!(
            "real-machine smoke: Codex timestamp id={} last_seen={} updated_at_ms={} delta_ms={} idle_ms={} db_idle_ms={}",
            worker.id.0,
            worker.last_seen,
            updated_at_ms,
            worker.last_seen - updated_at_ms,
            now.saturating_sub(worker.last_seen),
            now.saturating_sub(updated_at_ms),
        );
        checked += 1;
        if checked == target {
            break;
        }
    }
    assert_eq!(
        checked, target,
        "timestamp audit checked {checked} of {target} active Codex workers"
    );
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
#[test]
fn claude_keeps_one_worker_in_one_office_across_path_spellings() {
    let temp = TempDir::new();
    let repo = temp.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    let project_dir = temp.path().join("projects/demo");
    fs::create_dir_all(&project_dir).unwrap();
    let transcript = project_dir.join("session-spellings.jsonl");
    let unix = repo.join("apps/web").to_string_lossy().into_owned();
    let wsl = format!(
        r"\\wsl.localhost\Ubuntu-22.04{}",
        repo.to_string_lossy().replace('/', "\\")
    );
    let dotted = format!("{}/apps/web/../docs", repo.display());
    let root = normalize_office_path(&repo.to_string_lossy());
    let unix_root = repo.to_string_lossy().into_owned();
    assert_eq!(normalize_office_path(&unix_root), root);
    assert_eq!(normalize_office_path(&wsl), root);

    append_jsonl(
        &transcript,
        json!({
            "type": "system",
            "timestamp": 1_000_000,
            "sessionId": "session-spellings",
            "cwd": unix.clone(),
            "customTitle": "spelling worker"
        }),
    );
    let mut source = ClaudeSource::new(temp.path());
    let mut events = source.poll(2_000_000).unwrap();

    append_jsonl(
        &transcript,
        json!({
            "type": "assistant",
            "timestamp": 1_001_000,
            "sessionId": "session-spellings",
            "cwd": wsl,
            "message": {"content": [{"type": "text", "text": "wsl"}]}
        }),
    );
    events.extend(source.poll(2_001_000).unwrap());

    append_jsonl(
        &transcript,
        json!({
            "type": "user",
            "timestamp": 1_002_000,
            "sessionId": "session-spellings",
            "cwd": dotted,
            "message": {"content": [{"type": "text", "text": "dotted"}]}
        }),
    );
    events.extend(source.poll(2_002_000).unwrap());

    assert!(events.iter().all(|event| event.office_path == root));
    let mut world = World::new();
    for event in events {
        world.apply(event);
    }
    let workers: Vec<_> = world
        .offices()
        .flat_map(|office| office.workers.iter())
        .collect();
    assert_eq!(world.office_count(), 1);
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].id.0, "session-spellings");
}

fn create_acceptance_claude_fixture(home: &Path, repo: &Path) {
    fs::create_dir_all(repo.join(".git")).unwrap();
    let project_dir = home.join("projects/acceptance");
    fs::create_dir_all(project_dir.join("session-main/subagents")).unwrap();
    let transcript = project_dir.join("session-main.jsonl");
    append_jsonl(
        &transcript,
        json!({
            "type": "system",
            "timestamp": 1_000_000,
            "sessionId": "session-main",
            "cwd": repo.join("apps/web").to_string_lossy(),
            "customTitle": "Claude main"
        }),
    );
    append_jsonl(
        &transcript,
        json!({
            "type": "user",
            "timestamp": 1_010_000,
            "sessionId": "session-main",
            "cwd": repo.join("docs").to_string_lossy(),
            "message": {"content": [{"type": "text", "text": "go"}]}
        }),
    );
    append_jsonl(
        &transcript,
        json!({
            "type": "assistant",
            "timestamp": 1_020_000,
            "sessionId": "session-main",
            "cwd": repo.join("docs").to_string_lossy(),
            "message": {
                "usage": {"input_tokens": 2, "output_tokens": 3},
                "content": [{"type": "text", "text": "done"}]
            }
        }),
    );
    let duplicate_transcript = project_dir.join("session-other.jsonl");
    append_jsonl(
        &duplicate_transcript,
        json!({
            "type": "system",
            "timestamp": 1_005_000,
            "sessionId": "session-other",
            "cwd": repo.join("apps/web").to_string_lossy(),
            "customTitle": "Claude main"
        }),
    );
    append_jsonl(
        &duplicate_transcript,
        json!({
            "type": "user",
            "timestamp": 1_015_000,
            "sessionId": "session-other",
            "cwd": repo.join("docs").to_string_lossy(),
            "message": {"content": [{"type": "text", "text": "go"}]}
        }),
    );
    append_jsonl(
        &duplicate_transcript,
        json!({
            "type": "assistant",
            "timestamp": 1_025_000,
            "sessionId": "session-other",
            "cwd": repo.join("docs").to_string_lossy(),
            "message": {
                "usage": {"input_tokens": 6, "output_tokens": 7},
                "content": [{"type": "text", "text": "done"}]
            }
        }),
    );
    append_jsonl(
        &project_dir.join("session-main/subagents/agent-lint.jsonl"),
        json!({
            "type": "assistant",
            "timestamp": 1_030_000,
            "sessionId": "session-main",
            "agentId": "agent-lint",
            "agentName": "lint worker",
            "cwd": repo.join("apps/web").to_string_lossy(),
            "message": {
                "usage": {"input_tokens": 4, "output_tokens": 5},
                "content": [{"type": "text", "text": "linting"}]
            }
        }),
    );
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

#[allow(clippy::too_many_arguments)]
fn insert_codex_structural_thread(
    state: &Connection,
    id: &str,
    cwd: &str,
    title: Option<&str>,
    tokens_used: i64,
    name: Option<&str>,
    agent_nickname: Option<&str>,
    updated_at_ms: i64,
    source: Option<&str>,
    thread_source: Option<&str>,
) {
    state
        .execute(
            "INSERT INTO threads (
                id, cwd, title, tokens_used, archived, name, agent_nickname,
                updated_at_ms, source, thread_source
            ) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                cwd,
                title,
                tokens_used,
                name,
                agent_nickname,
                updated_at_ms,
                source,
                thread_source
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
                updated_at_ms INTEGER,
                source TEXT,
                thread_source TEXT
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
        1_700_000,
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
        1_900_000,
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
    insert_codex_structural_thread(
        &state,
        "deadbeef-named",
        "/workspace/repo/apps/web",
        Some("/goal named"),
        13,
        Some("Dev named"),
        None,
        1_900_000,
        None,
        None,
    );
    insert_codex_structural_thread(
        &state,
        "developer-structural",
        "/workspace/repo/apps/web",
        Some("/goal structural"),
        9,
        Some("Dev structural"),
        None,
        1_700_000,
        None,
        None,
    );
    insert_codex_structural_thread(
        &state,
        "assessor-structural",
        "/workspace/repo/apps/web",
        Some(""),
        0,
        None,
        None,
        2_000_000,
        Some(r#"{"approval_assessor":true}"#),
        Some("approval_assessor"),
    );
    insert_codex_structural_thread(
        &state,
        "guardian-empty",
        "/workspace/repo/apps/web",
        Some(assessor_title),
        0,
        None,
        None,
        1_900_000,
        Some(r#"{"subagent":{"other":"guardian"}}"#),
        Some("guardian_review"),
    );
    insert_codex_structural_thread(
        &state,
        "subagent-empty",
        "/workspace/repo/apps/web",
        Some(assessor_title),
        0,
        None,
        None,
        1_900_000,
        Some(r#"{"subagent":{"other":"guardian"}}"#),
        None,
    );
    insert_codex_structural_thread(
        &state,
        "subagent-thread",
        "/workspace/repo/apps/web",
        Some(""),
        0,
        None,
        None,
        1_900_000,
        None,
        Some("subagent"),
    );
    insert_codex_structural_thread(
        &state,
        "01a04930-first",
        "/workspace/repo/apps/web",
        Some(""),
        11,
        None,
        None,
        1_900_000,
        None,
        None,
    );
    insert_codex_structural_thread(
        &state,
        "01a04930-second",
        "/workspace/repo/apps/web",
        Some(""),
        12,
        None,
        None,
        1_900_000,
        None,
        None,
    );
    state
        .execute(
            "INSERT INTO thread_spawn_edges (parent_thread_id, child_thread_id, status)
             VALUES (?1, ?2, ?3)",
            params!["developer-edge", "assessor-edge", "pending"],
        )
        .unwrap();
    state
        .execute(
            "INSERT INTO thread_spawn_edges (parent_thread_id, child_thread_id, status)
             VALUES (?1, ?2, ?3)",
            params!["developer-structural", "assessor-structural", "pending"],
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
    insert_item(
        &history,
        "developer-structural",
        1_950_000,
        "commandExecution",
        json!({"command": "cargo deploy structural", "status": "awaitingApproval"}),
    );
    history
        .execute(
            "INSERT INTO thread_turns VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "developer-structural",
                "structural-turn",
                "inProgress",
                1_900,
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

fn is_bare_hex_id(name: &str) -> bool {
    name.len() >= 8 && name.chars().all(|character| character.is_ascii_hexdigit())
}

fn duplicate_worker_name(world: &World) -> Option<String> {
    for office in world.offices() {
        let mut names = HashMap::new();
        for worker in &office.workers {
            if let Some(first_id) = names.insert(worker.name.clone(), worker.id.0.clone()) {
                return Some(format!(
                    "{}: {} ({} and {})",
                    office.path, worker.name, first_id, worker.id.0
                ));
            }
        }
    }
    None
}

fn has_nested_office(world: &World) -> bool {
    let paths: Vec<_> = world.offices().map(|office| office.path.as_str()).collect();
    paths.iter().any(|parent| {
        paths
            .iter()
            .any(|child| parent != child && Path::new(child).starts_with(Path::new(parent)))
    })
}

#[test]
fn collector_acceptance_fixtures() {
    let mut report = AcceptanceReport::new("fixtures");

    let codex_temp = TempDir::new();
    create_codex_m3_fixture(codex_temp.path());
    let fixture_now = 2_100_000;
    let mut codex = CodexSource::with_paths_and_active_within(
        codex_temp.path(),
        Vec::new(),
        Duration::from_secs(60 * 60),
    );
    let codex_events = codex.poll(fixture_now).unwrap();

    let claude_temp = TempDir::new();
    let repo = claude_temp.path().join("repo");
    create_acceptance_claude_fixture(claude_temp.path(), &repo);
    let mut claude = ClaudeSource::with_paths_and_active_within(
        claude_temp.path(),
        Vec::new(),
        Duration::from_secs(60 * 60),
    );
    let claude_events = claude.poll(fixture_now).unwrap();

    let mut all_events = codex_events.clone();
    all_events.extend(claude_events.clone());
    let mut world = World::new();
    for event in all_events.iter().cloned() {
        world.apply(event);
    }

    let internal_visible = codex_events.iter().any(|event| {
        matches!(
            event.worker.0.as_str(),
            "assessor-edge"
                | "assessor-fallback"
                | "assessor-structural"
                | "guardian-empty"
                | "subagent-empty"
                | "subagent-thread"
        )
    });
    report.record(
        1,
        "internal Codex threads excluded",
        if internal_visible {
            AcceptanceStatus::Fail
        } else {
            AcceptanceStatus::Pass
        },
        "guardian, review, subagent, and assessor fixture threads are absent from the roster",
    );

    let invalid_name = world.offices().find_map(|office| {
        office.workers.iter().find_map(|worker| {
            if worker.name.is_empty() || worker.name.starts_with('/') {
                Some(format!(
                    "{} has invalid name {:?}",
                    worker.id.0, worker.name
                ))
            } else {
                None
            }
        })
    });
    let named_worker_ok = world.offices().any(|office| {
        office
            .workers
            .iter()
            .any(|worker| worker.id.0 == "deadbeef-named" && worker.name == "Dev named")
    });
    let names_ok = invalid_name.is_none() && named_worker_ok;
    let names_reason = if let Some(detail) = invalid_name {
        detail
    } else if !named_worker_ok {
        "deadbeef-named did not retain the real name column value".to_string()
    } else {
        "no empty or slash-prefixed names; real name beats command title and bare id fallback"
            .to_string()
    };
    report.record(
        2,
        "worker names are safe and meaningful",
        if names_ok {
            AcceptanceStatus::Pass
        } else {
            AcceptanceStatus::Fail
        },
        names_reason,
    );

    let duplicate = duplicate_worker_name(&world);
    report.record(
        3,
        "worker names are unique within each office",
        if duplicate.is_some() {
            AcceptanceStatus::Fail
        } else {
            AcceptanceStatus::Pass
        },
        duplicate.map_or_else(
            || "all fixture office name sets are unique".to_string(),
            |detail| format!("duplicate name: {detail}"),
        ),
    );

    let pinned_worker = world.offices().find_map(|office| {
        office
            .workers
            .iter()
            .find_map(|worker| (worker.last_seen == fixture_now).then(|| worker.id.0.clone()))
    });
    report.record(
        4,
        "workers are not pinned at poll time",
        if pinned_worker.is_some() {
            AcceptanceStatus::Fail
        } else {
            AcceptanceStatus::Pass
        },
        pinned_worker.map_or_else(
            || "all last_seen values come from fixture event timestamps".to_string(),
            |worker| format!("{worker} has last_seen equal to poll time"),
        ),
    );

    let timestamp_rows = snapshot_live_codex_timestamps(codex_temp.path());
    let codex_workers: Vec<_> = world
        .offices()
        .flat_map(|office| office.workers.iter())
        .filter(|worker| worker.agent == Agent::Codex)
        .collect();
    let direct_seen_count = timestamp_rows.as_ref().map_or(0, |timestamps| {
        codex_workers
            .iter()
            .filter(|worker| {
                let Some(updated_at_ms) = timestamps.get(&worker.id.0) else {
                    return false;
                };
                codex_events.iter().any(|event| {
                    event.worker.0 == worker.id.0
                        && event.at == *updated_at_ms
                        && matches!(event.kind, EventKind::Seen { .. })
                }) && worker.last_seen >= *updated_at_ms
            })
            .count()
    });
    let timestamps_ok = !codex_workers.is_empty() && direct_seen_count == codex_workers.len();
    report.record(
        5,
        "idle timestamps match the source clock",
        if timestamps_ok {
            AcceptanceStatus::Pass
        } else {
            AcceptanceStatus::Fail
        },
        format!(
            "direct updated_at_ms/Seen matches: {direct_seen_count}/{} Codex workers",
            codex_workers.len()
        ),
    );

    let repo_root = normalize_office_path(&repo.to_string_lossy());
    let repo_events_collapsed = !claude_events.is_empty()
        && claude_events
            .iter()
            .all(|event| event.office_path == repo_root)
        && !has_nested_office(&world);
    report.record(
        6,
        "one repository is one office",
        if repo_events_collapsed {
            AcceptanceStatus::Pass
        } else {
            AcceptanceStatus::Fail
        },
        if repo_events_collapsed {
            format!("nested Claude paths collapse to {repo_root}")
        } else {
            "nested repository paths opened multiple offices".to_string()
        },
    );

    let claude_workers: Vec<_> = world
        .offices()
        .flat_map(|office| office.workers.iter())
        .filter(|worker| worker.agent == Agent::Claude)
        .collect();
    let claude_tokens_ok =
        !claude_workers.is_empty() && claude_workers.iter().all(|worker| worker.tokens_used > 0);
    report.record(
        7,
        "Claude workers report non-zero tokens",
        if claude_tokens_ok {
            AcceptanceStatus::Pass
        } else {
            AcceptanceStatus::Fail
        },
        format!(
            "{} Claude workers, all with positive usage",
            claude_workers.len()
        ),
    );

    let subagent_ok = claude_workers
        .iter()
        .any(|worker| worker.id.0 == "agent-lint" && worker.name == "sub:lint worker");
    report.record(
        8,
        "Claude subagent transcripts surface with sub names",
        if subagent_ok {
            AcceptanceStatus::Pass
        } else {
            AcceptanceStatus::Fail
        },
        if subagent_ok {
            "agent-lint surfaced as sub:lint worker".to_string()
        } else {
            "the deterministic subagent transcript was not surfaced or named".to_string()
        },
    );

    let edge_waiting = codex_events.iter().any(|event| {
        matches!(
            &event.kind,
            EventKind::Acted(Activity::Waiting { detail })
                if event.worker.0 == "developer-edge"
                    && detail.contains("cargo deploy")
                    && !detail.contains("cwd/time fallback")
        )
    });
    let fallback_waiting = codex_events.iter().any(|event| {
        matches!(
            &event.kind,
            EventKind::Acted(Activity::Waiting { detail })
                if event.worker.0 == "developer-fallback"
                    && detail.contains("rm -rf target")
                    && detail.contains("cwd/time fallback")
        )
    });
    let blocked_ok = edge_waiting && fallback_waiting;
    report.record(
        9,
        "blocked detection reports pending approval",
        if blocked_ok {
            AcceptanceStatus::Pass
        } else {
            AcceptanceStatus::Fail
        },
        format!("spawn-edge={edge_waiting}, cwd/time fallback={fallback_waiting}"),
    );

    report.finish();
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

    let events = source.poll(2_100_000).unwrap();
    assert!(!has_worker(&events, "assessor-edge"));
    assert!(!has_worker(&events, "assessor-fallback"));
    assert!(!has_worker(&events, "assessor-structural"));
    assert!(!has_worker(&events, "guardian-empty"));
    assert!(!has_worker(&events, "subagent-empty"));
    assert!(!has_worker(&events, "subagent-thread"));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Acted(Activity::Waiting { detail })
            if event.worker.0 == "developer-edge"
                && detail.contains("cargo deploy")
                && !detail.contains("cwd/time fallback")
    )));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Acted(Activity::Waiting { detail })
            if event.worker.0 == "developer-fallback"
                && detail.contains("rm -rf target")
                && detail.contains("cwd/time fallback")
    )));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        EventKind::Acted(Activity::Waiting { detail })
            if event.worker.0 == "developer-structural"
                && detail.contains("cargo deploy structural")
                && !detail.contains("cwd/time fallback")
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
    let duplicate_names: Vec<_> = events
        .iter()
        .filter_map(|event| {
            if !matches!(
                event.worker.0.as_str(),
                "01a04930-first" | "01a04930-second"
            ) {
                return None;
            }
            match &event.kind {
                EventKind::Seen { name, .. } => Some(name.clone()),
                _ => None,
            }
        })
        .collect();
    assert_eq!(duplicate_names.len(), 2);
    assert_ne!(duplicate_names[0], duplicate_names[1]);
    assert!(duplicate_names
        .iter()
        .all(|name| name.starts_with("01a04930")));

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
    for office in world.offices() {
        let mut names = HashSet::new();
        for worker in &office.workers {
            assert!(
                names.insert(worker.name.clone()),
                "duplicate worker name in office {}",
                office.path
            );
            assert!(
                !(worker.name.is_empty() && worker.tokens_used == 0),
                "empty zero-token worker in office {}",
                office.path
            );
        }
    }
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
    let codex_timestamps = config
        .codex_home
        .as_deref()
        .and_then(snapshot_live_codex_timestamps);
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
    for office in world.offices() {
        let mut names = HashSet::new();
        for worker in &office.workers {
            assert!(
                names.insert(worker.name.clone()),
                "duplicate worker name in office {}",
                office.path
            );
            assert!(
                !(worker.name.is_empty() && worker.tokens_used == 0),
                "empty zero-token worker in office {}",
                office.path
            );
        }
    }
    if let Some(timestamps) = codex_timestamps.as_ref() {
        assert_live_codex_timestamps(&world, timestamps, now);
    }
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
    let claude_subagents = claude_workers
        .iter()
        .filter(|worker| worker.name.starts_with("sub:"))
        .count();
    let waiting_developers = world
        .offices()
        .flat_map(|office| office.workers.iter())
        .filter(|worker| {
            worker.agent == Agent::Codex && matches!(worker.activity, Activity::Waiting { .. })
        })
        .count();
    println!(
        "real-machine smoke: Claude workers={} subagents={}",
        claude_workers.len(),
        claude_subagents
    );
    println!("real-machine smoke: Codex developers Waiting={waiting_developers}");
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
    println!("real-machine smoke: Hugo worker count after M5 = {hugo_count}");
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
#[derive(Clone, Copy)]
enum AcceptanceStatus {
    Pass,
    Fail,
    Skip,
}

impl AcceptanceStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Skip => "SKIP",
        }
    }
}

struct AcceptanceReport {
    scope: &'static str,
    failures: Vec<String>,
}

impl AcceptanceReport {
    fn new(scope: &'static str) -> Self {
        println!("collector acceptance: {scope}");
        Self {
            scope,
            failures: Vec::new(),
        }
    }

    fn record(
        &mut self,
        number: u8,
        invariant: &'static str,
        status: AcceptanceStatus,
        reason: impl Into<String>,
    ) {
        let reason = reason.into();
        println!(
            "collector acceptance [{}] {}. {} - {}",
            status.label(),
            number,
            invariant,
            reason
        );
        if matches!(status, AcceptanceStatus::Fail) {
            self.failures
                .push(format!("{number}. {invariant}: {reason}"));
        }
    }

    fn finish(self) {
        if !self.failures.is_empty() {
            panic!(
                "collector acceptance {} failed: {}",
                self.scope,
                self.failures.join(" | ")
            );
        }
    }
}
#[derive(Clone)]
struct LiveCodexRow {
    name: Option<String>,
    nickname: Option<String>,
    title: Option<String>,
    source: Option<String>,
    thread_source: Option<String>,
}

fn snapshot_live_codex_rows(home: &Path) -> Option<HashMap<String, LiveCodexRow>> {
    let state_path = home.join("sqlite/state_5.sqlite");
    let state = Connection::open_with_flags(state_path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let mut statement = state
        .prepare("SELECT id, name, agent_nickname, title, source, thread_source FROM threads")
        .ok()?;
    let mut rows = statement.query([]).ok()?;
    let mut result = HashMap::new();
    loop {
        let row = match rows.next() {
            Ok(Some(row)) => row,
            Ok(None) => break,
            Err(_) => return None,
        };
        let id: String = row.get(0).ok()?;
        result.insert(
            id,
            LiveCodexRow {
                name: row.get(1).ok()?,
                nickname: row.get(2).ok()?,
                title: row.get(3).ok()?,
                source: row.get(4).ok()?,
                thread_source: row.get(5).ok()?,
            },
        );
    }
    Some(result)
}

fn acceptance_normalize_marker(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn acceptance_json_has_key(value: &Value, wanted: &str) -> bool {
    match value {
        Value::Object(object) => {
            object.contains_key(wanted)
                || object
                    .values()
                    .any(|value| acceptance_json_has_key(value, wanted))
        }
        Value::Array(values) => values
            .iter()
            .any(|value| acceptance_json_has_key(value, wanted)),
        _ => false,
    }
}

fn live_codex_row_is_internal(row: &LiveCodexRow) -> bool {
    let thread_source_is_internal = row.thread_source.as_deref().is_some_and(|value| {
        matches!(
            acceptance_normalize_marker(value).as_str(),
            "guardianreview"
                | "internalreview"
                | "systemreview"
                | "review"
                | "subagent"
                | "assessor"
                | "approval"
                | "approvalassessor"
                | "approvalreview"
        )
    });
    let source_is_subagent = row
        .source
        .as_deref()
        .and_then(|source| serde_json::from_str::<Value>(source).ok())
        .is_some_and(|value| acceptance_json_has_key(&value, "subagent"));
    let legacy_assessor = row.title.as_deref().is_some_and(|title| {
        title.starts_with(
            "The following is the Codex agent history whose request action you are assessing",
        )
    });
    thread_source_is_internal || source_is_subagent || legacy_assessor
}

fn valid_codex_candidate(value: Option<&String>) -> Option<&str> {
    let value = value?.trim();
    (!value.is_empty() && !value.starts_with('/')).then_some(value)
}

fn collect_claude_subagent_files(
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_claude_subagent_files(&path, files)?;
        } else if file_type.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("jsonl")
            && path
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                == Some("subagents")
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("agent-"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn claude_subagent_counts(home: &Path, now: i64) -> Result<(usize, usize), String> {
    let projects = home.join("projects");
    if !projects.exists() {
        return Ok((0, 0));
    }
    let mut files = Vec::new();
    collect_claude_subagent_files(&projects, &mut files)
        .map_err(|error| format!("could not scan Claude subagent transcripts: {error}"))?;
    let active_cutoff =
        now.saturating_sub(i64::try_from(DEFAULT_ACTIVE_WITHIN.as_millis()).unwrap_or(i64::MAX));
    let active = files
        .iter()
        .filter(|path| {
            fs::metadata(path)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .and_then(|duration| i64::try_from(duration.as_millis()).ok())
                .is_some_and(|modified_at| modified_at >= active_cutoff)
        })
        .count();
    Ok((files.len(), active))
}

#[test]
#[ignore = "requires the live ~/.claude and ~/.codex stores"]
fn collector_acceptance_live_when_homes_exist() {
    let mut report = AcceptanceReport::new("live");
    let config = Config::discover();
    let claude_home = config.claude_home.as_deref().filter(|home| home.is_dir());
    let codex_home = config.codex_home.as_deref().filter(|home| {
        home.join("sqlite").join("state_5.sqlite").is_file()
            && home
                .join("sqlite")
                .join("thread_history_1.sqlite")
                .is_file()
    });
    let codex_present = codex_home.is_some();
    let claude_present = claude_home.is_some();
    let codex_rows = codex_home.and_then(snapshot_live_codex_rows);
    let codex_timestamps = codex_home.and_then(snapshot_live_codex_timestamps);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_millis() as i64;

    let mut codex_events = Vec::new();
    let mut claude_events = Vec::new();
    let mut poll_errors = Vec::new();
    for source in &mut build_sources(&config) {
        let source_name = source.name();
        match source.poll(now) {
            Ok(mut events) => {
                if source_name == "codex" {
                    codex_events.append(&mut events);
                } else if source_name == "claude" {
                    claude_events.append(&mut events);
                }
            }
            Err(error) => poll_errors.push(format!("{source_name}: {error}")),
        }
    }
    let codex_poll_error = poll_errors
        .iter()
        .find(|error| error.starts_with("codex:"))
        .cloned();
    let claude_poll_error = poll_errors
        .iter()
        .find(|error| error.starts_with("claude:"))
        .cloned();
    let mut all_events = codex_events.clone();
    all_events.extend(claude_events.clone());
    all_events.sort_by_key(|event| event.at);
    let mut world = World::new();
    for event in all_events {
        world.apply(event);
    }
    let workers: Vec<_> = world
        .offices()
        .flat_map(|office| office.workers.iter())
        .collect();
    let codex_workers: Vec<_> = workers
        .iter()
        .copied()
        .filter(|worker| worker.agent == Agent::Codex)
        .collect();
    let claude_workers: Vec<_> = workers
        .iter()
        .copied()
        .filter(|worker| worker.agent == Agent::Claude)
        .collect();

    if !codex_present {
        report.record(
            1,
            "internal Codex threads excluded",
            AcceptanceStatus::Skip,
            "Codex home absent",
        );
    } else if let Some(rows) = codex_rows.as_ref() {
        let internal_rows = rows
            .values()
            .filter(|row| live_codex_row_is_internal(row))
            .count();
        let visible_internal = codex_workers
            .iter()
            .filter(|worker| {
                rows.get(&worker.id.0)
                    .is_some_and(live_codex_row_is_internal)
            })
            .count();
        let missing_rows = codex_workers
            .iter()
            .filter(|worker| !rows.contains_key(&worker.id.0))
            .count();
        let ok = visible_internal == 0 && missing_rows == 0 && codex_poll_error.is_none();
        report.record(
            1,
            "internal Codex threads excluded",
            if ok {
                AcceptanceStatus::Pass
            } else {
                AcceptanceStatus::Fail
            },
            format!(
                "{internal_rows} internal rows, {visible_internal} visible internal workers, {missing_rows} unverified workers"
            ),
        );
    } else {
        report.record(
            1,
            "internal Codex threads excluded",
            AcceptanceStatus::Fail,
            "could not inspect the structural thread columns",
        );
    }

    if workers.is_empty() {
        report.record(
            2,
            "worker names are safe and meaningful",
            AcceptanceStatus::Skip,
            "no active workers in the configured horizon",
        );
    } else {
        let invalid_name = workers.iter().find_map(|worker| {
            if worker.name.is_empty() || worker.name.starts_with('/') {
                Some(format!(
                    "{} has invalid name {:?}",
                    worker.id.0, worker.name
                ))
            } else {
                None
            }
        });
        let codex_name_mismatch = if codex_present {
            codex_rows.as_ref().and_then(|rows| {
                codex_workers.iter().find_map(|worker| {
                    let row = rows.get(&worker.id.0)?;
                    let expected = valid_codex_candidate(row.name.as_ref())
                        .or_else(|| valid_codex_candidate(row.nickname.as_ref()));
                    let expected = expected?;
                    let suffixed = worker
                        .name
                        .strip_prefix(expected)
                        .is_some_and(|suffix| suffix.starts_with(" ("));
                    (worker.name != expected && !suffixed || is_bare_hex_id(&worker.name)).then(
                        || {
                            format!(
                                "{} rendered {:?}, source name {:?}",
                                worker.id.0, worker.name, expected
                            )
                        },
                    )
                })
            })
        } else {
            None
        };
        let name_detail = invalid_name.or(codex_name_mismatch);
        let ok = name_detail.is_none() && claude_poll_error.is_none() && codex_poll_error.is_none();
        report.record(
            2,
            "worker names are safe and meaningful",
            if ok {
                AcceptanceStatus::Pass
            } else {
                AcceptanceStatus::Fail
            },
            name_detail.unwrap_or_else(|| {
                "no empty/slash names or bare ids replaced a source name".to_string()
            }),
        );
    }

    if workers.is_empty() {
        report.record(
            3,
            "worker names are unique within each office",
            AcceptanceStatus::Skip,
            "no active workers in the configured horizon",
        );
    } else {
        let duplicate = duplicate_worker_name(&world);
        report.record(
            3,
            "worker names are unique within each office",
            if duplicate.is_some() {
                AcceptanceStatus::Fail
            } else {
                AcceptanceStatus::Pass
            },
            duplicate.map_or_else(
                || "all office name sets are unique".to_string(),
                |detail| format!("duplicate name: {detail}"),
            ),
        );
    }

    if workers.is_empty() {
        report.record(
            4,
            "workers are not pinned at poll time",
            AcceptanceStatus::Skip,
            "no active workers in the configured horizon",
        );
    } else {
        let pinned = workers
            .iter()
            .find(|worker| worker.last_seen == now)
            .map(|worker| worker.id.0.clone());
        report.record(
            4,
            "workers are not pinned at poll time",
            if pinned.is_some() {
                AcceptanceStatus::Fail
            } else {
                AcceptanceStatus::Pass
            },
            pinned.map_or_else(
                || "no worker has last_seen equal to poll time".to_string(),
                |worker| format!("{worker} has last_seen equal to poll time"),
            ),
        );
    }

    if !codex_present {
        report.record(
            5,
            "idle timestamps match the source clock",
            AcceptanceStatus::Skip,
            "Codex home absent; updated_at_ms is unavailable",
        );
    } else if codex_workers.is_empty() {
        report.record(
            5,
            "idle timestamps match the source clock",
            AcceptanceStatus::Skip,
            "no active Codex workers to compare",
        );
    } else if let Some(timestamps) = codex_timestamps.as_ref() {
        let direct_matches = codex_workers
            .iter()
            .filter(|worker| {
                let Some(updated_at_ms) = timestamps.get(&worker.id.0) else {
                    return false;
                };
                codex_events.iter().any(|event| {
                    event.worker.0.as_str() == worker.id.0.as_str()
                        && event.at == *updated_at_ms
                        && matches!(&event.kind, EventKind::Seen { .. })
                }) && worker.last_seen >= *updated_at_ms
            })
            .count();
        let ok = direct_matches == codex_workers.len() && codex_poll_error.is_none();
        report.record(
            5,
            "idle timestamps match the source clock",
            if ok {
                AcceptanceStatus::Pass
            } else {
                AcceptanceStatus::Fail
            },
            format!(
                "direct updated_at_ms/Seen matches: {direct_matches}/{} Codex workers",
                codex_workers.len()
            ),
        );
    } else {
        report.record(
            5,
            "idle timestamps match the source clock",
            AcceptanceStatus::Fail,
            "could not read updated_at_ms from the Codex state store",
        );
    }

    if workers.is_empty() {
        report.record(
            6,
            "one repository is one office",
            AcceptanceStatus::Skip,
            "no active workers in the configured horizon",
        );
    } else {
        let nested = has_nested_office(&world);
        report.record(
            6,
            "one repository is one office",
            if nested {
                AcceptanceStatus::Fail
            } else {
                AcceptanceStatus::Pass
            },
            if nested {
                "a nested repository path created a second office".to_string()
            } else {
                format!(
                    "{} offices have no ancestor/descendant duplicates",
                    world.office_count()
                )
            },
        );
    }

    if !claude_present {
        report.record(
            7,
            "Claude workers report non-zero tokens",
            AcceptanceStatus::Skip,
            "Claude home absent",
        );
    } else if claude_workers.is_empty() {
        report.record(
            7,
            "Claude workers report non-zero tokens",
            AcceptanceStatus::Skip,
            "no active Claude workers in the configured horizon",
        );
    } else {
        let zero_tokens = claude_workers
            .iter()
            .filter(|worker| worker.tokens_used == 0)
            .map(|worker| worker.id.0.clone())
            .collect::<Vec<_>>();
        let ok = zero_tokens.is_empty() && claude_poll_error.is_none();
        report.record(
            7,
            "Claude workers report non-zero tokens",
            if ok {
                AcceptanceStatus::Pass
            } else {
                AcceptanceStatus::Fail
            },
            if zero_tokens.is_empty() {
                format!(
                    "{} active Claude workers all report usage",
                    claude_workers.len()
                )
            } else {
                format!("zero-token Claude workers: {}", zero_tokens.join(", "))
            },
        );
    }

    if !claude_present {
        report.record(
            8,
            "Claude subagent transcripts surface with sub names",
            AcceptanceStatus::Skip,
            "Claude home absent",
        );
    } else {
        match claude_subagent_counts(claude_home.expect("checked above"), now) {
            Err(reason) => report.record(
                8,
                "Claude subagent transcripts surface with sub names",
                AcceptanceStatus::Fail,
                reason,
            ),
            Ok((0, _active)) => report.record(
                8,
                "Claude subagent transcripts surface with sub names",
                AcceptanceStatus::Skip,
                "no subagent transcripts found",
            ),
            Ok((discovered, 0)) => report.record(
                8,
                "Claude subagent transcripts surface with sub names",
                AcceptanceStatus::Skip,
                format!("discovered {discovered} historical subagent transcript(s); none active"),
            ),
            Ok((discovered, active)) => {
                let active_workers = claude_workers
                    .iter()
                    .filter(|worker| worker.name.starts_with("sub:"))
                    .count();
                let ok = active_workers > 0 && claude_poll_error.is_none();
                report.record(
                    8,
                    "Claude subagent transcripts surface with sub names",
                    if ok {
                        AcceptanceStatus::Pass
                    } else {
                        AcceptanceStatus::Fail
                    },
                    format!("discovered {discovered}, active files {active}, surfaced workers {active_workers}"),
                );
            }
        }
    }

    if !codex_present {
        report.record(
            9,
            "blocked detection reports pending approval",
            AcceptanceStatus::Skip,
            "Codex home absent",
        );
    } else if let Some(error) = codex_poll_error {
        report.record(
            9,
            "blocked detection reports pending approval",
            AcceptanceStatus::Fail,
            error,
        );
    } else {
        let blocked = codex_events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::Acted(Activity::Waiting { detail }) => {
                    Some(format!("{}: {detail}", event.worker.0))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if blocked.is_empty() {
            report.record(
                9,
                "blocked detection reports pending approval",
                AcceptanceStatus::Skip,
                "no current blocked candidate; the source diagnostic above reports assessor, turn, and edge counts",
            );
        } else {
            report.record(
                9,
                "blocked detection reports pending approval",
                AcceptanceStatus::Pass,
                format!("current blocked set: {}", blocked.join(" | ")),
            );
        }
    }

    report.finish();
}
