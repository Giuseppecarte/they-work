use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{params_from_iter, Connection, OpenFlags};
use serde_json::Value;
use theywork_core::{
    Activity, Agent, Event, EventKind, Millis, OfficeId, Source, SourceError, WorkerId,
    BLOCKED_AFTER_MS,
};

use crate::util::{path_allowed, recency_cutoff, repository_root, short_id, truncate_detail};
use crate::DEFAULT_ACTIVE_WITHIN;
const ASSESSOR_TITLE_PREFIX: &str =
    "The following is the Codex agent history whose request action you are assessing";

/// Reads the current Codex roster and activity feed from its SQLite stores.
pub struct CodexSource {
    home: PathBuf,
    only_paths: Vec<PathBuf>,
    active_within: Duration,
    threads: BTreeMap<String, ThreadRecord>,
    item_watermarks: HashMap<String, i64>,
    office_cache: HashMap<String, String>,
    items_initialized: bool,
    turn_states: HashMap<String, TurnState>,
}

impl CodexSource {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self::with_paths_and_active_within(home, Vec::new(), DEFAULT_ACTIVE_WITHIN)
    }

    pub fn with_paths(home: impl Into<PathBuf>, only_paths: Vec<PathBuf>) -> Self {
        Self::with_paths_and_active_within(home, only_paths, DEFAULT_ACTIVE_WITHIN)
    }

    pub fn with_paths_and_active_within(
        home: impl Into<PathBuf>,
        only_paths: Vec<PathBuf>,
        active_within: Duration,
    ) -> Self {
        Self {
            home: home.into(),
            only_paths,
            active_within,
            threads: BTreeMap::new(),
            item_watermarks: HashMap::new(),
            items_initialized: false,
            turn_states: HashMap::new(),
            office_cache: HashMap::new(),
        }
    }

    pub fn new_with_paths(home: impl Into<PathBuf>, only_paths: Vec<PathBuf>) -> Self {
        Self::with_paths(home, only_paths)
    }

    pub(crate) fn sqlite_exists(home: &Path) -> bool {
        let sqlite = home.join("sqlite");
        sqlite.is_dir()
            && sqlite.join("state_5.sqlite").is_file()
            && sqlite.join("thread_history_1.sqlite").is_file()
    }

    fn state_path(&self) -> PathBuf {
        self.home.join("sqlite").join("state_5.sqlite")
    }

    fn history_path(&self) -> PathBuf {
        self.home.join("sqlite").join("thread_history_1.sqlite")
    }

    fn minimum_item_watermark(&self, current: &BTreeMap<String, ThreadRecord>) -> i64 {
        if !self.items_initialized {
            return -1;
        }
        current
            .keys()
            .map(|thread_id| {
                self.item_watermarks
                    .get(thread_id)
                    .copied()
                    .unwrap_or(DEFAULT_ITEM_WATERMARK)
            })
            .min()
            .unwrap_or(DEFAULT_ITEM_WATERMARK)
    }
}

impl Source for CodexSource {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn poll(&mut self, now: Millis) -> Result<Vec<Event>, SourceError> {
        let cutoff_ms = recency_cutoff(now, self.active_within);
        let Some(state) = open_read_only(&self.state_path()) else {
            debug_blocked_unavailable("state database could not be opened read-only");
            return Ok(Vec::new());
        };
        let Ok(roster) = read_threads(&state, cutoff_ms, &mut self.office_cache) else {
            debug_blocked_unavailable("thread roster query failed");
            return Ok(Vec::new());
        };
        let assessor_ids: Vec<String> = roster
            .iter()
            .filter(|thread| thread.classification.kind == ThreadKind::ApprovalAssessor)
            .map(|thread| thread.id.clone())
            .collect();
        let edges = read_spawn_edges(&state, &assessor_ids).unwrap_or_default();
        let Some(history) = open_read_only(&self.history_path()) else {
            debug_blocked_unavailable("history database could not be opened read-only");
            return Ok(Vec::new());
        };
        let turns = match read_turns(&history) {
            Ok(turns) => turns,
            // A partially upgraded older database may have the item table but
            // no turn table. The feed is still safe to display in that case.
            Err(error) if error.to_string().contains("no such table") => Vec::new(),
            Err(_) => {
                debug_blocked_unavailable("turn history query failed");
                return Ok(Vec::new());
            }
        };
        let turns_by_thread: HashMap<String, TurnRecord> = turns
            .iter()
            .cloned()
            .map(|turn| (turn.thread_id.clone(), turn))
            .collect();
        let mut assessors = Vec::new();
        let mut current = BTreeMap::new();
        let mut exclusions: BTreeMap<&'static str, (usize, String)> = BTreeMap::new();
        for thread in roster {
            match thread.classification.kind {
                ThreadKind::ApprovalAssessor => {
                    record_exclusion(&mut exclusions, &thread);
                    if path_allowed(&thread.raw_office_path, &self.only_paths) {
                        assessors.push(thread);
                    }
                }
                ThreadKind::Developer => {
                    if path_allowed(&thread.raw_office_path, &self.only_paths) {
                        current.insert(thread.id.clone(), thread);
                    }
                }
                ThreadKind::Subagent | ThreadKind::InternalReview => {
                    record_exclusion(&mut exclusions, &thread);
                }
            }
        }
        let parent_ids: Vec<String> = edges
            .iter()
            .filter(|edge| edge_is_active(&edge.status))
            .map(|edge| edge.parent_thread_id.clone())
            .filter(|parent_id| !current.contains_key(parent_id))
            .collect();
        for parent in
            read_threads_by_ids(&state, &parent_ids, &mut self.office_cache).unwrap_or_default()
        {
            match parent.classification.kind {
                ThreadKind::Developer => {
                    if path_allowed(&parent.raw_office_path, &self.only_paths) {
                        current.insert(parent.id.clone(), parent);
                    }
                }
                ThreadKind::ApprovalAssessor
                | ThreadKind::Subagent
                | ThreadKind::InternalReview => record_exclusion(&mut exclusions, &parent),
            }
        }
        debug_exclusion_summary(&exclusions);
        disambiguate_thread_names(&mut current);
        let Ok(items) = read_items(&history, self.minimum_item_watermark(&current)) else {
            debug_blocked_unavailable("activity item query failed");
            return Ok(Vec::new());
        };

