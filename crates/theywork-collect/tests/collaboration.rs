use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use theywork_collect::{ClaudeSource, CodexSource};
use theywork_core::*;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "they-work-graph-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        // macOS temp_dir() can use /var, a symlink to /private/var. The
        // collector deliberately opens SQLite with NOFOLLOW, so give these
        // ordinary-store fixtures their real path rather than a symlink alias.
        Self(fs::canonicalize(path).unwrap())
    }
    fn source(&self) -> SourceId {
        SourceId(
            fs::canonicalize(&self.0)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        )
    }
    fn codex(&self, id: &str) -> WorkerId {
        ThreadIdentity::new(Agent::Codex, self.source(), id).worker_id()
    }
    fn claude(&self, root: &str, child: Option<&str>) -> WorkerId {
        let mut identity = ThreadIdentity::new(Agent::Claude, self.source(), child.unwrap_or(root));
        identity.session_id = child.map(|_| root.into());
        identity.worker_id()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fold(world: &mut World, events: Vec<Event>) {
    for event in events {
        world.apply(event);
    }
}
fn append(path: &Path, value: Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{value}").unwrap();
}
fn codex_store(fixture: &Fixture, edges: bool) -> (Connection, Connection) {
    let state = Connection::open(fixture.0.join("state_5.sqlite")).unwrap();
    state.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY, cwd TEXT, title TEXT, source TEXT, thread_source TEXT, tokens_used INTEGER, git_branch TEXT, updated_at_ms INTEGER, archived INTEGER, forked_from_id TEXT);").unwrap();
    if edges {
        state.execute_batch("CREATE TABLE thread_spawn_edges(parent_thread_id TEXT, child_thread_id TEXT, status TEXT);").unwrap();
    }
    let history = Connection::open(fixture.0.join("thread_history_1.sqlite")).unwrap();
    history.execute_batch("CREATE TABLE thread_items(thread_id TEXT, turn_id TEXT, item_id TEXT PRIMARY KEY, created_at_ms INTEGER, item_type TEXT, item_json TEXT); CREATE TABLE thread_turns(thread_id TEXT, turn_id TEXT, status TEXT, started_at INTEGER, completed_at INTEGER);").unwrap();
    (state, history)
}
fn thread(state: &Connection, id: &str, kind: &str) {
    state.execute("INSERT INTO threads VALUES(?1, '/workspace/repo', ?1, NULL, ?2, 1, NULL, 1000, 0, NULL)", params![id, kind]).unwrap();
}
fn item(history: &Connection, owner: &str, id: &str, kind: &str, payload: Value) {
    history
        .execute(
            "INSERT INTO thread_items VALUES(?1, 'turn-1', ?2, 1000, ?3, ?4)",
            params![owner, id, kind, payload.to_string()],
        )
        .unwrap();
}

