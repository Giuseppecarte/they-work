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

use theywork_core::Source;

/// Default roster horizon. The world still evicts workers sooner when their
/// last actual event is old; this bound keeps a first SQLite scan small.
pub const DEFAULT_ACTIVE_WITHIN: Duration = Duration::from_secs(6 * 60 * 60);

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
        let pick = |env: &str, mount: &str, home_rel: &str| -> Option<PathBuf> {
            if let Ok(p) = std::env::var(env) {
                let p = PathBuf::from(p);
                if p.exists() {
                    return Some(p);
                }
            }
            let mounted = PathBuf::from(mount);
            if mounted.exists() {
                return Some(mounted);
            }
            let home = PathBuf::from(std::env::var("HOME").ok()?).join(home_rel);
            home.exists().then_some(home)
        };
        Self {
            claude_home: pick("THEYWORK_CLAUDE_HOME", "/data/claude", ".claude"),
            codex_home: pick("THEYWORK_CODEX_HOME", "/data/codex", ".codex"),
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
