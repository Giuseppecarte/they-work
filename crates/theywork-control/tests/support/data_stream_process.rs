use super::*;
use theywork_control::{reconcile_snapshot, BridgeCursor};
use theywork_core::{SequenceRange, WaitReason, World};

#[test]
fn real_provider_bursts_preserve_exact_gaps_requests_and_restart_as_unknown_lineage() {
    for count in [300, 3000] {
        let mut fixture = Fixture::new();
        fixture.config.codex_args = vec![
            format!(
                "{}/tests/fake_stream_provider.py",
                env!("CARGO_MANIFEST_DIR")
            ),
            fixture.root.to_string_lossy().into_owned(),
        ];
        let client = fixture.start();
        client
            .start_codex(
                fixture.project(),
                count.to_string(),
                format!("start-{count}"),
            )
            .unwrap();
        let initial = wait_for(&client, |state| {
            state.events.last().is_some_and(|event| event.sequence == 1)
        });
        let cursor = reconcile_snapshot(&initial, &BridgeCursor::default()).next_cursor;
        assert_eq!(cursor.applied_sequence, 1);
        fs::write(fixture.root.join("burst.go"), b"release synthetic burst").unwrap();
        let burst = wait_for(&client, |state| {
            state
                .events
                .last()
                .is_some_and(|event| event.sequence == count + 1)
        });
        let batch = reconcile_snapshot(&burst, &cursor);
        assert_eq!(batch.continuity.missing_events, count - 256);
        assert_eq!(
            batch.continuity.missing_ranges,
            vec![SequenceRange {
                first: 2,
                last: count - 255
            }]
        );
        assert_eq!(burst.threads.len(), 2);
        assert_eq!(burst.pending_requests.len(), 1);
        let before = fixture.requests();
        let mut world = World::new();
        for event in batch.events {
            world.apply(event);
        }
        for _ in 0..5 {
            let duplicate = reconcile_snapshot(&burst, &batch.next_cursor);
            assert_eq!(duplicate.continuity, batch.continuity);
            for event in duplicate.events {
                world.apply(event);
            }
        }
        assert_eq!(
            fixture.requests(),
            before,
            "observation reconciliation must issue no provider requests"
        );
        assert_eq!(world.worker_count(), 2);
        let parent = world
            .worker(&burst.threads["managed-1"].identity.worker_id())
            .unwrap();
        assert_eq!(parent.wait_reason, Some(WaitReason::HumanApproval));
        // The fixture's incoming request is a sequenced ProviderStreamEvent.
        assert_eq!(
            burst.events.last().unwrap().method,
            "item/commandExecution/requestApproval"
        );
        assert_eq!(
            before
                .iter()
                .filter(|request| request["method"] == "turn/start")
                .count(),
            1
        );
        let stream_id = burst.event_window.as_ref().unwrap().stream_id.clone();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let saved: ControlSnapshot = serde_json::from_slice(
                &fs::read(fixture.config.state_dir.join("state.json")).unwrap(),
            )
            .unwrap();
            if saved
                .event_window
                .as_ref()
                .is_some_and(|window| window.last_assigned_sequence == count + 1)
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(25));
        }
        fixture.stop();
        let restarted = fixture.start().snapshot().unwrap();
        assert_ne!(restarted.generation, burst.generation);
        assert_ne!(
            restarted.event_window.as_ref().unwrap().stream_id,
            stream_id
        );
        assert!(
            restarted
                .event_window
                .as_ref()
                .unwrap()
                .prior_lineage_unknown
        );
        let after_restart = reconcile_snapshot(&restarted, &batch.next_cursor);
        assert_eq!(after_restart.continuity.missing_events, 0);
        assert!(after_restart.continuity.prior_stream_unknown);
        assert!(!after_restart.continuity.lineage_known);
        assert_eq!(after_restart.next_cursor.applied_sequence, count + 1);
        assert_eq!(
            fixture.requests(),
            before,
            "restart must not resume or repeat a user instruction"
        );
    }
}

#[test]
fn restart_from_stale_saved_tail_never_reuses_the_exposed_live_lineage() {
    let mut fixture = Fixture::new();
    fixture.config.codex_args = vec![
        format!(
            "{}/tests/fake_stream_provider.py",
            env!("CARGO_MANIFEST_DIR")
        ),
        fixture.root.to_string_lossy().into_owned(),
    ];
    let client = fixture.start();
    client
        .start_codex(fixture.project(), "300", "one-start")
        .unwrap();
    let initial = wait_for(&client, |state| {
        state.events.last().is_some_and(|event| event.sequence == 1)
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    let stale_bytes = loop {
        let bytes = fs::read(fixture.config.state_dir.join("state.json")).unwrap();
        let saved: ControlSnapshot = serde_json::from_slice(&bytes).unwrap();
        if saved
            .event_window
            .as_ref()
            .is_some_and(|window| window.last_assigned_sequence == 1)
        {
            break bytes;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    };
    let cursor = reconcile_snapshot(&initial, &BridgeCursor::default()).next_cursor;
    fs::write(fixture.root.join("burst.go"), b"go").unwrap();
    let exposed = wait_for(&client, |state| {
        state
            .events
            .last()
            .is_some_and(|event| event.sequence == 301)
    });
    let cursor = reconcile_snapshot(&exposed, &cursor).next_cursor;
    let before = fixture.requests();
    fixture.stop();
    // Reproduce the state of disk after a crash before its next batched save.
    fs::write(fixture.config.state_dir.join("state.json"), stale_bytes).unwrap();
    let recovered = fixture.start().snapshot().unwrap();
    assert_eq!(
        recovered
            .event_window
            .as_ref()
            .unwrap()
            .last_assigned_sequence,
        1
    );
    assert_ne!(
        recovered.event_window.as_ref().unwrap().stream_id,
        exposed.event_window.as_ref().unwrap().stream_id
    );
    let batch = reconcile_snapshot(&recovered, &cursor);
    assert_eq!(batch.next_cursor.applied_sequence, 1);
    assert!(batch.continuity.prior_stream_unknown);
    assert!(!batch.continuity.lineage_known);
    assert_eq!(
        batch.continuity.missing_events, 0,
        "the exact lost unsaved tail is not recoverable from stale disk"
    );
    assert!(recovered.pending_requests.is_empty());
    assert_eq!(
        fixture.requests(),
        before,
        "recovery must not launch or replay provider work"
    );
}
