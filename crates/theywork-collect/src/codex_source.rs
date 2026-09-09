use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{params_from_iter, Connection, OpenFlags};
use serde_json::Value;
use theywork_core::{
    Activity, Agent, Beat, CollaborationEvent, CollaborationKind, CoverageLevel, Event, EventKind,
    Evidence, Millis, OfficeId, Outcome, Relationship, RelationshipKind, Source, SourceCoverage,
    SourceError, SourceId, ThreadIdentity, WaitReason, WorkerId, WorkerLifecycle, WorkerRole,
    BLOCKED_AFTER_MS,
};

use crate::util::{
    path_allowed, recency_cutoff, repository_root_or_recorded_project, short_id, truncate_detail,
    truncate_timeline_text, unified_diff_counts, NON_PROJECT_OFFICE,
};
use crate::DEFAULT_ACTIVE_WITHIN;
const ASSESSOR_TITLE_PREFIX: &str =
    "The following is the Codex agent history whose request action you are assessing";
// The world retains only a bounded timeline, so a newly discovered thread
// needs its recent tail rather than its entire historical transcript.
const INITIAL_ITEMS_PER_THREAD: i64 = 64;

/// Reads the current Codex roster and activity feed from its SQLite stores.
pub struct CodexSource {
    home: PathBuf,
    only_paths: Vec<PathBuf>,
    active_within: Duration,
    threads: BTreeMap<String, ThreadRecord>,
    item_watermarks: HashMap<String, i64>,
    last_item_watermark: i64,
    office_cache: HashMap<String, String>,
    items_initialized: bool,
    turn_states: HashMap<String, TurnState>,
    item_fingerprints: HashMap<String, BTreeMap<String, (Millis, String)>>,
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
            last_item_watermark: -1,
            items_initialized: false,
            turn_states: HashMap::new(),
            item_fingerprints: HashMap::new(),
            office_cache: HashMap::new(),
        }
    }

    pub fn new_with_paths(home: impl Into<PathBuf>, only_paths: Vec<PathBuf>) -> Self {
        Self::with_paths(home, only_paths)
    }

    pub(crate) fn home_exists(home: &Path) -> bool {
        home.is_dir()
            && ["state_5.sqlite", "thread_history_1.sqlite"]
                .iter()
                .all(|name| {
                    let path = Self::database_path(home, name);
                    let parent_safe = path.parent().is_none_or(|parent| {
                        fs::symlink_metadata(parent)
                            .map_or(true, |metadata| !metadata.file_type().is_symlink())
                    });
                    parent_safe
                        && fs::symlink_metadata(path).map_or(true, |metadata| {
                            !metadata.file_type().is_symlink() && metadata.is_file()
                        })
                })
    }

    /// Current stores live at the home root; older versions used sqlite/.
    /// Prefer the root when both remain after migration, resolving state and
    /// history independently to also support partially migrated layouts.
    pub(crate) fn database_path(home: &Path, name: &str) -> PathBuf {
        let root = home.join(name);
        if fs::symlink_metadata(&root).is_ok() {
            root
        } else {
            home.join("sqlite").join(name)
        }
    }

    pub(crate) fn inspect_home(
        home: &Path,
        active_within: Duration,
        now: Millis,
    ) -> crate::StoreReport {
        let mut report = crate::StoreReport::new(Agent::Codex, home.to_path_buf());
        report.home_found = home.is_dir();
        if !report.home_found {
            report.error = Some("home is not a directory".to_string());
            return report;
        }
        if !crate::path_allows_read(home) {
            report.error = Some(crate::unreadable_reason(
                home,
                "Codex home cannot be read",
                &std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            ));
            return report;
        }

        let sqlite = home.join("sqlite");
        let state_path = Self::database_path(home, "state_5.sqlite");
        let history_path = Self::database_path(home, "thread_history_1.sqlite");
        let state_metadata = match fs::symlink_metadata(&state_path) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                report.error = Some(crate::unreadable_reason(
                    &state_path,
                    "could not inspect Codex state database",
                    &error,
                ));
                return report;
            }
        };
        let history_metadata = match fs::symlink_metadata(&history_path) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                report.error = Some(crate::unreadable_reason(
                    &history_path,
                    "could not inspect Codex history database",
                    &error,
                ));
                return report;
            }
        };
        if state_metadata.is_none() && history_metadata.is_none() {
            report.readable = true;
            return report;
        }
        if state_metadata.is_none() || history_metadata.is_none() {
            let missing = if state_metadata.is_none() {
                "state_5.sqlite"
            } else {
                "thread_history_1.sqlite"
            };
            report.error = Some(format!(
                "incomplete Codex SQLite store: required {missing} is missing; {}",
                crate::metadata_access_details(&sqlite)
            ));
            return report;
        }
        if !state_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.is_file())
            || !history_metadata
                .as_ref()
                .is_some_and(|metadata| metadata.is_file())
        {
            report.error = Some(format!(
                "required Codex SQLite stores are not regular files; {}",
                crate::metadata_access_details(&sqlite)
            ));
            return report;
        }
        if !crate::path_allows_read(&state_path) {
            report.error = Some(crate::unreadable_reason(
                &state_path,
                "Codex state database cannot be read",
                &std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            ));
            return report;
        }
        if !crate::path_allows_read(&history_path) {
            report.error = Some(crate::unreadable_reason(
                &history_path,
                "Codex history database cannot be read",
                &std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            ));
            return report;
        }

        let state = match open_read_only(&state_path) {
            Ok(connection) => connection,
            Err(reason) => {
                report.error = Some(format!(
                    "state database could not be opened read-only: {reason}; {}",
                    crate::metadata_access_details(&state_path)
                ));
                return report;
            }
        };
        let mut office_cache = HashMap::new();
        let all_threads = match read_threads(&state, i64::MIN, &mut office_cache) {
            Ok(threads) => threads,
            Err(error) => {
                report.error = Some(crate::unreadable_reason(
                    &state_path,
                    "thread roster query failed",
                    &error,
                ));
                return report;
            }
        };
        let active_threads = match read_threads(
            &state,
            recency_cutoff(now, active_within),
            &mut office_cache,
        ) {
            Ok(threads) => threads,
            Err(error) => {
                report.error = Some(crate::unreadable_reason(
                    &state_path,
                    "active roster query failed",
                    &error,
                ));
                return report;
            }
        };
        let history = match open_read_only(&history_path) {
            Ok(connection) => connection,
            Err(reason) => {
                report.error = Some(format!(
                    "history database could not be opened read-only: {reason}; {}",
                    crate::metadata_access_details(&history_path)
                ));
                return report;
            }
        };
        if let Err(error) = read_turns(&history) {
            report.error = Some(crate::unreadable_reason(
                &history_path,
                "turn history query failed",
                &error,
            ));
            return report;
        }

        let projects: HashSet<String> = all_threads
            .iter()
            .filter(|thread| thread.office_path != NON_PROJECT_OFFICE)
            .map(|thread| thread.office_path.clone())
            .collect();
        report.readable = true;
        report.projects = projects.len();
        report.unresolved_paths = 0;
        report.threads = all_threads.len();
        report.active_threads = active_threads.len();
        report
    }

    fn state_path(&self) -> PathBuf {
        Self::database_path(&self.home, "state_5.sqlite")
    }

    fn history_path(&self) -> PathBuf {
        Self::database_path(&self.home, "thread_history_1.sqlite")
    }

    fn minimum_item_watermark(&self) -> i64 {
        if !self.items_initialized {
            return -1;
        }
        self.last_item_watermark
    }

    fn clear_runtime_state(&mut self) {
        self.threads.clear();
        self.item_watermarks.clear();
        self.last_item_watermark = -1;
        self.office_cache.clear();
        self.items_initialized = false;
        self.turn_states.clear();
        self.item_fingerprints.clear();
    }

    fn prune_runtime_state(&mut self) {
        let active_ids: HashSet<String> = self.threads.keys().cloned().collect();
        self.item_fingerprints
            .retain(|thread_id, _| active_ids.contains(thread_id));
        self.item_watermarks
            .retain(|thread_id, _| active_ids.contains(thread_id));
        self.turn_states
            .retain(|thread_id, _| active_ids.contains(thread_id));

        let active_offices: HashSet<String> = self
            .threads
            .values()
            .map(|thread| thread.office_path.clone())
            .collect();
        self.office_cache
            .retain(|_, office_path| active_offices.contains(office_path));
    }

    fn unavailable_poll(&mut self, reason: &str, now: Millis) -> Vec<Event> {
        let events = self
            .threads
            .values()
            .map(|thread| {
                thread.event(
                    now,
                    EventKind::Coverage(SourceCoverage {
                        available: false,
                        incomplete: true,
                        relationships: CoverageLevel::Unavailable,
                        messages: CoverageLevel::Unavailable,
                        lifecycle: CoverageLevel::Unavailable,
                        observed_at: now,
                        detail: "Local store unavailable; last observations retained.".into(),
                        ..SourceCoverage::default()
                    }),
                )
            })
            .collect();
        self.clear_runtime_state();
        debug_blocked_unavailable(reason);
        events
    }
}

