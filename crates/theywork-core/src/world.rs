use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use crate::{
    Activity, CollaborationEvent, Event, EventKind, Millis, Office, OfficeId, Relationship,
    RelationshipKind, TreeEntry, Worker, WorkerId, WorkerLifecycle,
};

/// Current offices plus bounded history for workers that leave the live roster.
#[derive(Debug, Default, Clone)]
pub struct World {
    offices: BTreeMap<OfficeId, Office>,
    desks: HashMap<WorkerId, OfficeId>,
    retired: BTreeMap<WorkerId, Worker>,
    relationships: BTreeMap<(WorkerId, WorkerId, RelationshipKind), Relationship>,
    collaboration: VecDeque<CollaborationEvent>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }
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
        self.desks.len()
    }

    /// Includes recently removed workers for delivery and relationship views.
    pub fn worker(&self, id: &WorkerId) -> Option<&Worker> {
        self.desks
            .get(id)
            .and_then(|office| self.offices.get(office))
            .and_then(|office| office.workers.iter().find(|w| &w.id == id))
            .or_else(|| self.retired.get(id))
    }
    pub fn is_present(&self, id: &WorkerId) -> bool {
        self.desks.contains_key(id)
    }
    pub fn retired_workers(&self) -> impl Iterator<Item = &Worker> {
        self.retired.values()
    }
    pub fn relationships(&self) -> impl Iterator<Item = &Relationship> {
        self.relationships.values()
    }
    pub fn collaboration(&self) -> impl DoubleEndedIterator<Item = &CollaborationEvent> {
        self.collaboration.iter()
    }
    pub fn collaboration_for<'a>(
        &'a self,
        id: &'a WorkerId,
    ) -> impl DoubleEndedIterator<Item = &'a CollaborationEvent> {
        self.collaboration
            .iter()
            .filter(move |event| &event.actor == id || event.recipient.as_ref() == Some(id))
    }
    /// Immediate structural children. Session membership is deliberately excluded.
    pub fn children(&self, id: &WorkerId) -> Vec<WorkerId> {
        self.relationships
            .values()
            .filter(|r| &r.parent == id && r.kind != RelationshipKind::SessionMembership)
            .map(|r| r.child.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    /// Immediate parents before more distant ancestors, stable and cycle-safe.
    pub fn ancestors(&self, id: &WorkerId) -> Vec<WorkerId> {
        let mut seen = HashSet::from([id.clone()]);
        let mut queue = VecDeque::from([id.clone()]);
        let mut result = Vec::new();
        while let Some(child) = queue.pop_front() {
            for r in self
                .relationships
                .values()
                .filter(|r| r.child == child && r.kind != RelationshipKind::SessionMembership)
            {
                if seen.insert(r.parent.clone()) {
                    result.push(r.parent.clone());
                    queue.push_back(r.parent.clone());
                }
            }
        }
        result
    }
    /// Connected component, including recorded session membership and missing IDs.
    pub fn family(&self, id: &WorkerId) -> Vec<WorkerId> {
        let mut seen = BTreeSet::from([id.clone()]);
        let mut queue = VecDeque::from([id.clone()]);
        while let Some(node) = queue.pop_front() {
            for r in self.relationships.values() {
                let neighbor = if r.parent == node {
                    Some(&r.child)
                } else if r.child == node {
                    Some(&r.parent)
                } else {
                    None
                };
                if let Some(neighbor) = neighbor {
                    if seen.insert(neighbor.clone()) {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }
        seen.into_iter().collect()
    }
    /// Projection of descendants. Prefer explicit parentage over membership;
    /// a family member with an explicit parent is not also drawn under its root.
    pub fn tree(&self, root: &WorkerId) -> Vec<TreeEntry> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = vec![(root.clone(), 0, None)];
        while let Some((id, depth, via)) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            out.push(TreeEntry {
                worker: id.clone(),
                depth,
                via,
            });
            let mut children = BTreeMap::new();
            for r in self.relationships.values().filter(|r| r.parent == id) {
                if r.kind == RelationshipKind::SessionMembership
                    && self.ancestors(&r.child).contains(root)
                {
                    continue;
                }
                children.entry(r.child.clone()).or_insert(r.kind);
            }
            for (child, kind) in children.into_iter().rev() {
                stack.push((child, depth + 1, Some(kind)));
            }
        }
        out
    }

    fn worker_mut(&mut self, id: &WorkerId) -> Option<&mut Worker> {
        if let Some(office) = self.desks.get(id) {
            return self
                .offices
                .get_mut(office)
                .and_then(|o| o.workers.iter_mut().find(|w| &w.id == id));
        }
        self.retired.get_mut(id)
    }
    fn take_worker(&mut self, id: &WorkerId) -> Option<Worker> {
        let office = self.desks.remove(id)?;
        let office = self.offices.get_mut(&office)?;
        let index = office.workers.iter().position(|worker| &worker.id == id)?;
        Some(office.workers.remove(index))
    }
    fn retire(&mut self, id: &WorkerId) {
        if let Some(mut worker) = self.take_worker(id) {
            if matches!(
                worker.lifecycle,
                WorkerLifecycle::Unknown | WorkerLifecycle::Active
            ) {
                worker.lifecycle = WorkerLifecycle::Removed;
            }
            worker.turn_in_flight = false;
            worker.wait_reason = None;
            self.retired.insert(id.clone(), worker);
        }
        while self.retired.len() > crate::RETIRED_WORKER_LIMIT {
            let oldest = self
                .retired
                .values()
                .min_by_key(|w| (w.last_seen, &w.id))
                .map(|w| w.id.clone());
            if let Some(oldest) = oldest {
                self.retired.remove(&oldest);
                self.relationships
                    .retain(|(parent, child, _), _| parent != &oldest && child != &oldest);
            }
        }
    }

    pub fn apply(&mut self, ev: Event) {
        match &ev.kind {
            EventKind::Identity { identity, role } => {
                if let Some(worker) = self.worker_mut(&ev.worker) {
                    worker.identity = Some(identity.clone());
                    worker.role = *role;
                    return;
                }
            }
            EventKind::Relationship(relationship) => {
                if relationship.parent == relationship.child {
                    return;
                }
                if relationship.kind != RelationshipKind::SessionMembership
                    && self
                        .ancestors(&relationship.parent)
                        .contains(&relationship.child)
                {
                    return;
                }
                let key = (
                    relationship.parent.clone(),
                    relationship.child.clone(),
                    relationship.kind,
                );
                if self
                    .relationships
                    .get(&key)
                    .is_none_or(|old| old.at <= relationship.at)
                {
                    self.relationships.insert(key, relationship.clone());
                }
                // Bound unknown-endpoint links as well as live-worker links.
                if self.relationships.len() > crate::COLLABORATION_HISTORY_LEN * 4 {
                    if let Some(key) = self
                        .relationships
                        .iter()
                        .min_by_key(|(_, r)| r.at)
                        .map(|(k, _)| k.clone())
                    {
                        self.relationships.remove(&key);
                    }
                }
                return;
            }
            EventKind::Collaboration(event) => {
                if let Some(index) = self
                    .collaboration
                    .iter()
                    .position(|old| old.actor == event.actor && old.id == event.id)
                {
                    if self.collaboration[index].at > event.at {
                        return;
                    }
                    self.collaboration.remove(index);
                }
                let index = self
                    .collaboration
                    .iter()
                    .position(|old| old.at > event.at)
                    .unwrap_or(self.collaboration.len());
                self.collaboration.insert(index, event.clone());
                if self.collaboration.len() > crate::COLLABORATION_HISTORY_LEN {
                    self.collaboration.pop_front();
                }
                return;
            }
            EventKind::Coverage(coverage) => {
                if let Some(worker) = self.worker_mut(&ev.worker) {
                    worker.coverage = coverage.clone();
                }
                return;
            }
            EventKind::Left => {
                self.retire(&ev.worker);
                self.offices.retain(|_, office| !office.workers.is_empty());
                return;
            }
            _ => {}
        }
        let moved = self
            .desks
            .get(&ev.worker)
            .is_some_and(|office| office != &ev.office);
        let existing = if moved {
            self.take_worker(&ev.worker)
        } else {
            self.retired.remove(&ev.worker)
        };
        let office = self
            .offices
            .entry(ev.office.clone())
            .or_insert_with(|| Office::new(ev.office.clone(), ev.office_path.clone()));
        let index = match office
            .workers
            .iter()
            .position(|worker| worker.id == ev.worker)
        {
            Some(index) => index,
            None => {
                let mut worker = existing.unwrap_or_else(|| {
                    Worker::new(
                        ev.worker.clone(),
                        ev.office.clone(),
                        ev.agent,
                        ev.worker.0.clone(),
                        ev.at,
                    )
                });
                worker.office = ev.office.clone();
                if worker.lifecycle == WorkerLifecycle::Removed {
                    worker.lifecycle = WorkerLifecycle::Unknown;
                }
                office.workers.push(worker);
                office.workers.len() - 1
            }
        };
        self.desks.insert(ev.worker.clone(), ev.office.clone());
        let worker = &mut office.workers[index];
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
            EventKind::Identity { identity, role } => {
                worker.identity = Some(identity);
                worker.role = role;
            }
            EventKind::Acted(activity) => worker.activity = activity,
            EventKind::Did(beat) => {
                worker.activity = beat.activity.clone();
                worker.remember(beat);
            }
            EventKind::Tokens(n) => worker.tokens_used = worker.tokens_used.max(n),
            EventKind::Turn { in_flight } => {
                worker.turn_in_flight = in_flight;
                if in_flight {
                    worker.lifecycle = WorkerLifecycle::Active;
                }
                if !in_flight {
                    worker.wait_reason = None;
                    if matches!(worker.activity, Activity::Waiting { .. }) {
                        worker.activity = Activity::Idle;
                    }
                }
            }
            EventKind::Lifecycle(lifecycle) => {
                worker.lifecycle = lifecycle;
                if matches!(
                    lifecycle,
                    WorkerLifecycle::Completed
                        | WorkerLifecycle::Failed
                        | WorkerLifecycle::Cancelled
                ) {
                    worker.turn_in_flight = false;
                    worker.wait_reason = None;
                    if matches!(worker.activity, Activity::Waiting { .. }) {
                        worker.activity = Activity::Idle;
                    }
                }
            }
            EventKind::Wait(reason) => {
                worker.wait_reason = reason;
                if reason.is_none() && matches!(worker.activity, Activity::Waiting { .. }) {
                    // The source explicitly cleared the request. Keep the turn
                    // and lifecycle unchanged; no new work has been observed.
                    worker.activity = Activity::Idle;
                }
            }
            EventKind::Relationship(_)
            | EventKind::Collaboration(_)
            | EventKind::Coverage(_)
            | EventKind::Left => unreachable!("handled above"),
        }
        self.offices.retain(|_, office| !office.workers.is_empty());
    }

    pub fn tick(&mut self, now: Millis) {
        let gone: Vec<_> = self
            .offices
            .values()
            .flat_map(|o| &o.workers)
            .filter(|worker| worker.is_offline_at(now))
            .map(|worker| worker.id.clone())
            .collect();
        for id in gone {
            self.retire(&id);
        }
        for office in self.offices.values_mut() {
            for worker in &mut office.workers {
                if worker.is_idle_at(now) && worker.activity.is_busy() {
                    worker.activity = Activity::Idle;
                }
            }
        }
        self.offices.retain(|_, office| !office.workers.is_empty());
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
        w.apply(ev(
            0,
            "t1",
            EventKind::Seen {
                name: "Dev 1".into(),
                git_branch: None,
            },
        ));
        assert_eq!(w.office_count(), 1);
        assert_eq!(w.worker_count(), 1);
        let office = w.office(&OfficeId("/proj".into())).unwrap();
        assert_eq!(office.name, "proj");
        assert_eq!(office.workers[0].name, "Dev 1");
    }

    #[test]
    fn explicit_request_needs_attention_immediately_and_completion_clears_it() {
        use crate::WorkerStatus;
        let mut world = World::new();
        world.apply(ev(1, "waiting", EventKind::Turn { in_flight: true }));
        world.apply(ev(
            2,
            "waiting",
            EventKind::Acted(Activity::Waiting {
                detail: "Approve command".into(),
            }),
        ));
        let status = |w: &World, now| w.offices().next().unwrap().workers[0].status_at(now);
        assert_eq!(status(&world, 2), WorkerStatus::Blocked);
        world.tick(crate::IDLE_AFTER_MS + 3);
        assert_eq!(
            status(&world, crate::IDLE_AFTER_MS + 3),
            WorkerStatus::Blocked
        );
        world.apply(ev(
            crate::IDLE_AFTER_MS + 4,
            "waiting",
            EventKind::Turn { in_flight: false },
        ));
        assert_eq!(status(&world, crate::IDLE_AFTER_MS + 4), WorkerStatus::Idle);
    }

    #[test]
    fn clearing_a_wait_releases_only_the_waiting_pose_without_finishing_the_turn() {
        use crate::{WaitReason, WorkerStatus};
        for (in_flight, status) in [(true, WorkerStatus::Running), (false, WorkerStatus::Idle)] {
            let mut world = World::new();
            world.apply(ev(1, "worker", EventKind::Turn { in_flight }));
            world.apply(ev(
                2,
                "worker",
                EventKind::Wait(Some(WaitReason::HumanApproval)),
            ));
            world.apply(ev(
                2,
                "worker",
                EventKind::Acted(Activity::Waiting {
                    detail: "Approve command one".into(),
                }),
            ));
            assert_eq!(
                world
                    .worker(&WorkerId("worker".into()))
                    .unwrap()
                    .status_at(2),
                WorkerStatus::Blocked
            );
            let lifecycle = world.worker(&WorkerId("worker".into())).unwrap().lifecycle;
            world.apply(ev(3, "worker", EventKind::Wait(None)));
            let worker = world.worker(&WorkerId("worker".into())).unwrap();
            assert_eq!(worker.wait_reason, None);
            assert_eq!(worker.activity, Activity::Idle);
            assert_eq!(worker.status_at(3), status);
            assert_eq!(worker.turn_in_flight, in_flight);
            assert_eq!(worker.lifecycle, lifecycle);
            assert!(
                worker.history.is_empty(),
                "Clearing a request cannot invent an activity beat"
            );
            assert_eq!(
                world.collaboration().count(),
                0,
                "Clearing a request is not a delivery"
            );
            world.apply(ev(
                4,
                "worker",
                EventKind::Wait(Some(WaitReason::HumanInput)),
            ));
            world.apply(ev(
                4,
                "worker",
                EventKind::Acted(Activity::Waiting {
                    detail: "Choose command two".into(),
                }),
            ));
            assert_eq!(
                world
                    .worker(&WorkerId("worker".into()))
                    .unwrap()
                    .status_at(4),
                WorkerStatus::Blocked
            );
        }
    }

    #[test]
    fn clearing_a_wait_preserves_an_observed_action_or_error() {
        for activity in [
            Activity::Typing {
                detail: "Command resumed".into(),
            },
            Activity::Thinking,
            Activity::Error {
                detail: "Command failed".into(),
            },
        ] {
            let mut world = World::new();
            world.apply(ev(
                1,
                "worker",
                EventKind::Wait(Some(crate::WaitReason::HumanApproval)),
            ));
            world.apply(ev(2, "worker", EventKind::Acted(activity.clone())));
            world.apply(ev(3, "worker", EventKind::Wait(None)));
            let worker = world.worker(&WorkerId("worker".into())).unwrap();
            assert_eq!(worker.wait_reason, None);
            assert_eq!(worker.activity, activity);
        }
    }

    #[test]
    fn terminal_lifecycle_clears_a_request_while_failure_remains_a_failure() {
        use crate::{WaitReason, WorkerStatus};
        for (lifecycle, status) in [
            (WorkerLifecycle::Completed, WorkerStatus::Idle),
            (WorkerLifecycle::Cancelled, WorkerStatus::Idle),
            (WorkerLifecycle::Failed, WorkerStatus::Failed),
        ] {
            let mut world = World::new();
            world.apply(ev(1, "worker", EventKind::Turn { in_flight: true }));
            world.apply(ev(
                2,
                "worker",
                EventKind::Wait(Some(WaitReason::HumanApproval)),
            ));
            world.apply(ev(
                2,
                "worker",
                EventKind::Acted(Activity::Waiting {
                    detail: "Pending approval".into(),
                }),
            ));
            world.apply(ev(3, "worker", EventKind::Lifecycle(lifecycle)));
            let worker = world.worker(&WorkerId("worker".into())).unwrap();
            assert_eq!(worker.lifecycle, lifecycle);
            assert_eq!(worker.activity, Activity::Idle);
            assert_eq!(worker.wait_reason, None);
            assert!(!worker.turn_in_flight);
            assert_eq!(worker.status_at(3), status);
            assert!(worker.history.is_empty());
            assert_eq!(
                world.collaboration().count(),
                0,
                "A terminal status does not invent a delivery"
            );
        }
    }

    #[test]
    fn quiet_workers_go_idle_then_offline_and_close_the_office() {
        let mut w = World::new();
        w.apply(ev(
            0,
            "t1",
            EventKind::Acted(Activity::Typing {
                detail: "ls".into(),
            }),
        ));
        assert!(w.office(&OfficeId("/proj".into())).unwrap().workers[0]
            .activity
            .is_busy());

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
        w.apply(ev(
            0,
            "t1",
            EventKind::Acted(Activity::Typing {
                detail: "npm i".into(),
            }),
        ));

        let at =
            |w: &World, now| w.office(&OfficeId("/proj".into())).unwrap().workers[0].status_at(now);

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
        assert_eq!(
            at(&w, crate::BLOCKED_AFTER_MS + 1),
            crate::WorkerStatus::Blocked
        );
    }

    #[test]
    fn a_finished_turn_reads_as_idle_and_ready_for_work() {
        let mut w = World::new();
        w.apply(ev(0, "t1", EventKind::Turn { in_flight: true }));
        w.apply(ev(10, "t1", EventKind::Turn { in_flight: false }));
        let worker = &w.office(&OfficeId("/proj".into())).unwrap().workers[0];
        assert_eq!(
            worker.status_at(crate::BLOCKED_AFTER_MS * 2),
            crate::WorkerStatus::Idle
        );
    }

    #[test]
    fn a_worker_idle_for_an_hour_is_still_in_the_office() {
        let mut w = World::new();
        w.apply(ev(
            0,
            "t1",
            EventKind::Acted(Activity::Talking {
                detail: "done".into(),
            }),
        ));
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
            kind: EventKind::Seen {
                name: "Dev 1".into(),
                git_branch: None,
            },
        };

        w.apply(seen(0, "/alpha"));
        assert_eq!(
            w.office(&OfficeId("/alpha".into())).unwrap().workers.len(),
            1
        );

        // The same thread now reports a different directory.
        w.apply(seen(10, "/beta"));

        assert_eq!(w.worker_count(), 1, "one thread is one worker, never two");
        assert_eq!(
            w.office(&OfficeId("/beta".into())).unwrap().workers[0].name,
            "Dev 1"
        );
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
        assert_eq!(
            seatings, 1,
            "a wandering thread must occupy exactly one desk"
        );
        assert_eq!(w.office_count(), 1);
    }

    #[test]
    fn history_keeps_the_newest_beats_and_stays_bounded() {
        use crate::model::{Beat, Outcome};
        let mut w = World::new();
        for i in 0..(crate::HISTORY_LEN as i64 + 20) {
            w.apply(ev(
                i,
                "t1",
                EventKind::Did(Beat {
                    at: i,
                    activity: Activity::Typing {
                        detail: format!("step {i}"),
                    },
                    outcome: Some(Outcome::Exited(0)),
                }),
            ));
        }
        let worker = &w.office(&OfficeId("/proj".into())).unwrap().workers[0];
        assert_eq!(
            worker.history.len(),
            crate::HISTORY_LEN,
            "history must stay bounded"
        );
        let newest = worker.recent().last().unwrap();
        assert_eq!(
            newest.at,
            crate::HISTORY_LEN as i64 + 19,
            "the newest beat is kept"
        );
        assert_eq!(
            worker.activity,
            Activity::Typing {
                detail: format!("step {}", crate::HISTORY_LEN + 19)
            },
            "a remembered beat is also the current activity"
        );
    }

    #[test]
    fn token_counts_never_go_backwards() {
        let mut w = World::new();
        w.apply(ev(0, "t1", EventKind::Tokens(500)));
        w.apply(ev(1, "t1", EventKind::Tokens(100)));
        assert_eq!(
            w.office(&OfficeId("/proj".into())).unwrap().workers[0].tokens_used,
            500
        );
    }
}
