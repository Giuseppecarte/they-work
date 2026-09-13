//! A shared, deterministic view of recorded work. No inferred progress or goals.
use std::collections::{BTreeMap, BTreeSet};

use theywork_core::{
    Activity, CollaborationEvent, CollaborationKind, Outcome, RelationshipKind, WorkerId, World,
};

use crate::{
    design::{profile_for, CharacterProfile},
    presentation,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkTab {
    #[default]
    Now,
    Activity,
    Team,
    Details,
}
impl WorkTab {
    pub const ALL: [Self; 4] = [Self::Now, Self::Activity, Self::Team, Self::Details];
    pub fn label(self) -> &'static str {
        match self {
            Self::Now => "Now",
            Self::Activity => "Activity",
            Self::Team => "Team",
            Self::Details => "Details",
        }
    }
}

#[derive(Clone, Debug)]
pub struct WorkRecord {
    pub key: String,
    pub at: i64,
    pub actor: WorkerId,
    pub author: String,
    pub heading: String,
    pub text: String,
    pub result: bool,
    pub human_request: bool,
    pub provenance: String,
}
impl WorkRecord {
    pub fn detail(&self) -> String {
        format!("{}\n{}", self.heading, self.text)
    }
}

#[derive(Clone, Debug)]
pub struct TeamMember {
    pub id: WorkerId,
    pub name: String,
    pub title: String,
    pub relation: String,
    pub state: String,
    pub present: bool,
}

#[derive(Clone, Debug)]
pub struct WorkBrief {
    pub worker: WorkerId,
    pub alias: String,
    pub title: String,
    pub project: String,
    pub provider: &'static str,
    pub state: &'static str,
    pub observed: String,
    pub observed_at: i64,
    pub records: Vec<WorkRecord>,
    pub team: Vec<TeamMember>,
    pub coverage: String,
    pub warning: bool,
    pub present: bool,
    pub details: String,
}

pub fn activity_text(activity: &Activity) -> String {
    match activity {
        Activity::Typing { detail } => format!("Running {detail}"),
        Activity::Reading { detail } => format!("Reading {detail}"),
        Activity::Editing { detail } => format!("Editing {detail}"),
        Activity::Searching { detail } => format!("Searching {detail}"),
        Activity::Talking { detail } => format!("Text update: {detail}"),
        Activity::Waiting { detail } => format!("Recorded wait: {detail}"),
        Activity::Error { detail } => format!("Error reported: {detail}"),
        Activity::Thinking => "Processing; no tool activity recorded".into(),
        Activity::Idle => "No current activity reported".into(),
    }
}

pub fn outcome_text(outcome: Option<Outcome>) -> String {
    match outcome {
        Some(Outcome::Exited(code)) => format!("command exited with code {code}"),
        Some(Outcome::Changed { added, removed }) => {
            format!("{added} lines added, {removed} removed")
        }
        None => String::new(),
    }
}

/// Search retained observations without building family graphs or cloning cards.
pub fn searchable_work(world: &World, worker: &theywork_core::Worker) -> String {
    let mut parts = vec![activity_text(&worker.activity)];
    parts.extend(
        worker
            .history
            .iter()
            .map(|beat| activity_text(&beat.activity)),
    );
    parts.extend(
        world
            .collaboration_for(&worker.id)
            .filter_map(|event| event.text.clone()),
    );
    parts.join(" ")
}

pub fn work_preview(world: &World, worker: &theywork_core::Worker, now: i64) -> String {
    let mut observed = activity_text(&worker.activity);
    if worker.coverage.observed_at > 0
        && (!worker.coverage.available || worker.coverage.is_stale_at(now))
    {
        observed = format!("Last recorded: {observed}");
    }
    world
        .collaboration_for(&worker.id)
        .rev()
        .find(|event| event.kind == CollaborationKind::Result)
        .and_then(|event| event.text.as_deref())
        .map_or(observed.clone(), |text| {
            format!("{observed}\nLatest result: {text}")
        })
}

pub fn event_record(
    world: &World,
    event: &CollaborationEvent,
    profiles: &BTreeMap<String, CharacterProfile>,
) -> WorkRecord {
    let label = |id: &WorkerId| {
        world
            .worker(id)
            .map(|worker| format!("{} · {}", profile_for(&id.0, profiles).name, worker.name))
    };
    let actor = label(&event.actor).unwrap_or_else(|| "Unavailable task".into());
    let recipient = event.recipient.as_ref().and_then(label);
    WorkRecord {
        key: format!(
            "event:{}:{}:{}",
            event.actor.0.len(),
            event.actor.0,
            event.id
        ),
        at: event.at,
        actor: event.actor.clone(),
        author: world.worker(&event.actor).map_or_else(
            || "Unavailable task".into(),
            |_| profile_for(&event.actor.0, profiles).name,
        ),
        heading: presentation::event_summary(event, &actor, recipient.as_deref()),
        text: event
            .text
            .clone()
            .unwrap_or_else(|| "No message body recorded.".into()),
        result: event.kind == CollaborationKind::Result,
        human_request: event.kind == CollaborationKind::HumanRequest,
        provenance: format!(
            "Turn: {}\nItem: {}\nEvidence: {:?}",
            event.native_turn_id.as_deref().unwrap_or("not recorded"),
            event.native_item_id.as_deref().unwrap_or("not recorded"),
            event.evidence
        ),
    }
}

