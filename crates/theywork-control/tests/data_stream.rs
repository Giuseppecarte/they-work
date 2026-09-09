use std::collections::BTreeMap;

use serde_json::json;
use theywork_control::{
    reconcile_snapshot, BridgeCursor, Capabilities, ControlEvent, ControlSnapshot, EventWindow,
    ManagedThread, PendingRequest, DEFERRED_EVENT_BYTE_LIMIT, DEFERRED_EVENT_LIMIT,
};
use theywork_core::{
    Activity, Agent, CollaborationKind, Event, EventKind, OfficeId, SequenceRange, SourceId,
    ThreadIdentity, WaitReason, WorkerLifecycle, WorkerRole, World,
};

fn worker(native: &str) -> ManagedThread {
    ManagedThread {
        identity: ThreadIdentity::new(Agent::Codex, SourceId("/fixture/home".into()), native),
        role: WorkerRole::Main,
        managed: native == "parent",
        project: "/fixture/project".into(),
        title: native.into(),
        active_turn_id: Some("current-turn".into()),
        status: "working".into(),
        latest_text: String::new(),
        capabilities: Capabilities::default(),
        updated_at: 500,
    }
}

fn observation(sequence: u64, actor: Option<&str>) -> ControlEvent {
    ControlEvent {
        sequence,
        at: sequence as i64,
        method: "item/completed".into(),
        thread_id: actor.map(str::to_owned),
        turn_id: Some("old-turn".into()),
        item_id: Some(format!("item-{sequence}")),
        params: json!({"item":{"id":format!("item-{sequence}"),"type":"agentMessage","phase":"final_answer","text":format!("Result {sequence}")}}),
    }
}

fn snapshot(events: Vec<ControlEvent>, last: u64) -> ControlSnapshot {
    ControlSnapshot {
        generation: "process-1".into(),
        codex_home: "/fixture/home".into(),
        connected: true,
        observed_at: 500,
        threads: BTreeMap::from([("parent".into(), worker("parent"))]),
        event_window: Some(EventWindow {
            version: EventWindow::VERSION,
            stream_id: "stream-1".into(),
            first_retained_sequence: events.first().map(|event| event.sequence),
            last_assigned_sequence: last,
            prior_lineage_unknown: false,
        }),
        events,
        ..ControlSnapshot::default()
    }
}

fn apply(world: &mut World, events: Vec<Event>) {
    for event in events {
        world.apply(event);
    }
}

