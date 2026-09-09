use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use serde_json::Value;
use theywork_core::{
    Activity, Agent, Beat, CollaborationEvent, CollaborationKind, CoverageLevel, Event, EventKind,
    Evidence, Millis, OfficeId, Outcome, Relationship, RelationshipKind, Source, SourceCoverage,
    SourceError, SourceId, ThreadIdentity, WaitReason, WorkerId, WorkerLifecycle, WorkerRole,
};

use crate::util::{
    normalize_office_path, path_allowed, recency_cutoff, repository_root,
    repository_root_with_project_hint, short_id, text_line_count, timestamp_value, truncate_detail,
    truncate_timeline_text, unified_diff_counts,
};
use crate::DEFAULT_ACTIVE_WITHIN;

const CHUNK_SIZE: usize = 64 * 1024;
const MAX_LINE_BYTES: usize = 1024 * 1024;

/// Tails Claude Code's JSONL transcripts without ever opening them for write.
pub struct ClaudeSource {
    home: PathBuf,
    only_paths: Vec<PathBuf>,
    active_within: Duration,
    files: BTreeMap<PathBuf, FileCursor>,
    office_cache: HashMap<String, String>,
    worker_names: HashMap<(String, String), NameAssignment>,
    identities: HashMap<WorkerId, ThreadIdentity>,
    delegations: HashMap<(WorkerId, String), WorkerId>,
    memberships: BTreeMap<WorkerId, Relationship>,
    delegation_events: HashMap<(WorkerId, String), Event>,
}

impl ClaudeSource {
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
            files: BTreeMap::new(),
            office_cache: HashMap::new(),
            worker_names: HashMap::new(),
            identities: HashMap::new(),
            delegations: HashMap::new(),
            memberships: BTreeMap::new(),
            delegation_events: HashMap::new(),
        }
    }

    /// Alias useful to callers that want to make the path filtering explicit.
    pub fn new_with_paths(home: impl Into<PathBuf>, only_paths: Vec<PathBuf>) -> Self {
        Self::with_paths(home, only_paths)
    }

    pub(crate) fn home_exists(home: &Path) -> bool {
        home.is_dir()
    }

    pub(crate) fn inspect_home(
        home: &Path,
        active_within: Duration,
        now: Millis,
    ) -> crate::StoreReport {
        let mut report = crate::StoreReport::new(Agent::Claude, home.to_path_buf());
        report.home_found = home.is_dir();
        if !report.home_found {
            report.error = Some("home is not a directory".to_string());
            return report;
        }
        if !crate::path_allows_read(home) {
            report.error = Some(crate::unreadable_reason(
                home,
                "Claude home cannot be read",
                &io::Error::from(io::ErrorKind::PermissionDenied),
            ));
            return report;
        }

        let projects = home.join("projects");
        match fs::metadata(&projects) {
            Ok(metadata) if !metadata.is_dir() => {
                report.error = Some(format!(
                    "Claude projects path is not a directory; {}",
                    crate::metadata_access_details(&projects)
                ));
                return report;
            }
            Ok(_) if !crate::path_allows_read(&projects) => {
                report.error = Some(crate::unreadable_reason(
                    &projects,
                    "Claude projects cannot be read",
                    &io::Error::from(io::ErrorKind::PermissionDenied),
                ));
                return report;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                report.readable = true;
                return report;
            }
            Err(error) => {
                report.error = Some(crate::unreadable_reason(
                    &projects,
                    "could not inspect Claude projects",
                    &error,
                ));
                return report;
            }
            Ok(_) => {}
        }
        let discovered = match discover_files(&projects) {
            Ok(files) => files,
            Err(error) => {
                report.error = Some(crate::unreadable_reason(
                    &projects,
                    "could not scan Claude projects",
                    &error,
                ));
                return report;
            }
        };
        let cutoff = recency_cutoff(now, active_within);
        let mut project_keys = HashSet::new();
        for file in &discovered {
            if !file.project_key.is_empty() {
                project_keys.insert(file.project_key.clone());
            }
            if fs::metadata(&file.path)
                .ok()
                .and_then(|metadata| modified_millis(&metadata))
                .is_some_and(|modified_at| modified_at >= cutoff)
            {
                report.active_threads += 1;
            }
        }
        report.readable = true;
        report.projects = project_keys.len();
        report.threads = discovered.len();
        report
    }
    fn clear_runtime_state(&mut self) {
        self.files.clear();
        self.office_cache.clear();
        self.worker_names.clear();
        self.identities.clear();
        self.delegations.clear();
        self.memberships.clear();
        self.delegation_events.clear();
    }

    fn prune_runtime_state(&mut self) {
        let active_workers: Vec<(String, String)> = self
            .files
            .values()
            .filter_map(|cursor| {
                let office = cursor.metadata.office_path.as_ref()?;
                Some((office.clone(), cursor.metadata.identity().worker_id().0))
            })
            .collect();
        let active_offices: HashSet<String> = active_workers
            .iter()
            .map(|(office, _)| repository_root(office, &mut self.office_cache))
            .collect();
        let active_worker_keys: HashSet<(String, String)> = active_workers
            .into_iter()
            .map(|(office, worker)| (repository_root(&office, &mut self.office_cache), worker))
            .collect();

        self.office_cache
            .retain(|_, office| active_offices.contains(office));
        self.worker_names
            .retain(|key, _| active_worker_keys.contains(key));
    }

    fn discover(&mut self, now: Millis) -> bool {
        let cutoff_ms = recency_cutoff(now, self.active_within);
        let projects = self.home.join("projects");
        let discovered = match discover_files(&projects) {
            Ok(discovered) => discovered,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.clear_runtime_state();
                return false;
            }
            Err(_) => {
                self.clear_runtime_state();
                return false;
            }
        };

        let discovered_count = discovered.len();
        let discovered_subagents = discovered.iter().filter(|file| file.is_subagent).count();
        let mut paths = BTreeMap::new();
        for discovery in discovered {
            let Ok(metadata) = fs::metadata(&discovery.path) else {
                continue;
            };
            let Some(modified_at) = modified_millis(&metadata) else {
                continue;
            };
            if modified_at < cutoff_ms {
                continue;
            }
            paths.insert(discovery.path.clone(), discovery);
        }

        let active_subagents = paths.values().filter(|file| file.is_subagent).count();
        if std::env::var_os("THEYWORK_COLLECT_DEBUG").is_some() {
            eprintln!(
                "claude discovered transcripts={discovered_count} active={} subagent_transcripts={discovered_subagents} active_subagents={active_subagents}",
                paths.len()
            );
        }
        self.files.retain(|path, _| paths.contains_key(path));
        for (path, discovery) in paths {
            self.files
                .entry(path)
                .or_insert_with(|| FileCursor::new(discovery));
        }
        true
    }
}