        let previous = std::mem::replace(&mut self.threads, current);
        let mut events = Vec::new();

        for (id, old_thread) in &previous {
            match self.threads.get(id) {
                None => events.push(old_thread.event(now, EventKind::Left)),
                Some(new_thread) if old_thread.office_path != new_thread.office_path => {
                    events.push(old_thread.event(now, EventKind::Left));
                }
                _ => {}
            }
        }
        for (id, thread) in &self.threads {
            let previous_thread = previous.get(id);
            let seen_changed = previous_thread.is_none_or(|old| {
                old.office_path != thread.office_path
                    || old.name != thread.name
                    || old.git_branch != thread.git_branch
            });
            if seen_changed {
                events.push(thread.event(
                    thread.updated_at_ms,
                    EventKind::Seen {
                        name: thread.name.clone(),
                        git_branch: thread.git_branch.clone(),
                    },
                ));
            }
            if previous_thread.is_none_or(|old| old.tokens_used != thread.tokens_used) {
                events.push(
                    thread.event(thread.updated_at_ms, EventKind::Tokens(thread.tokens_used)),
                );
            }
        }

        let mut watermark_updates: HashMap<String, i64> = HashMap::new();
        for item in items {
            let previous_watermark = self
                .item_watermarks
                .get(&item.thread_id)
                .copied()
                .unwrap_or({
                    if self.items_initialized {
                        DEFAULT_ITEM_WATERMARK
                    } else {
                        -1
                    }
                });
            if item.created_at_ms <= previous_watermark {
                continue;
            }

            watermark_updates
                .entry(item.thread_id.clone())
                .and_modify(|watermark| *watermark = (*watermark).max(item.created_at_ms))
                .or_insert(item.created_at_ms);

            let Some(thread) = self.threads.get(&item.thread_id) else {
                continue;
            };
            let payload = serde_json::from_str::<Value>(&item.item_json).unwrap_or(Value::Null);
            let Some(kind) = item_kind(&item.item_type, &payload) else {
                continue;
            };
            events.push(thread.event(item.created_at_ms, kind));
        }
        for (thread_id, watermark) in watermark_updates {
            self.item_watermarks
                .entry(thread_id)
                .and_modify(|current| *current = (*current).max(watermark))
                .or_insert(watermark);
        }
        self.items_initialized = true;

        self.turn_states
            .retain(|thread_id, _| self.threads.contains_key(thread_id));
        for turn in turns {
            let Some(thread) = self.threads.get(&turn.thread_id) else {
                continue;
            };
            let Some(in_flight) = turn_in_flight(&turn.status) else {
                continue;
            };
            let state = TurnState {
                turn_id: turn.turn_id.clone(),
                in_flight,
                error_detail: turn.error_detail(),
            };
            let previous_state = self.turn_states.get(&turn.thread_id);
            let turn_changed = previous_state
                .is_none_or(|old| old.turn_id != state.turn_id || old.in_flight != state.in_flight);
            let error_changed =
                previous_state.is_none_or(|old| old.error_detail != state.error_detail);
            let at = turn.event_at(thread);

            if turn_changed {
                events.push(thread.event(at, EventKind::Turn { in_flight }));
            }
            if error_changed {
                if let Some(detail) = state.error_detail.clone() {
                    events.push(thread.event(at, EventKind::Acted(Activity::Error { detail })));
                }
            }
            self.turn_states.insert(turn.thread_id, state);
        }

