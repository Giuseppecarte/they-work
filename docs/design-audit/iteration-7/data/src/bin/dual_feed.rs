//! Integrated cold collector + bounded managed snapshot discovery, with no I/O
//! to anything except the explicitly supplied synthetic scratch directory.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use theywork_collect::CodexSource;
use theywork_control::{
    snapshot_events, Capabilities, ControlEvent, ControlSnapshot, ManagedThread,
};
use theywork_core::{
    Agent, CollaborationKind, Event, Source, SourceId, ThreadIdentity, WorkerRole, World,
};
use theywork_render::views::workboard::{Channel, ReviewMemory, Workboard};

const BASE: i64 = 1_780_000_000_000;

fn create_store(
    home: &Path,
    project: &Path,
    events: &[ControlEvent],
    aggregate: bool,
) -> Result<()> {
    fs::create_dir_all(home)?;
    fs::create_dir_all(project)?;
    let state = Connection::open(home.join("state_5.sqlite"))?;
    state.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY, cwd TEXT, title TEXT, source TEXT, thread_source TEXT, tokens_used INTEGER, git_branch TEXT, updated_at_ms INTEGER, archived INTEGER, forked_from_id TEXT);
        CREATE TABLE thread_spawn_edges(parent_thread_id TEXT, child_thread_id TEXT, status TEXT);")?;
    for (id, kind) in [("parent", "cli"), ("child", "subagent")] {
        state.execute(
            "INSERT INTO threads VALUES(?1,?2,?1,NULL,?3,0,NULL,?4,0,NULL)",
            params![id, project.to_string_lossy(), kind, BASE + 10_000],
        )?;
    }
    state.execute(
        "INSERT INTO thread_spawn_edges VALUES('parent','child','running')",
        [],
    )?;
    let history = Connection::open(home.join("thread_history_1.sqlite"))?;
    history.execute_batch("CREATE TABLE thread_items(thread_id TEXT, turn_id TEXT, item_id TEXT PRIMARY KEY, created_at_ms INTEGER, item_type TEXT, item_json TEXT);
        CREATE TABLE thread_turns(thread_id TEXT, turn_id TEXT, status TEXT, started_at INTEGER, completed_at INTEGER);")?;
    for id in ["parent", "child"] {
        history.execute(
            "INSERT INTO thread_turns VALUES(?1,'audit-turn','inProgress',?2,NULL)",
            params![id, BASE],
        )?;
    }
    let transaction = history.unchecked_transaction()?;
    for event in events {
        if let Some(item) = event.params.get("item") {
            transaction.execute(
                "INSERT INTO thread_items VALUES(?1,'audit-turn',?2,?3,?4,?5)",
                params![
                    event.thread_id,
                    event.item_id,
                    event.at,
                    item["type"].as_str().unwrap(),
                    item.to_string()
                ],
            )?;
        }
    }
    if aggregate {
        // A stream of deltas updates two durable commentary items; it is not
        // 2996 distinct SQLite items. The completed results remain in the tail.
        for id in ["parent", "child"] {
            transaction.execute("INSERT INTO thread_items VALUES(?1,'audit-turn',?2,?3,'agentMessage',?4)",
                params![id, format!("aggregate-{id}"), BASE + 9_000,
                    json!({"id":format!("aggregate-{id}"),"type":"agentMessage","phase":"commentary","text":"Aggregated synthetic deltas"}).to_string()])?;
        }
    }
    transaction.commit()?;
    Ok(())
}

fn stream(count: usize, aggregate: bool) -> Vec<ControlEvent> {
    (0..count).map(|index| {
        let actor = if index == 1 || index >= 3 && index % 2 == 1 { "child" } else { "parent" };
        let (method, item) = match index {
            0 => ("item/completed", Some(json!({"id":"early-spawn","type":"collabToolCall","tool":"spawnAgent","senderThreadId":"parent","newThreadId":"child","status":"completed","prompt":"Synthetic delegated task"}))),
            1 => ("item/completed", Some(json!({"id":"early-child-result","type":"agentMessage","phase":"final_answer","text":"Synthetic child result"}))),
            2 => ("item/completed", Some(json!({"id":"early-parent-result","type":"agentMessage","phase":"final_answer","text":"Synthetic parent result"}))),
            i if i == count - 1 => ("audit/end", None),
            _ if aggregate => ("item/agentMessage/delta", None),
            _ => ("item/completed", Some(json!({"id":format!("noise-{index}"),"type":"reasoning","text":"Synthetic processing record"}))),
        };
        let item_id = item.as_ref().and_then(|value| value["id"].as_str()).map(str::to_owned);
        let params = if let Some(item) = item {
            json!({"threadId":actor,"turnId":"audit-turn","item":item})
        } else {
            json!({"threadId":actor,"turnId":"audit-turn","itemId":format!("aggregate-{actor}"),"delta":"synthetic "})
        };
        ControlEvent { sequence: index as u64 + 2, at: BASE + index as i64 + 1,
            method: method.into(), thread_id: Some(actor.into()), turn_id: Some("audit-turn".into()), item_id, params }
    }).collect()
}

