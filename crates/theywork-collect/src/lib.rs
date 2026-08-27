//! Read-only collectors.
//!
//! Every collector in this crate opens files for reading and nothing else. No
//! writes, no network, no spawning. That guarantee is the whole security story
//! of they-work, so keep it true.
//!
//! Owner: collectors dev. Do not edit `theywork-core` or `theywork-render`
//! from here; if the contract needs to change, raise it rather than editing it.

use std::path::PathBuf;

use theywork_core::Source;

/// Where to look for agent trails.
#[derive(Debug, Clone)]
pub struct Config {
    /// Usually `~/.claude`.
    pub claude_home: Option<PathBuf>,
    /// Usually `~/.codex`.
    pub codex_home: Option<PathBuf>,
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
        }
    }
}

/// Build every collector that has something to read.
///
/// A missing agent home is not an error: plenty of people run only one of the
/// two agents.
pub fn sources(_cfg: &Config) -> Vec<Box<dyn Source>> {
    // TODO(collectors dev): return the Claude and Codex sources here.
    Vec::new()
}