#[test]
fn exact_burst_gaps_preserve_current_roster_and_request_and_do_not_repeat() {
    for count in [300, 3000] {
        let initial = snapshot(vec![observation(1, Some("parent"))], 1);
        let cursor = reconcile_snapshot(&initial, &BridgeCursor::default()).next_cursor;
        let mut burst = snapshot(
            ((count - 254)..=(count + 1))
                .map(|sequence| observation(sequence, Some("parent")))
                .collect(),
            count + 1,
        );
        burst.threads.insert("child".into(), worker("child"));
        burst.pending_requests.push(PendingRequest {
            id: "current-exact-request".into(),
            native_id: json!(91),
            thread_id: "parent".into(),
            turn_id: Some("current-turn".into()),
            method: "item/commandExecution/requestApproval".into(),
            params: json!({"itemId":"command-current","command":"fixture-only command"}),
            received_at: 500,
            reply_sent: false,
            supported: true,
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
        let mut world = World::new();
        apply(&mut world, batch.events);
        assert_eq!(world.worker_count(), 2);
        let parent = world
            .worker(&worker("parent").identity.worker_id())
            .unwrap();
        assert_eq!(parent.wait_reason, Some(WaitReason::HumanApproval));
        assert_eq!(parent.lifecycle, WorkerLifecycle::Active);
        assert!(parent.turn_in_flight);
        assert_eq!(
            parent.coverage.stream.as_ref().unwrap().missing_events,
            count - 256
        );
        let replay = reconcile_snapshot(&burst, &batch.next_cursor);
        assert_eq!(replay.continuity, batch.continuity);
        assert!(!replay.events.iter().any(|event| matches!(&event.kind, EventKind::Collaboration(event) if event.kind == CollaborationKind::Result)));
        assert_eq!(burst.pending_requests[0].id, "current-exact-request");
        assert!(!burst.pending_requests[0].reply_sent);
    }
}

#[test]
fn missing_actor_resolves_once_as_history_without_regressing_live_work() {
    let initial = snapshot(Vec::new(), 0);
    let cursor = reconcile_snapshot(&initial, &BridgeCursor::default()).next_cursor;
    let unknown = snapshot(vec![observation(1, Some("child"))], 1);
    let pending = reconcile_snapshot(&unknown, &cursor);
    assert_eq!(pending.continuity.deferred_events, 1);
    assert!(!pending
        .events
        .iter()
        .any(|event| event.worker == worker("child").identity.worker_id()));
    let mut later = snapshot(Vec::new(), 1);
    later.threads.insert("child".into(), worker("child"));
    let mut world = World::new();
    let child = worker("child").identity.worker_id();
    world.apply(Event {
        at: 499,
        office: OfficeId("/fixture/project".into()),
        office_path: "/fixture/project".into(),
        worker: child.clone(),
        agent: Agent::Codex,
        kind: EventKind::Acted(Activity::Editing {
            detail: "current.rs".into(),
        }),
    });
    let recovered = reconcile_snapshot(&later, &pending.next_cursor);
    assert_eq!(recovered.continuity.deferred_events, 0);
    assert_eq!(recovered.next_cursor.deferred_bytes(), 0);
    assert!(recovered
        .events
        .iter()
        .any(|event| matches!(event.kind, EventKind::HistoricalBeat(_))));
    apply(&mut world, recovered.events);
    let child = world.worker(&child).unwrap();
    assert_eq!(
        child.activity,
        Activity::Editing {
            detail: "current.rs".into()
        }
    );
    assert_eq!(child.lifecycle, WorkerLifecycle::Active);
    assert!(child.turn_in_flight);
    assert_eq!(
        world
            .collaboration()
            .filter(|event| event.kind == CollaborationKind::Result)
            .count(),
        1
    );
    let replay = reconcile_snapshot(&later, &recovered.next_cursor);
    assert!(!replay.events.iter().any(|event| matches!(
        event.kind,
        EventKind::HistoricalBeat(_) | EventKind::Collaboration(_)
    )));
}

#[test]
fn deferred_old_completion_and_request_never_become_current_actions() {
    let cursor = reconcile_snapshot(&snapshot(Vec::new(), 0), &BridgeCursor::default()).next_cursor;
    let mut completed = observation(1, Some("child"));
    completed.method = "turn/completed".into();
    completed.params = json!({"turn":{"status":"completed"}});
    let mut request = observation(2, Some("child"));
    request.method = "item/commandExecution/requestApproval".into();
    request.params = json!({"command":"old request","itemId":"old-request-item"});
    request.item_id = Some("old-request-item".into());
    let pending = reconcile_snapshot(&snapshot(vec![completed, request], 2), &cursor);
    let mut later = snapshot(Vec::new(), 2);
    later.threads.insert("child".into(), worker("child"));
    let batch = reconcile_snapshot(&later, &pending.next_cursor);
    let mut world = World::new();
    apply(&mut world, batch.events);
    let child = world.worker(&worker("child").identity.worker_id()).unwrap();
    assert!(child.turn_in_flight);
    assert_eq!(child.lifecycle, WorkerLifecycle::Active);
    assert_eq!(child.wait_reason, None);
    assert_eq!(
        world
            .collaboration()
            .filter(|event| event.kind == CollaborationKind::HumanRequest)
            .count(),
        1
    );
    assert!(later.pending_requests.is_empty());
}

#[test]
fn missing_actor_queue_bounds_counts_and_bytes_without_double_counting_replay() {
    for text_size in [0, 32_000] {
        let mut cursor =
            reconcile_snapshot(&snapshot(Vec::new(), 0), &BridgeCursor::default()).next_cursor;
        let mut final_snapshot = snapshot(Vec::new(), 0);
        for chunk in 0..3 {
            let events = (chunk * 128 + 1..=chunk * 128 + 128)
                .map(|sequence| {
                    let mut event = observation(sequence, Some("missing"));
                    event.params["item"]["text"] = json!("x".repeat(text_size));
                    event
                })
                .collect();
            final_snapshot = snapshot(events, chunk * 128 + 128);
            cursor = reconcile_snapshot(&final_snapshot, &cursor).next_cursor;
            assert!(cursor.continuity.deferred_events <= DEFERRED_EVENT_LIMIT);
            assert!(cursor.deferred_bytes() <= DEFERRED_EVENT_BYTE_LIMIT);
            assert_eq!(cursor.continuity.missing_events, 0);
        }
        assert_eq!(
            cursor.continuity.dropped_deferred_events as usize + cursor.continuity.deferred_events,
            384
        );
        if text_size == 0 {
            assert_eq!(cursor.continuity.deferred_events, 256);
        } else {
            assert!(cursor.continuity.deferred_events < 64);
        }
        let replay = reconcile_snapshot(&final_snapshot, &cursor);
        assert_eq!(replay.continuity, cursor.continuity);
        assert_eq!(replay.next_cursor.deferred_bytes(), cursor.deferred_bytes());
    }
}

#[test]
fn oversized_deferred_observation_does_not_evict_other_pending_items() {
    let cursor = reconcile_snapshot(
        &snapshot(vec![observation(1, Some("missing"))], 1),
        &BridgeCursor::default(),
    )
    .next_cursor;
    let mut huge = observation(2, Some("missing"));
    huge.params = json!({"text":"x".repeat(DEFERRED_EVENT_BYTE_LIMIT)});
    let batch = reconcile_snapshot(&snapshot(vec![huge], 2), &cursor);
    assert_eq!(batch.continuity.deferred_events, 1);
    assert_eq!(batch.continuity.dropped_deferred_events, 1);
    assert_eq!(batch.next_cursor.deferred_bytes(), cursor.deferred_bytes());
}

#[test]
fn lineage_change_drops_deferred_without_crossing_identity_and_ignores_process_labels() {
    let initial = snapshot(vec![observation(1, Some("child"))], 1);
    let cursor = reconcile_snapshot(&initial, &BridgeCursor::default()).next_cursor;
    let mut restart = initial.clone();
    restart.generation = "different process".into();
    let batch = reconcile_snapshot(&restart, &cursor);
    assert_eq!(batch.continuity, cursor.continuity);
    let mut wrong_actor = snapshot(Vec::new(), 1);
    let mut foreign = worker("child");
    foreign.identity.source = SourceId("/other/home".into());
    wrong_actor.threads.insert("child".into(), foreign);
    let batch = reconcile_snapshot(&wrong_actor, &cursor);
    assert_eq!(batch.continuity.deferred_events, 1);
    assert!(!batch
        .events
        .iter()
        .any(|event| event.worker == worker("child").identity.worker_id()));
    let mut changed = snapshot(Vec::new(), 0);
    changed.event_window.as_mut().unwrap().stream_id = "stream-2".into();
    changed.threads.insert("child".into(), worker("child"));
    let batch = reconcile_snapshot(&changed, &cursor);
    assert_eq!(batch.continuity.deferred_events, 0);
    assert!(batch.continuity.prior_stream_unknown);
    assert_eq!(batch.continuity.missing_events, 0);
    assert!(!batch
        .events
        .iter()
        .any(|event| matches!(event.kind, EventKind::Collaboration(_))));
}

#[test]
fn gaps_are_coalesced_bounded_and_empty_or_stale_windows_do_not_recount() {
    let mut cursor = BridgeCursor::default();
    cursor = reconcile_snapshot(&snapshot(Vec::new(), 1), &cursor).next_cursor;
    cursor = reconcile_snapshot(&snapshot(Vec::new(), 2), &cursor).next_cursor;
    assert_eq!(
        cursor.continuity.missing_ranges,
        vec![SequenceRange { first: 1, last: 2 }]
    );
    for n in 0..20 {
        let retained = 3 + n * 2;
        cursor = reconcile_snapshot(
            &snapshot(vec![observation(retained, None)], retained),
            &cursor,
        )
        .next_cursor;
        cursor = reconcile_snapshot(&snapshot(Vec::new(), retained + 1), &cursor).next_cursor;
    }
    assert_eq!(cursor.continuity.missing_events, 22);
    assert_eq!(cursor.continuity.missing_ranges.len(), 16);
    assert_eq!(cursor.continuity.omitted_ranges, 5);
    let older = reconcile_snapshot(&snapshot(Vec::new(), 1), &cursor);
    assert_eq!(older.continuity, cursor.continuity);
    assert_eq!(older.next_cursor.applied_sequence, cursor.applied_sequence);
}

#[test]
fn legacy_unknown_prefix_is_not_a_fabricated_missing_count() {
    let mut legacy = snapshot(vec![observation(200, Some("parent"))], 200);
    legacy.event_window = None;
    let batch = reconcile_snapshot(&legacy, &BridgeCursor::default());
    assert!(!batch.continuity.lineage_known);
    assert!(batch.continuity.prior_stream_unknown);
    assert_eq!(batch.continuity.missing_events, 0);
    let duplicate = reconcile_snapshot(&legacy, &batch.next_cursor);
    assert_eq!(duplicate.continuity, batch.continuity);
    legacy.events[0].sequence = 500;
    let batch = reconcile_snapshot(&legacy, &batch.next_cursor);
    assert_eq!(batch.continuity.missing_events, 0);
    assert!(!batch.continuity.lineage_known);
}

#[path = "support/dual_feed_fixture.rs"]
mod dual_feed_fixture;

#[test]
fn cold_collector_and_managed_stream_keep_recovered_facts_without_claiming_backfill() {
    use std::fs;
    use theywork_collect::CodexSource;
    use theywork_core::Source;
    const BASE: i64 = 1_780_000_000_000;
    for (count, aggregate) in [(300, false), (3000, false), (3000, true)] {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/control-data-fixtures")
            .join(theywork_control::operation_id().unwrap());
        let home = root.join("home");
        let project = root.join("project");
        let observations = dual_feed_fixture::stream(count, aggregate);
        dual_feed_fixture::create_store(&home, &project, &observations, aggregate).unwrap();
        // An explicit disposable repository marker establishes the test's
        // project boundary even when the checkout itself is an ancestor.
        fs::create_dir(project.join(".git")).unwrap();
        let home = fs::canonicalize(home).unwrap();
        let project = fs::canonicalize(project).unwrap();
        let before = [
            fs::read(home.join("state_5.sqlite")).unwrap(),
            fs::read(home.join("thread_history_1.sqlite")).unwrap(),
        ];
        let mut stream = snapshot(
            observations[observations.len() - 256..].to_vec(),
            count as u64 + 1,
        );
        stream.codex_home = home.clone();
        stream.observed_at = BASE + 10_000;
        stream.threads.clear();
        for native in ["parent", "child"] {
            let mut record = worker(native);
            record.identity.source = SourceId(home.to_string_lossy().into_owned());
            record.project = project.clone();
            record.updated_at = BASE + 10_000;
            record.role = if native == "parent" {
                WorkerRole::Main
            } else {
                WorkerRole::Subagent
            };
            record.active_turn_id = Some("audit-turn".into());
            stream.threads.insert(native.into(), record);
        }
        // The independent log has one opening sequence before the burst.
        let mut initial = stream.clone();
        initial.events = vec![observation(1, None)];
        initial
            .event_window
            .as_mut()
            .unwrap()
            .first_retained_sequence = Some(1);
        initial
            .event_window
            .as_mut()
            .unwrap()
            .last_assigned_sequence = 1;
        let cursor = reconcile_snapshot(&initial, &BridgeCursor::default()).next_cursor;
        let mut collector = CodexSource::new(&home);
        let mut world = World::new();
        apply(&mut world, collector.poll(BASE + 10_000).unwrap());
        let batch = reconcile_snapshot(&stream, &cursor);
        assert_eq!(batch.continuity.missing_events, count as u64 - 256);
        apply(&mut world, batch.events);
        assert_eq!(world.worker_count(), 2);
        assert_eq!(world.office_count(), 1);
        assert_eq!(
            world.relationships().count(),
            1,
            "native metadata relation must survive both feeds"
        );
        let result_ids = |world: &World| {
            world
                .collaboration()
                .filter(|event| event.kind == CollaborationKind::Result)
                .map(|event| event.native_item_id.clone().unwrap())
                .collect::<std::collections::BTreeSet<_>>()
        };
        let expected = if aggregate {
            [
                "early-child-result".to_owned(),
                "early-parent-result".to_owned(),
            ]
            .into_iter()
            .collect()
        } else {
            std::collections::BTreeSet::new()
        };
        assert_eq!(result_ids(&world), expected);
        assert_eq!(
            world
                .collaboration()
                .filter(|event| event.kind == CollaborationKind::Delegated)
                .count(),
            usize::from(aggregate)
        );
        apply(&mut world, collector.poll(BASE + 10_010).unwrap());
        apply(
            &mut world,
            reconcile_snapshot(&stream, &batch.next_cursor).events,
        );
        assert_eq!(
            result_ids(&world),
            expected,
            "warm poll must neither invent backfill nor duplicate results"
        );
        assert_eq!(world.relationships().count(), 1);
        let worker = world
            .worker(&stream.threads["parent"].identity.worker_id())
            .unwrap();
        assert!(worker.coverage.incomplete);
        assert_eq!(
            worker.coverage.stream.as_ref().unwrap().missing_events,
            count as u64 - 256
        );
        assert_eq!(fs::read(home.join("state_5.sqlite")).unwrap(), before[0]);
        assert_eq!(
            fs::read(home.join("thread_history_1.sqlite")).unwrap(),
            before[1]
        );
        fs::remove_dir_all(root).unwrap();
    }
}
