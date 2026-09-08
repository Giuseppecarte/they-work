use crate::event::{Event, EventKind};
use crate::model::{Activity, Agent, OfficeId};
use crate::{
    CollaborationEvent, CollaborationKind, CoverageLevel, Evidence, Millis, Relationship,
    RelationshipKind, SourceCoverage, SourceId, ThreadIdentity, WaitReason, WorkerLifecycle,
    WorkerRole,
};

/// A deterministic imaginary company, for `--demo` and for developing the
/// renderer without any agents running.
///
/// `now` drives the animation, so calling this each frame produces a building
/// that visibly works. No randomness: the same `now` always gives the same
/// world, which keeps snapshot tests honest.
pub fn events(now: Millis) -> Vec<Event> {
    const STAFF: &[(&str, &str, Agent)] = &[
        ("/home/dev/checkout", "Checkout lead", Agent::Codex),
        ("/home/dev/checkout", "Retry endpoint", Agent::Codex),
        ("/home/dev/checkout", "Timeout tests", Agent::Codex),
        ("/home/dev/website", "Landing page", Agent::Claude),
        ("/home/dev/website", "Preview review", Agent::Claude),
        ("/home/dev/infra", "Integration checks", Agent::Claude),
    ];

    let mut events: Vec<Event> = STAFF
        .iter()
        .enumerate()
        .flat_map(|(i, (path, name, agent))| {
            let identity =
                ThreadIdentity::new(*agent, SourceId("demo".into()), format!("{path}#{name}"));
            let id = identity.worker_id();
            let office = OfficeId((*path).to_string());
            let phase = (now / 1500 + i as Millis * 2) % 7;
            let activity = match (i, phase) {
                (4, _) => Activity::Waiting {
                    detail: "Approve publishing the preview to staging (fictional demo request)."
                        .into(),
                },
                (5, _) => Activity::Error {
                    detail: "Integration checks failed: the demo database is unavailable.".into(),
                },
                (_, 0) => Activity::Typing {
                    detail: "cargo test --workspace".into(),
                },
                (_, 1) => Activity::Reading {
                    detail: "src/world.rs".into(),
                },
                (_, 2) => Activity::Editing {
                    detail: "src/render/canvas.rs".into(),
                },
                (_, 3) => Activity::Searching {
                    detail: "fn apply".into(),
                },
                (_, 4) => Activity::Thinking,
                (_, 5) => Activity::Talking {
                    detail: "Tests pass, pushing.".into(),
                },
                _ => Activity::Idle,
            };
            let mk = |kind| Event {
                at: now,
                office: office.clone(),
                office_path: (*path).to_string(),
                worker: id.clone(),
                agent: *agent,
                kind,
            };
            [
                mk(EventKind::Identity {
                    identity,
                    role: if matches!(i, 1 | 2) {
                        WorkerRole::Subagent
                    } else {
                        WorkerRole::Main
                    },
                }),
                mk(EventKind::Coverage(SourceCoverage {
                    available: true,
                    incomplete: false,
                    relationships: CoverageLevel::Supported,
                    messages: CoverageLevel::Supported,
                    lifecycle: CoverageLevel::Supported,
                    observed_at: now,
                    detail: "Fictional demo scenario.".into(),
                })),
                mk(EventKind::Seen {
                    name: (*name).to_string(),
                    git_branch: Some("main".into()),
                }),
                mk(EventKind::Tokens(
                    12_000 + i as u64 * 4_250 + (now.rem_euclid(60_000) / 90) as u64,
                )),
                mk(EventKind::Turn {
                    in_flight: activity.is_busy(),
                }),
                mk(EventKind::Acted(activity)),
            ]
        })
        .collect();
    let identity = |i: usize| {
        ThreadIdentity::new(
            STAFF[i].2,
            SourceId("demo".into()),
            format!("{}#{}", STAFF[i].0, STAFF[i].1),
        )
    };
    let parent = identity(0).worker_id();
    let api = identity(1).worker_id();
    let tests = identity(2).worker_id();
    let emit = |i: usize, at: Millis, kind| Event {
        at,
        office: OfficeId(STAFF[i].0.into()),
        office_path: STAFF[i].0.into(),
        worker: identity(i).worker_id(),
        agent: STAFF[i].2,
        kind,
    };
    for (child, request) in [(&api, "demo-api-task"), (&tests, "demo-tests-task")] {
        events.push(emit(
            0,
            now.saturating_sub(6000),
            EventKind::Relationship(Relationship {
                parent: parent.clone(),
                child: child.clone(),
                kind: RelationshipKind::Delegation,
                evidence: Evidence::Demo,
                at: now.saturating_sub(6000),
                correlation_id: Some(request.into()),
            }),
        ));
    }
    for (i, id, recipient, kind, body, correlation, age) in [
        (0, "demo-delegate-api", Some(api.clone()), CollaborationKind::Delegated, "Implement the retry endpoint and summarize the compatibility change.", Some("demo-api-task"), 6000),
        (0, "demo-delegate-tests", Some(tests.clone()), CollaborationKind::Delegated, "Test the retry flow; ask before changing the public timeout.", Some("demo-tests-task"), 5500),
        (1, "demo-api-message", Some(tests.clone()), CollaborationKind::Message, "The retry response includes attempt_count. Please cover a failed first attempt.", Some("demo-api-task"), 4500),
        (1, "demo-api-result", Some(parent.clone()), CollaborationKind::Result, "Retry endpoint ready for review.\n\nChanged: src/api/retry.rs and tests/retry.rs. The response adds attempt_count without changing existing fields.\nValidation: 12 focused tests pass, including a failed first attempt and timeout handling.\nDecision: keep the current 30-second timeout until the product owner confirms the new limit. This is a fictional demo delivery.", Some("demo-api-task"), 2500),
        (2, "demo-tests-question", Some(parent.clone()), CollaborationKind::HumanRequest, "Should the public timeout stay at 30 seconds, or increase to 60? Existing clients rely on 30 seconds. (Fictional demo question.)", Some("demo-tests-task"), 1000),
    ] {
        events.push(emit(i, now.saturating_sub(age), EventKind::Collaboration(CollaborationEvent {
            id: id.into(), at: now.saturating_sub(age), actor: identity(i).worker_id(), recipient,
            kind, text: Some(body.into()), correlation_id: correlation.map(str::to_owned),
            native_turn_id: Some("demo-iteration".into()), native_item_id: Some(id.into()), evidence: Evidence::Demo,
        })));
    }
    events.push(emit(
        1,
        now,
        EventKind::Lifecycle(WorkerLifecycle::Completed),
    ));
    events.push(emit(1, now, EventKind::Acted(Activity::Idle)));
    events.push(emit(1, now, EventKind::Turn { in_flight: false }));
    events.push(emit(0, now, EventKind::Wait(Some(WaitReason::Child))));
    events.push(emit(0, now, EventKind::Turn { in_flight: true }));
    events.push(emit(0, now, EventKind::Lifecycle(WorkerLifecycle::Active)));
    events.push(emit(2, now, EventKind::Turn { in_flight: true }));
    events.push(emit(2, now, EventKind::Lifecycle(WorkerLifecycle::Active)));
    events.push(emit(2, now, EventKind::Wait(Some(WaitReason::HumanInput))));
    events.push(emit(
        2,
        now,
        EventKind::Acted(Activity::Waiting {
            detail: "Choose the public timeout: 30 or 60 seconds (demo).".into(),
        }),
    ));
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WorkerStatus, World};

    #[test]
    fn demo_at_real_clock_times_exposes_attention_without_epoch_sized_usage() {
        for now in [0, -1, 1_788_818_294_150, i64::MAX / 2] {
            let mut world = World::new();
            for event in events(now) {
                world.apply(event);
            }
            let workers = world
                .offices()
                .flat_map(|office| &office.workers)
                .collect::<Vec<_>>();
            assert_eq!(workers.len(), 6);
            assert!(workers
                .iter()
                .any(|worker| matches!(worker.activity, Activity::Waiting { .. })));
            assert!(workers
                .iter()
                .any(|worker| worker.status_at(now) == WorkerStatus::Failed));
            assert!(workers
                .iter()
                .all(|worker| worker.tokens_used > 0 && worker.tokens_used < 100_000));
        }
    }
}
