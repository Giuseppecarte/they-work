//! Read-only collectors.
//!
//! Every collector in this crate opens files for reading and nothing else. No
//! writes, no network, no spawning. That guarantee is the whole security story
//! of they-work, so keep it true.
//!
//! Owner: collectors dev. Do not edit `theywork-core` or `theywork-render`
//! from here; if the contract needs to change, raise it rather than editing it.

pub mod claude;
pub mod codex;
mod codex_source;
mod util;

use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

pub use claude::ClaudeSource;
pub use codex::CodexSource;
pub use util::{normalize_office_path, NON_PROJECT_OFFICE};

use theywork_core::{Agent, Millis, Source};

/// Default roster horizon. The world still evicts workers sooner when their
/// last actual event is old; this bound keeps a first SQLite scan small.
pub const DEFAULT_ACTIVE_WITHIN: Duration = Duration::from_secs(6 * 60 * 60);

/// Where a selected agent home came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryKind {
    /// An explicit `THEYWORK_*_HOME` override.
    Override,
    /// The conventional container bind mount under `/data`.
    ContainerMount,
    /// The current user's native home directory.
    Home,
    /// A Windows user profile visible through a WSL mount.
    WslCrossover,
    /// A path supplied directly by a caller-built [`Config`].
    Configured,
}

impl DiscoveryKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Override => "override",
            Self::ContainerMount => "container",
            Self::Home => "home",
            Self::WslCrossover => "wsl-crossover",
            Self::Configured => "configured",
        }
    }
}

/// A read-only summary of one agent's discovered store.
///
/// This is deliberately separate from [`Source`]: the doctor command needs to
/// explain a missing or unreadable home without constructing a live poller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreReport {
    pub agent: Agent,
    pub path: PathBuf,
    /// Every candidate considered for this agent, in discovery order.
    pub candidates: Vec<PathBuf>,
    pub discovery: DiscoveryKind,
    pub home_found: bool,
    pub readable: bool,
    pub projects: usize,
    /// Distinct recorded working directories that could not be inspected on
    /// this machine, so they cannot truthfully be counted as projects.
    pub unresolved_paths: usize,
    pub threads: usize,
    pub active_threads: usize,
    pub error: Option<String>,
}

impl StoreReport {
    fn new(agent: Agent, path: PathBuf) -> Self {
        Self {
            agent,
            path,
            candidates: Vec::new(),
            discovery: DiscoveryKind::Configured,
            home_found: false,
            readable: false,
            projects: 0,
            unresolved_paths: 0,
            threads: 0,
            active_threads: 0,
            error: None,
        }
    }
}

/// Return the path selected for each agent, including a missing environment
/// override. Keeping the candidate visible is useful in `--doctor`, where a
/// typo in an override must not silently fall through to another home.
pub fn discovery_paths() -> Vec<(Agent, PathBuf)> {
    discovery_selections()
        .into_iter()
        .map(|selection| (selection.agent, selection.path))
        .collect()
}

/// Return all paths considered for each agent, in the order used by
/// [`Config::discover`]. This is intentionally metadata-only: it never opens
/// a transcript or database.
pub fn discovery_candidates() -> Vec<(Agent, Vec<PathBuf>)> {
    discovery_plans()
        .into_iter()
        .map(|plan| {
            (
                plan.agent,
                plan.candidates
                    .into_iter()
                    .map(|candidate| candidate.path)
                    .collect(),
            )
        })
        .collect()
}

#[derive(Debug, Clone)]
struct DiscoveryCandidate {
    path: PathBuf,
    kind: DiscoveryKind,
}

#[derive(Debug, Clone)]
struct DiscoveryPlan {
    agent: Agent,
    candidates: Vec<DiscoveryCandidate>,
}

#[derive(Debug, Clone)]
struct DiscoverySelection {
    agent: Agent,
    path: PathBuf,
    kind: DiscoveryKind,
    candidates: Vec<PathBuf>,
}

fn discovery_selections() -> Vec<DiscoverySelection> {
    discovery_plans()
        .into_iter()
        .map(select_candidate)
        .collect()
}

