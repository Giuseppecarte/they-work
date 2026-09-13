//! Offline discovery probe: reports known losses instead of changing production.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{ensure, Context, Result};
use ratatui::{backend::TestBackend, Terminal};
use serde_json::{json, Value};
use theywork_control::{
    snapshot_events, Capabilities, ControlClient, ControlConfig, ControlEvent, ControlSnapshot,
    ManagedThread, OperationStatus,
};
use theywork_core::{
    Agent, CollaborationEvent, CollaborationKind, Event, EventKind, Evidence, OfficeId, SourceId,
    ThreadIdentity, WorkerRole, World,
};
use theywork_render::views::workboard::{Channel, ReviewMemory, Workboard};
use theywork_render::work_brief::WorkBrief;

const AT: i64 = 10_000;

fn identity(native: &str) -> ThreadIdentity {
    ThreadIdentity::new(
        Agent::Codex,
        SourceId("/audit/synthetic-source".into()),
        native,
    )
}

fn thread(native: &str, project: &str) -> ManagedThread {
    ManagedThread {
        identity: identity(native),
        role: if native == "child" {
            WorkerRole::Subagent
        } else {
            WorkerRole::Main
        },
        managed: native != "child",
        project: PathBuf::from(project),
        title: format!("Synthetic {native}"),
        active_turn_id: Some("audit-turn".into()),
        status: "active".into(),
        latest_text: String::new(),
        capabilities: Capabilities::default(),
        updated_at: AT,
    }
}

fn snapshot(threads: &[(&str, &str)], events: Vec<ControlEvent>) -> ControlSnapshot {
    ControlSnapshot {
        connected: true,
        observed_at: AT,
        generation: "synthetic-host-1".into(),
        connection_generation: "synthetic-connection-1".into(),
        threads: threads
            .iter()
            .map(|(id, project)| ((*id).into(), thread(id, project)))
            .collect(),
        events,
        ..ControlSnapshot::default()
    }
}

fn item(native: &str, id: &str, at: i64, seq: u64, body: Value) -> ControlEvent {
    ControlEvent {
        sequence: seq,
        at,
        method: "item/completed".into(),
        thread_id: Some(native.into()),
        turn_id: Some("audit-turn".into()),
        item_id: Some(id.into()),
        params: json!({"threadId":native, "turnId":"audit-turn", "item":body}),
    }
}

fn result(native: &str, id: &str, at: i64, seq: u64, text: &str) -> ControlEvent {
    item(
        native,
        id,
        at,
        seq,
        json!({"id":id,"type":"agentMessage","phase":"final_answer","text":text}),
    )
}

fn fold(world: &mut World, state: &ControlSnapshot, cursor: u64) {
    for event in snapshot_events(state, cursor) {
        world.apply(event);
    }
}

fn records(world: &World) -> Vec<Value> {
    world
        .collaboration()
        .map(|event| {
            json!({
                "actor":event.actor.native_id(), "id":event.id, "kind":event.kind,
                "text":event.text, "at":event.at,
                "recipient":event.recipient.as_ref().map(|id|id.native_id()),
            })
        })
        .collect()
}

fn result_ids(world: &World) -> BTreeSet<String> {
    world
        .collaboration()
        .filter(|e| e.kind == CollaborationKind::Result)
        .map(|e| e.native_item_id.clone().unwrap_or_else(|| e.id.clone()))
        .collect()
}

fn board(world: &World, office: Option<&OfficeId>, channel: Channel, now: i64) -> Workboard {
    let mut board = Workboard::default();
    board.open = true;
    board.channel = channel;
    board.refresh(
        world,
        office.and_then(|id| world.office(id)),
        &ReviewMemory::default(),
        &BTreeMap::new(),
        now,
    );
    board
}

fn board_summary(board: &Workboard) -> Value {
    json!({"coverage":board.coverage,"scope":board.scope,"rows":board.rows.iter().map(|row|json!({
        "key":row.key,"label":row.label,"title":row.title,"reviewed":row.reviewed,
        "detail":row.detail,
    })).collect::<Vec<_>>()})
}

