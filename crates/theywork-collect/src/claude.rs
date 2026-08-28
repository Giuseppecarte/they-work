use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::Value;
use theywork_core::{
    Activity, Agent, Event, EventKind, Millis, OfficeId, Source, SourceError, WorkerId,
};

use crate::util::{
    normalize_office_path, path_allowed, recency_cutoff, repository_root, short_id,
    timestamp_value, truncate_detail,
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
        }
    }

    /// Alias useful to callers that want to make the path filtering explicit.
    pub fn new_with_paths(home: impl Into<PathBuf>, only_paths: Vec<PathBuf>) -> Self {
        Self::with_paths(home, only_paths)
    }

    pub(crate) fn home_exists(home: &Path) -> bool {
        home.is_dir()
    }

    fn discover(&mut self, now: Millis) {
        let cutoff_ms = recency_cutoff(now, self.active_within);
        let projects = self.home.join("projects");
        let discovered = match discover_files(&projects) {
            Ok(discovered) => discovered,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.files.clear();
                return;
            }
            Err(_) => return,
        };

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

        self.files.retain(|path, _| paths.contains_key(path));
        for (path, discovery) in paths {
            self.files
                .entry(path)
                .or_insert_with(|| FileCursor::new(discovery));
        }
    }
}

impl Source for ClaudeSource {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn poll(&mut self, now: Millis) -> Result<Vec<Event>, SourceError> {
        self.discover(now);

        let paths: Vec<PathBuf> = self.files.keys().cloned().collect();
        let mut events = Vec::new();
        for path in paths {
            let Some(mut cursor) = self.files.remove(&path) else {
                continue;
            };
            read_file(
                &path,
                &mut cursor,
                &self.only_paths,
                &mut self.office_cache,
                &mut events,
            );
            self.files.insert(path, cursor);
        }

        // A source may discover several sessions and subagents at once. The
        // UI gets a stable chronological feed even though directory order is
        // unspecified by the filesystem.
        events.sort_by_key(|event| event.at);
        Ok(events)
    }
}

#[derive(Clone)]
struct Discovery {
    path: PathBuf,
    is_subagent: bool,
    session_id: String,
}

struct FileCursor {
    offset: u64,
    pending: Vec<u8>,
    discarding_line: bool,
    last_timestamp: Option<Millis>,
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
            tokens_used: 0,
            metadata: SessionMetadata::new(discovery),
        }
    }

    fn reset(&mut self) {
        self.offset = 0;
        self.pending.clear();
        self.discarding_line = false;
        self.last_timestamp = None;
        self.tokens_used = 0;
        self.metadata.reset_identity();
    }
}

struct ParseState<'a> {
    metadata: &'a mut SessionMetadata,
    last_timestamp: &'a mut Option<Millis>,
    tokens_used: &'a mut u64,
}

struct SessionMetadata {
    is_subagent: bool,
    session_id: String,
    office_path: Option<String>,
    git_branch: Option<String>,
    custom_title: Option<String>,
    ai_title: Option<String>,
    agent_name: Option<String>,
    agent_id: Option<String>,
}

impl SessionMetadata {
    fn new(discovery: Discovery) -> Self {
        Self {
            is_subagent: discovery.is_subagent,
            session_id: discovery.session_id,
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
    }

    fn update(&mut self, line: &Value) {
        update_string(&mut self.session_id, line.get("sessionId"), false);
        update_optional_string(&mut self.office_path, line.get("cwd"));
        update_optional_string(&mut self.git_branch, line.get("gitBranch"));
        update_optional_string(&mut self.custom_title, line.get("customTitle"));
        update_optional_string(&mut self.ai_title, line.get("aiTitle"));
        update_optional_string(&mut self.agent_name, line.get("agentName"));
        update_optional_string(&mut self.agent_id, line.get("agentId"));
    }

    fn worker_id(&self) -> String {
        if self.is_subagent {
            self.agent_id
                .as_deref()
                .filter(|id| !id.is_empty())
                .unwrap_or(&self.session_id)
                .to_string()
        } else {
            self.session_id.clone()
        }
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
    walk_directory(projects, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn walk_directory(directory: &Path, files: &mut Vec<Discovery>) -> io::Result<()> {
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
        if file_type.is_dir() {
            walk_directory(&path, files)?;
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
) {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return,
    };
    let length = metadata.len();
    let file_mtime = modified_millis(&metadata);
    if length < cursor.offset {
        cursor.reset();
    }
    if length == cursor.offset {
        return;
    }

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return,
    };
    if file.seek(SeekFrom::Start(cursor.offset)).is_err() {
        return;
    }

    let mut buffer = [0_u8; CHUNK_SIZE];
    while let Ok(read) = file.read(&mut buffer) {
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
                    },
                    file_mtime,
                    only_paths,
                    office_cache,
                    events,
                );
            },
        );
    }
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
    if normalized_raw_path.is_empty() || !path_allowed(raw_office_path, only_paths) {
        return;
    }

    let office_path = repository_root(raw_office_path, office_cache);
    let worker = WorkerId(state.metadata.worker_id());
    let office = OfficeId(office_path.clone());
    let make_event = |kind| Event {
        at,
        office: office.clone(),
        office_path: office_path.clone(),
        worker: worker.clone(),
        agent: Agent::Claude,
        kind,
    };

    events.push(make_event(EventKind::Seen {
        name: state.metadata.display_name(),
        git_branch: state.metadata.git_branch.clone(),
    }));
    if object.get("type").and_then(Value::as_str) == Some("assistant") {
        let increment = usage_tokens(&value);
        if increment > 0 {
            *state.tokens_used = (*state.tokens_used).saturating_add(increment);
            events.push(make_event(EventKind::Tokens(*state.tokens_used)));
        }
    }

    match object.get("type").and_then(Value::as_str) {
        Some("user") => events.push(make_event(EventKind::Turn { in_flight: true })),
        Some("assistant") => parse_assistant(&value, make_event, events),
        _ => {}
    }
}

// Keep each content block as an event: a single assistant line can contain a
// tool call followed by explanatory text, and the latest event is the useful
// activity for the desk.
fn parse_assistant<F>(value: &Value, make_event: F, events: &mut Vec<Event>)
where
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
                events.push(make_event(EventKind::Acted(tool_activity(
                    name,
                    block.get("input"),
                ))));
            }
            "text" => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    events.push(make_event(EventKind::Acted(Activity::Talking {
                        detail: truncate_detail(text),
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
        .map(truncate_detail)
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