impl Source for CodexSource {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn poll(&mut self, now: Millis) -> Result<Vec<Event>, SourceError> {
        let cutoff_ms = recency_cutoff(now, self.active_within);
        let state = match open_read_only(&self.state_path()) {
            Ok(connection) => connection,
            Err(reason) => return Ok(self.unavailable_poll(&reason, now)),
        };
        let Ok(roster) = read_threads(&state, cutoff_ms, &mut self.office_cache) else {
            return Ok(self.unavailable_poll("thread roster query failed", now));
        };
        let roster_ids: Vec<String> = roster.iter().map(|thread| thread.id.clone()).collect();
        let edges_supported = table_has_column(&state, "thread_spawn_edges", "parent_thread_id")
            .unwrap_or(false)
            && table_has_column(&state, "thread_spawn_edges", "child_thread_id").unwrap_or(false);
        let edges = read_spawn_edges(&state, &roster_ids).unwrap_or_default();
        let source = crate::util::source_id(&self.home);
        let endpoint_ids = edges
            .iter()
            .flat_map(|edge| [edge.parent_thread_id.clone(), edge.child_thread_id.clone()])
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .take(4096)
            .collect::<Vec<_>>();
        let mut related = read_related_metadata(&state, &endpoint_ids, &mut self.office_cache)
            .unwrap_or_default();
        for thread in related.values_mut() {
            thread.source_id = source.clone();
        }
        let excluded_ids: HashSet<_> = roster
            .iter()
            .chain(related.values())
            .filter(|thread| {
                matches!(
                    thread.classification.kind,
                    ThreadKind::ApprovalAssessor | ThreadKind::InternalReview
                )
            })
            .map(|thread| ThreadIdentity::new(Agent::Codex, source.clone(), &thread.id).worker_id())
            .collect();
        let history = match open_read_only(&self.history_path()) {
            Ok(connection) => connection,
            Err(reason) => return Ok(self.unavailable_poll(&reason, now)),
        };
        let turns = match read_turns(&history) {
            Ok(turns) => turns,
            Err(_) => {
                return Ok(self.unavailable_poll("turn history query failed", now));
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
        for mut thread in roster {
            thread.source_id = source.clone();
            match thread.classification.kind {
                ThreadKind::ApprovalAssessor => {
                    record_exclusion(&mut exclusions, &thread);
                    if path_allowed(&thread.raw_office_path, &self.only_paths)
                        || path_allowed(&thread.office_path, &self.only_paths)
                    {
                        assessors.push(thread);
                    }
                }
                ThreadKind::Developer | ThreadKind::Subagent => {
                    if path_allowed(&thread.raw_office_path, &self.only_paths)
                        || path_allowed(&thread.office_path, &self.only_paths)
                    {
                        current.insert(thread.id.clone(), thread);
                    }
                }
                ThreadKind::InternalReview => {
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
        for mut parent in
            read_threads_by_ids(&state, &parent_ids, &mut self.office_cache).unwrap_or_default()
        {
            parent.source_id = source.clone();
            match parent.classification.kind {
                ThreadKind::Developer | ThreadKind::Subagent => {
                    if path_allowed(&parent.raw_office_path, &self.only_paths) {
                        current.insert(parent.id.clone(), parent);
                    }
                }
                ThreadKind::ApprovalAssessor | ThreadKind::InternalReview => {
                    record_exclusion(&mut exclusions, &parent)
                }
            }
        }
        debug_exclusion_summary(&exclusions);
        disambiguate_thread_names(&mut current);
        let previous = std::mem::replace(&mut self.threads, current);
        self.prune_runtime_state();
        // The global cursor keeps steady-state polls incremental. Threads with
        // no observed item still get a targeted backfill because their first
        // item can be older than that cursor.
        let minimum_watermark = self.minimum_item_watermark();

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
                    EventKind::Identity {
                        identity: thread.identity(),
                        role: thread.role(),
                    },
                ));
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

        for thread in self.threads.values() {
            events.push(thread.event(now, EventKind::Coverage(SourceCoverage {
                available: true, incomplete: true,
                relationships: if edges_supported { CoverageLevel::Supported } else { CoverageLevel::Partial },
                messages: CoverageLevel::Partial, lifecycle: CoverageLevel::Supported,
                observed_at: now, detail: "Local history tail; only typed collaboration and explicit final messages are known.".into(),
                ..SourceCoverage::default()
            })));
            if let Some(parent) = &thread.delegated_from {
                events.push(thread.event(
                    thread.updated_at_ms,
                    EventKind::Relationship(Relationship {
                        parent:
                            ThreadIdentity::new(Agent::Codex, source.clone(), parent).worker_id(),
                        child: thread.identity().worker_id(),
                        kind: RelationshipKind::Delegation,
                        evidence: Evidence::NativeMetadata,
                        at: thread.updated_at_ms,
                        correlation_id: None,
                    }),
                ));
            }
            if let Some(parent) = &thread.forked_from {
                events.push(thread.event(
                    thread.updated_at_ms,
                    EventKind::Relationship(Relationship {
                        parent:
                            ThreadIdentity::new(Agent::Codex, source.clone(), parent).worker_id(),
                        child: thread.identity().worker_id(),
                        kind: RelationshipKind::Fork,
                        at: thread.updated_at_ms,
                        evidence: Evidence::NativeMetadata,
                        correlation_id: None,
                    }),
                ));
            }
        }
        for edge in &edges {
            // Classify the child before exposing it: internal assessors are not workers.
            let Some(child) = self
                .threads
                .get(&edge.child_thread_id)
                .or_else(|| related.get(&edge.child_thread_id))
            else {
                continue;
            };
            if matches!(
                child.classification.kind,
                ThreadKind::ApprovalAssessor | ThreadKind::InternalReview
            ) {
                continue;
            }
            let parent = ThreadIdentity::new(Agent::Codex, source.clone(), &edge.parent_thread_id)
                .worker_id();
            events.push(child.event(
                child.updated_at_ms,
                EventKind::Relationship(Relationship {
                    parent,
                    child: child.identity().worker_id(),
                    kind: RelationshipKind::Delegation,
                    evidence: Evidence::NativeMetadata,
                    at: child.updated_at_ms,
                    correlation_id: None,
                }),
            ));
        }
        let mut watermark_updates: HashMap<String, i64> = HashMap::new();
        let mut last_item_watermark = self.last_item_watermark;
        let read_result = read_items(
            &history,
            minimum_watermark,
            &self.threads.keys().cloned().collect::<Vec<_>>(),
            |item| {
                last_item_watermark = last_item_watermark.max(item.created_at_ms);
                let Some(thread) = self.threads.get(&item.thread_id) else {
                    return;
                };
                let fingerprint_key = item.item_id.clone().unwrap_or_else(|| {
                    format!(
                        "{}:{}:{}",
                        item.created_at_ms, item.item_type, item.item_json
                    )
                });
                let fingerprints = self
                    .item_fingerprints
                    .entry(item.thread_id.clone())
                    .or_default();
                if fingerprints
                    .get(&fingerprint_key)
                    .is_some_and(|(_, payload)| payload == &item.item_json)
                {
                    return;
                }
                fingerprints.insert(
                    fingerprint_key,
                    (item.created_at_ms, item.item_json.clone()),
                );
                while fingerprints.len() > INITIAL_ITEMS_PER_THREAD as usize * 2 {
                    if let Some(key) = fingerprints
                        .iter()
                        .min_by_key(|(_, (at, _))| *at)
                        .map(|(key, _)| key.clone())
                    {
                        fingerprints.remove(&key);
                    }
                }
                watermark_updates
                    .entry(item.thread_id.clone())
                    .and_modify(|watermark| *watermark = (*watermark).max(item.created_at_ms))
                    .or_insert(item.created_at_ms);

                let payload = serde_json::from_str::<Value>(&item.item_json).unwrap_or(Value::Null);
                events.extend(collaboration_items(thread, &item, &payload));
                if let Some(kind) = item_kind(&item.item_type, &payload, item.created_at_ms) {
                    events.push(thread.event(item.created_at_ms, kind));
                }
                if item.item_type == "userMessage" {
                    events.push(thread.event(
                        item.created_at_ms,
                        message_beat(&payload, item.created_at_ms),
                    ));
                }
            },
        );
        if read_result.is_err() {
            return Ok(self.unavailable_poll("activity item query failed", now));
        }
        self.last_item_watermark = last_item_watermark.max(DEFAULT_ITEM_WATERMARK);
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
                if let Some(lifecycle) = lifecycle_from_status(&turn.status) {
                    events.push(thread.event(at, EventKind::Lifecycle(lifecycle)));
                }
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
                    truncate_timeline_text(&format!("cwd/time fallback: {detail}"))
                }
            };
            debug_waiting(&signal, &detail);
            let reason = if matches!(signal.strategy, WaitingStrategy::SpawnEdge) {
                WaitReason::AutomaticReview
            } else {
                WaitReason::Unknown
            };
            events.push(thread.event(at, EventKind::Wait(Some(reason))));
            // The assessor is an automated process. A quiet turn still surfaces
            // via WorkerStatus timing, but does not become a human approval.
            let _ = detail;
        }
        self.prune_runtime_state();
        events.retain(|event| match &event.kind {
            EventKind::Relationship(edge) => {
                !excluded_ids.contains(&edge.parent) && !excluded_ids.contains(&edge.child)
            }
            EventKind::Collaboration(collab) => {
                !excluded_ids.contains(&collab.actor)
                    && collab
                        .recipient
                        .as_ref()
                        .is_none_or(|id| !excluded_ids.contains(id))
            }
            _ => true,
        });
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
    source_id: SourceId,
    forked_from: Option<String>,
    delegated_from: Option<String>,
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
    fn identity(&self) -> ThreadIdentity {
        ThreadIdentity::new(Agent::Codex, self.source_id.clone(), &self.id)
    }
    fn role(&self) -> WorkerRole {
        if self.classification.kind == ThreadKind::Subagent {
            WorkerRole::Subagent
        } else {
            WorkerRole::Main
        }
    }
    fn event(&self, at: Millis, kind: EventKind) -> Event {
        Event {
            at,
            office: OfficeId(self.office_path.clone()),
            office_path: self.office_path.clone(),
            worker: self.identity().worker_id(),
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
    item_id: Option<String>,
    turn_id: Option<String>,
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

fn open_read_only(path: &Path) -> Result<Connection, String> {
    require_regular_non_symlink(path, "database")?;
    if let Some(parent) = path.parent() {
        let metadata = fs::symlink_metadata(parent)
            .map_err(|error| format!("could not inspect SQLite directory: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(
                "SQLite directory is not a real directory inside the configured store".into(),
            );
        }
    }

    let wal = sidecar_path(path, "-wal");
    let shm = sidecar_path(path, "-shm");
    let wal_exists = require_optional_regular_non_symlink(&wal, "WAL")?;
    let shm_exists = require_optional_regular_non_symlink(&shm, "shared-memory")?;
    let wal_mode = database_uses_wal(path)?;
    if (wal_mode || wal_exists) && (!wal_exists || !shm_exists) {
        return Err(
            "WAL database lacks complete WAL/shared-memory sidecars; refusing to create files in the agent store"
                .into(),
        );
    }

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW;
    let connection = Connection::open_with_flags(path, flags)
        .map_err(|error| format!("SQLite open failed: {error}"))?;
    connection
        .busy_timeout(Duration::ZERO)
        .map_err(|error| format!("could not set SQLite busy timeout: {error}"))?;
    Ok(connection)
}

fn database_uses_wal(path: &Path) -> Result<bool, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("could not read SQLite database header: {error}"))?;
    let mut header = [0_u8; 20];
    match file.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
        Err(error) => return Err(format!("could not read SQLite database header: {error}")),
    }
    Ok(&header[..16] == b"SQLite format 3\0" && (header[18] == 2 || header[19] == 2))
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn require_regular_non_symlink(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect SQLite {label}: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("SQLite {label} is not a regular non-symlink file"));
    }
    Ok(())
}