pub fn retained_records(
    world: &World,
    worker: &theywork_core::Worker,
    profiles: &BTreeMap<String, CharacterProfile>,
) -> Vec<WorkRecord> {
    let mut records: Vec<_> = world
        .collaboration_for(&worker.id)
        .map(|event| event_record(world, event, profiles))
        .collect();
    records.extend(worker.history.iter().map(|beat| WorkRecord {
        key: format!("beat:{}:{}:{:?}", worker.id.0, beat.at, beat.activity),
        at: beat.at,
        actor: worker.id.clone(),
        author: profile_for(&worker.id.0, profiles).name,
        heading: format!(
            "{} · {}",
            beat.activity.label(),
            profile_for(&worker.id.0, profiles).name
        ),
        text: format!(
            "{}{}",
            activity_text(&beat.activity),
            beat.outcome.map_or(String::new(), |outcome| format!(
                "\n{}",
                outcome_text(Some(outcome))
            ))
        ),
        result: false,
        human_request: false,
        provenance: "Recorded tool or activity observation".into(),
    }));
    records.sort_by(|a, b| b.at.cmp(&a.at).then(a.key.cmp(&b.key)));
    records
}

impl WorkBrief {
    pub fn new(
        world: &World,
        id: &WorkerId,
        profiles: &BTreeMap<String, CharacterProfile>,
        now: i64,
    ) -> Option<Self> {
        let worker = world.worker(id)?;
        let alias = profile_for(&id.0, profiles).name;
        let records = retained_records(world, worker, profiles);
        let team = world
            .family(id)
            .into_iter()
            .filter(|other| other != id)
            .map(|other| {
                let member = world.worker(&other);
                let relation = world
                    .relationships()
                    .filter(|link| link.child == other)
                    .map(|link| {
                        let parent = world.worker(&link.parent).map_or_else(
                            || "unavailable parent".into(),
                            |_| profile_for(&link.parent.0, profiles).name,
                        );
                        match link.kind {
                            RelationshipKind::Delegation => format!("Delegated by {parent}"),
                            RelationshipKind::Fork => format!("Forked from {parent}"),
                            RelationshipKind::SessionMembership => {
                                format!("Session with {parent}; immediate parent unknown")
                            }
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" · ");
                TeamMember {
                    id: other.clone(),
                    name: member.map_or_else(
                        || "Unavailable task".into(),
                        |_| profile_for(&other.0, profiles).name,
                    ),
                    title: member
                        .map_or("Transcript unavailable", |member| member.name.as_str())
                        .into(),
                    relation: if relation.is_empty() {
                        "Recorded family root".into()
                    } else {
                        relation
                    },
                    state: member
                        .map_or("State unavailable", |member| {
                            if world.is_present(&other) {
                                presentation::state_label(member, now)
                            } else {
                                "No longer in current roster"
                            }
                        })
                        .into(),
                    present: world.is_present(&other),
                }
            })
            .collect();
        let history = world.history_window(&worker.office);
        let local_loss = history.evicted_count > 0 || history.prior_local_evictions_unknown;
        let warning = local_loss
            || presentation::coverage_has_loss(&worker.coverage)
            || worker.coverage.observed_at == 0
            || !worker.coverage.available
            || worker.coverage.incomplete
            || worker.coverage.is_stale_at(now);
        let observed = if worker.coverage.observed_at > 0
            && (!worker.coverage.available || worker.coverage.is_stale_at(now))
        {
            format!("Last recorded: {}", activity_text(&worker.activity))
        } else {
            activity_text(&worker.activity)
        };
        let usage = if worker.tokens_used == 0 {
            "Token usage unavailable".into()
        } else {
            format!(
                "{} tokens reported by the source",
                crate::views::human_tokens(worker.tokens_used)
            )
        };
        Some(Self {
            worker:id.clone(),alias,title:worker.name.clone(),project:world.office(&worker.office).map_or_else(||worker.office.0.clone(),|office|office.name.clone()),provider:worker.agent.label(),state:presentation::state_label(worker,now),
            observed,observed_at:worker.last_seen,records,team,coverage:presentation::coverage_headline(&worker.coverage,now,local_loss),warning,present:world.is_present(id),
            details:format!("CONVERSATION\n{}\n\nPROJECT\n{}\nBranch: {}\n\nSOURCE\n{}\n\n{}\n\nUSAGE\n{}\n\nIDENTITY\n{}\n\nRetained observations are bounded; they are not a complete transcript.",worker.name,worker.office.0,worker.git_branch.as_deref().unwrap_or("not recorded"),presentation::coverage_text(&worker.coverage,now),presentation::history_text(&history),usage,worker.identity.as_ref().map_or_else(||"Native identity not recorded".into(),|identity|format!("{} · {}",identity.provider.label(),identity.native_id))),
        })
    }
    pub fn latest_result(&self) -> Option<&WorkRecord> {
        self.records.iter().find(|record| record.result)
    }
    pub fn search_text(&self) -> String {
        format!(
            "{} {} {} {}",
            self.alias,
            self.title,
            self.observed,
            self.records
                .iter()
                .map(|record| record.detail())
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

/// Reading position follows a retained record, not a changing row index.
#[derive(Clone, Debug, Default)]
pub struct ReadingAnchor {
    pub key: Option<String>,
    pub line: usize,
    pub following: bool,
    seen: BTreeSet<String>,
    new_keys: BTreeSet<String>,
    pub expired: bool,
}
impl ReadingAnchor {
    pub fn latest(&mut self) {
        self.key = None;
        self.line = 0;
        self.following = true;
        self.new_keys.clear();
        self.expired = false;
    }
    pub fn observe(&mut self, keys: impl IntoIterator<Item = String>) {
        let keys: BTreeSet<_> = keys.into_iter().collect();
        if !self.following && !self.seen.is_empty() {
            self.new_keys.extend(keys.difference(&self.seen).cloned());
        }
        self.new_keys.retain(|key| keys.contains(key));
        self.expired = self.key.as_ref().is_some_and(|key| !keys.contains(key));
        self.seen = keys;
        if self.following {
            self.new_keys.clear();
        }
    }
    pub fn new_count(&self) -> usize {
        self.new_keys.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activity_preserves_verbs_and_observed_outcomes() {
        assert_eq!(
            activity_text(&Activity::Editing {
                detail: "src/accounts.rs".into()
            }),
            "Editing src/accounts.rs"
        );
        assert_eq!(
            outcome_text(Some(Outcome::Exited(7))),
            "command exited with code 7"
        );
    }

    #[test]
    fn finder_preview_does_not_claim_stale_activity_is_current() {
        let world = World::new();
        let mut worker = theywork_core::Worker::new(
            WorkerId("a".into()),
            theywork_core::OfficeId("p".into()),
            theywork_core::Agent::Codex,
            "Task".into(),
            1,
        );
        worker.activity = Activity::Editing {
            detail: "src/retry.rs".into(),
        };
        worker.coverage.observed_at = 1;
        worker.coverage.available = true;
        assert_eq!(work_preview(&world, &worker, 2), "Editing src/retry.rs");
        assert_eq!(
            work_preview(&world, &worker, i64::MAX),
            "Last recorded: Editing src/retry.rs"
        );
        worker.coverage.available = false;
        assert_eq!(
            work_preview(&world, &worker, 2),
            "Last recorded: Editing src/retry.rs"
        );
        let mut world = World::new();
        world.apply(theywork_core::Event {
            at: 1,
            office: worker.office.clone(),
            office_path: "p".into(),
            worker: worker.id.clone(),
            agent: worker.agent,
            kind: theywork_core::EventKind::Seen {
                name: "Task".into(),
                git_branch: None,
            },
        });
        let brief = WorkBrief::new(&world, &worker.id, &BTreeMap::new(), 2).unwrap();
        assert!(brief.details.contains("Token usage unavailable"));
        assert!(!brief.details.contains("0 tokens reported"));
    }
    #[test]
    fn anchor_counts_new_records_without_losing_selected_identity() {
        let mut anchor = ReadingAnchor::default();
        anchor.latest();
        anchor.observe(["one".into(), "two".into()]);
        anchor.key = Some("one".into());
        anchor.following = false;
        anchor.line = 2;
        anchor.observe(["new".into(), "one".into(), "two".into()]);
        assert_eq!(anchor.key.as_deref(), Some("one"));
        assert_eq!(anchor.line, 2);
        assert_eq!(anchor.new_count(), 1);
        anchor.observe(["new".into(), "two".into()]);
        assert!(anchor.expired);
        anchor.latest();
        assert_eq!(anchor.new_count(), 0);
    }
}
