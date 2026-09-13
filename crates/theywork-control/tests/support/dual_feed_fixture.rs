//! SQL fixture recipes from iteration 7, now an automated reconciliation oracle.
use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::json;
use std::fs;
use std::path::Path;
use theywork_control::ControlEvent;
const BASE: i64 = 1_780_000_000_000;
pub(super) fn create_store(
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

pub(super) fn stream(count: usize, aggregate: bool) -> Vec<ControlEvent> {
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