fn require_optional_regular_non_symlink(path: &Path, label: &str) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => Ok(true),
        Ok(_) => Err(format!(
            "SQLite {label} sidecar is not a regular non-symlink file"
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("could not inspect SQLite {label} sidecar: {error}")),
    }
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

fn read_items<F>(
    connection: &Connection,
    minimum_watermark: i64,
    thread_ids: &[String],
    mut visit: F,
) -> rusqlite::Result<()>
where
    F: FnMut(ItemRecord),
{
    let item_id = if table_has_column(connection, "thread_items", "item_id")? {
        "item_id"
    } else {
        "NULL"
    };
    let turn_id = if table_has_column(connection, "thread_items", "turn_id")? {
        "turn_id"
    } else {
        "NULL"
    };
    let columns = format!("thread_id, created_at_ms, item_type, item_json, {item_id}, {turn_id}");
    let decode = |row: &rusqlite::Row<'_>| -> rusqlite::Result<ItemRecord> {
        Ok(ItemRecord {
            thread_id: row.get(0)?,
            created_at_ms: row.get(1)?,
            item_type: row.get(2)?,
            item_json: row.get(3)?,
            item_id: row.get(4)?,
            turn_id: row.get(5)?,
        })
    };
    let mut items = Vec::new();
    // Revisit the bounded tail: native rows may be completed in-place, and two
    // messages can share a millisecond. Content fingerprints suppress repeats.
    let mut recent = connection.prepare(&format!("SELECT {columns} FROM thread_items WHERE thread_id = ?1 ORDER BY created_at_ms DESC LIMIT ?2"))?;
    for id in thread_ids {
        let mut rows = recent.query(rusqlite::params![id, INITIAL_ITEMS_PER_THREAD])?;
        while let Some(row) = rows.next()? {
            items.push(decode(row)?);
        }
    }
    if minimum_watermark >= 0 {
        let ids: HashSet<_> = thread_ids.iter().collect();
        let mut incremental = connection.prepare(&format!("SELECT {columns} FROM thread_items WHERE created_at_ms >= ?1 ORDER BY created_at_ms ASC"))?;
        let mut rows = incremental.query([minimum_watermark])?;
        while let Some(row) = rows.next()? {
            let item = decode(row)?;
            if ids.contains(&item.thread_id) {
                items.push(item);
            }
        }
    }
    items.sort_by(|a, b| (a.created_at_ms, &a.item_id).cmp(&(b.created_at_ms, &b.item_id)));
    for item in items {
        visit(item);
    }
    Ok(())
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