fn discovery_plans() -> Vec<DiscoveryPlan> {
    vec![
        discovery_plan(
            Agent::Claude,
            "THEYWORK_CLAUDE_HOME",
            "/data/claude",
            ".claude",
        ),
        discovery_plan(Agent::Codex, "THEYWORK_CODEX_HOME", "/data/codex", ".codex"),
    ]
}

fn discovery_plan(agent: Agent, env: &str, mount: &str, home_rel: &str) -> DiscoveryPlan {
    if let Some(value) =
        std::env::var_os(env).filter(|value| !value.to_string_lossy().trim().is_empty())
    {
        let mut candidates = Vec::new();
        add_candidate(
            &mut candidates,
            DiscoveryCandidate {
                path: PathBuf::from(&value),
                kind: DiscoveryKind::Override,
            },
        );
        if let Some(path) = windows_path_to_unix(&value) {
            add_candidate(
                &mut candidates,
                DiscoveryCandidate {
                    path,
                    kind: DiscoveryKind::Override,
                },
            );
        }
        return DiscoveryPlan { agent, candidates };
    }

    let mut candidates = Vec::new();
    add_candidate(
        &mut candidates,
        DiscoveryCandidate {
            path: PathBuf::from(mount),
            kind: DiscoveryKind::ContainerMount,
        },
    );
    if let Some(home) = std::env::var_os("HOME") {
        add_candidate(
            &mut candidates,
            DiscoveryCandidate {
                path: PathBuf::from(home).join(home_rel),
                kind: DiscoveryKind::Home,
            },
        );
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        if let Some(path) = windows_path_to_unix(&profile) {
            let kind = DiscoveryKind::WslCrossover;
            add_candidate(
                &mut candidates,
                DiscoveryCandidate {
                    path: path.join(home_rel),
                    kind,
                },
            );
        } else {
            let path = PathBuf::from(profile).join(home_rel);
            let kind = if is_wsl_profile_path(&path) {
                DiscoveryKind::WslCrossover
            } else {
                DiscoveryKind::Home
            };
            add_candidate(&mut candidates, DiscoveryCandidate { path, kind });
        }
    }
    for path in wsl_profile_candidates(home_rel) {
        add_candidate(
            &mut candidates,
            DiscoveryCandidate {
                path,
                kind: DiscoveryKind::WslCrossover,
            },
        );
    }
    DiscoveryPlan { agent, candidates }
}

fn add_candidate(candidates: &mut Vec<DiscoveryCandidate>, candidate: DiscoveryCandidate) {
    if !candidates
        .iter()
        .any(|existing| existing.path == candidate.path)
    {
        candidates.push(candidate);
    }
}

fn select_candidate(plan: DiscoveryPlan) -> DiscoverySelection {
    let selected_index = (0..plan.candidates.len())
        .max_by_key(|index| {
            let candidate = &plan.candidates[*index];
            (
                candidate_quality(plan.agent, &candidate.path),
                candidate_activity(plan.agent, &candidate.path),
                Reverse(*index),
            )
        })
        .unwrap_or(0);
    let selected = plan
        .candidates
        .get(selected_index)
        .cloned()
        .unwrap_or(DiscoveryCandidate {
            path: PathBuf::from("/data"),
            kind: DiscoveryKind::ContainerMount,
        });
    DiscoverySelection {
        agent: plan.agent,
        path: selected.path,
        kind: selected.kind,
        candidates: plan
            .candidates
            .into_iter()
            .map(|candidate| candidate.path)
            .collect(),
    }
}