        let waiting = waiting_signals(&assessors, &edges, &self.threads, &turns_by_thread, now);
        let diagnostic = waiting_diagnostic(
            &assessors,
            &edges,
            &self.threads,
            &turns_by_thread,
            &waiting,
            now,
        );
        debug_blocked_diagnostic(&diagnostic, &waiting);
        let waiting_parent_ids: Vec<String> = waiting
            .iter()
            .map(|signal| signal.parent_thread_id.clone())
            .collect();
        let pending_commands =
            read_pending_commands(&history, &waiting_parent_ids).unwrap_or_default();
        for signal in waiting {
            let Some(thread) = self.threads.get(&signal.parent_thread_id) else {
                continue;
            };
            let pending = pending_commands.get(&signal.parent_thread_id);
            let at = pending.map_or(signal.at, |pending| signal.at.max(pending.at));
            let detail = pending
                .map(|pending| pending.detail.clone())
                .unwrap_or_else(|| {
                    format!(
                        "approval required ({})",
                        short_id(&signal.assessor_thread_id)
                    )
                });
            let detail = match signal.strategy {
                WaitingStrategy::SpawnEdge => detail,
                WaitingStrategy::CwdTimeFallback => {
                    truncate_detail(&format!("cwd/time fallback: {detail}"))
                }
            };
            debug_waiting(&signal, &detail);
            events.push(thread.event(at, EventKind::Acted(Activity::Waiting { detail })));
        }
        events.sort_by_key(|event| event.at);
        Ok(events)
    }
}

#[derive(Clone)]
struct ThreadRecord {
    id: String,
    raw_office_path: String,
    office_path: String,
    name: String,
    tokens_used: u64,
    git_branch: Option<String>,
    updated_at_ms: Millis,
    classification: ThreadClassification,
    name_is_fallback: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThreadKind {
    Developer,
    ApprovalAssessor,
    Subagent,
    InternalReview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ThreadClassification {
    kind: ThreadKind,
    reason: &'static str,
}

impl ThreadRecord {
    fn event(&self, at: Millis, kind: EventKind) -> Event {
        Event {
            at,
            office: OfficeId(self.office_path.clone()),
            office_path: self.office_path.clone(),
            worker: WorkerId(self.id.clone()),
            agent: Agent::Codex,
            kind,
        }
    }
}

struct ItemRecord {
    thread_id: String,
    created_at_ms: i64,
    item_type: String,
    item_json: String,
}

#[derive(Clone)]
struct TurnRecord {
    thread_id: String,
    turn_id: String,
    status: String,
    started_at: Option<i64>,
    completed_at: Option<i64>,
    error_json: Option<String>,
}

impl TurnRecord {
    fn event_at(&self, thread: &ThreadRecord) -> Millis {
        let timestamp = match self.status.as_str() {
            "completed" => self.completed_at.or(self.started_at),
            _ => self.started_at.or(self.completed_at),
        };
        timestamp.map(epoch_millis).unwrap_or(thread.updated_at_ms)
    }

    fn error_detail(&self) -> Option<String> {
        error_detail(self.error_json.as_deref())
    }
}

#[derive(PartialEq, Eq)]
struct TurnState {
    turn_id: String,
    in_flight: bool,
    error_detail: Option<String>,
}

fn open_read_only(path: &Path) -> Option<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()
}

fn table_has_column(connection: &Connection, table: &str, wanted: &str) -> rusqlite::Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == wanted {
            return Ok(true);
        }
    }
    Ok(false)
}

fn read_threads(
    connection: &Connection,
    cutoff_ms: Millis,
    office_cache: &mut HashMap<String, String>,
) -> rusqlite::Result<Vec<ThreadRecord>> {
    let columns = thread_columns(connection)?;
    let query = thread_query(
        &columns,
        &format!("({}) >= ?1", columns.updated_at_expression),
    );
    let mut statement = connection.prepare(&query)?;
    let mut rows = statement.query([cutoff_ms])?;
    let mut threads = Vec::new();
    while let Some(row) = rows.next()? {
        if let Some(thread) = decode_thread_row(row, office_cache)? {
            threads.push(thread);
        }
    }
    Ok(threads)
}

fn read_items(
    connection: &Connection,
    minimum_watermark: i64,
) -> rusqlite::Result<Vec<ItemRecord>> {
    let mut statement = connection.prepare(
        "SELECT thread_id, created_at_ms, item_type, item_json \
         FROM thread_items WHERE created_at_ms >= ?1 ORDER BY created_at_ms ASC",
    )?;
    let mut rows = statement.query([minimum_watermark])?;
    let mut items = Vec::new();
    while let Some(row) = rows.next()? {
        items.push(ItemRecord {
            thread_id: row.get(0)?,
            created_at_ms: row.get(1)?,
            item_type: row.get(2)?,
            item_json: row.get(3)?,
        });
    }
    Ok(items)
}

fn read_turns(connection: &Connection) -> rusqlite::Result<Vec<TurnRecord>> {
    let has_error_json = table_has_column(connection, "thread_turns", "error_json")?;
    let error_expression = if has_error_json { "error_json" } else { "NULL" };
    let query = format!(
        "SELECT thread_id, turn_id, status, started_at, completed_at, error_json \
         FROM (SELECT thread_id, turn_id, status, started_at, completed_at, \
         {error_expression} AS error_json, \
         ROW_NUMBER() OVER (PARTITION BY thread_id ORDER BY started_at DESC, turn_id DESC) \
         AS row_number FROM thread_turns) \
         WHERE row_number = 1 ORDER BY started_at DESC"
    );
    let mut statement = connection.prepare(&query)?;
    let mut rows = statement.query([])?;
    let mut turns = Vec::new();
    while let Some(row) = rows.next()? {
        turns.push(TurnRecord {
            thread_id: row.get(0)?,
            turn_id: row.get(1)?,
            status: row.get(2)?,
            started_at: row.get(3)?,
            completed_at: row.get(4)?,
            error_json: row.get(5)?,
        });
    }
    Ok(turns)
}