fn save_board(board: &mut Workboard, destination: &Path) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(120, 32))?;
    terminal.draw(|frame| board.draw(frame))?;
    let buffer = terminal.backend().buffer();
    let text = (0..32)
        .map(|y| {
            (0..120)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .map(|line| line.trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(destination, format!("{text}\n"))?;
    Ok(())
}

struct HostProcess {
    child: Child,
}
impl Drop for HostProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_snapshot(
    client: &ControlClient,
    predicate: impl Fn(&ControlSnapshot) -> bool,
) -> Result<ControlSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let state = client.snapshot()?;
        if predicate(&state) {
            return Ok(state);
        }
        ensure!(
            Instant::now() < deadline,
            "Fixture did not converge in 15 seconds"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn run_gap(root: &Path, out: &Path, count: usize) -> Result<Value> {
    let dir = root.join(format!("gap-{count}"));
    fs::create_dir_all(dir.join("home"))?;
    fs::create_dir_all(dir.join("project"))?;
    let mut scenario = vec![
        json!({"method":"item/completed","params":{"threadId":"parent","turnId":"audit-turn","item":{
            "id":"early-spawn","type":"collabToolCall","tool":"spawnAgent","status":"completed",
            "senderThreadId":"parent","newThreadId":"child","prompt":"Synthetic delegated task"}}}),
        json!({"method":"item/completed","params":{"threadId":"child","turnId":"audit-turn","item":{
            "id":"early-child-result","type":"agentMessage","phase":"final_answer","text":"Synthetic child result"}}}),
        json!({"method":"item/completed","params":{"threadId":"parent","turnId":"audit-turn","item":{
            "id":"early-parent-result","type":"agentMessage","phase":"final_answer","text":"Synthetic parent result"}}}),
        json!({"id":"audit-approval","method":"item/commandExecution/requestApproval","params":{
            "threadId":"parent","turnId":"audit-turn","itemId":"audit-command","command":"echo fixture-only",
            "availableDecisions":["accept","decline","cancel"]}}),
    ];
    while scenario.len() < count - 1 {
        let index = scenario.len();
        scenario.push(json!({"method":"item/agentMessage/delta","params":{
            "threadId":"parent","turnId":"audit-turn","itemId":"noise","delta":format!("{index} ")}}));
    }
    scenario.push(json!({"method":"audit/end","params":{"threadId":"parent","count":count}}));
    fs::write(
        dir.join("scenario.jsonl"),
        scenario
            .iter()
            .map(|v| format!("{v}\n"))
            .collect::<String>(),
    )?;
    let mut config = ControlConfig::new(dir.join("control"), dir.join("home"));
    config.codex_program =
        theywork_control::native::find_executable("python3").context("Python 3 required")?;
    config.codex_args = vec![
        format!("{}/fake_provider.py", env!("CARGO_MANIFEST_DIR")),
        dir.to_string_lossy().into(),
    ];
    config.rpc_timeout_ms = 2_000;
    let config_path = dir.join("fixture-config.json");
    fs::write(&config_path, serde_json::to_vec(&config)?)?;
    let _host = HostProcess {
        child: Command::new(std::env::current_exe()?)
            .arg("--fixture-host")
            .arg(&config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(dir.join("host.log"))?)
            .spawn()?,
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    let client = loop {
        if let Ok(client) = ControlClient::connect(config.clone()) {
            break client;
        }
        ensure!(
            Instant::now() < deadline,
            "Fixture supervisor startup timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let receipt = client.start_codex(dir.join("project"), "Synthetic audit only", "start-once")?;
    ensure!(
        receipt.status == OperationStatus::Confirmed,
        "Fixture start not confirmed"
    );
    let before = wait_snapshot(&client, |state| {
        state.events.iter().any(|e| e.method == "turn/started")
    })?;
    let cursor = before
        .events
        .last()
        .context("Missing starting event")?
        .sequence;
    let mut world = World::new();
    fold(&mut world, &before, 0);
    // The consumer is deliberately paused, not merely slow at rendering.
    fs::write(dir.join("release"), "release\n")?;
    let after = wait_snapshot(&client, |state| {
        state.events.last().is_some_and(|e| e.method == "audit/end")
            && dir.join("emitted-count").exists()
    })?;
    fold(&mut world, &after, cursor);
    let emitted: Vec<Value> = fs::read_to_string(dir.join("emitted.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    ensure!(
        emitted == scenario,
        "Provider emitted stream differs from independent input log"
    );
    let first = after.events.first().context("Empty end snapshot")?.sequence;
    let last = after.events.last().unwrap().sequence;
    ensure!(
        last - cursor == count as u64,
        "Unexpected event count (not the planned burst)"
    );
    let requests: Vec<Value> = fs::read_to_string(dir.join("requests.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    ensure!(
        requests
            .iter()
            .filter(|m| m["method"] == "turn/start")
            .count()
            == 1,
        "Turn replayed"
    );
    ensure!(
        !requests.iter().any(|m| m.get("method").is_none()),
        "Fixture received unexpected approval reply"
    );
    let now = after.observed_at;
    let mut deliveries = board(&world, None, Channel::Deliveries, now);
    let mut changes = board(&world, None, Channel::Changes, now);
    save_board(
        &mut deliveries,
        &out.join(format!("gap-{count}-deliveries.txt")),
    )?;
    save_board(&mut changes, &out.join(format!("gap-{count}-changes.txt")))?;
    let parent = after.threads["parent"].identity.worker_id();
    let brief = WorkBrief::new(&world, &parent, &BTreeMap::new(), now).unwrap();
    Ok(json!({
        "case":format!("supervisor_paused_{count}"),"execution":"real supervisor and authenticated local IPC; fake Python provider",
        "oracle":{"emitted_after_cursor":count,"result_ids":["early-child-result","early-parent-result"],
            "relationship_edges":[["parent","child"]],"pending_request":"audit-approval"},
        "actual":{"cursor_before":cursor,"first_retained_sequence":first,"last_sequence":last,
            "retained_events":after.events.len(),"missing_sequence_count":first.saturating_sub(cursor+1),
            "result_ids":result_ids(&world),"relationships":world.relationships().count(),
            "roster_threads":after.threads.len(),"pending_requests":after.pending_requests.len(),
            "core_wait":world.worker(&parent).unwrap().wait_reason,"collaboration":records(&world),
            "coverage":world.worker(&parent).unwrap().coverage,
            "work_brief":{"coverage":brief.coverage,"state":brief.state,"result_records":brief.records.iter().filter(|r|r.result).count(),"team_members":brief.team.len()},
            "deliveries":board_summary(&deliveries),"changes":board_summary(&changes),
            "turn_start_calls":1,"automatic_replies":0},
        "finding":"reproduced: bounded event ring loses early results/delegation for paused consumer; current roster and pending request survive; generic partial coverage does not identify the sequence gap"
    }))
}

fn run_ordering() -> Result<Vec<Value>> {
    let base = [
        ("parent", "/audit/project-a"),
        ("child", "/audit/project-a"),
    ];
    let mut cases = Vec::new();
    let mut world = World::new();
    let duplicate = result("parent", "same-result", AT - 10, 1, "One result");
    fold(
        &mut world,
        &snapshot(&base, vec![duplicate.clone(), duplicate.clone()]),
        0,
    );
    fold(&mut world, &snapshot(&base, vec![duplicate]), 0);
    ensure!(
        result_ids(&world).len() == 1,
        "Duplicate result did not deduplicate"
    );
    cases.push(json!({"case":"duplicate_event_and_replayed_snapshot","oracle":{"result_count":1},"actual":records(&world),"finding":"pass: one semantic result after duplicate arrival and replay"}));

    let mut world = World::new();
    fold(
        &mut world,
        &snapshot(
            &base,
            vec![
                result("parent", "updated", AT, 1, "new"),
                result("parent", "updated", AT - 10, 2, "old"),
            ],
        ),
        0,
    );
    ensure!(
        world.collaboration().next().unwrap().text.as_deref() == Some("new"),
        "Older result replaced newer evidence"
    );
    cases.push(json!({"case":"newer_then_older_same_native_item","oracle":{"result_count":1,"text":"new"},"actual":records(&world),"finding":"pass: older duplicate does not replace newer collaboration evidence"}));

    let spawn = item(
        "parent",
        "spawn",
        AT - 20,
        1,
        json!({"id":"spawn","type":"collabToolCall","tool":"spawnAgent",
        "senderThreadId":"parent","newThreadId":"child","receiverThreadIds":["child","child","parent"]}),
    );
    let mut world = World::new();
    fold(&mut world, &snapshot(&base, vec![spawn]), 0);
    ensure!(
        world.relationships().count() == 1,
        "Recipients not deduplicated/self edge accepted"
    );
    cases.push(json!({"case":"duplicate_recipients_and_self_recipient","oracle":{"edge_count":1},"actual":{"edges":world.relationships().collect::<Vec<_>>(),"records":records(&world)},"finding":"pass: duplicate recipients and self edge do not fabricate extra workers/links"}));

    let forged = item(
        "parent",
        "mismatch",
        AT,
        1,
        json!({"id":"mismatch","type":"collabToolCall","tool":"spawnAgent","senderThreadId":"another-source-actor","newThreadId":"child"}),
    );
    let mut world = World::new();
    fold(&mut world, &snapshot(&base, vec![forged]), 0);
    ensure!(
        world.relationships().count() == 0,
        "Mismatched native actor accepted"
    );
    cases.push(json!({"case":"sender_mismatch","oracle":{"edge_count":0},"actual":{"edge_count":world.relationships().count()},"finding":"pass: mismatched sender is rejected"}));

    let early = result(
        "child",
        "before-roster",
        AT - 10,
        1,
        "Child completed before roster arrived",
    );
    let mut world = World::new();
    fold(&mut world, &snapshot(&base[..1], vec![early.clone()]), 0);
    let no_roster = result_ids(&world);
    fold(&mut world, &snapshot(&base, vec![early.clone()]), 1);
    let after_cursor = result_ids(&world);
    fold(&mut world, &snapshot(&base, vec![early]), 0);
    let after_replay = result_ids(&world);
    ensure!(
        no_roster.is_empty() && after_cursor.is_empty() && after_replay.len() == 1,
        "Missing-actor diagnostic changed"
    );
    cases.push(json!({"case":"result_before_actor_roster","scope":"public bridge API scenario; not a claim that every provider emits this ordering",
        "oracle":{"result_ids_after_actor_known":["before-roster"],"fabricated_actor_before_roster":false},
        "actual":{"without_roster":no_roster,"roster_added_after_cursor_advanced":after_cursor,"explicit_retained_event_replay":after_replay},
        "finding":"reproduced at bridge boundary: event skipped without actor is not retried when roster appears after cursor advances; replay recovers it"}));
    Ok(cases)
}

fn run_retention(out: &Path) -> Result<Value> {
    let projects = [("a", "/audit/project-a"), ("b", "/audit/project-b")];
    let state = snapshot(
        &projects,
        vec![result(
            "b",
            "unread-b",
            AT - 1_000,
            1,
            "Unread delivery in quiet project B",
        )],
    );
    let mut world = World::new();
    fold(&mut world, &state, 0);
    let office = OfficeId("/audit/project-b".into());
    let mut before = board(&world, Some(&office), Channel::Deliveries, AT);
    save_board(&mut before, &out.join("retention-b-before-deliveries.txt"))?;
    ensure!(
        before.rows.len() == 1 && !before.rows[0].reviewed,
        "Expected unread B result missing before flood"
    );
    for i in 0..513 {
        let at = AT - 999 + i;
        let actor = identity("a").worker_id();
        world.apply(Event {
            at,
            office: OfficeId("/audit/project-a".into()),
            office_path: "/audit/project-a".into(),
            worker: actor.clone(),
            agent: Agent::Codex,
            kind: EventKind::Collaboration(CollaborationEvent {
                id: format!("noise-{i}"),
                at,
                actor,
                recipient: None,
                kind: CollaborationKind::Message,
                text: Some(format!("Synthetic A message {i}")),
                correlation_id: None,
                native_turn_id: None,
                native_item_id: None,
                evidence: Evidence::NativeEvent,
            }),
        });
    }
    let mut after = board(&world, Some(&office), Channel::Deliveries, AT);
    let mut changes = board(&world, Some(&office), Channel::Changes, AT);
    save_board(&mut after, &out.join("retention-b-after-deliveries.txt"))?;
    save_board(&mut changes, &out.join("retention-b-after-changes.txt"))?;
    let brief = WorkBrief::new(&world, &identity("b").worker_id(), &BTreeMap::new(), AT).unwrap();
    ensure!(
        world.collaboration().count() == 512 && after.rows.is_empty(),
        "Global retention behavior differs from discovery"
    );
    Ok(
        json!({"case":"quiet_project_unread_delivery_evicted_by_other_project",
            "oracle":{"project_b_unread_results":["unread-b"],"project_a_events":513,"total_evidence_events":514},
            "actual":{"retained_collaboration":world.collaboration().count(),"project_b_present":world.worker(&identity("b").worker_id()).is_some(),
                "project_b_history_beats":world.worker(&identity("b").worker_id()).unwrap().history.len(),
                "before_deliveries":board_summary(&before),"after_deliveries":board_summary(&after),"after_changes":board_summary(&changes),
                "work_brief":{"coverage":brief.coverage,"result_records":brief.records.iter().filter(|r|r.result).count(),"remaining_records":brief.records.iter().map(|r|json!({"text":r.text,"result":r.result})).collect::<Vec<_>>()},
                "review_markers_written":0},
            "finding":"reproduced: B remains present and its text beat may remain, but its unread delivery/result classification is evicted by A; generic history warning has no per-project eviction interval"
        }),
    )
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "--fixture-host") {
        let config: ControlConfig =
            serde_json::from_slice(&fs::read(args.get(1).context("Missing fixture config")?)?)?;
        return theywork_control::run_supervisor(config);
    }
    ensure!(
        args.len() == 2,
        "Usage: theywork-data-audit SCRATCH_DIR EVIDENCE_DIR"
    );
    let scratch = PathBuf::from(&args[0]);
    let out = PathBuf::from(&args[1]);
    fs::create_dir_all(&scratch)?;
    fs::create_dir_all(&out)?;
    let gap300 = run_gap(&scratch, &out, 300)?;
    println!("completed supervisor pause: 300 events");
    let gap3000 = run_gap(&scratch, &out, 3000)?;
    println!("completed supervisor pause: 3000 events");
    let ordering = run_ordering()?;
    let retention = run_retention(&out)?;
    let report = json!({"schema_version":1,"kind":"bounded offline discovery; not a production acceptance pass",
        "boundaries":["real control supervisor and loopback IPC","public snapshot_events adapter","core World","public WorkBrief and Workboard"],
        "excluded":["live provider","personal stores","terminal screenshot or usability test","long-running soak","collector transcript recovery"],
        "supervisor_gaps":[gap300,gap3000],"ordering_and_identity":ordering,"retention":retention});
    fs::write(
        out.join("results.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!("completed 8 discovery cases; see results.json for reproduced findings and preserved guarantees");
    Ok(())
}