fn candidate_quality(agent: Agent, path: &Path) -> u8 {
    let Ok(metadata) = fs::metadata(path) else {
        return 0;
    };
    if !metadata.is_dir() {
        return 1;
    }
    if !metadata_allows_read(&metadata) {
        return 1;
    }

    match agent {
        Agent::Claude => {
            let projects = path.join("projects");
            let Ok(projects_metadata) = fs::metadata(&projects) else {
                return 2;
            };
            if !projects_metadata.is_dir() || !metadata_allows_read(&projects_metadata) {
                return 1;
            }
            let Ok(entries) = fs::read_dir(projects) else {
                return 1;
            };
            let has_transcript = entries.flatten().any(|entry| {
                let Ok(file_type) = entry.file_type() else {
                    return false;
                };
                if file_type.is_symlink() {
                    return false;
                }
                if file_type.is_file() {
                    return entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "jsonl");
                }
                file_type.is_dir()
                    && fs::read_dir(entry.path())
                        .ok()
                        .into_iter()
                        .flatten()
                        .filter_map(Result::ok)
                        .any(|nested| {
                            nested
                                .path()
                                .extension()
                                .is_some_and(|extension| extension == "jsonl")
                        })
            });
            if has_transcript {
                3
            } else {
                2
            }
        }
        Agent::Codex => {
            let sqlite = path.join("sqlite");
            let Ok(sqlite_metadata) = fs::metadata(&sqlite) else {
                return 2;
            };
            if !sqlite_metadata.is_dir() || !metadata_allows_read(&sqlite_metadata) {
                return 1;
            }
            let state = sqlite.join("state_5.sqlite");
            let history = sqlite.join("thread_history_1.sqlite");
            if !state.is_file() || !history.is_file() {
                return 2;
            }
            if ![state.as_path(), history.as_path()].iter().all(|path| {
                fs::metadata(path).is_ok_and(|metadata| metadata_allows_read(&metadata))
            }) {
                return 1;
            }
            if [state, history]
                .iter()
                .any(|path| fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0))
            {
                3
            } else {
                2
            }
        }
    }
}

fn candidate_activity(agent: Agent, path: &Path) -> u128 {
    match agent {
        Agent::Claude => {
            let mut budget = 4_096;
            latest_modified_tree(path, 0, &mut budget)
        }
        Agent::Codex => [
            path.to_path_buf(),
            path.join("sqlite"),
            path.join("sqlite/state_5.sqlite"),
            path.join("sqlite/thread_history_1.sqlite"),
        ]
        .iter()
        .filter_map(|path| {
            fs::metadata(path)
                .ok()
                .and_then(|metadata| modified_millis(&metadata))
        })
        .max()
        .unwrap_or(0),
    }
}

fn latest_modified_tree(path: &Path, depth: usize, budget: &mut usize) -> u128 {
    if *budget == 0 {
        return 0;
    }
    *budget -= 1;
    let mut latest = fs::metadata(path)
        .ok()
        .and_then(|metadata| modified_millis(&metadata))
        .unwrap_or(0);
    if depth >= 5 {
        return latest;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return latest;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            break;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let entry_path = entry.path();
        let entry_modified = fs::metadata(&entry_path)
            .ok()
            .and_then(|metadata| modified_millis(&metadata))
            .unwrap_or(0);
        latest = latest.max(entry_modified);
        if file_type.is_dir() {
            latest = latest.max(latest_modified_tree(&entry_path, depth + 1, budget));
        }
    }
    latest
}