impl Source for ClaudeSource {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn poll(&mut self, now: Millis) -> Result<Vec<Event>, SourceError> {
        let previous = self.roster_events(now);
        let discovered = self.discover(now);
        if !discovered {
            return Ok(previous
                .into_values()
                .map(|mut event| {
                    event.kind = EventKind::Coverage(claude_coverage(now, false));
                    event
                })
                .collect());
        }
        let source_id = crate::util::source_id(&self.home);
        let paths: Vec<PathBuf> = self.files.keys().cloned().collect();
        let mut events = Vec::new();
        let mut unreadable = HashSet::new();
        for path in paths {
            let Some(mut cursor) = self.files.remove(&path) else {
                continue;
            };
            cursor.metadata.source_id = source_id.clone();
            if !read_file(
                &path,
                &mut cursor,
                &self.only_paths,
                &mut self.office_cache,
                &mut events,
            ) {
                unreadable.insert(cursor.metadata.identity().worker_id());
            }
            self.files.insert(path, cursor);
        }
        let current = self.roster_events(now);
        for (id, mut event) in previous {
            if !current.contains_key(&id) {
                event.kind = EventKind::Left;
                events.push(event);
            }
        }
        for (id, mut event) in current {
            event.kind = EventKind::Coverage(claude_coverage(now, !unreadable.contains(&id)));
            events.push(event);
        }
        self.resolve_collaboration(&mut events);
        self.prune_runtime_state();
        events.sort_by_key(|event| event.at);
        disambiguate_names(&mut self.worker_names, &mut events);
        Ok(events)
    }
}

fn claude_coverage(now: Millis, available: bool) -> SourceCoverage {
    SourceCoverage { available, incomplete: true,
        relationships: CoverageLevel::Partial, messages: CoverageLevel::Partial,
        lifecycle: CoverageLevel::Partial, observed_at: now,
        detail: if available { "Local transcripts; session membership is known, immediate parents and final output require explicit metadata." }
            else { "Local transcript source unavailable; last observations are retained." }.into(),
        ..SourceCoverage::default()
    }
}

impl ClaudeSource {
    fn roster_events(&mut self, now: Millis) -> HashMap<WorkerId, Event> {
        let mut events = HashMap::new();
        for cursor in self.files.values() {
            let Some(raw) = cursor.metadata.office_path.as_deref() else {
                continue;
            };
            let office_path = repository_root_with_project_hint(
                raw,
                &mut self.office_cache,
                Some(&cursor.metadata.project_key),
            );
            if !path_allowed(raw, &self.only_paths) && !path_allowed(&office_path, &self.only_paths)
            {
                continue;
            }
            let worker = cursor.metadata.identity().worker_id();
            events.insert(
                worker.clone(),
                Event {
                    at: now,
                    office: OfficeId(office_path.clone()),
                    office_path,
                    worker,
                    agent: Agent::Claude,
                    kind: EventKind::Left,
                },
            );
        }
        events
    }

    fn resolve_collaboration(&mut self, events: &mut Vec<Event>) {
        for event in events.iter() {
            if let EventKind::Identity { identity, .. } = &event.kind {
                self.identities
                    .insert(event.worker.clone(), identity.clone());
            }
            if let EventKind::Relationship(relationship) = &event.kind {
                if relationship.kind == RelationshipKind::SessionMembership {
                    self.memberships
                        .insert(relationship.child.clone(), relationship.clone());
                }
            }
        }
        let root_for = |actor: &WorkerId| -> Option<WorkerId> {
            let identity = self.identities.get(actor)?;
            Some(if let Some(root) = &identity.session_id {
                ThreadIdentity::new(Agent::Claude, identity.source.clone(), root).worker_id()
            } else {
                actor.clone()
            })
        };
        for event in events.iter() {
            if let EventKind::Collaboration(collab) = &event.kind {
                if collab.kind == CollaborationKind::Delegated {
                    if let (Some(root), Some(correlation)) =
                        (root_for(&collab.actor), &collab.correlation_id)
                    {
                        self.delegations
                            .insert((root.clone(), correlation.clone()), collab.actor.clone());
                        self.delegation_events
                            .insert((root, correlation.clone()), event.clone());
                    }
                }
            }
        }
        // Join on the scoped native tool ID only. Filename membership alone
        // never establishes which nested subagent actually delegated the work.
        let mut relations = Vec::new();
        for membership in self.memberships.values() {
            let Some(correlation) = membership.correlation_id.as_ref() else {
                continue;
            };
            let Some(parent) = self
                .delegations
                .get(&(membership.parent.clone(), correlation.clone()))
            else {
                continue;
            };
            let Some(template) = events.iter().find(|event| event.worker == membership.child)
            else {
                continue;
            };
            let mut event = template.clone();
            event.kind = EventKind::Relationship(Relationship {
                parent: parent.clone(),
                kind: RelationshipKind::Delegation,
                evidence: Evidence::NativeEvent,
                ..membership.clone()
            });
            relations.push(event);
            let key = (membership.parent.clone(), correlation.clone());
            if let Some(event) = self.delegation_events.get(&key) {
                let mut updated = event.clone();
                if let EventKind::Collaboration(collab) = &mut updated.kind {
                    if self
                        .memberships
                        .values()
                        .filter(|r| {
                            r.parent == membership.parent
                                && r.correlation_id.as_ref() == Some(correlation)
                        })
                        .count()
                        == 1
                    {
                        collab.recipient = Some(membership.child.clone());
                        relations.push(updated);
                    }
                }
            }
        }
        for event in events.iter_mut() {
            if let EventKind::Collaboration(collab) = &mut event.kind {
                let Some(root) = root_for(&collab.actor) else {
                    continue;
                };
                let Some(correlation) = &collab.correlation_id else {
                    continue;
                };
                let children: Vec<_> = self
                    .memberships
                    .values()
                    .filter(|r| r.parent == root && r.correlation_id.as_ref() == Some(correlation))
                    .map(|r| r.child.clone())
                    .collect();
                if children.len() == 1
                    && matches!(
                        collab.kind,
                        CollaborationKind::Delegated | CollaborationKind::Result
                    )
                {
                    if collab.kind == CollaborationKind::Delegated {
                        collab.recipient = Some(children[0].clone());
                    } else if collab.id.ends_with(":tool-result") && collab.recipient.is_none() {
                        collab.recipient = Some(collab.actor.clone());
                        collab.actor = children[0].clone();
                    }
                }
            }
        }
        events.extend(relations);
        let live: HashSet<_> = self
            .files
            .values()
            .map(|cursor| cursor.metadata.identity().worker_id())
            .collect();
        self.identities.retain(|id, _| live.contains(id));
        self.memberships.retain(|id, _| live.contains(id));
        self.delegations.retain(|_, actor| live.contains(actor));
        self.delegation_events
            .retain(|_, event| live.contains(&event.worker));
        if self.delegations.len() > 4096 {
            self.delegations.clear();
            self.delegation_events.clear();
        }
    }
}