fn item_kind(item_type: &str, payload: &Value, at: Millis) -> Option<EventKind> {
    match item_type {
        "commandExecution" => {
            let activity = Activity::Typing {
                detail: json_detail(payload, &["command", "cmd"]),
            };
            if command_status(payload).is_some_and(command_status_is_terminal) {
                return Some(match exit_code(payload) {
                    Some(exit_code) => EventKind::Did(Beat {
                        at,
                        activity,
                        outcome: Some(Outcome::Exited(exit_code)),
                    }),
                    None => EventKind::Acted(activity),
                });
            }
            Some(EventKind::Acted(activity))
        }
        "reasoning" => Some(EventKind::Acted(Activity::Thinking)),
        "agentMessage" => Some(EventKind::Did(Beat {
            at,
            activity: Activity::Talking {
                detail: timeline_detail(payload, &["message", "text", "content"]),
            },
            outcome: None,
        })),
        "mcpToolCall" => Some(EventKind::Acted(Activity::Searching {
            detail: json_detail(payload, &["name", "tool", "tool_name", "query", "url"]),
        })),
        "webSearch" => Some(EventKind::Acted(Activity::Searching {
            detail: json_detail(payload, &["query", "url"]),
        })),
        "fileChange" => Some(EventKind::Did(Beat {
            at,
            activity: Activity::Editing {
                detail: json_detail(payload, &["path", "file_path", "filename", "changes"]),
            },
            outcome: file_change_counts(payload)
                .map(|(added, removed)| Outcome::Changed { added, removed }),
        })),
        "userMessage" => Some(EventKind::Turn { in_flight: true }),
        "contextCompaction" => None,
        _ => None,
    }
}