fn item_kind(item_type: &str, payload: &Value) -> Option<EventKind> {
    match item_type {
        "commandExecution" => Some(EventKind::Acted(Activity::Typing {
            detail: json_detail(payload, &["command", "cmd"]),
        })),
        "reasoning" => Some(EventKind::Acted(Activity::Thinking)),
        "agentMessage" => Some(EventKind::Acted(Activity::Talking {
            detail: json_detail(payload, &["message", "text", "content"]),
        })),
        "mcpToolCall" => Some(EventKind::Acted(Activity::Searching {
            detail: json_detail(payload, &["name", "tool", "tool_name", "query", "url"]),
        })),
        "webSearch" => Some(EventKind::Acted(Activity::Searching {
            detail: json_detail(payload, &["query", "url"]),
        })),
        "fileChange" => Some(EventKind::Acted(Activity::Editing {
            detail: json_detail(payload, &["path", "file_path", "filename", "changes"]),
        })),
        "userMessage" => Some(EventKind::Turn { in_flight: true }),
        "contextCompaction" => None,
        _ => None,
    }
}

fn turn_in_flight(status: &str) -> Option<bool> {
    match status {
        "inProgress" => Some(true),
        "completed" => Some(false),
        _ => None,
    }
}

fn epoch_millis(value: i64) -> Millis {
    if value > -100_000_000_000 && value < 100_000_000_000 {
        value.saturating_mul(1_000)
    } else {
        value
    }
}

fn error_detail(error_json: Option<&str>) -> Option<String> {
    let raw = error_json?.trim();
    if raw.is_empty() || raw == "null" {
        return None;
    }
    let payload = serde_json::from_str::<Value>(raw).unwrap_or_else(|_| Value::String(raw.into()));
    let detail = value_detail(&payload).unwrap_or_else(|| raw.to_string());
    let detail = truncate_detail(&detail);
    Some(if detail.is_empty() {
        "turn failed".to_string()
    } else {
        detail
    })
}

fn json_detail(payload: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(value_detail))
        .map(|detail| truncate_detail(&detail))
        .unwrap_or_default()
}

fn value_detail(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Array(values) => values.iter().find_map(value_detail),
        Value::Object(object) => [
            "text",
            "message",
            "content",
            "path",
            "file_path",
            "name",
            "command",
            "query",
            "url",
            "tool",
            "detail",
            "error",
            "reason",
            "changes",
        ]
        .iter()
        .find_map(|key| object.get(*key).and_then(value_detail)),
        _ => None,
    }
}

const DEFAULT_ITEM_WATERMARK: i64 = 0;

struct ThreadColumns {
    name_expression: &'static str,
    nickname_expression: &'static str,
    title_expression: &'static str,
    source_expression: &'static str,
    thread_source_expression: &'static str,
    updated_at_expression: &'static str,
    archived_expression: &'static str,
}

fn thread_columns(connection: &Connection) -> rusqlite::Result<ThreadColumns> {
    let name_expression = if table_has_column(connection, "threads", "name")? {
        "name"
    } else {
        "NULL"
    };
    let nickname_expression = if table_has_column(connection, "threads", "agent_nickname")? {
        "agent_nickname"
    } else {
        "NULL"
    };
    let title_expression = if table_has_column(connection, "threads", "title")? {
        "title"
    } else {
        "NULL"
    };
    let source_expression = if table_has_column(connection, "threads", "source")? {
        "source"
    } else {
        "NULL"
    };
    let thread_source_expression = if table_has_column(connection, "threads", "thread_source")? {
        "thread_source"
    } else {
        "NULL"
    };
    let has_updated_at = table_has_column(connection, "threads", "updated_at")?;
    let has_updated_at_ms = table_has_column(connection, "threads", "updated_at_ms")?;
    let updated_at_expression = match (has_updated_at, has_updated_at_ms) {
        (true, true) => "COALESCE(updated_at_ms, updated_at * 1000)",
        (true, false) => "updated_at * 1000",
        (false, true) => "updated_at_ms",
        (false, false) => "0",
    };
    let archived_expression = if table_has_column(connection, "threads", "archived")? {
        "COALESCE(archived, 0) != 1"
    } else {
        "1 = 1"
    };
    Ok(ThreadColumns {
        name_expression,
        nickname_expression,
        title_expression,
        source_expression,
        thread_source_expression,
        updated_at_expression,
        archived_expression,
    })
}

fn thread_query(columns: &ThreadColumns, filter: &str) -> String {
    format!(
        "SELECT id, cwd, {name_expression}, {nickname_expression}, {title_expression}, {source_expression}, {thread_source_expression},          tokens_used, git_branch, {updated_at_expression} AS observed_at_ms FROM threads          WHERE {archived_expression} AND {filter}",
        name_expression = columns.name_expression,
        nickname_expression = columns.nickname_expression,
        title_expression = columns.title_expression,
        source_expression = columns.source_expression,
        thread_source_expression = columns.thread_source_expression,
        updated_at_expression = columns.updated_at_expression,
        archived_expression = columns.archived_expression,
        filter = filter,
    )
}

