use std::collections::BTreeMap;

use crate::event::{Event, EventKind};
use crate::model::{Activity, Office, OfficeId, Worker};
use crate::Millis;

/// The whole building: every office, every worker, folded from events.
///
/// Offices are kept in a `BTreeMap` so the camera grid has a stable,
/// non-jittering order between frames.
#[derive(Debug, Default, Clone)]
pub struct World {
    offices: BTreeMap<OfficeId, Office>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    /// Offices in stable display order.
    pub fn offices(&self) -> impl Iterator<Item = &Office> {
        self.offices.values()
    }

    pub fn office(&self, id: &OfficeId) -> Option<&Office> {
        self.offices.get(id)
    }

    pub fn office_count(&self) -> usize {
        self.offices.len()
    }

    pub fn worker_count(&self) -> usize {
        self.offices.values().map(|o| o.workers.len()).sum()
    }

    /// Fold one event into the world.
    pub fn apply(&mut self, ev: Event) {
        let office = self
            .offices
            .entry(ev.office.clone())
            .or_insert_with(|| Office::new(ev.office.clone(), ev.office_path.clone()));

        if matches!(ev.kind, EventKind::Left) {
            office.workers.retain(|w| w.id != ev.worker);
            return;
        }

        let idx = match office.workers.iter().position(|w| w.id == ev.worker) {
            Some(i) => i,
            None => {
                office.workers.push(Worker::new(
                    ev.worker.clone(),
                    ev.office.clone(),
                    ev.agent,
                    ev.worker.0.clone(),
                    ev.at,
                ));
                office.workers.len() - 1
            }
        };
        let worker = &mut office.workers[idx];
        worker.last_seen = worker.last_seen.max(ev.at);

        match ev.kind {
            EventKind::Seen { name, git_branch } => {
                if !name.is_empty() {
                    worker.name = name;
                }
                if git_branch.is_some() {
                    worker.git_branch = git_branch;
                }
            }
            EventKind::Acted(activity) => worker.activity = activity,
            EventKind::Tokens(n) => worker.tokens_used = worker.tokens_used.max(n),
            EventKind::Turn { in_flight } => worker.turn_in_flight = in_flight,
            EventKind::Left => unreachable!("handled above"),
        }
    }

    /// Age the world: quiet workers go idle, long-quiet ones go home, and
    /// offices that empty out are closed.
    ///
    /// Call once per frame, after applying the frame's events.
    pub fn tick(&mut self, now: Millis) {
        for office in self.offices.values_mut() {
            office.workers.retain(|w| !w.is_offline_at(now));
            for worker in &mut office.workers {
                if worker.is_idle_at(now) && worker.activity.is_busy() {
                    worker.activity = Activity::Idle;
                    worker.turn_in_flight = false;
                }
            }
        }
        self.offices.retain(|_, o| !o.workers.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, WorkerId};

    fn ev(at: Millis, worker: &str, kind: EventKind) -> Event {
        Event {
            at,
            office: OfficeId("/proj".into()),
            office_path: "/proj".into(),
            worker: WorkerId(worker.into()),
            agent: Agent::Codex,
            kind,
        }
    }

    #[test]
    fn first_event_opens_an_office_and_hires_a_worker() {
        let mut w = World::new();
        w.apply(ev(0, "t1", EventKind::Seen { name: "Dev 1".into(), git_branch: None }));
        assert_eq!(w.office_count(), 1);
        assert_eq!(w.worker_count(), 1);
        let office = w.office(&OfficeId("/proj".into())).unwrap();
        assert_eq!(office.name, "proj");
        assert_eq!(office.workers[0].name, "Dev 1");
    }

    #[test]
    fn quiet_workers_go_idle_then_offline_and_close_the_office() {
        let mut w = World::new();
        w.apply(ev(0, "t1", EventKind::Acted(Activity::Typing { detail: "ls".into() })));
        assert!(w.office(&OfficeId("/proj".into())).unwrap().workers[0].activity.is_busy());

        w.tick(crate::IDLE_AFTER_MS + 1);
        assert_eq!(
            w.office(&OfficeId("/proj".into())).unwrap().workers[0].activity,
            Activity::Idle
        );

        w.tick(crate::OFFLINE_AFTER_MS + 1);
        assert_eq!(w.office_count(), 0, "empty offices close");
    }

    #[test]
    fn token_counts_never_go_backwards() {
        let mut w = World::new();
        w.apply(ev(0, "t1", EventKind::Tokens(500)));
        w.apply(ev(1, "t1", EventKind::Tokens(100)));
        assert_eq!(w.office(&OfficeId("/proj".into())).unwrap().workers[0].tokens_used, 500);
    }
}
