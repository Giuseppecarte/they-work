use theywork_core::*;

fn record(world: &mut World, project: &str, actor: &str, id: &str, at: Millis) {
    world.apply(Event {
        at,
        office: OfficeId(project.into()),
        office_path: project.into(),
        worker: WorkerId(actor.into()),
        agent: Agent::Codex,
        kind: EventKind::Collaboration(CollaborationEvent {
            id: id.into(),
            at,
            actor: WorkerId(actor.into()),
            recipient: None,
            kind: CollaborationKind::Result,
            text: Some("Recorded delivery".into()),
            correlation_id: None,
            native_turn_id: None,
            native_item_id: None,
            evidence: Evidence::NativeEvent,
        }),
    });
}

#[test]
fn busy_project_cannot_displace_quiet_project_delivery() {
    let mut world = World::new();
    record(&mut world, "B", "b", "quiet", 1);
    for n in 0..513 {
        record(&mut world, "A", "a", &n.to_string(), n + 2);
    }
    assert_eq!(world.collaboration().count(), 512);
    assert_eq!(
        world.history_window(&OfficeId("B".into())).retained_count,
        1
    );
    let a = world.history_window(&OfficeId("A".into()));
    assert_eq!((a.retained_count, a.evicted_count), (511, 2));
    assert_eq!(a.oldest_retained_at, Some(4));
    assert_eq!(a.last_eviction_ordinal, Some(514));
    assert_eq!(a.last_evicted_record_at, Some(3));
    assert!(a.prior_history_unknown);
}

#[test]
fn twenty_projects_share_capacity_and_ties_are_reproducible() {
    let mut world = World::new();
    for n in 0..50 {
        for project in 0..20 {
            let project = format!("p{project:02}");
            record(&mut world, &project, &project, &n.to_string(), n);
        }
    }
    let counts: Vec<_> = world
        .history_windows()
        .map(|(_, window)| window.retained_count)
        .collect();
    assert_eq!(counts.iter().sum::<usize>(), 512);
    assert_eq!(counts.iter().filter(|&&count| count == 25).count(), 8);
    assert_eq!(counts.iter().filter(|&&count| count == 26).count(), 12);
    assert!(counts[..8].iter().all(|&count| count == 25));
    assert!(world
        .collaboration()
        .zip(world.collaboration().skip(1))
        .all(|(a, b)| a.at <= b.at));
}

#[test]
fn duplicate_and_older_upserts_do_not_consume_retention_or_ordinals() {
    let mut world = World::new();
    for n in 0..512 {
        record(&mut world, "A", "a", &n.to_string(), n);
    }
    for _ in 0..10 {
        record(&mut world, "A", "a", "511", 511);
        record(&mut world, "A", "a", "511", 10);
    }
    record(&mut world, "B", "a", "511", 512);
    assert_eq!(world.collaboration().count(), 512);
    assert_eq!(
        world.history_window(&OfficeId("B".into())).retained_count,
        0
    );
    assert_eq!(
        world.history_window(&OfficeId("A".into())).retained_count,
        512
    );
    assert_eq!(world.history_window(&OfficeId("A".into())).evicted_count, 0);
    record(&mut world, "A", "a", "new", 513);
    assert_eq!(
        world
            .history_window(&OfficeId("A".into()))
            .last_eviction_ordinal,
        Some(514)
    );
}

#[test]
fn project_metadata_is_bounded_and_returned_projects_disclose_unknown_counts() {
    let mut world = World::new();
    for project in 0..513 {
        let project = format!("p{project:03}");
        record(&mut world, &project, &project, "delivery", 0);
    }
    assert_eq!(world.history_windows().count(), 512);
    assert!(world.older_project_windows_unknown());
    let missing = world.history_window(&OfficeId("p000".into()));
    assert_eq!(
        missing.evicted_count, 0,
        "forgotten count is not manufactured"
    );
    assert!(missing.prior_local_evictions_unknown);
    record(&mut world, "p000", "p000", "later", 1);
    let returned = world.history_window(&OfficeId("p000".into()));
    assert_eq!(returned.retained_count, 1);
    assert!(returned.prior_local_evictions_unknown);
    assert_eq!(world.history_windows().count(), 512);
}