fn decode_thread_row(
    row: &rusqlite::Row<'_>,
    office_cache: &mut HashMap<String, String>,
) -> rusqlite::Result<Option<ThreadRecord>> {
    let id: String = row.get(0)?;
    let Some(raw_office_path) = row
        .get::<_, Option<String>>(1)?
        .filter(|path| !path.trim().is_empty())
    else {
        return Ok(None);
    };
    let office_path = repository_root(&raw_office_path, office_cache);
    if office_path.is_empty() {
        return Ok(None);
    }

    let name = row.get::<_, Option<String>>(2)?;
    let agent_nickname = row.get::<_, Option<String>>(3)?;
    let title = row.get::<_, Option<String>>(4)?;
    let source = row.get::<_, Option<String>>(5)?;
    let thread_source = row.get::<_, Option<String>>(6)?;
    let classification = classify_thread(
        title.as_deref(),
        source.as_deref(),
        thread_source.as_deref(),
    );
    let (name, name_is_fallback) = choose_thread_name(&id, [name, agent_nickname, title]);
    let tokens_used = row.get::<_, Option<i64>>(7)?.unwrap_or_default().max(0) as u64;
    let git_branch = row
        .get::<_, Option<String>>(8)?
        .filter(|branch| !branch.trim().is_empty());
    let updated_at_ms = row.get::<_, Option<i64>>(9)?.unwrap_or_default();

    Ok(Some(ThreadRecord {
        id,
        raw_office_path,
        office_path,
        name,
        tokens_used,
        git_branch,
        updated_at_ms,
        classification,
        name_is_fallback,
    }))
}

fn choose_thread_name(id: &str, candidates: [Option<String>; 3]) -> (String, bool) {
    for candidate in candidates.into_iter().flatten() {
        let candidate = candidate.trim();
        if candidate.is_empty() || candidate.starts_with('/') {
            continue;
        }
        let candidate = truncate_detail(candidate);
        if !candidate.is_empty() && !candidate.starts_with('/') {
            return (candidate, false);
        }
    }

    let fallback = short_id(id);
    if fallback.starts_with('/') {
        ("worker".to_string(), true)
    } else {
        (fallback, true)
    }
}

fn classify_thread(
    title: Option<&str>,
    source: Option<&str>,
    thread_source: Option<&str>,
) -> ThreadClassification {
    if structural_assessor_marker(source, thread_source) {
        return ThreadClassification {
            kind: ThreadKind::ApprovalAssessor,
            reason: "structural approval assessor",
        };
    }

    let thread_source_marker = thread_source.map(normalize_marker);
    if thread_source_marker
        .as_deref()
        .is_some_and(is_internal_review_marker)
    {
        return ThreadClassification {
            kind: ThreadKind::InternalReview,
            reason: "thread_source=guardian_review/internal review",
        };
    }
    if thread_source_marker.as_deref() == Some("subagent") {
        return ThreadClassification {
            kind: ThreadKind::Subagent,
            reason: "thread_source=subagent",
        };
    }
    if source.is_some_and(|source| source_has_json_key(source, "subagent")) {
        return ThreadClassification {
            kind: ThreadKind::Subagent,
            reason: "source.subagent",
        };
    }

    if source.is_none_or(|source| source.trim().is_empty())
        && thread_source.is_none_or(|thread_source| thread_source.trim().is_empty())
        && title.is_some_and(|title| title.starts_with(ASSESSOR_TITLE_PREFIX))
    {
        // Older state stores have no structural provenance columns. Keep the
        // title check only as a compatibility fallback for those rows.
        return ThreadClassification {
            kind: ThreadKind::ApprovalAssessor,
            reason: "title prefix (legacy assessor fallback)",
        };
    }

    ThreadClassification {
        kind: ThreadKind::Developer,
        reason: "developer",
    }
}

fn structural_assessor_marker(source: Option<&str>, thread_source: Option<&str>) -> bool {
    thread_source.is_some_and(|value| is_assessor_marker(&normalize_marker(value)))
        || source.is_some_and(|source| {
            serde_json::from_str::<Value>(source)
                .ok()
                .is_some_and(|value| json_has_assessor_marker(&value))
        })
}

fn is_assessor_marker(marker: &str) -> bool {
    matches!(
        marker,
        "assessor" | "approval" | "approvalassessor" | "approvalreview"
    )
}

fn is_internal_review_marker(marker: &str) -> bool {
    matches!(
        marker,
        "guardianreview" | "internalreview" | "systemreview" | "review"
    )
}