fn message_beat(payload: &Value, at: Millis) -> EventKind {
    EventKind::Did(Beat {
        at,
        activity: Activity::Talking {
            detail: timeline_detail(payload, &["message", "text", "content"]),
        },
        outcome: None,
    })
}

fn lifecycle_from_status(status: &str) -> Option<WorkerLifecycle> {
    match normalize_marker(status).as_str() {
        "inprogress" | "running" | "pendinginit" => Some(WorkerLifecycle::Active),
        "completed" | "done" => Some(WorkerLifecycle::Completed),
        "failed" | "errored" | "error" => Some(WorkerLifecycle::Failed),
        "interrupted" | "cancelled" | "canceled" | "shutdown" => Some(WorkerLifecycle::Cancelled),
        _ => None,
    }
}

fn collaboration_items(thread: &ThreadRecord, item: &ItemRecord, payload: &Value) -> Vec<Event> {
    let at = item.created_at_ms;
    let native_id = item
        .item_id
        .as_deref()
        .or_else(|| payload.get("id").and_then(Value::as_str));
    // Missing IDs are not a reason to invent a durable delivery identity.
    let Some(native_id) = native_id else {
        return Vec::new();
    };
    let actor = thread.identity().worker_id();
    let make = |kind, recipient: Option<WorkerId>, text: Option<String>, suffix: &str| {
        thread.event(
            at,
            EventKind::Collaboration(CollaborationEvent {
                id: format!(
                    "{}:{native_id}:{suffix}",
                    item.turn_id.as_deref().unwrap_or("")
                ),
                at,
                actor: actor.clone(),
                recipient,
                kind,
                text,
                correlation_id: Some(native_id.to_string()),
                native_turn_id: item.turn_id.clone(),
                native_item_id: Some(native_id.to_string()),
                evidence: Evidence::NativeEvent,
            }),
        )
    };
    let mut events = Vec::new();
    if item.item_type == "agentMessage" {
        if payload.get("phase").and_then(Value::as_str) == Some("final_answer") {
            let text = timeline_detail(payload, &["text", "message", "content"]);
            if !text.is_empty() {
                events.push(make(CollaborationKind::Result, None, Some(text), "final"));
            }
        }
        return events;
    }
    if matches!(
        item.item_type.as_str(),
        "requestUserInput" | "toolRequestUserInput" | "requestApproval"
    ) {
        let reason = if item.item_type == "requestApproval" {
            WaitReason::HumanApproval
        } else {
            WaitReason::HumanInput
        };
        let text = timeline_detail(payload, &["question", "questions", "reason", "message"]);
        events.push(make(
            CollaborationKind::HumanRequest,
            None,
            Some(text.clone()),
            "request",
        ));
        events.push(thread.event(at, EventKind::Wait(Some(reason))));
        events.push(thread.event(at, EventKind::Acted(Activity::Waiting { detail: text })));
        return events;
    }
    if !matches!(
        item.item_type.as_str(),
        "collabToolCall" | "collabAgentToolCall"
    ) {
        return events;
    }
    // A record attributed to a different actor must not impersonate this worker.
    if payload
        .get("senderThreadId")
        .and_then(Value::as_str)
        .is_some_and(|sender| sender != thread.id)
    {
        return events;
    }
    let tool = payload
        .get("tool")
        .and_then(Value::as_str)
        .map(normalize_marker)
        .unwrap_or_default();
    let kind = match tool.as_str() {
        "spawnagent" | "spawn" => CollaborationKind::Delegated,
        "sendmessage" | "sendinput" => CollaborationKind::Message,
        "wait" | "waitagent" => CollaborationKind::Waiting,
        "closeagent" | "close" => CollaborationKind::Cancelled,
        "resumeagent" | "resume" => CollaborationKind::Message,
        _ => return events,
    };
    let mut receivers: Vec<&str> = ["newThreadId", "receiverThreadId"]
        .into_iter()
        .filter_map(|key| payload.get(key).and_then(Value::as_str))
        .collect();
    if let Some(ids) = payload.get("receiverThreadIds").and_then(Value::as_array) {
        receivers.extend(ids.iter().filter_map(Value::as_str));
    }
    receivers.sort_unstable();
    receivers.dedup();
    let text = payload
        .get("prompt")
        .and_then(Value::as_str)
        .map(truncate_timeline_text);
    if receivers.is_empty() {
        events.push(make(kind, None, text.clone(), "collaboration"));
    }
    for receiver in receivers {
        let recipient =
            ThreadIdentity::new(Agent::Codex, thread.source_id.clone(), receiver).worker_id();
        if recipient == actor {
            continue;
        }
        events.push(make(kind, Some(recipient.clone()), text.clone(), receiver));
        if kind == CollaborationKind::Delegated {
            events.push(thread.event(
                at,
                EventKind::Relationship(Relationship {
                    parent: actor.clone(),
                    child: recipient.clone(),
                    kind: RelationshipKind::Delegation,
                    evidence: Evidence::NativeEvent,
                    at,
                    correlation_id: Some(native_id.to_string()),
                }),
            ));
        }
        if let Some(status) = payload
            .get("agentStatus")
            .and_then(Value::as_str)
            .and_then(lifecycle_from_status)
        {
            let final_kind = match status {
                WorkerLifecycle::Completed => Some(CollaborationKind::Completed),
                WorkerLifecycle::Failed => Some(CollaborationKind::Failed),
                WorkerLifecycle::Cancelled => Some(CollaborationKind::Cancelled),
                _ => None,
            };
            if let Some(final_kind) = final_kind {
                events.push(make(
                    final_kind,
                    Some(recipient),
                    None,
                    &format!("{receiver}:status"),
                ));
            }
        }
    }
    if kind == CollaborationKind::Waiting {
        let pending = payload
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| {
                matches!(normalize_marker(status).as_str(), "inprogress" | "running")
            });
        events.push(thread.event(at, EventKind::Wait(pending.then_some(WaitReason::Child))));
    }
    events
}

