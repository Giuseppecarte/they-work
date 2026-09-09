//! Durable sequence lineage, separate from supervisor authority generations.

use crate::{ControlEvent, ControlSnapshot, EventWindow};

pub(crate) const EVENT_WINDOW_LIMIT: usize = 256;

pub(crate) fn valid_window(snapshot: &ControlSnapshot) -> bool {
    let Some(window) = &snapshot.event_window else {
        return false;
    };
    window.version == EventWindow::VERSION
        && !window.stream_id.is_empty()
        && window.first_retained_sequence == snapshot.events.first().map(|event| event.sequence)
        && snapshot.events.len() <= EVENT_WINDOW_LIMIT
        && snapshot
            .events
            .first()
            .is_none_or(|event| event.sequence > 0)
        && snapshot
            .events
            .windows(2)
            .all(|pair| pair[0].sequence.checked_add(1) == Some(pair[1].sequence))
        && snapshot
            .events
            .last()
            .is_none_or(|event| event.sequence == window.last_assigned_sequence)
}

/// Old snapshots remain readable; their recorded facts are kept, but their
/// previous sequence lineage cannot be presented as known continuity.
pub(crate) fn initialize_event_window(
    snapshot: &mut ControlSnapshot,
    existing_state: bool,
) -> anyhow::Result<()> {
    if valid_window(snapshot) {
        if existing_state {
            // Live snapshots can expose events before the batched state save.
            // A restarted host cannot prove which tail its clients observed,
            // so disk recovery must not reuse that live sequence lineage.
            let stream_id = crate::security::random_token()?;
            let window = snapshot.event_window.as_mut().expect("valid window");
            window.stream_id = stream_id;
            window.prior_lineage_unknown = true;
        }
        return Ok(());
    }
    let stream_id = crate::security::random_token()?;
    // Repair malformed/oversized legacy windows without discarding semantic
    // facts merely because their old sequence IDs cannot establish continuity.
    if snapshot.events.len() > EVENT_WINDOW_LIMIT {
        snapshot
            .events
            .drain(..snapshot.events.len() - EVENT_WINDOW_LIMIT);
    }
    if snapshot
        .events
        .first()
        .is_some_and(|event| event.sequence == 0)
        || !snapshot
            .events
            .windows(2)
            .all(|pair| pair[0].sequence.checked_add(1) == Some(pair[1].sequence))
    {
        for (index, event) in snapshot.events.iter_mut().enumerate() {
            event.sequence = index as u64 + 1;
        }
    }
    snapshot.event_window = Some(EventWindow {
        version: EventWindow::VERSION,
        stream_id,
        first_retained_sequence: snapshot.events.first().map(|event| event.sequence),
        last_assigned_sequence: snapshot.events.last().map_or(0, |event| event.sequence),
        prior_lineage_unknown: existing_state,
    });
    Ok(())
}

pub(crate) fn append_event(
    snapshot: &mut ControlSnapshot,
    mut event: ControlEvent,
) -> anyhow::Result<()> {
    if !valid_window(snapshot) {
        initialize_event_window(snapshot, !snapshot.events.is_empty())?;
    }
    let window = snapshot.event_window.as_mut().expect("window initialized");
    if window.last_assigned_sequence == u64::MAX {
        // Overflow starts a different lineage. Pending requests/authority and
        // the operation ledger remain unchanged; continuity becomes unknown.
        window.stream_id = crate::security::random_token()?;
        window.prior_lineage_unknown = true;
        window.last_assigned_sequence = 0;
        snapshot.events.clear();
    }
    window.last_assigned_sequence += 1;
    event.sequence = window.last_assigned_sequence;
    snapshot.events.push(event);
    if snapshot.events.len() > EVENT_WINDOW_LIMIT {
        snapshot.events.remove(0);
    }
    window.first_retained_sequence = snapshot.events.first().map(|event| event.sequence);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn observation() -> ControlEvent {
        ControlEvent {
            sequence: 0,
            at: 1,
            method: "fixture".into(),
            thread_id: None,
            turn_id: None,
            item_id: None,
            params: json!({}),
        }
    }

    #[test]
    fn restart_rotates_lineage_but_live_empty_windows_keep_their_high_water_mark() {
        let mut snapshot = ControlSnapshot::default();
        initialize_event_window(&mut snapshot, false).unwrap();
        let id = snapshot.event_window.as_ref().unwrap().stream_id.clone();
        for _ in 0..300 {
            append_event(&mut snapshot, observation()).unwrap();
        }
        assert_eq!(snapshot.events.len(), 256);
        assert_eq!(snapshot.events[0].sequence, 45);
        let encoded = serde_json::to_vec(&snapshot).unwrap();
        let mut restored: ControlSnapshot = serde_json::from_slice(&encoded).unwrap();
        restored.generation = "new process".into();
        initialize_event_window(&mut restored, true).unwrap();
        assert_ne!(restored.event_window.as_ref().unwrap().stream_id, id);
        assert!(
            restored
                .event_window
                .as_ref()
                .unwrap()
                .prior_lineage_unknown
        );
        let restarted_id = restored.event_window.as_ref().unwrap().stream_id.clone();
        restored.events.clear();
        restored
            .event_window
            .as_mut()
            .unwrap()
            .first_retained_sequence = None;
        append_event(&mut restored, observation()).unwrap();
        assert_eq!(restored.events[0].sequence, 301);
        assert_eq!(
            restored.event_window.as_ref().unwrap().stream_id,
            restarted_id
        );
    }

    #[test]
    fn legacy_and_repaired_sequences_never_claim_known_prior_lineage() {
        let mut snapshot: ControlSnapshot = serde_json::from_value(json!({})).unwrap();
        let mut event = observation();
        event.sequence = 7;
        snapshot.events.push(event.clone());
        initialize_event_window(&mut snapshot, true).unwrap();
        assert!(
            snapshot
                .event_window
                .as_ref()
                .unwrap()
                .prior_lineage_unknown
        );
        assert_eq!(snapshot.events[0].sequence, 7);
        let first_id = snapshot.event_window.as_ref().unwrap().stream_id.clone();
        snapshot.events.push(event);
        initialize_event_window(&mut snapshot, true).unwrap();
        assert_ne!(snapshot.event_window.as_ref().unwrap().stream_id, first_id);
        assert_eq!(
            snapshot
                .events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn sequence_exhaustion_rotates_lineage_without_reusing_a_sequence() {
        let mut snapshot = ControlSnapshot::default();
        initialize_event_window(&mut snapshot, false).unwrap();
        let id = snapshot.event_window.as_ref().unwrap().stream_id.clone();
        snapshot
            .event_window
            .as_mut()
            .unwrap()
            .last_assigned_sequence = u64::MAX;
        append_event(&mut snapshot, observation()).unwrap();
        assert_ne!(snapshot.event_window.as_ref().unwrap().stream_id, id);
        assert!(
            snapshot
                .event_window
                .as_ref()
                .unwrap()
                .prior_lineage_unknown
        );
        assert_eq!(snapshot.events[0].sequence, 1);
    }
}