fn normalize_marker(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn source_has_json_key(source: &str, wanted: &str) -> bool {
    serde_json::from_str::<Value>(source)
        .ok()
        .is_some_and(|value| json_has_key(&value, wanted))
}

fn json_has_key(value: &Value, wanted: &str) -> bool {
    match value {
        Value::Object(object) => {
            object.contains_key(wanted) || object.values().any(|value| json_has_key(value, wanted))
        }
        Value::Array(values) => values.iter().any(|value| json_has_key(value, wanted)),
        _ => false,
    }
}

fn json_has_assessor_marker(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            is_assessor_marker(&normalize_marker(key)) || json_has_assessor_marker(value)
        }),
        Value::Array(values) => values.iter().any(json_has_assessor_marker),
        Value::String(value) => is_assessor_marker(&normalize_marker(value)),
        _ => false,
    }
}

fn disambiguate_thread_names(threads: &mut BTreeMap<String, ThreadRecord>) {
    let mut used_by_office: HashMap<String, HashSet<String>> = HashMap::new();
    for thread in threads.values_mut() {
        let used = used_by_office
            .entry(thread.office_path.clone())
            .or_default();
        if used.insert(thread.name.clone()) {
            continue;
        }

        let original = thread.name.clone();
        let candidate = if thread.name_is_fallback {
            unique_id_name(&thread.id, used)
        } else {
            String::new()
        };
        let candidate = if candidate.is_empty() || used.contains(&candidate) {
            unique_suffix(&original, used)
        } else {
            candidate
        };
        used.insert(candidate.clone());
        thread.name = candidate;
    }
}

fn unique_id_name(id: &str, used: &HashSet<String>) -> String {
    let characters: Vec<char> = id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect();
    if characters.is_empty() {
        return String::new();
    }

    let start = characters.len().min(8);
    for length in start..=characters.len() {
        let candidate: String = characters.iter().take(length).collect();
        if !used.contains(&candidate) {
            return candidate;
        }
    }
    String::new()
}