fn snapshot(home: &Path, project: &Path, events: &[ControlEvent]) -> Result<ControlSnapshot> {
    let source = SourceId(fs::canonicalize(home)?.to_string_lossy().into_owned());
    let threads = [
        ("parent", WorkerRole::Main),
        ("child", WorkerRole::Subagent),
    ]
    .into_iter()
    .map(|(native, role)| {
        (
            native.into(),
            ManagedThread {
                identity: ThreadIdentity::new(Agent::Codex, source.clone(), native),
                role,
                managed: native == "parent",
                project: project.to_owned(),
                title: native.into(),
                active_turn_id: Some("audit-turn".into()),
                status: "active".into(),
                latest_text: "Synthetic latest commentary".into(),
                capabilities: Capabilities::default(),
                updated_at: BASE + 10_000,
            },
        )
    })
    .collect();
    Ok(ControlSnapshot {
        connected: true,
        generation: "same-host-after-view-reopen".into(),
        observed_at: BASE + 10_000,
        codex_home: home.to_owned(),
        threads,
        events: events[events.len().saturating_sub(256)..].to_vec(),
        ..ControlSnapshot::default()
    })
}

fn summary(world: &World) -> Value {
    let mut deliveries = Workboard::default();
    deliveries.channel = Channel::Deliveries;
    deliveries.refresh(
        world,
        None,
        &ReviewMemory::default(),
        &BTreeMap::new(),
        BASE + 10_000,
    );
    json!({
        "worker_count":world.worker_count(), "office_count":world.office_count(),
        "result_ids":world.collaboration().filter(|event|event.kind == CollaborationKind::Result).map(|event|event.native_item_id.clone()).collect::<Vec<_>>(),
        "delegation_record_count":world.collaboration().filter(|event|event.kind == CollaborationKind::Delegated).count(),
        "edges":world.relationships().map(|edge|json!({"parent":edge.parent.native_id(),"child":edge.child.native_id(),"evidence":edge.evidence})).collect::<Vec<_>>(),
        "delivery_rows":deliveries.rows.len(),"coverage":deliveries.coverage,
    })
}

fn apply(world: &mut World, events: Vec<Event>) {
    for event in events {
        world.apply(event);
    }
}

