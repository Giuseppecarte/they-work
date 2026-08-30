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

use std::path::PathBuf;
use std::time::Duration;

pub use claude::ClaudeSource;
pub use codex::CodexSource;
pub use util::normalize_office_path;

use theywork_core::{Agent, Millis, Source};

/// Default roster horizon. The world still evicts workers sooner when their
/// last actual event is old; this bound keeps a first SQLite scan small.
pub const DEFAULT_ACTIVE_WITHIN: Duration = Duration::from_secs(6 * 60 * 60);

/// A read-only summary of one agent's discovered store.
///
/// This is deliberately separate from [`Source`]: the doctor command needs to
/// explain a missing or unreadable home without constructing a live poller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreReport {
    pub agent: Agent,
    pub path: PathBuf,
    pub home_found: bool,
    pub readable: bool,
    pub projects: usize,
    pub threads: usize,
    pub active_threads: usize,
    pub error: Option<String>,
}

impl StoreReport {
    fn new(agent: Agent, path: PathBuf) -> Self {
        Self {
            agent,
            path,
            home_found: false,
            readable: false,
            projects: 0,
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
    vec![
        (
            Agent::Claude,
            discovery_path("THEYWORK_CLAUDE_HOME", "/data/claude", ".claude"),
        ),
        (
            Agent::Codex,
            discovery_path("THEYWORK_CODEX_HOME", "/data/codex", ".codex"),
        ),
    ]
}

fn discovery_path(env: &str, mount: &str, home_rel: &str) -> PathBuf {
    if let Ok(value) = std::env::var(env) {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }
    let mounted = PathBuf::from(mount);
    if mounted.exists() {
        return mounted;
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(home_rel))
        .unwrap_or(mounted)
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
        let paths = discovery_paths();
        Self {
            claude_home: paths
                .iter()
                .find(|(agent, _)| *agent == Agent::Claude)
                .and_then(|(_, path)| path.is_dir().then_some(path.clone())),
            codex_home: paths
                .iter()
                .find(|(agent, _)| *agent == Agent::Codex)
                .and_then(|(_, path)| path.is_dir().then_some(path.clone())),
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
    let candidates = discovery_paths();
    let claude_home = cfg.claude_home.clone().or_else(|| {
        candidates
            .iter()
            .find(|(agent, _)| *agent == Agent::Claude)
            .map(|(_, path)| path.clone())
    });
    let codex_home = cfg.codex_home.clone().or_else(|| {
        candidates
            .iter()
            .find(|(agent, _)| *agent == Agent::Codex)
            .map(|(_, path)| path.clone())
    });

    vec![
        claude_home.map_or_else(
            || StoreReport::new(Agent::Claude, PathBuf::from("/data/claude")),
            |home| ClaudeSource::inspect_home(&home, cfg.active_within, now),
        ),
        codex_home.map_or_else(
            || StoreReport::new(Agent::Codex, PathBuf::from("/data/codex")),
            |home| CodexSource::inspect_home(&home, cfg.active_within, now),
        ),
    ]
}
