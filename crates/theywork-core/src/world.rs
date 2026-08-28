use std::collections::{BTreeMap, HashMap};

use crate::event::{Event, EventKind};
use crate::model::{Activity, Office, OfficeId, Worker, WorkerId};
use crate::Millis;

/// The whole building: every office, every worker, folded from events.
///
/// Offices are kept in a `BTreeMap` so the camera grid has a stable,
/// non-jittering order between frames.
#[derive(Debug, Default, Clone)]
pub struct World {
    offices: BTreeMap<OfficeId, Office>,
    /// Which office each worker currently sits in.
    ///
    /// A worker belongs to exactly one office, always. Agents do report
    /// different working directories over their life, and without this index a
    /// thread that moved would be added to the new office while still sitting
    /// in the old one, putting one developer in two companies at once.
    desks: HashMap<WorkerId, OfficeId>,
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
        // If this worker is already seated somewhere else, they moved. Clear the
        // old desk first so they can never be drawn in two offices at once.
        if let Some(previous) = self.desks.get(&ev.worker) {
            if previous != &ev.office {
                let previous = previous.clone();
                if let Some(old) = self.offices.get_mut(&previous) {
                    old.workers.retain(|w| w.id != ev.worker);
                }
                self.offices.retain(|_, o| !o.workers.is_empty());
            }
        }

        let office = self
            .offices
            .entry(ev.office.clone())
            .or_insert_with(|| Office::new(ev.office.clone(), ev.office_path.clone()));

        if matches!(ev.kind, EventKind::Left) {
            office.workers.retain(|w| w.id != ev.worker);
            self.desks.remove(&ev.worker);
            self.offices.retain(|_, o| !o.workers.is_empty());
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
        self.desks.insert(ev.worker.clone(), ev.office.clone());
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
        let desks = &mut self.desks;
        for office in self.offices.values_mut() {
            office.workers.retain(|w| {
                let stays = !w.is_offline_at(now);
                if !stays {
                    desks.remove(&w.id);
                }
                stays
            });
            for worker in &mut office.workers {
                // Stop animating a quiet worker, but leave `turn_in_flight`
                // alone: an open turn that has gone silent is exactly what
                // `status_at` reads as blocked, and clearing it here would hide
                // every blockage behind a coffee cup.
                if worker.is_idle_at(now) && worker.activity.is_busy() {
                    worker.activity = Activity::Idle;
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
    fn an_open_turn_gone_silent_reads_as_blocked_not_idle() {
        let mut w = World::new();
        w.apply(ev(0, "t1", EventKind::Turn { in_flight: true }));
        w.apply(ev(0, "t1", EventKind::Acted(Activity::Typing { detail: "npm i".into() })));

        let at = |w: &World, now| {
            w.office(&OfficeId("/proj".into())).unwrap().workers[0].status_at(now)
        };

        assert_eq!(at(&w, 1_000), crate::WorkerStatus::Running);

        // Quiet long enough to stop the animation, but not long enough to worry.
        w.tick(crate::IDLE_AFTER_MS + 1);
        assert_eq!(
            at(&w, crate::IDLE_AFTER_MS + 1),
            crate::WorkerStatus::Running,
            "going quiet must not by itself look like a blockage"
        );

        // Still nothing much later: this one needs a human.
        w.tick(crate::BLOCKED_AFTER_MS + 1);
        assert_eq!(at(&w, crate::BLOCKED_AFTER_MS + 1), crate::WorkerStatus::Blocked);
    }

    #[test]
    fn a_finished_turn_reads_as_idle_and_ready_for_work() {
        let mut w = World::new();
        w.apply(ev(0, "t1", EventKind::Turn { in_flight: true }));
        w.apply(ev(10, "t1", EventKind::Turn { in_flight: false }));
        let worker = &w.office(&OfficeId("/proj".into())).unwrap().workers[0];
        assert_eq!(worker.status_at(crate::BLOCKED_AFTER_MS * 2), crate::WorkerStatus::Idle);
    }

    #[test]
    fn a_worker_idle_for_an_hour_is_still_in_the_office() {
        let mut w = World::new();
        w.apply(ev(0, "t1", EventKind::Acted(Activity::Talking { detail: "done".into() })));
        w.apply(ev(0, "t1", EventKind::Turn { in_flight: false }));

        let an_hour = 60 * 60_000;
        w.tick(an_hour);

        // Finished an hour ago and waiting for the next goal. That is exactly
        // the person you want to be able to see.
        assert_eq!(w.worker_count(), 1, "an idle worker must not be sent home");
        let worker = &w.office(&OfficeId("/proj".into())).unwrap().workers[0];
        assert_eq!(worker.status_at(an_hour), crate::WorkerStatus::Idle);
    }

    #[test]
    fn a_worker_who_moves_project_leaves_the_old_office() {
        let mut w = World::new();
        let seen = |at, office: &str| Event {
            at,
            office: OfficeId(office.into()),
            office_path: office.into(),
            worker: WorkerId("t1".into()),
            agent: Agent::Codex,
            kind: EventKind::Seen { name: "Dev 1".into(), git_branch: None },
        };

        w.apply(seen(0, "/alpha"));
        assert_eq!(w.office(&OfficeId("/alpha".into())).unwrap().workers.len(), 1);

        // The same thread now reports a different directory.
        w.apply(seen(10, "/beta"));

        assert_eq!(w.worker_count(), 1, "one thread is one worker, never two");
        assert_eq!(w.office(&OfficeId("/beta".into())).unwrap().workers[0].name, "Dev 1");
        assert!(
            w.office(&OfficeId("/alpha".into())).is_none(),
            "the office they left must not keep a ghost of them"
        );
    }

    #[test]
    fn no_worker_is_ever_seated_in_two_offices() {
        let mut w = World::new();
        for (i, office) in ["/alpha", "/beta", "/alpha", "/gamma"].iter().enumerate() {
            w.apply(Event {
                at: i as Millis,
                office: OfficeId((*office).into()),
                office_path: (*office).into(),
                worker: WorkerId("wanderer".into()),
                agent: Agent::Claude,
                kind: EventKind::Acted(Activity::Thinking),
            });
        }
        let seatings = w.offices().flat_map(|o| &o.workers).count();
        assert_eq!(seatings, 1, "a wandering thread must occupy exactly one desk");
        assert_eq!(w.office_count(), 1);
    }

    #[test]
    fn token_counts_never_go_backwards() {
        let mut w = World::new();
        w.apply(ev(0, "t1", EventKind::Tokens(500)));
        w.apply(ev(1, "t1", EventKind::Tokens(100)));
        assert_eq!(w.office(&OfficeId("/proj".into())).unwrap().workers[0].tokens_used, 500);
    }
}
