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
            let activity = match phase {
                0 => Activity::Typing {
                    detail: "cargo test --workspace".into(),
                },
                1 => Activity::Reading {
                    detail: "src/world.rs".into(),
                },
                2 => Activity::Editing {
                    detail: "src/render/canvas.rs".into(),
                },
                3 => Activity::Searching {
                    detail: "fn apply".into(),
                },
                4 => Activity::Thinking,
                5 => Activity::Talking {
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
                    12_000 + (now / 90) as u64 * (i as u64 + 1),
                )),
                mk(EventKind::Turn {
                    in_flight: activity.is_busy(),
                }),
                mk(EventKind::Acted(activity)),
            ]
        })
        .collect()
}
