use crate::event::{Event, EventKind};
use crate::model::{Activity, Agent, OfficeId, WorkerId};
use crate::Millis;

/// A deterministic imaginary company, for `--demo` and for developing the
/// renderer without any agents running.
///
/// `now` drives the animation, so calling this each frame produces a building
/// that visibly works. No randomness: the same `now` always gives the same
/// world, which keeps snapshot tests honest.
pub fn events(now: Millis) -> Vec<Event> {
    const STAFF: &[(&str, &str, Agent)] = &[
        ("/home/dev/checkout", "Dev 1", Agent::Codex),
        ("/home/dev/checkout", "Dev 2", Agent::Codex),
        ("/home/dev/checkout", "orchestrator", Agent::Claude),
        ("/home/dev/website", "Dev 1", Agent::Codex),
        ("/home/dev/website", "reviewer", Agent::Claude),
        ("/home/dev/infra", "Dev 1", Agent::Claude),
    ];

    STAFF
        .iter()
        .enumerate()
        .flat_map(|(i, (path, name, agent))| {
            let id = WorkerId(format!("{path}#{name}"));
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
        .collect()
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