fn unique_suffix(base: &str, used: &HashSet<String>) -> String {
    let mut suffix = 2;
    loop {
        let candidate = format!("{base} ({suffix})");
        if !used.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn record_exclusion(summary: &mut BTreeMap<&'static str, (usize, String)>, thread: &ThreadRecord) {
    let entry = summary
        .entry(thread.classification.reason)
        .or_insert_with(|| (0, thread.id.clone()));
    entry.0 += 1;
}

fn debug_exclusion_summary(summary: &BTreeMap<&'static str, (usize, String)>) {
    if std::env::var_os("THEYWORK_COLLECT_DEBUG").is_none() {
        return;
    }
    if summary.is_empty() {
        eprintln!("codex exclusions: none");
        return;
    }
    for (reason, (count, example)) in summary {
        eprintln!("codex excluded {count} thread(s): {reason}; example={example}");
    }
}

fn debug_waiting(signal: &WaitingSignal, detail: &str) {
    if std::env::var_os("THEYWORK_COLLECT_DEBUG").is_some() {
        eprintln!(
            "codex waiting: parent={} strategy={} detail={detail}",
            signal.parent_thread_id,
            signal.strategy.label()
        );
    }
}

struct WaitingDiagnostic {
    assessor_threads_seen: usize,
    open_turns: usize,
    silent_open_turns: usize,
    active_spawn_edges: usize,
    spawn_edges_resolved: usize,
    cwd_time_fallbacks: usize,
}

fn waiting_diagnostic(
    assessors: &[ThreadRecord],
    edges: &[SpawnEdge],
    developers: &BTreeMap<String, ThreadRecord>,
    turns: &HashMap<String, TurnRecord>,
    waiting: &[WaitingSignal],
    now: Millis,
) -> WaitingDiagnostic {
    let mut open_turns = 0;
    let mut silent_open_turns = 0;
    for parent in developers.values() {
        if turns
            .get(&parent.id)
            .and_then(|turn| turn_in_flight(&turn.status))
            != Some(true)
        {
            continue;
        }
        open_turns += 1;
        if now.saturating_sub(parent.updated_at_ms) > BLOCKED_AFTER_MS {
            silent_open_turns += 1;
        }
    }

    let active_spawn_edges = edges
        .iter()
        .filter(|edge| edge_is_active(&edge.status))
        .count();
    let spawn_edges_resolved = edges
        .iter()
        .filter(|edge| {
            edge_is_active(&edge.status)
                && assessors
                    .iter()
                    .any(|assessor| assessor.id == edge.child_thread_id)
                && developers
                    .get(&edge.parent_thread_id)
                    .is_some_and(|parent| parent.classification.kind == ThreadKind::Developer)
        })
        .count();
    let cwd_time_fallbacks = waiting
        .iter()
        .filter(|signal| signal.strategy == WaitingStrategy::CwdTimeFallback)
        .count();

    WaitingDiagnostic {
        assessor_threads_seen: assessors.len(),
        open_turns,
        silent_open_turns,
        active_spawn_edges,
        spawn_edges_resolved,
        cwd_time_fallbacks,
    }
}

fn debug_blocked_diagnostic(diagnostic: &WaitingDiagnostic, waiting: &[WaitingSignal]) {
    if std::env::var_os("THEYWORK_COLLECT_DEBUG").is_none() {
        return;
    }

    let blocked_set = waiting
        .iter()
        .map(|signal| format!("{}({})", signal.parent_thread_id, signal.strategy.label()))
        .collect::<Vec<_>>()
        .join(",");
    let blocked_set = if blocked_set.is_empty() {
        "none".to_string()
    } else {
        blocked_set
    };
    eprintln!(
        "codex blocked: result={blocked_set}; assessor_threads_seen={} open_turns={} silent_open_turns={} active_spawn_edges={} spawn_edges_resolved={} cwd_time_fallbacks={}",
        diagnostic.assessor_threads_seen,
        diagnostic.open_turns,
        diagnostic.silent_open_turns,
        diagnostic.active_spawn_edges,
        diagnostic.spawn_edges_resolved,
        diagnostic.cwd_time_fallbacks,
    );
    if waiting.is_empty() {
        eprintln!(
            "codex blocked: empty; why={}",
            empty_blocked_reason(diagnostic)
        );
    }
}

fn empty_blocked_reason(diagnostic: &WaitingDiagnostic) -> String {
    let mut reasons = Vec::new();
    if diagnostic.assessor_threads_seen == 0 {
        reasons.push("no assessor threads seen");
    }
    if diagnostic.open_turns == 0 {
        reasons.push("no open turns");
    } else if diagnostic.silent_open_turns == 0 {
        reasons.push("no open turns silent past the blocked threshold");
    }
    if diagnostic.active_spawn_edges == 0 && diagnostic.assessor_threads_seen > 0 {
        reasons.push("no active assessor spawn edges");
    }
    if diagnostic.silent_open_turns > 0
        && diagnostic.spawn_edges_resolved == 0
        && diagnostic.cwd_time_fallbacks == 0
    {
        reasons.push("no assessor matched a developer by spawn edge or cwd/time");
    }
    if reasons.is_empty() {
        reasons.push("no correlation produced a blocked developer");
    }
    reasons.join("; ")
}

fn debug_blocked_unavailable(reason: &str) {
    if std::env::var_os("THEYWORK_COLLECT_DEBUG").is_some() {
        eprintln!(
            "codex blocked: result=unavailable; assessor_threads_seen=0; open_turns=0; silent_open_turns=0; active_spawn_edges=0; spawn_edges_resolved=0; cwd_time_fallbacks=0; why={reason}"
        );
    }
}

fn read_threads_by_ids(
    connection: &Connection,
    ids: &[String],
    office_cache: &mut HashMap<String, String>,
) -> rusqlite::Result<Vec<ThreadRecord>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let columns = thread_columns(connection)?;
    let query = thread_query(&columns, "id = ?1");
    let mut statement = connection.prepare(&query)?;
    let mut threads = Vec::new();
    for id in ids {
        let mut rows = statement.query([id])?;
        while let Some(row) = rows.next()? {
            if let Some(thread) = decode_thread_row(row, office_cache)? {
                threads.push(thread);
            }
        }
    }
    Ok(threads)
}

#[derive(Clone)]
struct SpawnEdge {
    parent_thread_id: String,
    child_thread_id: String,
    status: String,
}

fn read_spawn_edges(
    connection: &Connection,
    assessor_ids: &[String],
) -> rusqlite::Result<Vec<SpawnEdge>> {
    if assessor_ids.is_empty()
        || !table_has_column(connection, "thread_spawn_edges", "parent_thread_id")?
        || !table_has_column(connection, "thread_spawn_edges", "child_thread_id")?
        || !table_has_column(connection, "thread_spawn_edges", "status")?
    {
        return Ok(Vec::new());
    }

    let placeholders = (0..assessor_ids.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "SELECT parent_thread_id, child_thread_id, status FROM thread_spawn_edges          WHERE child_thread_id IN ({placeholders})"
    );
    let mut statement = connection.prepare(&query)?;
    let mut rows = statement.query(params_from_iter(assessor_ids.iter()))?;
    let mut edges = Vec::new();
    while let Some(row) = rows.next()? {
        edges.push(SpawnEdge {
            parent_thread_id: row.get(0)?,
            child_thread_id: row.get(1)?,
            status: row.get(2)?,
        });
    }
    Ok(edges)
}

fn edge_is_active(status: &str) -> bool {
    let status: String = status
        .trim()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    !matches!(
        status.as_str(),
        "completed"
            | "closed"
            | "resolved"
            | "cancelled"
            | "canceled"
            | "rejected"
            | "declined"
            | "failed"
            | "error"
            | "approved"
            | "success"
            | "done"
            | "timeout"
            | "timedout"
            | "killed"
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WaitingStrategy {
    SpawnEdge,
    CwdTimeFallback,
}

impl WaitingStrategy {
    fn label(self) -> &'static str {
        match self {
            Self::SpawnEdge => "spawn edge",
            Self::CwdTimeFallback => "cwd/time fallback",
        }
    }
}

#[derive(Clone)]
struct WaitingSignal {
    parent_thread_id: String,
    assessor_thread_id: String,
    at: Millis,
    strategy: WaitingStrategy,
}

fn waiting_signals(
    assessors: &[ThreadRecord],
    edges: &[SpawnEdge],
    developers: &BTreeMap<String, ThreadRecord>,
    turns: &HashMap<String, TurnRecord>,
    now: Millis,
) -> Vec<WaitingSignal> {
    let mut signals = BTreeMap::new();

    for edge in edges.iter().filter(|edge| edge_is_active(&edge.status)) {
        let Some(assessor) = assessors
            .iter()
            .find(|assessor| assessor.id == edge.child_thread_id)
        else {
            continue;
        };
        let Some(parent) = developers.get(&edge.parent_thread_id) else {
            continue;
        };
        if parent.classification.kind != ThreadKind::Developer {
            continue;
        }
        if turns
            .get(&parent.id)
            .and_then(|turn| turn_in_flight(&turn.status))
            != Some(true)
            || now.saturating_sub(parent.updated_at_ms) <= BLOCKED_AFTER_MS
        {
            continue;
        }

        // Spawn-edge strategy: state_5.sqlite explicitly links this assessor
        // to the developer whose command is awaiting approval.
        let signal = WaitingSignal {
            parent_thread_id: parent.id.clone(),
            assessor_thread_id: assessor.id.clone(),
            at: parent.updated_at_ms.max(assessor.updated_at_ms),
            strategy: WaitingStrategy::SpawnEdge,
        };
        signals
            .entry(signal.parent_thread_id.clone())
            .and_modify(|old: &mut WaitingSignal| old.at = old.at.max(signal.at))
            .or_insert(signal);
    }

    for assessor in assessors {
        if edges
            .iter()
            .any(|edge| edge.child_thread_id == assessor.id && edge_is_active(&edge.status))
        {
            continue;
        }
        let parent = developers
            .values()
            .filter(|parent| {
                parent.office_path == assessor.office_path
                    && parent.classification.kind == ThreadKind::Developer
                    && turns
                        .get(&parent.id)
                        .and_then(|turn| turn_in_flight(&turn.status))
                        == Some(true)
                    && now.saturating_sub(parent.updated_at_ms) > BLOCKED_AFTER_MS
            })
            .max_by_key(|parent| parent.updated_at_ms);
        let Some(parent) = parent else {
            continue;
        };

        // Cwd/time fallback strategy: older state stores may not have a
        // usable spawn edge, so a recent same-office assessor is paired with
        // the developer's open turn after the normal silence bound.
        let signal = WaitingSignal {
            parent_thread_id: parent.id.clone(),
            assessor_thread_id: assessor.id.clone(),
            at: parent.updated_at_ms.max(assessor.updated_at_ms),
            strategy: WaitingStrategy::CwdTimeFallback,
        };
        signals
            .entry(signal.parent_thread_id.clone())
            .and_modify(|old: &mut WaitingSignal| old.at = old.at.max(signal.at))
            .or_insert(signal);
    }

    signals.into_values().collect()
}

#[derive(Clone)]
struct PendingCommand {
    at: Millis,
    detail: String,
}

fn read_pending_commands(
    connection: &Connection,
    parent_ids: &[String],
) -> rusqlite::Result<HashMap<String, PendingCommand>> {
    if parent_ids.is_empty()
        || !table_has_column(connection, "thread_items", "thread_id")?
        || !table_has_column(connection, "thread_items", "created_at_ms")?
        || !table_has_column(connection, "thread_items", "item_type")?
        || !table_has_column(connection, "thread_items", "item_json")?
    {
        return Ok(HashMap::new());
    }

    let placeholders = (0..parent_ids.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "SELECT thread_id, created_at_ms, item_json FROM thread_items          WHERE item_type = 'commandExecution' AND thread_id IN ({placeholders})          ORDER BY created_at_ms DESC"
    );
    let mut statement = connection.prepare(&query)?;
    let mut rows = statement.query(params_from_iter(parent_ids.iter()))?;
    let mut commands = HashMap::new();
    while let Some(row) = rows.next()? {
        let thread_id: String = row.get(0)?;
        if commands.contains_key(&thread_id) {
            continue;
        }
        let at: Millis = row.get(1)?;
        let item_json: String = row.get(2)?;
        let payload = serde_json::from_str::<Value>(&item_json).unwrap_or(Value::Null);
        if command_status(&payload).is_some_and(command_status_is_terminal) {
            continue;
        }
        let command = json_detail(&payload, &["command", "cmd", "description", "prompt"]);
        let detail = if command.is_empty() {
            "approval required".to_string()
        } else {
            truncate_detail(&format!("awaiting approval: {command}"))
        };
        commands.insert(thread_id, PendingCommand { at, detail });
    }
    Ok(commands)
}

fn command_status(payload: &Value) -> Option<&str> {
    payload.get("status").and_then(Value::as_str).or_else(|| {
        payload
            .get("command")
            .and_then(|command| command.get("status"))
            .and_then(Value::as_str)
    })
}

fn command_status_is_terminal(status: &str) -> bool {
    let status: String = status
        .trim()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        status.as_str(),
        "completed"
            | "closed"
            | "resolved"
            | "cancelled"
            | "canceled"
            | "rejected"
            | "declined"
            | "failed"
            | "error"
            | "approved"
            | "success"
            | "done"
            | "timeout"
            | "timedout"
            | "killed"
    )
}