#[test]
fn codex_reads_work_families_forks_and_correlated_events_without_internal_workers() {
    let fixture = Fixture::new();
    let (state, history) = codex_store(&fixture, true);
    for (id, kind) in [
        ("parent", "cli"),
        ("child", "subagent"),
        ("leaf", "subagent"),
        ("orphan", "subagent"),
        ("fork", "cli"),
        ("assessor", "approval_assessor"),
        ("guardian", "guardian_review"),
    ] {
        thread(&state, id, kind);
    }
    state
        .execute(
            "UPDATE threads SET forked_from_id='parent' WHERE id='fork'",
            [],
        )
        .unwrap();
    for (parent, child) in [
        ("parent", "child"),
        ("child", "leaf"),
        ("parent", "child"),
        ("absent", "orphan"),
        ("parent", "assessor"),
    ] {
        state
            .execute(
                "INSERT INTO thread_spawn_edges VALUES(?1, ?2, 'running')",
                params![parent, child],
            )
            .unwrap();
    }
    item(
        &history,
        "parent",
        "spawn",
        "collabToolCall",
        json!({"tool":"spawnAgent","senderThreadId":"parent","newThreadId":"child","prompt":"Check retry semantics","status":"completed"}),
    );
    item(
        &history,
        "child",
        "message",
        "collabToolCall",
        json!({"tool":"sendMessage","senderThreadId":"child","receiverThreadId":"leaf","prompt":"Cover the timeout","status":"completed"}),
    );
    item(
        &history,
        "child",
        "answer",
        "agentMessage",
        json!({"text":"Timeout covered and tests pass.","phase":"final_answer"}),
    );
    item(
        &history,
        "parent",
        "progress",
        "agentMessage",
        json!({"text":"Working on it","phase":"commentary"}),
    );
    item(
        &history,
        "parent",
        "input",
        "userMessage",
        json!({"text":"This is a user prompt, not a delivery."}),
    );
    item(
        &history,
        "assessor",
        "hidden",
        "agentMessage",
        json!({"text":"Internal verdict","phase":"final_answer"}),
    );
    let mut source =
        CodexSource::with_paths_and_active_within(&fixture.0, vec![], Duration::from_secs(3600));
    let mut world = World::new();
    fold(&mut world, source.poll(1100).unwrap());
    assert_eq!(world.worker_count(), 5);
    assert!(world.worker(&fixture.codex("assessor")).is_none());
    assert_eq!(
        world.worker(&fixture.codex("child")).unwrap().role,
        WorkerRole::Subagent
    );
    assert_eq!(
        world.ancestors(&fixture.codex("leaf")),
        vec![fixture.codex("child"), fixture.codex("parent")]
    );
    assert_eq!(world.relationships().count(), 4);
    assert_eq!(
        world.tree(&fixture.codex("absent"))[1].worker,
        fixture.codex("orphan")
    );
    let output: Vec<_> = world
        .collaboration()
        .filter(|event| event.kind == CollaborationKind::Result)
        .collect();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].actor, fixture.codex("child"));
    assert_eq!(output[0].native_item_id.as_deref(), Some("answer"));
    assert_eq!(output[0].native_turn_id.as_deref(), Some("turn-1"));
    assert!(world
        .collaboration()
        .any(|event| event.kind == CollaborationKind::Message
            && event.recipient == Some(fixture.codex("leaf"))));
    let count = world.collaboration().count();
    fold(&mut world, source.poll(1200).unwrap());
    assert_eq!(world.collaboration().count(), count);
    assert_eq!(
        world.worker(&fixture.codex("parent")).unwrap().last_seen,
        1000
    );
}

#[test]
fn codex_updates_same_timestamp_items_and_retains_removed_deliveries_without_writing_stores() {
    let fixture = Fixture::new();
    let (state, history) = codex_store(&fixture, false);
    thread(&state, "same", "cli");
    item(
        &history,
        "same",
        "answer",
        "agentMessage",
        json!({"text":"Still checking","phase":"commentary"}),
    );
    let mut source =
        CodexSource::with_paths_and_active_within(&fixture.0, vec![], Duration::from_secs(3600));
    let mut world = World::new();
    fold(&mut world, source.poll(1100).unwrap());
    assert_eq!(world.collaboration().count(), 0);
    history
        .execute(
            "UPDATE thread_items SET item_json=?1 WHERE item_id='answer'",
            [json!({"text":"Final verified output","phase":"final_answer"}).to_string()],
        )
        .unwrap();
    item(
        &history,
        "same",
        "same-time-new",
        "agentMessage",
        json!({"text":"Another output","phase":"final_answer"}),
    );
    let before = fs::read(fixture.0.join("thread_history_1.sqlite")).unwrap();
    fold(&mut world, source.poll(1200).unwrap());
    fold(&mut world, source.poll(1300).unwrap());
    assert_eq!(world.collaboration().count(), 2);
    assert_eq!(
        before,
        fs::read(fixture.0.join("thread_history_1.sqlite")).unwrap()
    );
    assert_eq!(
        world
            .worker(&fixture.codex("same"))
            .unwrap()
            .coverage
            .relationships,
        CoverageLevel::Partial
    );
    state
        .execute("UPDATE threads SET archived=1 WHERE id='same'", [])
        .unwrap();
    fold(&mut world, source.poll(1400).unwrap());
    assert_eq!(world.worker_count(), 0);
    assert_eq!(world.collaboration().count(), 2);
    assert_eq!(
        world.worker(&fixture.codex("same")).unwrap().lifecycle,
        WorkerLifecycle::Removed
    );
}

