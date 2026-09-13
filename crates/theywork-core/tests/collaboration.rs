use theywork_core::*;

fn id(value: &str) -> WorkerId {
    WorkerId(value.into())
}
fn event(worker: &str, at: Millis, kind: EventKind) -> Event {
    Event {
        at,
        office: OfficeId("/project".into()),
        office_path: "/project".into(),
        worker: id(worker),
        agent: Agent::Codex,
        kind,
    }
}
fn relation(world: &mut World, parent: &str, child: &str, kind: RelationshipKind) {
    world.apply(event(
        parent,
        10,
        EventKind::Relationship(Relationship {
            parent: id(parent),
            child: id(child),
            kind,
            evidence: Evidence::NativeMetadata,
            at: 10,
            correlation_id: None,
        }),
    ));
}
fn delivery(worker: &str, key: &str, at: Millis) -> Event {
    event(
        worker,
        at,
        EventKind::Collaboration(CollaborationEvent {
            id: key.into(),
            at,
            actor: id(worker),
            recipient: Some(id("parent")),
            kind: CollaborationKind::Result,
            text: Some("Verified output".into()),
            correlation_id: Some("task-1".into()),
            native_turn_id: Some("turn-1".into()),
            native_item_id: Some(key.into()),
            evidence: Evidence::NativeEvent,
        }),
    )
}

#[test]
fn nested_graph_deduplicates_membership_and_refuses_structural_cycles() {
    let mut world = World::new();
    relation(&mut world, "root", "child", RelationshipKind::Delegation);
    relation(&mut world, "child", "leaf", RelationshipKind::Delegation);
    relation(
        &mut world,
        "root",
        "leaf",
        RelationshipKind::SessionMembership,
    );
    relation(&mut world, "child", "leaf", RelationshipKind::Delegation);
    relation(&mut world, "leaf", "root", RelationshipKind::Delegation);
    relation(&mut world, "root", "root", RelationshipKind::Fork);
    assert_eq!(world.relationships().count(), 3);
    let tree = world.tree(&id("root"));
    assert_eq!(
        tree.iter()
            .map(|node| (&node.worker, node.depth))
            .collect::<Vec<_>>(),
        vec![(&id("root"), 0), (&id("child"), 1), (&id("leaf"), 2)]
    );
    assert_eq!(world.ancestors(&id("leaf")), vec![id("child"), id("root")]);
    assert_eq!(
        world.family(&id("leaf")),
        vec![id("child"), id("leaf"), id("root")]
    );
    assert_eq!(world.worker_count(), 0, "edges never invent workers");
}

#[test]
fn membership_does_not_claim_immediate_parent_and_unknown_parents_stay_visible() {
    let mut world = World::new();
    relation(
        &mut world,
        "session",
        "child",
        RelationshipKind::SessionMembership,
    );
    assert!(world.ancestors(&id("child")).is_empty());
    assert!(world.children(&id("session")).is_empty());
    assert_eq!(
        world.tree(&id("session"))[1].via,
        Some(RelationshipKind::SessionMembership)
    );
    relation(
        &mut world,
        "missing-parent",
        "child",
        RelationshipKind::Delegation,
    );
    assert_eq!(
        world.tree(&id("session")).len(),
        2,
        "unknown parent must not hide its known family member"
    );
    assert!(world.worker(&id("missing-parent")).is_none());
    assert!(world.family(&id("session")).contains(&id("missing-parent")));
}

#[test]
fn delivery_and_history_survive_removal_and_a_move_without_duplicate_seats() {
    let mut world = World::new();
    world.apply(event(
        "child",
        1,
        EventKind::Seen {
            name: "Tester".into(),
            git_branch: None,
        },
    ));
    world.apply(event(
        "child",
        2,
        EventKind::Did(Beat {
            at: 2,
            activity: Activity::Reading {
                detail: "tests.rs".into(),
            },
            outcome: None,
        }),
    ));
    relation(&mut world, "parent", "child", RelationshipKind::Delegation);
    world.apply(delivery("child", "answer", 3));
    world.apply(delivery("child", "answer", 3));
    world.apply(event("child", 4, EventKind::Left));
    assert_eq!(world.worker_count(), 0);
    assert_eq!(world.collaboration_for(&id("parent")).count(), 1);
    assert_eq!(world.worker(&id("child")).unwrap().history.len(), 1);
    assert_eq!(
        world.worker(&id("child")).unwrap().lifecycle,
        WorkerLifecycle::Removed
    );
    let mut moved = event(
        "child",
        5,
        EventKind::Seen {
            name: "Tester".into(),
            git_branch: None,
        },
    );
    moved.office = OfficeId("/other".into());
    moved.office_path = "/other".into();
    world.apply(moved);
    assert_eq!(world.worker_count(), 1);
    assert_eq!(world.worker(&id("child")).unwrap().history.len(), 1);
    assert_eq!(world.collaboration().count(), 1);
}

