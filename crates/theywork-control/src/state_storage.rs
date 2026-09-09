use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};

use crate::{security, ControlSnapshot};

const RECOVERY: &str = "Control storage needs recovery: the established state is missing or unreadable. Restore the canonical state file and restart the host; nothing was resent";

#[derive(Default)]
struct StorageStatus {
    initialized: bool,
    fault: Option<String>,
}

/// Shared by admission and provider-event persistence. A failed publication
/// cannot create an unbounded series of candidates or erase recovery evidence.
pub(crate) struct StateStorage {
    directory: PathBuf,
    status: Mutex<StorageStatus>,
}

impl StateStorage {
    pub(crate) fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            status: Mutex::new(StorageStatus::default()),
        }
    }

    pub(crate) fn load(&self) -> Result<Option<ControlSnapshot>> {
        let mut status = self.status.lock().unwrap();
        let snapshot = Self::read_snapshot(&self.directory)?;
        status.initialized = snapshot.is_some();
        Ok(snapshot)
    }

    pub(crate) fn read_snapshot(directory: &Path) -> Result<Option<ControlSnapshot>> {
        let path = directory.join("state.json");
        match security::read_private_optional(&path).context(RECOVERY)? {
            Some(bytes) => Ok(Some(decode_snapshot(&bytes).context(RECOVERY)?)),
            None => {
                // The endpoint is first published after state initialization;
                // its presence proves this was not an unused control directory.
                anyhow::ensure!(
                    !security::private_file_exists(&directory.join("endpoint.json"))
                        .context(RECOVERY)?,
                    RECOVERY
                );
                Ok(None)
            }
        }
    }

    pub(crate) fn write(&self, bytes: &[u8]) -> Result<()> {
        self.write_with(bytes, |path, bytes| {
            security::write_private_checked(path, bytes, valid_snapshot)
        })
    }

    pub(crate) fn fault(&self) -> Option<String> {
        self.status.lock().unwrap().fault.clone()
    }

    /// Explicit provider operations check storage before contacting a process.
    /// Routine snapshot polling can read the already-latched fault cheaply.
    pub(crate) fn check_health(&self) -> Result<()> {
        let mut status = self.status.lock().unwrap();
        if let Some(fault) = &status.fault {
            bail!("{fault}");
        }
        let result = Self::read_snapshot(&self.directory).and_then(|snapshot| {
            anyhow::ensure!(snapshot.is_some() || !status.initialized, RECOVERY);
            Ok(())
        });
        if let Err(error) = &result {
            status.fault = Some(format!("{error:#}"));
        }
        result
    }

    fn write_with(
        &self,
        bytes: &[u8],
        writer: impl FnOnce(&Path, &[u8]) -> Result<()>,
    ) -> Result<()> {
        let mut status = self.status.lock().unwrap();
        if let Some(fault) = &status.fault {
            bail!("{fault}");
        }
        let path = self.directory.join("state.json");
        let accessible = security::private_file_exists(&path);
        if matches!(accessible, Ok(false)) && !status.initialized {
            // Only a fresh host can create its first state. An endpoint on
            // disk still prevents a stale writer from recreating lost state.
            if let Err(error) = Self::read_snapshot(&self.directory) {
                status.fault = Some(format!("{error:#}"));
                return Err(error);
            }
        } else if !matches!(accessible, Ok(true)) {
            let detail = accessible
                .err()
                .map(|error| format!(": {error:#}"))
                .unwrap_or_default();
            let message = format!("{RECOVERY}{detail}");
            status.fault = Some(message.clone());
            bail!("{message}");
        }
        match writer(&path, bytes) {
            Ok(()) => {
                status.initialized = true;
                Ok(())
            }
            Err(error) => {
                // A valid canonical snapshot is the only proof that another
                // write is safe. Never promote an older or orphan file.
                if !security::read_private(&path)
                    .map(|saved| valid_snapshot(&saved))
                    .unwrap_or(false)
                {
                    status.fault = Some(format!("{RECOVERY}: {error:#}"));
                }
                Err(error)
            }
        }
    }
}