#[derive(Clone)]
struct Discovery {
    path: PathBuf,
    is_subagent: bool,
    session_id: String,
    project_key: String,
}

struct NameAssignment {
    base: String,
    assigned: String,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> Option<FileIdentity> {
    Some(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(not(unix))]
type FileIdentity = ();

#[cfg(not(unix))]
fn file_identity(_: &fs::Metadata) -> Option<FileIdentity> {
    None
}

struct FileCursor {
    offset: u64,
    pending: Vec<u8>,
    discarding_line: bool,
    last_timestamp: Option<Millis>,
    file_identity: Option<FileIdentity>,
    last_modified: Option<SystemTime>,
    checkpoint: Vec<u8>,
    pending_tools: HashMap<String, PendingTool>,
    tokens_used: u64,
    metadata: SessionMetadata,
}

impl FileCursor {
    fn new(discovery: Discovery) -> Self {
        Self {
            offset: 0,
            pending: Vec::new(),
            discarding_line: false,
            last_timestamp: None,
            file_identity: None,
            last_modified: None,
            checkpoint: Vec::new(),
            tokens_used: 0,
            pending_tools: HashMap::new(),
            metadata: SessionMetadata::new(discovery),
        }
    }

    fn reset(&mut self) {
        self.offset = 0;
        self.pending.clear();
        self.discarding_line = false;
        self.last_timestamp = None;
        self.file_identity = None;
        self.last_modified = None;
        self.checkpoint.clear();
        self.pending_tools.clear();
        self.tokens_used = 0;
        self.metadata.reset_identity();
    }
}

#[derive(Clone)]
struct PendingTool {
    kind: PendingToolKind,
    activity: Activity,
    input_counts: Option<(u32, u32)>,
    background: bool,
}

#[derive(Clone, Copy)]
enum PendingToolKind {
    Command,
    Edit,
    Delegation,
}

struct ParseState<'a> {
    metadata: &'a mut SessionMetadata,
    pending_tools: &'a mut HashMap<String, PendingTool>,
    last_timestamp: &'a mut Option<Millis>,
    tokens_used: &'a mut u64,
}

struct SessionMetadata {
    is_subagent: bool,
    session_id: String,
    project_key: String,
    office_path: Option<String>,
    git_branch: Option<String>,
    custom_title: Option<String>,
    ai_title: Option<String>,
    agent_name: Option<String>,
    agent_id: Option<String>,
    fallback_agent_id: String,
    source_id: SourceId,
    parent_tool_use_id: Option<String>,
    parent_agent_id: Option<String>,
    forked_from: Option<String>,
}

impl SessionMetadata {
    fn new(discovery: Discovery) -> Self {
        Self {
            fallback_agent_id: discovery
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown-agent")
                .into(),
            source_id: SourceId(String::new()),
            parent_tool_use_id: None,
            parent_agent_id: None,
            forked_from: None,
            is_subagent: discovery.is_subagent,
            session_id: discovery.session_id,
            project_key: discovery.project_key,
            office_path: None,
            git_branch: None,
            custom_title: None,
            ai_title: None,
            agent_name: None,
            agent_id: None,
        }
    }

    fn reset_identity(&mut self) {
        self.office_path = None;
        self.git_branch = None;
        self.custom_title = None;
        self.ai_title = None;
        self.agent_name = None;
        self.agent_id = None;
        self.parent_tool_use_id = None;
        self.parent_agent_id = None;
        self.forked_from = None;
    }

    fn update(&mut self, line: &Value) {
        update_string(&mut self.session_id, line.get("sessionId"), false);
        update_optional_string(&mut self.office_path, line.get("cwd"));
        update_optional_string(&mut self.git_branch, line.get("gitBranch"));
        update_optional_string(&mut self.custom_title, line.get("customTitle"));
        update_optional_string(&mut self.ai_title, line.get("aiTitle"));
        update_optional_string(&mut self.agent_name, line.get("agentName"));
        update_optional_string(&mut self.agent_id, line.get("agentId"));
        update_optional_string(
            &mut self.parent_tool_use_id,
            line.get("parent_tool_use_id")
                .or_else(|| line.get("parentToolUseId")),
        );
        update_optional_string(
            &mut self.parent_agent_id,
            line.get("parentAgentId")
                .or_else(|| line.get("parent_agent_id")),
        );
        update_optional_string(
            &mut self.forked_from,
            line.get("forkedFromSessionId")
                .or_else(|| line.get("forked_from_session_id")),
        );
    }

    fn worker_id(&self) -> String {
        if self.is_subagent {
            self.agent_id
                .as_deref()
                .filter(|id| !id.is_empty())
                .unwrap_or(&self.fallback_agent_id)
                .to_string()
        } else {
            self.session_id.clone()
        }
    }

    fn identity(&self) -> ThreadIdentity {
        let mut identity =
            ThreadIdentity::new(Agent::Claude, self.source_id.clone(), self.worker_id());
        if self.is_subagent {
            identity.session_id = Some(self.session_id.clone());
        }
        identity
    }