#[test]
fn source_health_never_refreshes_activity_and_expired_workers_do_not_finish() {
    let mut world = World::new();
    world.apply(event("child", 10, EventKind::Turn { in_flight: true }));
    world.apply(event(
        "child",
        1000,
        EventKind::Coverage(SourceCoverage {
            available: true,
            observed_at: 1000,
            ..SourceCoverage::default()
        }),
    ));
    world.apply(event(
        "child",
        1001,
        EventKind::Identity {
            identity: ThreadIdentity::new(Agent::Codex, SourceId("fixture".into()), "child"),
            role: WorkerRole::Subagent,
        },
    ));
    let worker = world.worker(&id("child")).unwrap();
    assert_eq!(worker.last_seen, 10);
    assert!(!worker.coverage.is_stale_at(1001));
    assert!(worker.coverage.is_stale_at(1001 + BLOCKED_AFTER_MS));
    world.tick(OFFLINE_AFTER_MS + 11);
    assert!(!world.is_present(&id("child")));
    assert_eq!(
        world.worker(&id("child")).unwrap().lifecycle,
        WorkerLifecycle::Removed
    );
    assert_eq!(
        world.collaboration().count(),
        0,
        "silence cannot manufacture a result"
    );
    world.apply(event(
        "child",
        OFFLINE_AFTER_MS + 12,
        EventKind::Identity {
            identity: ThreadIdentity::new(Agent::Codex, SourceId("fixture".into()), "child"),
            role: WorkerRole::Subagent,
        },
    ));
    assert!(
        !world.is_present(&id("child")),
        "metadata alone cannot rehire a retired worker"
    );
}

#[test]
fn automatic_waits_keep_running_until_silent_while_human_requests_block_immediately() {
    for reason in [
        WaitReason::AutomaticReview,
        WaitReason::Child,
        WaitReason::Process,
    ] {
        let mut worker = Worker::new(
            id("worker"),
            OfficeId("/project".into()),
            Agent::Codex,
            "Worker".into(),
            10,
        );
        worker.turn_in_flight = true;
        worker.activity = Activity::Waiting {
            detail: "Waiting".into(),
        };
        worker.wait_reason = Some(reason);
        assert_eq!(worker.status_at(11), WorkerStatus::Running);
        assert_eq!(
            worker.status_at(11 + BLOCKED_AFTER_MS),
            WorkerStatus::Blocked
        );
        worker.wait_reason = Some(WaitReason::HumanApproval);
        assert_eq!(worker.status_at(11), WorkerStatus::Blocked);
        worker.lifecycle = WorkerLifecycle::Failed;
        assert_eq!(worker.status_at(11), WorkerStatus::Failed);
    }
}

#[test]
fn history_is_bounded_deduplicated_and_sorted_even_when_backfill_arrives_late() {
    let mut world = World::new();
    for n in (0..COLLABORATION_HISTORY_LEN + 20).rev() {
        world.apply(delivery("child", &n.to_string(), n as Millis));
    }
    assert_eq!(world.collaboration().count(), COLLABORATION_HISTORY_LEN);
    assert_eq!(world.collaboration().next().unwrap().at, 20);
    assert_eq!(
        world.collaboration().last().unwrap().at,
        (COLLABORATION_HISTORY_LEN + 19) as Millis
    );
    let mut worker = Worker::new(
        id("one"),
        OfficeId("/project".into()),
        Agent::Codex,
        "One".into(),
        0,
    );
    for n in (0..HISTORY_LEN + 20).rev() {
        let beat = Beat {
            at: n as Millis,
            activity: Activity::Thinking,
            outcome: None,
        };
        worker.remember(beat.clone());
        worker.remember(beat);
    }
    assert_eq!(worker.history.len(), HISTORY_LEN);
    assert_eq!(worker.history.front().unwrap().at, 20);
}

#[test]
fn demo_has_a_reviewable_delivery_two_children_and_an_explicit_question() {
    let mut world = World::new();
    for at in [10000, 10001] {
        for event in demo::events(at) {
            world.apply(event);
        }
    }
    assert_eq!(world.worker_count(), 6);
    assert_eq!(world.relationships().count(), 2);
    assert_eq!(world.collaboration().count(), 5);
    assert_eq!(
        world
            .collaboration()
            .filter(|event| event.kind == CollaborationKind::Result)
            .count(),
        1
    );
    assert!(world
        .collaboration()
        .any(|event| event.kind == CollaborationKind::HumanRequest));
    assert!(world
        .offices()
        .flat_map(|office| &office.workers)
        .any(|worker| worker.wait_reason == Some(WaitReason::HumanInput)));
}