#[test]
fn codex_source_loss_marks_coverage_without_turn_completion_or_clock_refresh() {
    let fixture = Fixture::new();
    let (state, history) = codex_store(&fixture, false);
    thread(&state, "main", "cli");
    history
        .execute(
            "INSERT INTO thread_turns VALUES('main','active','inProgress',1,NULL)",
            [],
        )
        .unwrap();
    let mut source =
        CodexSource::with_paths_and_active_within(&fixture.0, vec![], Duration::from_secs(3600));
    let mut world = World::new();
    fold(&mut world, source.poll(1100).unwrap());
    drop(history);
    fs::rename(
        fixture.0.join("thread_history_1.sqlite"),
        fixture.0.join("history-hidden"),
    )
    .unwrap();
    fold(&mut world, source.poll(1200).unwrap());
    let worker = world.worker(&fixture.codex("main")).unwrap();
    assert!(!worker.coverage.available);
    assert!(worker.turn_in_flight);
    assert_eq!(worker.last_seen, 1000);
    assert_eq!(world.collaboration().count(), 0);
}

fn assistant(
    root: &str,
    agent: Option<&str>,
    parent_tool: Option<&str>,
    uuid: &str,
    content: Value,
    stop: Option<&str>,
) -> Value {
    json!({"type":"assistant","timestamp":1000,"sessionId":root,"agentId":agent,"parent_tool_use_id":parent_tool,
        "cwd":"/workspace/repo","uuid":uuid,"message":{"content":content,"stop_reason":stop}})
}

#[test]
fn claude_joins_nested_delegation_by_tool_id_and_does_not_promote_membership_to_parentage() {
    let fixture = Fixture::new();
    let projects = fixture.0.join("projects/repo");
    append(
        &projects.join("root.jsonl"),
        assistant(
            "root",
            None,
            None,
            "root-msg",
            json!([
                {"type":"tool_use","id":"task-a","name":"Agent","input":{"prompt":"Implement API"}}
            ]),
            Some("tool_use"),
        ),
    );
    append(
        &projects.join("root/subagents/a.jsonl"),
        assistant(
            "root",
            Some("a"),
            Some("task-a"),
            "a-msg",
            json!([
                {"type":"tool_use","id":"task-b","name":"Task","input":{"prompt":"Test nested API"}}
            ]),
            Some("tool_use"),
        ),
    );
    append(
        &projects.join("root/subagents/b.jsonl"),
        assistant(
            "root",
            Some("b"),
            Some("task-b"),
            "b-result",
            json!([
                {"type":"text","text":"Nested tests passed."}
            ]),
            Some("end_turn"),
        ),
    );
    append(
        &projects.join("root/subagents/unmapped.jsonl"),
        assistant(
            "root",
            None,
            None,
            "unmapped-message",
            json!([
                {"type":"text","text":"No immediate parent metadata."}
            ]),
            None,
        ),
    );
    let mut source =
        ClaudeSource::with_paths_and_active_within(&fixture.0, vec![], Duration::from_secs(3600));
    let mut world = World::new();
    fold(&mut world, source.poll(1100).unwrap());
    let root = fixture.claude("root", None);
    let a = fixture.claude("root", Some("a"));
    let b = fixture.claude("root", Some("b"));
    let unmapped = fixture.claude("root", Some("unmapped"));
    assert_eq!(world.worker_count(), 4);
    assert_eq!(world.ancestors(&b), vec![a.clone(), root.clone()]);
    assert!(world.ancestors(&unmapped).is_empty());
    assert_ne!(unmapped, root);
    assert_eq!(world.tree(&root).len(), 4);
    assert_eq!(
        world
            .collaboration()
            .filter(|e| e.kind == CollaborationKind::Result)
            .count(),
        1
    );
    assert_eq!(
        world.worker(&b).unwrap().lifecycle,
        WorkerLifecycle::Completed
    );
    assert!(world.collaboration().any(|e| e.actor == a
        && e.recipient == Some(b.clone())
        && e.correlation_id.as_deref() == Some("task-b")));
    let count = world.collaboration().count();
    fold(&mut world, source.poll(1200).unwrap());
    assert_eq!(world.collaboration().count(), count);
    fs::remove_file(projects.join("root/subagents/b.jsonl")).unwrap();
    fold(&mut world, source.poll(1300).unwrap());
    assert!(!world.is_present(&b));
    assert_eq!(world.collaboration_for(&b).count(), 2);
}