    fn display_name(&self) -> String {
        if self.is_subagent {
            let name = [
                self.agent_name.as_deref(),
                self.custom_title.as_deref(),
                self.ai_title.as_deref(),
            ]
            .into_iter()
            .flatten()
            .filter_map(display_candidate)
            .next()
            .unwrap_or_else(|| safe_short_id(self.agent_id.as_deref().unwrap_or(&self.session_id)));
            format!("sub:{name}")
        } else {
            [self.custom_title.as_deref(), self.ai_title.as_deref()]
                .into_iter()
                .flatten()
                .filter_map(display_candidate)
                .next()
                .unwrap_or_else(|| safe_short_id(&self.session_id))
        }
    }
}

fn display_candidate(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.starts_with('/') {
        return None;
    }
    let value = truncate_detail(value);
    if value.is_empty() || value.starts_with('/') {
        None
    } else {
        Some(value)
    }
}

fn safe_short_id(id: &str) -> String {
    let fallback = short_id(id);
    if fallback.starts_with('/') {
        "worker".to_string()
    } else {
        fallback
    }
}

fn update_string(target: &mut String, value: Option<&Value>, allow_empty: bool) {
    let Some(value) = value.and_then(Value::as_str) else {
        return;
    };
    if allow_empty || !value.is_empty() {
        *target = value.to_string();
    }
}

fn update_optional_string(target: &mut Option<String>, value: Option<&Value>) {
    let Some(value) = value.and_then(Value::as_str) else {
        return;
    };
    if !value.is_empty() {
        *target = Some(value.to_string());
    }
}

fn discover_files(projects: &Path) -> io::Result<Vec<Discovery>> {
    let mut files = Vec::new();
    walk_directory(projects, None, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn walk_directory(
    directory: &Path,
    project_key: Option<&str>,
    files: &mut Vec<Discovery>,
) -> io::Result<()> {
    let entries = fs::read_dir(directory)?;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        // Following a symlink would allow a transcript tree to escape the
        // configured home; the collector only needs the regular tree.
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        let next_project_key = project_key.map(str::to_owned).or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
        });
        if file_type.is_dir() {
            walk_directory(&path, next_project_key.as_deref(), files)?;
            continue;
        }
        if !file_type.is_file()
            || path.extension().and_then(|extension| extension.to_str()) != Some("jsonl")
        {
            continue;
        }

        let is_subagent = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some("subagents");
        let session_id = if is_subagent {
            path.parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string()
        } else {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_string()
        };
        files.push(Discovery {
            path,
            is_subagent,
            session_id,
            project_key: next_project_key.unwrap_or_default(),
        });
    }
    Ok(())
}

fn read_file(
    path: &Path,
    cursor: &mut FileCursor,
    only_paths: &[PathBuf],
    office_cache: &mut HashMap<String, String>,
    events: &mut Vec<Event>,
) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    let length = metadata.len();
    let file_mtime = modified_millis(&metadata);
    let identity = file_identity(&metadata);
    let modified = metadata.modified().ok();
    let replaced = cursor
        .file_identity
        .zip(identity)
        .is_some_and(|(old, new)| old != new);
    let rewritten_in_place = cursor.offset > 0
        && cursor
            .last_modified
            .zip(modified)
            .is_some_and(|(old, new)| old != new)
        && length <= cursor.offset;
    if replaced || length < cursor.offset || rewritten_in_place {
        cursor.reset();
    }
    cursor.file_identity = identity;
    cursor.last_modified = modified;
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    // Mounted filesystems can round mtime to whole seconds. Check a bounded
    // transcript tail for in-place rewrites with unchanged size and timestamp.
    if cursor.offset > 0 && !cursor.checkpoint.is_empty() {
        if let Ok(checkpoint) = read_checkpoint(&mut file, cursor.offset) {
            if checkpoint != cursor.checkpoint {
                cursor.reset();
            }
        }
    }
    if length == cursor.offset {
        return true;
    }
    if file.seek(SeekFrom::Start(cursor.offset)).is_err() {
        return false;
    }

    let mut buffer = [0_u8; CHUNK_SIZE];
    loop {
        let read = match file.read(&mut buffer) {
            Ok(read) => read,
            Err(_) => return false,
        };
        if read == 0 {
            break;
        }
        cursor.offset = cursor.offset.saturating_add(read as u64);
        consume_bytes(
            &mut cursor.pending,
            &mut cursor.discarding_line,
            &buffer[..read],
            |line| {
                parse_line(
                    line,
                    &mut ParseState {
                        metadata: &mut cursor.metadata,
                        last_timestamp: &mut cursor.last_timestamp,
                        tokens_used: &mut cursor.tokens_used,
                        pending_tools: &mut cursor.pending_tools,
                    },
                    file_mtime,
                    only_paths,
                    office_cache,
                    events,
                );
            },
        );
    }
    cursor.checkpoint = read_checkpoint(&mut file, cursor.offset).unwrap_or_default();
    true
}