#[test]
fn recorded_project_survives_actor_move_and_missing_participants() {
    let mut world = World::new();
    record(&mut world, "original", "actor", "delivery", 2);
    world.apply(Event {
        at: 3,
        office: OfficeId("moved".into()),
        office_path: "moved".into(),
        worker: WorkerId("actor".into()),
        agent: Agent::Codex,
        kind: EventKind::Seen {
            name: "Actor".into(),
            git_branch: None,
        },
    });
    assert_eq!(
        world.collaboration_office(world.collaboration().next().unwrap()),
        Some(&OfficeId("original".into()))
    );
    assert_eq!(
        world
            .history_window(&OfficeId("original".into()))
            .retained_count,
        1
    );
    record(&mut world, "missing", "absent", "delivery", 4);
    assert!(world.worker(&WorkerId("absent".into())).is_none());
    assert_eq!(
        world
            .history_window(&OfficeId("missing".into()))
            .retained_count,
        1
    );
}

#[test]
fn historical_beats_never_clear_a_live_request_or_rehire_worker() {
    let mut world = World::new();
    let event = |kind| Event {
        at: 100,
        office: OfficeId("project".into()),
        office_path: "project".into(),
        worker: WorkerId("actor".into()),
        agent: Agent::Codex,
        kind,
    };
    world.apply(event(EventKind::Turn { in_flight: true }));
    world.apply(event(EventKind::Wait(Some(WaitReason::HumanInput))));
    let before = world.worker(&WorkerId("actor".into())).unwrap().clone();
    let beat = Beat {
        at: 1,
        activity: Activity::Thinking,
        outcome: None,
    };
    world.apply(event(EventKind::HistoricalBeat(beat.clone())));
    let after = world.worker(&WorkerId("actor".into())).unwrap();
    assert_eq!(after.last_seen, before.last_seen);
    assert_eq!(after.wait_reason, before.wait_reason);
    assert_eq!(after.activity, before.activity);
    assert!(after.turn_in_flight);
    world.apply(event(EventKind::Left));
    world.apply(event(EventKind::HistoricalBeat(beat)));
    assert_eq!(world.worker_count(), 0);
}

#[test]
fn independent_coverage_producers_preserve_losses_and_epochs() {
    let mut coverage = SourceCoverage {
        stream: Some(StreamContinuity {
            source: SourceId("host".into()),
            stream_id: Some("first".into()),
            lineage_known: true,
            missing_events: 300,
            missing_ranges: vec![SequenceRange {
                first: 1,
                last: 300,
            }],
            ..StreamContinuity::default()
        }),
        ..SourceCoverage::default()
    };
    coverage.merge_observation(SourceCoverage {
        tool_correlation: Some(ToolCorrelationCoverage {
            epoch: 1,
            evicted: 7,
            ..ToolCorrelationCoverage::default()
        }),
        available: true,
        observed_at: 500,
        ..SourceCoverage::default()
    });
    assert_eq!(coverage.stream.as_ref().unwrap().missing_events, 300);
    coverage.merge_observation(SourceCoverage {
        tool_correlation: Some(ToolCorrelationCoverage {
            epoch: 1,
            evicted: 2,
            ..ToolCorrelationCoverage::default()
        }),
        ..SourceCoverage::default()
    });
    assert_eq!(coverage.tool_correlation.as_ref().unwrap().evicted, 7);
    coverage.merge_observation(SourceCoverage {
        tool_correlation: Some(ToolCorrelationCoverage {
            epoch: 2,
            ..ToolCorrelationCoverage::default()
        }),
        stream: Some(StreamContinuity {
            source: SourceId("host".into()),
            stream_id: Some("second".into()),
            lineage_known: true,
            ..StreamContinuity::default()
        }),
        ..SourceCoverage::default()
    });
    let stream = coverage.stream.unwrap();
    assert!(stream.prior_stream_unknown);
    assert_eq!(stream.missing_events, 0);
    let tools = coverage.tool_correlation.unwrap();
    assert!(tools.prior_loss);
    assert_eq!(tools.evicted, 0);
}
