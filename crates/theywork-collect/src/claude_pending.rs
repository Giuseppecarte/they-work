//! Bounded observer-only tool correlation. Native identifiers are never shortened.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};

use theywork_core::{Activity, Millis, ToolCorrelationCoverage};

use super::PendingTool;

pub(super) const CURSOR_ENTRIES: usize = 256;
pub(super) const CURSOR_BYTES: usize = 256 * 1024;
pub(super) const SOURCE_ENTRIES: usize = 8_192;
pub(super) const SOURCE_BYTES: usize = 8 * 1024 * 1024;

static NEXT_CURSOR: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub(super) struct PendingStore {
    cursors: BTreeMap<u64, CursorTools>,
    // The order index stores only fixed-size tokens: no second native-ID copy,
    // stale FIFO nodes, or tombstones that outlive the bounded entries.
    order: BTreeMap<u64, u64>,
    next_ordinal: u64,
    owned_bytes: usize,
}

pub(super) struct PendingScope<'a> {
    pub(super) store: &'a mut PendingStore,
    pub(super) cursor: u64,
}

impl PendingScope<'_> {
    pub(super) fn get(&self, id: &str) -> Option<&PendingTool> {
        self.store.get(self.cursor, id)
    }
    pub(super) fn remove(&mut self, id: &str) -> Option<PendingTool> {
        self.store.remove(self.cursor, id)
    }
    pub(super) fn insert(&mut self, id: &str, tool: PendingTool) {
        self.store.insert(self.cursor, id, tool);
    }
}

#[derive(Default)]
struct CursorTools {
    entries: HashMap<Box<str>, Entry>,
    owned_bytes: usize,
    coverage: ToolCorrelationCoverage,
}

struct Entry {
    ordinal: u64,
    // A conflicting ID stays ambiguous until its result or ordinary retirement.
    tool: Option<PendingTool>,
    owned_bytes: usize,
}

impl CursorTools {
    fn compact(&mut self) {
        if self.entries.is_empty() {
            self.entries = HashMap::new();
        } else if self.entries.capacity() > self.entries.len().saturating_mul(4) {
            self.entries.shrink_to(self.entries.len().saturating_mul(2));
        }
    }
}