fn read_checkpoint(file: &mut File, offset: u64) -> io::Result<Vec<u8>> {
    let size = offset.min(4096);
    file.seek(SeekFrom::Start(offset - size))?;
    let mut bytes = vec![0; size as usize];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn consume_bytes<F>(pending: &mut Vec<u8>, discarding_line: &mut bool, bytes: &[u8], mut line: F)
where
    F: FnMut(&[u8]),
{
    for &byte in bytes {
        if byte == b'\n' {
            if !*discarding_line && !pending.is_empty() {
                line(pending);
            }
            pending.clear();
            *discarding_line = false;
        } else if !*discarding_line {
            if pending.len() < MAX_LINE_BYTES {
                pending.push(byte);
            } else {
                pending.clear();
                *discarding_line = true;
            }
        }
    }
}

fn parse_line(
    line: &[u8],
    state: &mut ParseState<'_>,
    file_mtime: Option<Millis>,
    only_paths: &[PathBuf],
    office_cache: &mut HashMap<String, String>,
    events: &mut Vec<Event>,
) {
    let Ok(value) = serde_json::from_slice::<Value>(line) else {
        return;
    };
    let Some(object) = value.as_object() else {
        return;
    };
    let previous_identity = state
        .metadata
        .office_path
        .as_ref()
        .map(|_| state.metadata.identity());
    state.metadata.update(&value);

    let at = if let Some(at) = timestamp_value(value.get("timestamp")) {
        *state.last_timestamp = Some(at);
        at
    } else if let Some(at) = (*state.last_timestamp).or(file_mtime) {
        at
    } else {
        return;
    };

    let Some(raw_office_path) = state.metadata.office_path.as_deref() else {
        return;
    };
    let normalized_raw_path = normalize_office_path(raw_office_path);
    if normalized_raw_path.is_empty() {
        return;
    }

    let office_path = repository_root_with_project_hint(
        raw_office_path,
        office_cache,
        Some(&state.metadata.project_key),
    );
    if !path_allowed(raw_office_path, only_paths) && !path_allowed(&office_path, only_paths) {
        return;
    }
    let identity = state.metadata.identity();
    let worker = identity.worker_id();
    let office = OfficeId(office_path.clone());
    let make_event = |kind| Event {
        at,
        office: office.clone(),
        office_path: office_path.clone(),
        worker: worker.clone(),
        agent: Agent::Claude,
        kind,
    };

    if let Some(previous) = previous_identity.filter(|previous| previous.worker_id() != worker) {
        let mut old = make_event(EventKind::Left);
        old.worker = previous.worker_id();
        events.push(old);
    }
    events.push(make_event(EventKind::Identity {
        identity: identity.clone(),
        role: if state.metadata.is_subagent {
            WorkerRole::Subagent
        } else {
            WorkerRole::Main
        },
    }));
    let root_identity = ThreadIdentity::new(
        Agent::Claude,
        identity.source.clone(),
        &state.metadata.session_id,
    );
    if state.metadata.is_subagent {
        events.push(make_event(EventKind::Relationship(Relationship {
            parent: root_identity.worker_id(),
            child: worker.clone(),
            kind: RelationshipKind::SessionMembership,
            evidence: Evidence::TranscriptLayout,
            at,
            correlation_id: state.metadata.parent_tool_use_id.clone(),
        })));
        if let Some(parent_id) = &state.metadata.parent_agent_id {
            let mut parent = ThreadIdentity::new(Agent::Claude, identity.source.clone(), parent_id);
            parent.session_id = Some(state.metadata.session_id.clone());
            events.push(make_event(EventKind::Relationship(Relationship {
                parent: parent.worker_id(),
                child: worker.clone(),
                kind: RelationshipKind::Delegation,
                evidence: Evidence::NativeMetadata,
                at,
                correlation_id: state.metadata.parent_tool_use_id.clone(),
            })));
        }
    } else if let Some(parent) = &state.metadata.forked_from {
        events.push(make_event(EventKind::Relationship(Relationship {
            parent: ThreadIdentity::new(Agent::Claude, identity.source.clone(), parent).worker_id(),
            child: worker.clone(),
            kind: RelationshipKind::Fork,
            evidence: Evidence::NativeMetadata,
            at,
            correlation_id: None,
        })));
    }
    events.push(make_event(EventKind::Seen {
        name: state.metadata.display_name(),
        git_branch: state.metadata.git_branch.as_deref().map(truncate_detail),
    }));
    if object.get("type").and_then(Value::as_str) == Some("assistant") {
        let increment = usage_tokens(&value);
        if increment > 0 {
            *state.tokens_used = (*state.tokens_used).saturating_add(increment);
            events.push(make_event(EventKind::Tokens(*state.tokens_used)));
        }
    }

    record_claude_collaboration(
        &value,
        at,
        &identity,
        state.pending_tools,
        &make_event,
        events,
    );
    match object.get("type").and_then(Value::as_str) {
        Some("user") => parse_user(&value, at, state.pending_tools, make_event, events),
        Some("assistant") => parse_assistant(&value, at, state.pending_tools, make_event, events),
        _ => {}
    }
}

// Keep each content block as an event: a single assistant line can contain a
// tool call followed by explanatory text, and the latest event is the useful
// activity for the desk.
fn record_claude_collaboration<F>(
    value: &Value,
    at: Millis,
    identity: &ThreadIdentity,
    pending: &HashMap<String, PendingTool>,
    make_event: &F,
    events: &mut Vec<Event>,
) where
    F: Fn(EventKind) -> Event,
{
    let actor = identity.worker_id();
    let line_kind = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message_id = value
        .get("uuid")
        .or_else(|| value.pointer("/message/id"))
        .and_then(Value::as_str);
    let make = |id: &str, suffix: &str, kind, text, correlation: Option<String>, recipient| {
        make_event(EventKind::Collaboration(CollaborationEvent {
            id: format!("{id}:{suffix}"),
            at,
            actor: actor.clone(),
            recipient,
            kind,
            text,
            correlation_id: correlation,
            native_turn_id: None,
            native_item_id: Some(id.to_string()),
            evidence: Evidence::NativeEvent,
        }))
    };
    let content = message_content(value).unwrap_or_default();
    for block in content {
        match (line_kind, block.get("type").and_then(Value::as_str)) {
            ("assistant", Some("tool_use")) => {
                let Some(id) = block.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let input = block.get("input");
                match block.get("name").and_then(Value::as_str) {
                    Some("Task" | "Agent") => {
                        let text = input_text(input, &["prompt", "description"])
                            .map(truncate_timeline_text);
                        events.push(make(
                            id,
                            "delegated",
                            CollaborationKind::Delegated,
                            text,
                            Some(id.into()),
                            None,
                        ));
                    }
                    Some("SendMessage") => {
                        // A display name or `broadcast` is not a native agent ID.
                        let recipient =
                            input_text(input, &["agent_id", "target_agent_id"]).map(|id| {
                                let mut peer =
                                    ThreadIdentity::new(Agent::Claude, identity.source.clone(), id);
                                peer.session_id = Some(
                                    identity
                                        .session_id
                                        .as_deref()
                                        .unwrap_or(&identity.native_id)
                                        .into(),
                                );
                                peer.worker_id()
                            });
                        let text =
                            input_text(input, &["message", "content"]).map(truncate_timeline_text);
                        events.push(make(
                            id,
                            "message",
                            CollaborationKind::Message,
                            text,
                            Some(id.into()),
                            recipient,
                        ));
                    }
                    Some("AskUserQuestion" | "ExitPlanMode") => {
                        let text = if block.get("name").and_then(Value::as_str)
                            == Some("AskUserQuestion")
                        {
                            question_detail(input)
                        } else {
                            "Review the proposed plan in the original session.".into()
                        };
                        events.push(make(
                            id,
                            "human-request",
                            CollaborationKind::HumanRequest,
                            Some(text),
                            Some(id.into()),
                            None,
                        ));
                        events.push(make_event(EventKind::Wait(Some(WaitReason::HumanInput))));
                    }
                    _ => {}
                }
            }
            ("user", Some("tool_result")) => {
                let Some(id) = block.get("tool_use_id").and_then(Value::as_str) else {
                    continue;
                };
                if let Some(tool) = pending
                    .get(id)
                    .filter(|tool| matches!(tool.kind, PendingToolKind::Delegation))
                {
                    let root = identity
                        .session_id
                        .as_deref()
                        .unwrap_or(&identity.native_id);
                    let child = value
                        .pointer("/toolUseResult/agentId")
                        .or_else(|| value.pointer("/toolUseResult/agent_id"))
                        .and_then(Value::as_str)
                        .map(|native| {
                            let mut child =
                                ThreadIdentity::new(Agent::Claude, identity.source.clone(), native);
                            child.session_id = Some(root.into());
                            child.worker_id()
                        });
                    if let Some(child) = &child {
                        events.push(make_event(EventKind::Relationship(Relationship {
                            parent: actor.clone(),
                            child: child.clone(),
                            kind: RelationshipKind::Delegation,
                            evidence: Evidence::NativeEvent,
                            at,
                            correlation_id: Some(id.into()),
                        })));
                    }
                    let explicitly_finished = value
                        .pointer("/toolUseResult/status")
                        .and_then(Value::as_str)
                        .is_some_and(|status| matches!(status, "completed" | "failed"));
                    if tool.background && !explicitly_finished {
                        continue;
                    }
                    let text = block
                        .get("content")
                        .and_then(|content| {
                            content.as_str().map(str::to_owned).or_else(|| {
                                content.as_array().map(|blocks| {
                                    blocks
                                        .iter()
                                        .filter_map(|b| b.get("text").and_then(Value::as_str))
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                })
                            })
                        })
                        .map(|text| truncate_timeline_text(&text));
                    let kind = if block.get("is_error").and_then(Value::as_bool) == Some(true) {
                        CollaborationKind::Failed
                    } else {
                        CollaborationKind::Result
                    };
                    let mut result = make(id, "tool-result", kind, text, Some(id.into()), None);
                    if let (Some(child), EventKind::Collaboration(result)) =
                        (child, &mut result.kind)
                    {
                        result.actor = child;
                        result.recipient = Some(actor.clone());
                    }
                    events.push(result);
                }
            }
            _ => {}
        }
    }
    if line_kind == "assistant"
        && value
            .pointer("/message/stop_reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| matches!(reason, "end_turn" | "stop_sequence"))
        && !content
            .iter()
            .any(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
    {
        let text = content
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(id) = message_id.filter(|_| !text.is_empty()) {
            events.push(make(
                id,
                "final",
                CollaborationKind::Result,
                Some(truncate_timeline_text(&text)),
                None,
                None,
            ));
        }
        events.push(make_event(EventKind::Lifecycle(WorkerLifecycle::Completed)));
    }
}

fn parse_assistant<F>(
    value: &Value,
    at: Millis,
    pending_tools: &mut HashMap<String, PendingTool>,
    make_event: F,
    events: &mut Vec<Event>,
) where
    F: Fn(EventKind) -> Event,
{
    let content = value
        .get("message")
        .and_then(|message| message.get("content"))
        .or_else(|| value.get("content"))
        .and_then(Value::as_array);
    let Some(content) = content else {
        events.push(make_event(EventKind::Turn { in_flight: false }));
        return;
    };

    let mut has_tool_use = false;
    for block in content {
        let Some(block_type) = block.get("type").and_then(Value::as_str) else {
            continue;
        };
        match block_type {
            "tool_use" => {
                has_tool_use = true;
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let activity = tool_activity(name, block.get("input"));
                events.push(make_event(EventKind::Acted(activity.clone())));
                if let (Some(tool_id), Some(kind)) = (
                    block.get("id").and_then(Value::as_str),
                    pending_tool_kind(name),
                ) {
                    pending_tools.insert(
                        tool_id.to_string(),
                        PendingTool {
                            kind,
                            activity,
                            input_counts: edit_input_counts(name, block.get("input")),
                            background: block
                                .pointer("/input/run_in_background")
                                .or_else(|| block.pointer("/input/runInBackground"))
                                .and_then(Value::as_bool)
                                == Some(true),
                        },
                    );
                }
            }
            "text" => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    events.push(make_event(EventKind::Did(Beat {
                        at,
                        activity: Activity::Talking {
                            detail: truncate_timeline_text(text),
                        },
                        outcome: None,
                    })));
                }
            }
            _ => {}
        }
    }

    if !has_tool_use {
        events.push(make_event(EventKind::Turn { in_flight: false }));
    }
}

fn parse_user<F>(
    value: &Value,
    at: Millis,
    pending_tools: &mut HashMap<String, PendingTool>,
    make_event: F,
    events: &mut Vec<Event>,
) where
    F: Fn(EventKind) -> Event,
{
    if let Some(content) = message_content(value) {
        for block in content {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(Value::as_str) {
                        events.push(make_event(message_beat(at, text)));
                    }
                }
                Some("tool_result") => {
                    let Some(tool_id) = block.get("tool_use_id").and_then(Value::as_str) else {
                        continue;
                    };
                    let Some(pending) = pending_tools.remove(tool_id) else {
                        continue;
                    };
                    let outcome = tool_result_outcome(&pending, block, value);
                    if matches!(pending.kind, PendingToolKind::Command)
                        && !matches!(outcome, Some(Outcome::Exited(_)))
                    {
                        continue;
                    }
                    events.push(make_event(EventKind::Did(Beat {
                        at,
                        activity: pending.activity,
                        outcome,
                    })));
                }
                _ => {}
            }
        }
    } else if let Some(text) = value
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
    {
        events.push(make_event(message_beat(at, text)));
    }
    events.push(make_event(EventKind::Turn { in_flight: true }));
}