fn valid_snapshot(bytes: &[u8]) -> bool {
    decode_snapshot(bytes).is_ok()
}

fn decode_snapshot(bytes: &[u8]) -> Result<ControlSnapshot> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    for key in ["generation", "codex_home", "threads", "operations"] {
        anyhow::ensure!(
            value.get(key).is_some(),
            "Saved control state is missing {key}"
        );
    }
    let mut snapshot: ControlSnapshot = serde_json::from_value(value)?;
    // This is a live response flag, never a durable recovery instruction.
    snapshot.storage_recovery_required = false;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "theywork-state-{}",
                security::random_token().unwrap()
            ));
            security::private_dir(&root).unwrap();
            Self(root)
        }
        fn storage(&self) -> StateStorage {
            StateStorage::new(self.0.clone())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn fresh_state_initializes_but_missing_established_state_never_resets() {
        let fixture = Fixture::new();
        let store = fixture.storage();
        assert!(store.load().unwrap().is_none());
        let bytes = serde_json::to_vec(&ControlSnapshot::default()).unwrap();
        store.write(&bytes).unwrap();
        security::write_private(&fixture.0.join("endpoint.json"), b"{}").unwrap();
        fs::remove_file(fixture.0.join("state.json")).unwrap();
        assert!(store
            .write(&bytes)
            .unwrap_err()
            .to_string()
            .contains("recovery"));
        assert!(fixture.storage().load().is_err());
        assert!(!fixture.0.join("state.json").exists());
    }

    #[test]
    fn ambiguous_publication_retains_one_candidate_and_blocks_every_writer() {
        let fixture = Fixture::new();
        let store = fixture.storage();
        let bytes = serde_json::to_vec(&ControlSnapshot::default()).unwrap();
        store.write(&bytes).unwrap();
        security::write_private(&fixture.0.join("endpoint.json"), b"{}").unwrap();
        let error = store
            .write_with(&bytes, |path, bytes| {
                security::write_private_with_publisher(path, bytes, valid_snapshot, |_tmp, path| {
                    fs::remove_file(path)?;
                    bail!("injected native publication failure")
                })
            })
            .unwrap_err();
        assert!(format!("{error:#}").contains("staged candidate"));
        let candidates: Vec<_> = fs::read_dir(&fixture.0)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "tmp"))
            .collect();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            security::read_private(&candidates[0].path()).unwrap(),
            bytes
        );
        let calls = AtomicUsize::new(0);
        for _ in 0..300 {
            assert!(store
                .write_with(&bytes, |_, _| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .is_err());
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 2);
        assert!(fixture.storage().load().is_err());
        assert!(candidates[0].path().exists());
    }

    #[test]
    fn failed_replace_with_valid_state_can_retry_without_leaving_candidates() {
        let fixture = Fixture::new();
        let store = fixture.storage();
        let bytes = serde_json::to_vec(&ControlSnapshot::default()).unwrap();
        store.write(&bytes).unwrap();
        assert!(store
            .write_with(&bytes, |path, bytes| {
                security::write_private_with_publisher(path, bytes, valid_snapshot, |_, _| {
                    bail!("injected sharing violation")
                })
            })
            .is_err());
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
        store.write(&bytes).unwrap();
        assert_eq!(
            security::read_private(&fixture.0.join("state.json")).unwrap(),
            bytes
        );
    }

    #[test]
    fn invalid_canonical_state_fails_without_selecting_a_valid_orphan() {
        let fixture = Fixture::new();
        security::write_private(&fixture.0.join("state.json"), b"broken").unwrap();
        security::write_private(
            &fixture.0.join("state.orphan.tmp"),
            &serde_json::to_vec(&ControlSnapshot::default()).unwrap(),
        )
        .unwrap();
        assert!(fixture.storage().load().is_err());
        assert_eq!(fs::read(fixture.0.join("state.json")).unwrap(), b"broken");
        security::write_private(&fixture.0.join("state.json"), b"{}").unwrap();
        assert!(fixture.storage().load().is_err());
    }
}