impl PendingStore {
    pub(super) fn register(&mut self) -> u64 {
        let token = NEXT_CURSOR.fetch_add(1, Ordering::Relaxed);
        assert_ne!(token, 0, "correlation cursor identifier exhausted");
        self.cursors.insert(
            token,
            CursorTools {
                coverage: ToolCorrelationCoverage {
                    epoch: token,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        token
    }

    pub(super) fn unregister(&mut self, cursor: u64) {
        if let Some(state) = self.cursors.remove(&cursor) {
            self.owned_bytes -= state.owned_bytes;
            for entry in state.entries.into_values() {
                self.order.remove(&entry.ordinal);
            }
        }
    }

    pub(super) fn restart(&mut self, cursor: u64) {
        self.unregister(cursor);
        let epoch = NEXT_CURSOR.fetch_add(1, Ordering::Relaxed);
        assert_ne!(epoch, 0, "correlation epoch exhausted");
        self.cursors.insert(
            cursor,
            CursorTools {
                coverage: ToolCorrelationCoverage {
                    epoch,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
    }

    pub(super) fn coverage(&self, cursor: u64, now: Millis) -> Option<ToolCorrelationCoverage> {
        self.cursors.get(&cursor).map(|state| {
            let mut coverage = state.coverage.clone();
            coverage.observed_at = now;
            coverage
        })
    }

    pub(super) fn reset_coverage(
        &mut self,
        cursor: u64,
        now: Millis,
    ) -> Option<ToolCorrelationCoverage> {
        let state = self.cursors.get_mut(&cursor)?;
        state.coverage.reset_discarded = state
            .coverage
            .reset_discarded
            .saturating_add(state.entries.len() as u64);
        self.coverage(cursor, now)
    }

    pub(super) fn get(&self, cursor: u64, id: &str) -> Option<&PendingTool> {
        self.cursors.get(&cursor)?.entries.get(id)?.tool.as_ref()
    }

    pub(super) fn remove(&mut self, cursor: u64, id: &str) -> Option<PendingTool> {
        let state = self.cursors.get_mut(&cursor)?;
        let entry = state.entries.remove(id)?;
        state.owned_bytes -= entry.owned_bytes;
        self.owned_bytes -= entry.owned_bytes;
        self.order.remove(&entry.ordinal);
        state.compact();
        entry.tool
    }

    pub(super) fn insert(&mut self, cursor: u64, id: &str, tool: PendingTool) {
        let Some(state) = self.cursors.get_mut(&cursor) else {
            return;
        };
        if let Some(entry) = state.entries.get_mut(id) {
            if entry.tool.as_ref().is_some_and(|old| old != &tool) {
                let released = entry
                    .tool
                    .take()
                    .map_or(0, |old| activity_capacity(&old.activity));
                entry.owned_bytes -= released;
                state.owned_bytes -= released;
                self.owned_bytes -= released;
                state.coverage.ambiguous = state.coverage.ambiguous.saturating_add(1);
            }
            // Neither identical replay nor conflicting replay refreshes its age.
            return;
        }
        let owned_bytes = id.len().saturating_add(activity_capacity(&tool.activity));
        if owned_bytes > CURSOR_BYTES || owned_bytes > SOURCE_BYTES {
            state.coverage.oversized = state.coverage.oversized.saturating_add(1);
            return;
        }
        while self.cursors.get(&cursor).is_some_and(|state| {
            state.entries.len() >= CURSOR_ENTRIES || state.owned_bytes + owned_bytes > CURSOR_BYTES
        }) {
            let oldest = self.cursors[&cursor]
                .entries
                .values()
                .map(|entry| entry.ordinal)
                .min()
                .unwrap();
            self.evict(oldest);
        }
        while self.order.len() >= SOURCE_ENTRIES || self.owned_bytes + owned_bytes > SOURCE_BYTES {
            let oldest = *self.order.first_key_value().unwrap().0;
            self.evict(oldest);
        }
        if self.next_ordinal == u64::MAX {
            self.rebase_ordinals();
        }
        let ordinal = self.next_ordinal;
        self.next_ordinal += 1;
        let state = self.cursors.get_mut(&cursor).unwrap();
        state.entries.insert(
            id.into(),
            Entry {
                ordinal,
                tool: Some(tool),
                owned_bytes,
            },
        );
        state.owned_bytes += owned_bytes;
        self.owned_bytes += owned_bytes;
        self.order.insert(ordinal, cursor);
    }

    fn evict(&mut self, ordinal: u64) {
        let cursor = self.order.remove(&ordinal).unwrap();
        let state = self.cursors.get_mut(&cursor).unwrap();
        let (_, entry) = state
            .entries
            .extract_if(|_, entry| entry.ordinal == ordinal)
            .next()
            .unwrap();
        state.owned_bytes -= entry.owned_bytes;
        self.owned_bytes -= entry.owned_bytes;
        state.coverage.evicted = state.coverage.evicted.saturating_add(1);
        state.compact();
    }

    fn rebase_ordinals(&mut self) {
        let old = std::mem::take(&mut self.order);
        for (ordinal, (previous, cursor)) in old.into_iter().enumerate() {
            let entry = self
                .cursors
                .get_mut(&cursor)
                .unwrap()
                .entries
                .values_mut()
                .find(|entry| entry.ordinal == previous)
                .unwrap();
            entry.ordinal = ordinal as u64;
            self.order.insert(ordinal as u64, cursor);
        }
        self.next_ordinal = self.order.len() as u64;
    }

    #[cfg(test)]
    pub(super) fn stats(&self) -> serde_json::Value {
        serde_json::json!({
            "entries":self.order.len(), "owned_string_bytes":self.owned_bytes,
            "lookup_capacity":self.cursors.values().map(|c|c.entries.capacity()).sum::<usize>(),
            "order_entries":self.order.len(), "cursors":self.cursors.len(),
            "evicted":self.cursors.values().fold(0_u64,|n,c|n.saturating_add(c.coverage.evicted)),
            "oversized":self.cursors.values().fold(0_u64,|n,c|n.saturating_add(c.coverage.oversized)),
            "ambiguous":self.cursors.values().fold(0_u64,|n,c|n.saturating_add(c.coverage.ambiguous))
        })
    }
}

fn activity_capacity(activity: &Activity) -> usize {
    match activity {
        Activity::Typing { detail }
        | Activity::Reading { detail }
        | Activity::Editing { detail }
        | Activity::Searching { detail }
        | Activity::Talking { detail }
        | Activity::Waiting { detail }
        | Activity::Error { detail } => detail.capacity(),
        Activity::Thinking | Activity::Idle => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::PendingToolKind;

    fn tool(sequence: u64) -> PendingTool {
        PendingTool {
            kind: PendingToolKind::Command,
            activity: Activity::Typing {
                detail: format!("command-{sequence}"),
            },
            input_counts: None,
            background: false,
            fingerprint: sequence,
        }
    }

    fn assert_accounting(store: &PendingStore) {
        let entries: usize = store.cursors.values().map(|c| c.entries.len()).sum();
        let bytes: usize = store.cursors.values().map(|c| c.owned_bytes).sum();
        assert_eq!(store.order.len(), entries);
        assert_eq!(store.owned_bytes, bytes);
        assert!(entries <= SOURCE_ENTRIES && bytes <= SOURCE_BYTES);
        for (cursor, state) in &store.cursors {
            assert!(state.entries.len() <= CURSOR_ENTRIES && state.owned_bytes <= CURSOR_BYTES);
            assert!(state.entries.capacity() <= state.entries.len() * 4);
            assert_eq!(
                state.owned_bytes,
                state
                    .entries
                    .iter()
                    .map(|(id, entry)| {
                        assert_eq!(store.order.get(&entry.ordinal), Some(cursor));
                        assert_eq!(
                            entry.owned_bytes,
                            id.len()
                                + entry
                                    .tool
                                    .as_ref()
                                    .map_or(0, |tool| activity_capacity(&tool.activity))
                        );
                        entry.owned_bytes
                    })
                    .sum::<usize>()
            );
        }
    }

    #[test]
    fn count_limits_retire_oldest_observations_without_refreshing_duplicate_age() {
        let mut store = PendingStore::default();
        let cursor = store.register();
        for seq in 0..CURSOR_ENTRIES as u64 {
            store.insert(cursor, &format!("{seq}"), tool(seq));
        }
        store.insert(cursor, "0", tool(0));
        store.insert(cursor, "new", tool(999));
        assert!(store.get(cursor, "0").is_none());
        assert!(store.get(cursor, "1").is_some());
        assert_eq!(store.coverage(cursor, 100).unwrap().evicted, 1);
        assert_accounting(&store);
    }

    #[test]
    fn byte_limits_count_full_ids_utf8_capacity_and_oversize_rejection() {
        let mut store = PendingStore::default();
        let cursor = store.register();
        let first = "a".repeat(CURSOR_BYTES / 2);
        let second = "界".repeat(CURSOR_BYTES / 6);
        let mut unicode = tool(1);
        unicode.activity = Activity::Editing {
            detail: "界".repeat(120),
        };
        store.insert(cursor, &first, unicode.clone());
        store.insert(cursor, &second, unicode);
        assert!(store.get(cursor, &first).is_none());
        assert!(store.get(cursor, &second).is_some());
        store.insert(cursor, &"x".repeat(CURSOR_BYTES + 1), tool(2));
        assert_eq!(store.coverage(cursor, 1).unwrap().oversized, 1);
        assert_eq!(store.coverage(cursor, 1).unwrap().evicted, 1);
        assert_accounting(&store);
    }

    #[test]
    fn aggregate_count_and_byte_pressure_cannot_be_bypassed_with_more_cursors() {
        for long_ids in [false, true] {
            let mut store = PendingStore::default();
            let cursors: Vec<_> = (0..50).map(|_| store.register()).collect();
            for seq in 0..256 {
                for cursor in &cursors {
                    let id = if long_ids {
                        format!("{seq}-{}", "x".repeat(32 * 1024))
                    } else {
                        seq.to_string()
                    };
                    store.insert(*cursor, &id, tool(seq));
                    assert!(
                        store.order.len() <= SOURCE_ENTRIES && store.owned_bytes <= SOURCE_BYTES
                    );
                }
                assert_accounting(&store);
            }
            assert!(
                store
                    .cursors
                    .values()
                    .map(|c| c.coverage.evicted)
                    .sum::<u64>()
                    > 0
            );
            if !long_ids {
                assert_eq!(store.order.len(), SOURCE_ENTRIES);
            }
            for cursor in cursors {
                store.unregister(cursor);
            }
            assert!(store.cursors.is_empty() && store.order.is_empty());
            assert_eq!(store.owned_bytes, 0);
        }
    }

    #[test]
    fn conflicts_stay_ambiguous_and_complete_removal_frees_all_indices() {
        let mut store = PendingStore::default();
        let cursor = store.register();
        store.insert(cursor, "same", tool(1));
        let initial_bytes = store.owned_bytes;
        store.insert(cursor, "same", tool(1));
        assert_eq!(store.owned_bytes, initial_bytes);
        store.insert(cursor, "same", tool(2));
        store.insert(cursor, "same", tool(3));
        assert!(store.get(cursor, "same").is_none());
        assert_eq!(store.coverage(cursor, 1).unwrap().ambiguous, 1);
        assert_eq!(store.owned_bytes, 4);
        assert_accounting(&store);
        assert!(store.remove(cursor, "same").is_none());
        assert_accounting(&store);
        assert_eq!(store.cursors[&cursor].entries.capacity(), 0);
    }

    #[test]
    fn ordinal_wrap_keeps_retirement_order_and_reset_releases_accounting() {
        let mut store = PendingStore::default();
        let cursor = store.register();
        store.insert(cursor, "old", tool(1));
        store.next_ordinal = u64::MAX;
        store.insert(cursor, "new", tool(2));
        assert_eq!(store.next_ordinal, 2);
        assert_eq!(store.cursors[&cursor].entries["old"].ordinal, 0);
        assert_eq!(store.cursors[&cursor].entries["new"].ordinal, 1);
        let before = store.reset_coverage(cursor, 5).unwrap();
        assert_eq!(before.reset_discarded, 2);
        store.restart(cursor);
        assert_ne!(store.coverage(cursor, 6).unwrap().epoch, before.epoch);
        assert_eq!(store.cursors[&cursor].entries.capacity(), 0);
        assert_accounting(&store);
    }
}