fn message_content(value: &Value) -> Option<&[Value]> {
    value
        .get("message")
        .and_then(|message| message.get("content"))
        .or_else(|| value.get("content"))
        .and_then(Value::as_array)
        .map(|values| values.as_slice())
}

fn message_beat(at: Millis, text: &str) -> EventKind {
    EventKind::Did(Beat {
        at,
        activity: Activity::Talking {
            detail: truncate_timeline_text(text),
        },
        outcome: None,
    })
}

fn pending_tool_kind(name: &str) -> Option<PendingToolKind> {
    match name {
        "Bash" => Some(PendingToolKind::Command),
        "Edit" | "Write" | "NotebookEdit" => Some(PendingToolKind::Edit),
        "Task" | "Agent" => Some(PendingToolKind::Delegation),
        _ => None,
    }
}

fn input_text<'a>(input: Option<&'a Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        input
            .and_then(|input| input.get(*key))
            .and_then(Value::as_str)
    })
}

fn edit_input_counts(name: &str, input: Option<&Value>) -> Option<(u32, u32)> {
    match name {
        "Edit" => {
            let old = input_text(input, &["old_string", "oldString"])?;
            let new = input_text(input, &["new_string", "newString"])?;
            Some((text_line_count(new), text_line_count(old)))
        }
        "Write" => input_text(input, &["content"]).map(|content| (text_line_count(content), 0)),
        _ => None,
    }
}