fn inspect_store(home: &Path) -> Result<Value> {
    let history = Connection::open_with_flags(
        home.join("thread_history_1.sqlite"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let mut proof = Vec::new();
    for id in ["early-spawn", "early-child-result", "early-parent-result"] {
        let (owner, at): (String, i64) = history.query_row(
            "SELECT thread_id,created_at_ms FROM thread_items WHERE item_id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let newer: i64 = history.query_row(
            "SELECT COUNT(*) FROM thread_items WHERE thread_id=?1 AND created_at_ms>?2",
            params![owner, at],
            |row| row.get(0),
        )?;
        proof.push(json!({"item_id":id,"owner":owner,"exists_in_store":true,"newer_items_same_thread":newer,"within_64_item_tail":newer<64}));
    }
    let count: i64 =
        history.query_row("SELECT COUNT(*) FROM thread_items", [], |row| row.get(0))?;
    Ok(json!({"item_count":count,"early_evidence":proof,"native_metadata_edge_present":true}))
}

fn run_case(root: &Path, count: usize, aggregate: bool) -> Result<Value> {
    let name = if aggregate {
        "recent_records_3000_deltas".to_string()
    } else {
        format!("buried_records_{count}_items")
    };
    let home = root.join(&name).join("home");
    let project = root.join(&name).join("project");
    let events = stream(count, aggregate);
    create_store(&home, &project, &events, aggregate)?;
    fs::write(
        root.join(&name).join("expected-stream.json"),
        serde_json::to_vec(&events)?,
    )?;
    let proof = inspect_store(&home)?;
    let before_state = fs::read(home.join("state_5.sqlite"))?;
    let before_history = fs::read(home.join("thread_history_1.sqlite"))?;
    let snapshot = snapshot(&home, &project, &events)?;
    let mut source = CodexSource::new(&home);
    let mut world = World::new();
    // Same feed ordering as main: source poll observations first, managed
    // snapshot observations second. Reopening creates a cold collector cursor.
    let first_collector_events = source.poll(BASE + 10_000)?;
    let first_count = first_collector_events.len();
    apply(&mut world, first_collector_events);
    let collector_only = summary(&world);
    apply(&mut world, snapshot_events(&snapshot, 0));
    let integrated = summary(&world);
    let second = source.poll(BASE + 10_010)?;
    let second_count = second.len();
    apply(&mut world, second);
    apply(
        &mut world,
        snapshot_events(&snapshot, snapshot.events.last().unwrap().sequence),
    );
    let after_warm_poll = summary(&world);
    ensure!(
        before_state == fs::read(home.join("state_5.sqlite"))?
            && before_history == fs::read(home.join("thread_history_1.sqlite"))?,
        "Collector changed a synthetic store"
    );
    ensure!(
        integrated["worker_count"] == 2 && integrated["office_count"] == 1,
        "Feeds failed to join canonical identities/project"
    );
    ensure!(
        integrated["edges"].as_array().unwrap().len() == 1,
        "Persisted metadata edge was not recovered"
    );
    let expected_results = if aggregate { 2 } else { 0 };
    ensure!(
        integrated["result_ids"].as_array().unwrap().len() == expected_results,
        "Recovery differs from expected tail boundary"
    );
    ensure!(
        integrated["result_ids"] == after_warm_poll["result_ids"],
        "Warm poll unexpectedly backfilled or duplicated results"
    );
    ensure!(
        proof["early_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["within_64_item_tail"] == aggregate),
        "Fixture did not place all early facts on intended side of64 boundary"
    );
    Ok(
        json!({"case":name,"execution":"actual CodexSource + snapshot_events + same World in host feed order; synthetic bounded snapshot, not another supervisor process run",
        "oracle":{"result_ids":["early-child-result","early-parent-result"],"delegation_record":"early-spawn","edge":["parent","child"]},
        "store":proof,"snapshot":{"input_observations":count,"after_sequence":0,"first_retained_sequence":snapshot.events.first().unwrap().sequence,"last_sequence":snapshot.events.last().unwrap().sequence,"retained":snapshot.events.len()},
        "actual":{"first_collector_event_count":first_count,"collector_only":collector_only,"integrated":integrated,"second_collector_event_count":second_count,"after_warm_poll":after_warm_poll,"store_bytes_unchanged":true},
        "finding":if aggregate {"collector recovers both recent results and delegation item; final combined feeds do not duplicate identities or deliveries"} else {"collector recovers durable relationship edge, but not results/delegation items outside per-thread tail on cold load; a subsequent warm poll does not backfill them"}}),
    )
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ensure!(args.len() == 2, "Usage: dual_feed SCRATCH EVIDENCE_DIR");
    let scratch = PathBuf::from(&args[0]);
    let out = PathBuf::from(&args[1]);
    fs::create_dir_all(&scratch)?;
    fs::create_dir_all(&out)?;
    let cases = vec![
        run_case(&scratch, 300, false)?,
        run_case(&scratch, 3000, false)?,
        run_case(&scratch, 3000, true)?,
    ];
    fs::write(
        out.join("results.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(
                &json!({"schema_version":1,"kind":"bounded integrated dual-feed discovery","cases":cases,
        "limits":["cold collector startup/reopen; not a continuously running collector","synthetic supported SQLite schema with persisted spawn edge","snapshot window modeled from already-proven256-event retention","no live provider or personal data","no performance claim"]})
            )?
        ),
    )?;
    println!("Completed3 integrated cases:2 bounded-tail misses,1 recent-record recovery; persisted relationship recovered in all3.");
    Ok(())
}