fn modified_millis(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

fn metadata_allows_read(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        metadata.mode() & 0o444 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub(crate) fn path_allows_read(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata_allows_read(&metadata))
}

pub(crate) fn unreadable_reason(
    path: &Path,
    operation: &str,
    error: &dyn std::fmt::Display,
) -> String {
    format!("{operation}: {error}; {}", metadata_access_details(path))
}

pub(crate) fn metadata_access_details(path: &Path) -> String {
    let Ok(metadata) = fs::metadata(path) else {
        return "owner=unknown permissions=unknown".to_string();
    };
    #[cfg(unix)]
    {
        format!(
            "owner={}:{} permissions=0o{:04o}",
            metadata.uid(),
            metadata.gid(),
            metadata.mode() & 0o7777
        )
    }
    #[cfg(not(unix))]
    {
        let permissions = if metadata.permissions().readonly() {
            "read-only"
        } else {
            "standard"
        };
        format!("owner=unknown permissions={permissions}")
    }
}

fn wsl_profile_candidates(home_rel: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let Ok(mounts) = fs::read_dir("/mnt") else {
        return candidates;
    };
    for mount in mounts.flatten() {
        let Ok(mount_type) = mount.file_type() else {
            continue;
        };
        if !mount_type.is_dir() {
            continue;
        }
        for users_name in ["Users", "users"] {
            let users = mount.path().join(users_name);
            let Ok(users_entries) = fs::read_dir(&users) else {
                continue;
            };
            for user in users_entries.flatten() {
                let Ok(user_type) = user.file_type() else {
                    continue;
                };
                if user_type.is_dir() {
                    candidates.push(user.path().join(home_rel));
                }
            }
        }
    }
    candidates.sort();
    candidates
}

fn windows_path_to_unix(value: &std::ffi::OsStr) -> Option<PathBuf> {
    let value = value.to_string_lossy();
    let slashed = value.replace('\\', "/");
    let bytes = slashed.as_bytes();
    if bytes.len() >= 3 && bytes[1] == b':' && bytes[2] == b'/' {
        let drive = bytes[0].to_ascii_lowercase() as char;
        let rest = slashed[3..].trim_start_matches('/');
        return Some(PathBuf::from(format!("/mnt/{drive}/{rest}")));
    }
    if slashed
        .trim_start_matches('/')
        .split('/')
        .next()
        .is_some_and(|component| component.eq_ignore_ascii_case("wsl.localhost"))
    {
        let mut components = slashed.split('/').filter(|component| !component.is_empty());
        components.next();
        components.next();
        let rest = components.collect::<Vec<_>>().join("/");
        return Some(PathBuf::from(format!("/{rest}")));
    }
    None
}

fn is_wsl_profile_path(path: &Path) -> bool {
    let components = path
        .to_string_lossy()
        .replace('\\', "/")
        .split('/')
        .filter(|component| !component.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    components.len() >= 5
        && components[0] == "mnt"
        && components[1].len() == 1
        && components[2] == "users"
}

/// Where to look for agent trails.
#[derive(Debug, Clone)]
pub struct Config {
    /// Usually `~/.claude`.
    pub claude_home: Option<PathBuf>,
    /// Usually `~/.codex`.
    pub codex_home: Option<PathBuf>,
    /// Only query Codex threads touched within this interval.
    pub active_within: Duration,
    /// Only report offices whose path starts with one of these. Empty = all.
    pub only_paths: Vec<PathBuf>,
}

impl Config {
    /// Locate the agent homes from the environment, honouring the container
    /// mount points first so the same binary works inside and outside Docker.
    pub fn discover() -> Self {
        let selections = discovery_selections();
        Self {
            claude_home: selections
                .iter()
                .find(|selection| selection.agent == Agent::Claude)
                .and_then(|selection| selection.path.is_dir().then_some(selection.path.clone())),
            codex_home: selections
                .iter()
                .find(|selection| selection.agent == Agent::Codex)
                .and_then(|selection| selection.path.is_dir().then_some(selection.path.clone())),
            only_paths: Vec::new(),
            active_within: DEFAULT_ACTIVE_WITHIN,
        }
    }
}

/// Build every collector that has something to read.
///
/// A missing agent home is not an error: plenty of people run only one of the
/// two agents.
pub fn sources(cfg: &Config) -> Vec<Box<dyn Source>> {
    let mut sources: Vec<Box<dyn Source>> = Vec::new();

    if let Some(home) = cfg.claude_home.as_deref() {
        if ClaudeSource::home_exists(home) {
            sources.push(Box::new(ClaudeSource::with_paths_and_active_within(
                home.to_path_buf(),
                cfg.only_paths.clone(),
                cfg.active_within,
            )));
        }
    }

    if let Some(home) = cfg.codex_home.as_deref() {
        if CodexSource::sqlite_exists(home) {
            sources.push(Box::new(CodexSource::with_paths_and_active_within(
                home.to_path_buf(),
                cfg.only_paths.clone(),
                cfg.active_within,
            )));
        }
    }

    sources
}

/// Inspect both configured agent homes using only filesystem metadata and
/// read-only SQLite connections. The result includes missing homes so a
/// setup check can explain what it looked for.
pub fn inspect(cfg: &Config, now: Millis) -> Vec<StoreReport> {
    let selections = discovery_selections();
    [
        (Agent::Claude, cfg.claude_home.as_ref()),
        (Agent::Codex, cfg.codex_home.as_ref()),
    ]
    .into_iter()
    .map(|(agent, configured)| {
        let discovered = selections
            .iter()
            .find(|selection| selection.agent == agent)
            .cloned()
            .unwrap_or_else(|| DiscoverySelection {
                agent,
                path: PathBuf::from(match agent {
                    Agent::Claude => "/data/claude",
                    Agent::Codex => "/data/codex",
                }),
                kind: DiscoveryKind::ContainerMount,
                candidates: Vec::new(),
            });
        let selection = configured.map_or_else(
            || discovered.clone(),
            |path| {
                if path == &discovered.path {
                    discovered.clone()
                } else {
                    DiscoverySelection {
                        agent,
                        path: path.clone(),
                        kind: DiscoveryKind::Configured,
                        candidates: vec![path.clone()],
                    }
                }
            },
        );
        let mut report = match agent {
            Agent::Claude => ClaudeSource::inspect_home(&selection.path, cfg.active_within, now),
            Agent::Codex => CodexSource::inspect_home(&selection.path, cfg.active_within, now),
        };
        report.candidates = selection.candidates;
        report.discovery = selection.kind;
        report
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "theywork-discovery-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock is before Unix epoch")
                    .as_nanos()
            ));
            fs::create_dir_all(&path).expect("create temporary directory");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn set_modified(path: &Path, millis: u64) {
        fs::File::open(path)
            .expect("open fixture path")
            .set_modified(UNIX_EPOCH + Duration::from_millis(millis))
            .expect("set fixture mtime");
    }

    fn codex_fixture(path: &Path, millis: u64) {
        let sqlite = path.join("sqlite");
        fs::create_dir_all(&sqlite).expect("create SQLite directory");
        let state = sqlite.join("state_5.sqlite");
        let history = sqlite.join("thread_history_1.sqlite");
        fs::write(&state, b"state").expect("write state marker");
        fs::write(&history, b"history").expect("write history marker");
        set_modified(path, millis);
        set_modified(&sqlite, millis);
        set_modified(&state, millis);
        set_modified(&history, millis);
    }

    #[test]
    fn recent_populated_candidate_wins_over_stale_candidate() {
        let temp = TempDir::new();
        let stale = temp.path.join("stale");
        let recent = temp.path.join("recent");
        fs::create_dir_all(&stale).expect("create stale home");
        fs::create_dir_all(&recent).expect("create recent home");
        codex_fixture(&stale, 1_000);
        codex_fixture(&recent, 2_000);

        let selection = select_candidate(DiscoveryPlan {
            agent: Agent::Codex,
            candidates: vec![
                DiscoveryCandidate {
                    path: stale,
                    kind: DiscoveryKind::Home,
                },
                DiscoveryCandidate {
                    path: recent.clone(),
                    kind: DiscoveryKind::WslCrossover,
                },
            ],
        });
        assert_eq!(selection.path, recent);
        assert_eq!(selection.kind, DiscoveryKind::WslCrossover);
    }

    #[test]
    fn windows_profile_spellings_map_to_wsl_mounts() {
        assert_eq!(
            windows_path_to_unix(std::ffi::OsStr::new(r"C:\Users\Example\.codex")),
            Some(PathBuf::from("/mnt/c/Users/Example/.codex"))
        );
        assert_eq!(
            windows_path_to_unix(std::ffi::OsStr::new(
                r"\\wsl.localhost\Ubuntu-22.04\home\dev\.claude"
            )),
            Some(PathBuf::from("/home/dev/.claude"))
        );
    }
}