fn tool_result_outcome(pending: &PendingTool, block: &Value, line: &Value) -> Option<Outcome> {
    match pending.kind {
        PendingToolKind::Delegation => None,
        PendingToolKind::Command => exit_code_from_values(line, block).map(Outcome::Exited),
        PendingToolKind::Edit => change_counts_from_values(line, block)
            .or(pending.input_counts)
            .map(|(added, removed)| Outcome::Changed { added, removed }),
    }
}

fn change_counts_from_values(first: &Value, second: &Value) -> Option<(u32, u32)> {
    change_counts_from_value(first).or_else(|| change_counts_from_value(second))
}

fn change_counts_from_value(value: &Value) -> Option<(u32, u32)> {
    match value {
        Value::Object(object) => {
            if let Some(counts) = explicit_change_counts(value) {
                return Some(counts);
            }
            if let Some(counts) = object
                .get("structuredPatch")
                .and_then(structured_patch_counts)
            {
                return Some(counts);
            }
            if let (Some(old), Some(new)) = (
                object
                    .get("oldString")
                    .or_else(|| object.get("old_string"))
                    .and_then(Value::as_str),
                object
                    .get("newString")
                    .or_else(|| object.get("new_string"))
                    .and_then(Value::as_str),
            ) {
                return Some((text_line_count(new), text_line_count(old)));
            }
            if let Some(counts) = object
                .get("diff")
                .and_then(Value::as_str)
                .and_then(unified_diff_counts)
            {
                return Some(counts);
            }
            object.values().find_map(change_counts_from_value)
        }
        Value::Array(values) => values.iter().find_map(change_counts_from_value),
        Value::String(text) => unified_diff_counts(text),
        _ => None,
    }
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

fn structured_patch_counts(value: &Value) -> Option<(u32, u32)> {
    let patches = value.as_array()?;
    let mut total = (0_u32, 0_u32);
    for patch in patches {
        let removed = patch.get("oldLines").and_then(u32_value)?;
        let added = patch.get("newLines").and_then(u32_value)?;
        total.0 = total.0.saturating_add(added);
        total.1 = total.1.saturating_add(removed);
    }
    Some(total)
}

fn u32_value(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .or_else(|| value.as_i64().and_then(|value| u32::try_from(value).ok()))
        .or_else(|| value.as_str()?.trim().parse::<u32>().ok())
}

fn exit_code_from_values(first: &Value, second: &Value) -> Option<i32> {
    find_exit_code(first).or_else(|| find_exit_code(second))
}

fn find_exit_code(value: &Value) -> Option<i32> {
    if let Some(code) = find_numeric_field(value) {
        return Some(code);
    }
    find_text_exit_code(value)
}

fn find_numeric_field(value: &Value) -> Option<i32> {
    match value {
        Value::Object(object) => {
            for key in ["exitCode", "exit_code", "returnCode", "return_code"] {
                if let Some(number) = object.get(key).and_then(integer_value) {
                    return Some(number);
                }
            }
            object.values().find_map(find_numeric_field)
        }
        Value::Array(values) => values.iter().find_map(find_numeric_field),
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

fn find_text_exit_code(value: &Value) -> Option<i32> {
    match value {
        Value::String(text) => parse_exit_code_text(text),
        Value::Array(values) => values.iter().find_map(find_text_exit_code),
        Value::Object(object) => object.values().find_map(find_text_exit_code),
        _ => None,
    }
}

fn parse_exit_code_text(text: &str) -> Option<i32> {
    for marker in [
        "process exited with code",
        "exit code",
        "exit status",
        "exited with code",
        "returned",
    ] {
        if let Some(start) = find_ascii_case_insensitive(text, marker) {
            if let Some(code) = integer_after(text, start + marker.len()) {
                return Some(code);
            }
        }
    }
    None
}

fn find_ascii_case_insensitive(text: &str, needle: &str) -> Option<usize> {
    let needle = needle.as_bytes();
    text.as_bytes().windows(needle.len()).position(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    })
}

fn integer_after(text: &str, mut index: usize) -> Option<i32> {
    let bytes = text.as_bytes();
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b':' || *byte == b'=')
    {
        index += 1;
    }
    let number_start = index;
    if bytes
        .get(index)
        .is_some_and(|byte| *byte == b'+' || *byte == b'-')
    {
        index += 1;
    }
    let digits_start = index;
    while bytes.get(index).is_some_and(|byte| byte.is_ascii_digit()) {
        index += 1;
    }
    if digits_start == index {
        return None;
    }
    text.get(number_start..index)?.parse::<i32>().ok()
}

fn tool_activity(name: &str, input: Option<&Value>) -> Activity {
    match name {
        "Bash" => Activity::Typing {
            detail: input_detail(input, &["command"]),
        },
        "Read" => Activity::Reading {
            detail: input_detail(input, &["file_path"]),
        },
        "Edit" | "Write" | "NotebookEdit" => Activity::Editing {
            detail: input_detail(input, &["file_path"]),
        },
        "Grep" | "Glob" => Activity::Searching {
            detail: input_detail(input, &["pattern"]),
        },
        "WebSearch" => Activity::Searching {
            detail: input_detail(input, &["query"]),
        },
        "WebFetch" => Activity::Searching {
            detail: input_detail(input, &["url"]),
        },
        "AskUserQuestion" => Activity::Waiting {
            detail: question_detail(input),
        },
        "Task" | "Agent" => Activity::Thinking,
        _ => Activity::Thinking,
    }
}

fn input_detail(input: Option<&Value>, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            input
                .and_then(|input| input.get(*key))
                .and_then(Value::as_str)
        })
        .map(truncate_detail)
        .unwrap_or_default()
}