#[test]
fn claude_same_agent_ids_in_different_sessions_and_providers_remain_distinct() {
    let fixture = Fixture::new();
    for root in ["one", "two"] {
        append(
            &fixture
                .0
                .join(format!("projects/repo/{root}/subagents/shared.jsonl")),
            assistant(
                root,
                Some("same"),
                None,
                root,
                json!([
                    {"type":"tool_use","id":"question","name":"AskUserQuestion","input":{"questions":[{"question":"Which timeout?"}]}}
                ]),
                Some("tool_use"),
            ),
        );
    }
    let mut source =
        ClaudeSource::with_paths_and_active_within(&fixture.0, vec![], Duration::from_secs(3600));
    let mut world = World::new();
    fold(&mut world, source.poll(1100).unwrap());
    assert_eq!(world.worker_count(), 2);
    assert_eq!(world.collaboration().count(), 2);
    for root in ["one", "two"] {
        let worker = world.worker(&fixture.claude(root, Some("same"))).unwrap();
        assert_eq!(worker.wait_reason, Some(WaitReason::HumanInput));
        assert_ne!(worker.id, fixture.codex("same"));
    }
    assert_eq!(
        world
            .collaboration()
            .filter(|e| e.kind == CollaborationKind::Result)
            .count(),
        0
    );
}

#[test]
fn old_worker_json_defaults_to_unknown_coverage_and_no_control_identity() {
    let worker = Worker::new(
        WorkerId("legacy".into()),
        OfficeId("/repo".into()),
        Agent::Codex,
        "Legacy".into(),
        5,
    );
    let mut value = serde_json::to_value(worker).unwrap();
    for field in ["identity", "role", "lifecycle", "wait_reason", "coverage"] {
        value.as_object_mut().unwrap().remove(field);
    }
    let old: Worker = serde_json::from_value(value).unwrap();
    assert!(old.identity.is_none());
    assert_eq!(old.role, WorkerRole::Main);
    assert_eq!(old.lifecycle, WorkerLifecycle::Unknown);
    assert!(old.coverage.incomplete);
}

#[test]
fn claude_background_launch_ack_is_not_a_delivery_and_late_child_resolves_the_original_event() {
    let fixture = Fixture::new();
    let projects = fixture.0.join("projects/repo");
    let main = projects.join("root.jsonl");
    append(
        &main,
        assistant(
            "root",
            None,
            None,
            "start",
            json!([
                {"type":"tool_use","id":"background-task","name":"Agent","input":{"prompt":"Run background checks","run_in_background":true}}
            ]),
            Some("tool_use"),
        ),
    );
    append(
        &main,
        json!({"type":"user","timestamp":1001,"sessionId":"root","cwd":"/workspace/repo",
        "toolUseResult":{"agentId":"late-child","status":"running"},
        "message":{"content":[{"type":"tool_result","tool_use_id":"background-task","content":"Agent launched; this is not its result."}]}}),
    );
    let mut source =
        ClaudeSource::with_paths_and_active_within(&fixture.0, vec![], Duration::from_secs(3600));
    let mut world = World::new();
    fold(&mut world, source.poll(1100).unwrap());
    assert_eq!(
        world
            .collaboration()
            .filter(|e| e.kind == CollaborationKind::Result)
            .count(),
        0
    );
    append(
        &projects.join("root/subagents/late.jsonl"),
        assistant(
            "root",
            Some("late-child"),
            Some("background-task"),
            "final",
            json!([
                {"type":"text","text":"Actual completed check result"}
            ]),
            Some("end_turn"),
        ),
    );
    fold(&mut world, source.poll(1200).unwrap());
    let child = fixture.claude("root", Some("late-child"));
    assert!(world
        .collaboration()
        .any(|e| e.kind == CollaborationKind::Delegated && e.recipient == Some(child.clone())));
    assert_eq!(
        world
            .collaboration()
            .filter(|e| e.kind == CollaborationKind::Delegated)
            .count(),
        1
    );
    assert_eq!(
        world
            .collaboration()
            .filter(|e| e.kind == CollaborationKind::Result)
            .count(),
        1
    );
    assert_eq!(world.ancestors(&child), vec![fixture.claude("root", None)]);
}