fn timeline_detail(payload: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(value_detail))
        .map(|detail| truncate_timeline_text(&detail))
        .unwrap_or_default()
}

fn exit_code(payload: &Value) -> Option<i32> {
    find_numeric_field(
        payload,
        &["exitCode", "exit_code", "returnCode", "return_code"],
    )
}

fn find_numeric_field(value: &Value, keys: &[&str]) -> Option<i32> {
    match value {
        Value::Object(object) => {
            for key in keys {
                if let Some(number) = object.get(*key).and_then(integer_value) {
                    return Some(number);
                }
            }
            object
                .values()
                .find_map(|value| find_numeric_field(value, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|value| find_numeric_field(value, keys)),
        _ => None,
    }
}

fn integer_value(value: &Value) -> Option<i32> {
    value
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
        .or_else(|| value.as_u64().and_then(|value| i32::try_from(value).ok()))
        .or_else(|| value.as_str()?.trim().parse::<i32>().ok())
}

fn explicit_change_counts(value: &Value) -> Option<(u32, u32)> {
    let object = value.as_object()?;
    let added = [
        "added",
        "addedLines",
        "added_lines",
        "linesAdded",
        "lines_added",
        "additions",
        "insertions",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(u32_value));
    let removed = [
        "removed",
        "removedLines",
        "removed_lines",
        "linesRemoved",
        "lines_removed",
        "deletions",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(u32_value));
    added.zip(removed)
}
fn file_change_counts(payload: &Value) -> Option<(u32, u32)> {
    if let Some(counts) = explicit_change_counts(payload) {
        return Some(counts);
    }
    if let Some(changes) = payload.get("changes").and_then(Value::as_array) {
        if changes.is_empty() {
            return None;
        }
        let mut total = (0_u32, 0_u32);
        for change in changes {
            let counts = explicit_change_counts(change).or_else(|| {
                change
                    .get("diff")
                    .and_then(Value::as_str)
                    .and_then(unified_diff_counts)
            })?;
            total.0 = total.0.saturating_add(counts.0);
            total.1 = total.1.saturating_add(counts.1);
        }
        return Some(total);
    }
    payload
        .get("diff")
        .and_then(Value::as_str)
        .and_then(unified_diff_counts)
}

fn u32_value(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .or_else(|| value.as_i64().and_then(|value| u32::try_from(value).ok()))
        .or_else(|| value.as_str()?.trim().parse::<u32>().ok())
}
fn turn_in_flight(status: &str) -> Option<bool> {
    match status {
        "inProgress" => Some(true),
        "completed" | "failed" | "interrupted" | "cancelled" | "canceled" => Some(false),
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
    fork_expression: &'static str,
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
    let fork_expression = if table_has_column(connection, "threads", "forked_from_id")? {
        "forked_from_id"
    } else if table_has_column(connection, "threads", "forked_from")? {
        "forked_from"
    } else {
        "NULL"
    };
    Ok(ThreadColumns {
        fork_expression,
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
        "SELECT id, cwd, {name_expression}, {nickname_expression}, {title_expression}, {source_expression}, {thread_source_expression},          tokens_used, git_branch, {updated_at_expression} AS observed_at_ms, {fork_expression} FROM threads          WHERE {archived_expression} AND {filter}",
        fork_expression = columns.fork_expression,
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
    let office_path = repository_root_or_recorded_project(&raw_office_path, office_cache);
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
        .filter(|branch| !branch.trim().is_empty())
        .map(|branch| truncate_detail(&branch));
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
        source_id: SourceId(String::new()),
        forked_from: row
            .get::<_, Option<String>>(10)?
            .filter(|id| !id.is_empty()),
        delegated_from: source
            .as_deref()
            .and_then(|source| serde_json::from_str::<Value>(source).ok())
            .and_then(|source| {
                source
                    .pointer("/subagent/thread_spawn/parent_thread_id")
                    .or_else(|| source.pointer("/subagent/threadSpawn/parentThreadId"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            }),
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

    if source.is_some_and(|source| {
        serde_json::from_str::<Value>(source)
            .ok()
            .is_some_and(|value| {
                value
                    .pointer("/subagent/other")
                    .and_then(Value::as_str)
                    .is_some_and(|marker| {
                        let marker = normalize_marker(marker);
                        marker == "guardian" || is_internal_review_marker(&marker)
                    })
            })
    }) {
        return ThreadClassification {
            kind: ThreadKind::InternalReview,
            reason: "source.subagent.other internal review",
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

fn read_related_metadata(
    connection: &Connection,
    ids: &[String],
    cache: &mut HashMap<String, String>,
) -> rusqlite::Result<HashMap<String, ThreadRecord>> {
    let mut columns = thread_columns(connection)?;
    columns.archived_expression = "1=1";
    let query = thread_query(&columns, "id = ?1");
    let mut statement = connection.prepare(&query)?;
    let mut records = HashMap::new();
    for id in ids {
        let mut rows = statement.query([id])?;
        while let Some(row) = rows.next()? {
            if let Some(record) = decode_thread_row(row, cache)? {
                records.insert(record.id.clone(), record);
            }
        }
    }
    Ok(records)
}

#[derive(Clone)]
struct SpawnEdge {
    parent_thread_id: String,
    child_thread_id: String,
    status: String,
}

fn read_spawn_edges(connection: &Connection, ids: &[String]) -> rusqlite::Result<Vec<SpawnEdge>> {
    if ids.is_empty()
        || !table_has_column(connection, "thread_spawn_edges", "parent_thread_id")?
        || !table_has_column(connection, "thread_spawn_edges", "child_thread_id")?
    {
        return Ok(Vec::new());
    }
    let status = if table_has_column(connection, "thread_spawn_edges", "status")? {
        "COALESCE(status, 'unknown')"
    } else {
        "'unknown'"
    };
    let mut edges = BTreeMap::new();
    // Keep below SQLite's variable limit and deduplicate edges shared by batches.
    for batch in ids.chunks(200) {
        let placeholders = vec!["?"; batch.len()].join(",");
        let query = format!("SELECT parent_thread_id, child_thread_id, {status} FROM thread_spawn_edges WHERE parent_thread_id IN ({placeholders}) OR child_thread_id IN ({placeholders})");
        let mut statement = connection.prepare(&query)?;
        let mut rows = statement.query(params_from_iter(batch.iter().chain(batch.iter())))?;
        while let Some(row) = rows.next()? {
            let edge = SpawnEdge {
                parent_thread_id: row.get(0)?,
                child_thread_id: row.get(1)?,
                status: row.get(2)?,
            };
            edges.insert(
                (edge.parent_thread_id.clone(), edge.child_thread_id.clone()),
                edge,
            );
        }
    }
    Ok(edges.into_values().collect())
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

// Real blocked-state recipe: launch Codex with approval policy
// --ask-for-approval on-request in a disposable checkout, ask it to run
// rm -rf target (or another command that requires approval), and leave the
// approval unanswered for more than BLOCKED_AFTER_MS (180 seconds). Poll once
// more; the developer should emit Waiting with the pending command detail.
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
        let command = timeline_detail(&payload, &["command", "cmd", "description", "prompt"]);
        let detail = if command.is_empty() {
            "approval required".to_string()
        } else {
            truncate_timeline_text(&format!("awaiting approval: {command}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_requests_preserve_command_suffixes_and_mark_the_resource_limit() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE thread_items (thread_id TEXT, created_at_ms INTEGER, item_type TEXT, item_json TEXT);").unwrap();
        let command = format!(
            "deploy --project {} --require-backup verified --migration customer-account-id-index",
            "production-".repeat(12)
        );
        for (id, text) in [
            ("complete", command.clone()),
            ("bounded", "界".repeat(3_000)),
        ] {
            connection
                .execute(
                    "INSERT INTO thread_items VALUES (?1,1,'commandExecution',?2)",
                    rusqlite::params![
                        id,
                        serde_json::json!({"command":text,"status":"inProgress"}).to_string()
                    ],
                )
                .unwrap();
        }
        let requests =
            read_pending_commands(&connection, &["complete".into(), "bounded".into()]).unwrap();
        assert_eq!(
            requests["complete"].detail,
            format!("awaiting approval: {command}")
        );
        assert!(requests["bounded"].detail.ends_with('…'));
        assert_eq!(
            requests["bounded"].detail.chars().count(),
            crate::util::TIMELINE_TEXT_LIMIT
        );
    }

    fn thread(id: &str, office_path: &str) -> ThreadRecord {
        ThreadRecord {
            source_id: SourceId("fixture".into()),
            forked_from: None,
            delegated_from: None,
            id: id.to_string(),
            raw_office_path: office_path.to_string(),
            office_path: office_path.to_string(),
            name: id.to_string(),
            tokens_used: 1,
            git_branch: None,
            updated_at_ms: 1,
            classification: ThreadClassification {
                kind: ThreadKind::Developer,
                reason: "developer",
            },
            name_is_fallback: false,
        }
    }

    #[test]
    fn ended_threads_leave_every_codex_runtime_map() {
        let mut source = CodexSource::new("/tmp/does-not-exist");
        source
            .threads
            .insert("active".to_string(), thread("active", "/repo"));
        source
            .threads
            .insert("ended".to_string(), thread("ended", "/gone"));
        source.item_watermarks.insert("active".to_string(), 10);
        source.item_watermarks.insert("ended".to_string(), 20);
        source.turn_states.insert(
            "active".to_string(),
            TurnState {
                turn_id: "turn-active".to_string(),
                in_flight: true,
                error_detail: None,
            },
        );
        source.turn_states.insert(
            "ended".to_string(),
            TurnState {
                turn_id: "turn-ended".to_string(),
                in_flight: false,
                error_detail: None,
            },
        );
        source
            .office_cache
            .insert("/repo/src".to_string(), "/repo".to_string());
        source
            .office_cache
            .insert("/gone/src".to_string(), "/gone".to_string());

        source.threads.remove("ended");
        source.prune_runtime_state();

        assert_eq!(source.threads.len(), 1);
        assert_eq!(source.item_watermarks.len(), 1);
        assert_eq!(source.turn_states.len(), 1);
        assert_eq!(source.office_cache.len(), 1);
        assert!(source.item_watermarks.contains_key("active"));
        assert!(source.turn_states.contains_key("active"));
        assert!(source.office_cache.contains_key("/repo/src"));
        assert!(!source.item_watermarks.contains_key("ended"));
        assert!(!source.turn_states.contains_key("ended"));
        assert!(!source.office_cache.contains_key("/gone/src"));

        source.clear_runtime_state();
        assert!(source.threads.is_empty());
        assert!(source.item_watermarks.is_empty());
        assert!(source.turn_states.is_empty());
        assert!(source.office_cache.is_empty());
        assert!(!source.items_initialized);
    }
}