fn question_detail(input: Option<&Value>) -> String {
    input
        .and_then(|input| input.get("question"))
        .and_then(Value::as_str)
        .or_else(|| {
            input
                .and_then(|input| input.get("questions"))
                .and_then(Value::as_array)
                .and_then(|questions| questions.first())
                .and_then(|question| question.get("question"))
                .and_then(Value::as_str)
        })
        .map(truncate_timeline_text)
        .unwrap_or_default()
}

fn modified_millis(metadata: &fs::Metadata) -> Option<Millis> {
    let millis = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();
    millis.try_into().ok()
}

fn usage_tokens(value: &Value) -> u64 {
    let Some(usage) = value
        .get("message")
        .and_then(|message| message.get("usage"))
    else {
        return 0;
    };
    [
        "input_tokens",
        "output_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
    ]
    .into_iter()
    .filter_map(|field| usage.get(field).and_then(Value::as_u64))
    .fold(0, |total, value| total.saturating_add(value))
}
fn disambiguate_names(
    worker_names: &mut HashMap<(String, String), NameAssignment>,
    events: &mut [Event],
) {
    let mut base_names = BTreeMap::new();
    for event in events.iter() {
        if let EventKind::Seen { name, .. } = &event.kind {
            base_names.insert(
                (event.office_path.clone(), event.worker.0.clone()),
                name.clone(),
            );
        }
    }
    if base_names.is_empty() {
        return;
    }

    let mut groups: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for ((office, worker_id), base) in base_names {
        groups.entry((office, base)).or_default().push(worker_id);
    }

    let mut assigned = HashMap::new();
    for ((office, base), mut worker_ids) in groups {
        worker_ids.sort_by(|left, right| {
            let left_existing = worker_names
                .get(&(office.clone(), left.clone()))
                .is_some_and(|assignment| assignment.base == base);
            let right_existing = worker_names
                .get(&(office.clone(), right.clone()))
                .is_some_and(|assignment| assignment.base == base);
            right_existing
                .cmp(&left_existing)
                .then_with(|| left.cmp(right))
        });
        let mut used = HashSet::new();
        for worker_id in worker_ids {
            let key = (office.clone(), worker_id.clone());
            let name = worker_names
                .get(&key)
                .filter(|assignment| {
                    assignment.base == base && !used.contains(&assignment.assigned)
                })
                .map(|assignment| assignment.assigned.clone())
                .unwrap_or_else(|| next_unique_name(&base, &worker_id, &used));
            used.insert(name.clone());
            assigned.insert(key.clone(), name.clone());
            worker_names.insert(
                key,
                NameAssignment {
                    base: base.clone(),
                    assigned: name,
                },
            );
        }
    }

    for event in events.iter_mut() {
        if let EventKind::Seen { name, .. } = &mut event.kind {
            if let Some(assigned_name) =
                assigned.get(&(event.office_path.clone(), event.worker.0.clone()))
            {
                *name = assigned_name.clone();
            }
        }
    }
}

fn next_unique_name(base: &str, worker_id: &str, used: &HashSet<String>) -> String {
    if !used.contains(base) {
        return base.to_string();
    }
    let candidate = format!("{base} ({})", short_id(worker_id));
    if !used.contains(&candidate) {
        return candidate;
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base} ({suffix})");
        if !used.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_questions_keep_their_context_beyond_caption_length() {
        let question = format!(
            "Review {} before choosing production or staging?",
            "the migration plan ".repeat(20)
        );
        let input = serde_json::json!({"questions":[{"question":question}]});
        assert_eq!(question_detail(Some(&input)), question);
        let oversized = serde_json::json!({"question":"界".repeat(3_000)});
        assert!(question_detail(Some(&oversized)).ends_with('…'));
    }

    fn cursor(path: &str, session_id: &str, office_path: &str) -> (PathBuf, FileCursor) {
        let path = PathBuf::from(path);
        let mut cursor = FileCursor::new(Discovery {
            path: path.clone(),
            is_subagent: false,
            session_id: session_id.to_string(),
            project_key: String::new(),
        });
        cursor.metadata.office_path = Some(office_path.to_string());
        (path, cursor)
    }

    #[test]
    fn ended_files_leave_every_claude_runtime_cache() {
        let mut source = ClaudeSource::new("/tmp/does-not-exist");
        let (active_path, active_cursor) = cursor("/repo/active.jsonl", "active", "/repo/src");
        let (ended_path, ended_cursor) = cursor("/gone/ended.jsonl", "ended", "/gone/src");
        source.files.insert(active_path.clone(), active_cursor);
        source.files.insert(ended_path.clone(), ended_cursor);
        source
            .office_cache
            .insert("/repo/src".to_string(), "/repo".to_string());
        source
            .office_cache
            .insert("/gone/src".to_string(), "/gone".to_string());
        source.worker_names.insert(
            (
                "/repo".to_string(),
                ThreadIdentity::new(Agent::Claude, SourceId(String::new()), "active")
                    .worker_id()
                    .0,
            ),
            NameAssignment {
                base: "worker".to_string(),
                assigned: "worker".to_string(),
            },
        );
        source.worker_names.insert(
            (
                "/gone".to_string(),
                ThreadIdentity::new(Agent::Claude, SourceId(String::new()), "ended")
                    .worker_id()
                    .0,
            ),
            NameAssignment {
                base: "worker".to_string(),
                assigned: "worker (ended)".to_string(),
            },
        );

        source.files.remove(&ended_path);
        source.prune_runtime_state();

        assert_eq!(source.files.len(), 1);
        assert_eq!(source.office_cache.len(), 1);
        assert_eq!(source.worker_names.len(), 1);
        assert!(source.files.contains_key(&active_path));
        assert!(source.office_cache.contains_key("/repo/src"));
        assert!(source.worker_names.contains_key(&(
            "/repo".to_string(),
            ThreadIdentity::new(Agent::Claude, SourceId(String::new()), "active")
                .worker_id()
                .0
        )));
        assert!(!source.office_cache.contains_key("/gone/src"));
        assert!(!source.worker_names.contains_key(&(
            "/gone".to_string(),
            ThreadIdentity::new(Agent::Claude, SourceId(String::new()), "ended")
                .worker_id()
                .0
        )));

        source.clear_runtime_state();
        assert!(source.files.is_empty());
        assert!(source.office_cache.is_empty());
        assert!(source.worker_names.is_empty());
    }
}