#[test]
fn claude_explicit_child_result_keeps_its_parent_as_recipient() {
    let fixture = Fixture::new();
    let projects = fixture.0.join("projects/repo");
    let main = projects.join("root.jsonl");
    append(
        &main,
        assistant(
            "root",
            None,
            None,
            "launch",
            json!([
                {"type":"tool_use","id":"task-child","name":"Agent","input":{"prompt":"Check the result"}}
            ]),
            Some("tool_use"),
        ),
    );
    append(
        &projects.join("root/subagents/child.jsonl"),
        assistant(
            "root",
            Some("child"),
            Some("task-child"),
            "child-start",
            json!([{"type":"text","text":"Checking"}]),
            None,
        ),
    );
    let mut source =
        ClaudeSource::with_paths_and_active_within(&fixture.0, vec![], Duration::from_secs(3600));
    let mut world = World::new();
    fold(&mut world, source.poll(1100).unwrap());
    append(
        &main,
        json!({"type":"user","timestamp":1101,"sessionId":"root","cwd":"/workspace/repo",
        "toolUseResult":{"agentId":"child","status":"completed"},
        "message":{"content":[{"type":"tool_result","tool_use_id":"task-child","content":"Verified child output."}]}}),
    );
    fold(&mut world, source.poll(1200).unwrap());
    let result = world
        .collaboration()
        .find(|e| e.kind == CollaborationKind::Result)
        .unwrap();
    assert_eq!(result.actor, fixture.claude("root", Some("child")));
    assert_eq!(result.recipient, Some(fixture.claude("root", None)));
    assert_eq!(result.text.as_deref(), Some("Verified child output."));
}

#[test]
fn codex_archived_endpoints_are_graph_history_and_internal_tool_calls_do_not_leak_nodes() {
    let fixture = Fixture::new();
    let (state, history) = codex_store(&fixture, true);
    for (id, role) in [
        ("parent", "cli"),
        ("old-child", "subagent"),
        ("assessor", "approval_assessor"),
    ] {
        thread(&state, id, role);
    }
    state
        .execute("UPDATE threads SET archived=1 WHERE id='old-child'", [])
        .unwrap();
    state
        .execute(
            "INSERT INTO thread_spawn_edges VALUES('parent','old-child','completed')",
            [],
        )
        .unwrap();
    item(
        &history,
        "parent",
        "internal-spawn",
        "collabToolCall",
        json!({"tool":"spawnAgent","senderThreadId":"parent","newThreadId":"assessor","status":"completed","prompt":"Internal assessment"}),
    );
    let mut source =
        CodexSource::with_paths_and_active_within(&fixture.0, vec![], Duration::from_secs(3600));
    let mut world = World::new();
    fold(&mut world, source.poll(1100).unwrap());
    assert_eq!(world.worker_count(), 1);
    assert_eq!(
        world.children(&fixture.codex("parent")),
        vec![fixture.codex("old-child")]
    );
    assert!(world.worker(&fixture.codex("old-child")).is_none());
    assert_eq!(world.collaboration().count(), 0);
}

#[test]
fn codex_structural_provenance_supplies_parentage_when_edge_table_is_absent() {
    let fixture = Fixture::new();
    let (state, _history) = codex_store(&fixture, false);
    thread(&state, "child", "subagent");
    state
        .execute(
            "UPDATE threads SET source=?1 WHERE id='child'",
            [
                json!({"subagent":{"thread_spawn":{"parent_thread_id":"unavailable-parent"}}})
                    .to_string(),
            ],
        )
        .unwrap();
    let mut source =
        CodexSource::with_paths_and_active_within(&fixture.0, vec![], Duration::from_secs(3600));
    let mut world = World::new();
    fold(&mut world, source.poll(1100).unwrap());
    assert_eq!(
        world.ancestors(&fixture.codex("child")),
        vec![fixture.codex("unavailable-parent")]
    );
    assert_eq!(world.worker_count(), 1);
}
