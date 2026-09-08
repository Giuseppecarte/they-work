//! The project notebook: attention, deliveries, changes and task relationships.

use crate::{
    design::{profile_for, CharacterProfile},
    interaction::{Action as HitAction, HitRegion},
    presentation,
};
use std::collections::{BTreeMap, BTreeSet};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::Frame;
use theywork_core::{
    Activity, CollaborationEvent, CollaborationKind, Millis, Office, OfficeId, RelationshipKind,
    WaitReason, Worker, WorkerId, WorkerStatus, World,
};

use super::{paint_opaque, safe_display, ACCENT, BACKGROUND, INK, MUTED, PANEL, PANEL_HIGHLIGHT};

/// Local reading markers, never provider approvals or task completion state.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ReviewMemory {
    pub version: u32,
    pub last_visits: BTreeMap<String, Millis>,
    pub reviewed: BTreeMap<String, Millis>,
    pub acknowledged: BTreeMap<String, Millis>,
}

impl ReviewMemory {
    pub fn prune(&mut self) {
        self.version = 1;
        for markers in [&mut self.reviewed, &mut self.acknowledged] {
            if markers.len() > 4096 {
                let mut oldest: Vec<_> = markers.iter().map(|(id, at)| (id.clone(), *at)).collect();
                oldest.sort_by_key(|(_, at)| *at);
                for (id, _) in oldest.into_iter().take(markers.len() - 4096) {
                    markers.remove(&id);
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Channel {
    #[default]
    Attention,
    Deliveries,
    Changes,
    Team,
}

impl Channel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Attention => "Attention",
            Self::Deliveries => "Deliveries",
            Self::Changes => "Since your visit",
            Self::Team => "Team",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Attention => 0,
            Self::Deliveries => 1,
            Self::Changes => 2,
            Self::Team => 3,
        }
    }
    fn at(index: usize) -> Self {
        [Self::Attention, Self::Deliveries, Self::Changes, Self::Team][index % 4]
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub key: String,
    pub worker: Option<WorkerId>,
    pub office: OfficeId,
    pub title: String,
    pub label: String,
    pub detail: String,
    pub at: Millis,
    pub reviewed: bool,
    pub depth: usize,
    pub branch: bool,
}

pub struct Workboard {
    pub open: bool,
    pub channel: Channel,
    pub rows: Vec<Entry>,
    pub coverage: String,
    pub scope: String,
    pub folded: BTreeSet<String>,
    selected: usize,
    scroll: u16,
    blocked_by_size: bool,
    pub scope_all: bool,
    details_expanded: bool,
    human_requests: BTreeSet<String>,
    hits: Vec<HitRegion>,
    pending_worker: Option<WorkerId>,
}

impl Default for Workboard {
    fn default() -> Self {
        Self {
            open: false,
            channel: Channel::default(),
            rows: Vec::new(),
            coverage: String::new(),
            scope: String::new(),
            folded: BTreeSet::new(),
            selected: 0,
            scroll: 0,
            blocked_by_size: false,
            scope_all: true,
            details_expanded: false,
            human_requests: BTreeSet::new(),
            hits: Vec::new(),
            pending_worker: None,
        }
    }
}

pub enum Action {
    Open(WorkerId),
    Review(WorkerId),
    Mark(String),
}

fn event_key(event: &CollaborationEvent) -> String {
    format!("{}:{}:{}", event.actor.0.len(), event.actor.0, event.id)
}

fn task_label(world: &World, id: &WorkerId) -> String {
    world.worker(id).map_or_else(
        || {
            format!(
                "Unavailable task ({})",
                id.native_id().chars().take(18).collect::<String>()
            )
        },
        |worker| worker.name.clone(),
    )
}

fn coverage_detail(worker: &Worker, now: Millis) -> String {
    presentation::coverage_text(&worker.coverage, now)
}

fn coverage_summary(workers: &[&Worker], now: Millis) -> String {
    let mut unavailable = 0;
    let mut stale = 0;
    let mut partial = 0;
    let mut unknown = 0;
    for worker in workers {
        let coverage = &worker.coverage;
        if coverage.observed_at == 0 {
            unknown += 1;
        } else if !coverage.available {
            unavailable += 1;
        } else if coverage.is_stale_at(now) {
            stale += 1;
        } else if coverage.incomplete {
            partial += 1;
        }
    }
    let mut parts = vec!["Local history".to_string()];
    for (count, label) in [
        (unavailable, "unavailable"),
        (stale, "stale"),
        (partial, "partial"),
        (unknown, "unknown"),
    ] {
        if count > 0 {
            parts.push(format!("{count} {label}"));
        }
    }
    if workers.is_empty() {
        parts.push("coverage unknown".into());
    }
    parts.join(" · ")
}

impl Workboard {
    /// Select a requested person after the next family roster is populated.
    pub fn focus_worker(&mut self, id: WorkerId) {
        self.pending_worker = Some(id);
        self.scope_all = true;
        self.scroll = 0;
    }
    pub fn hit_regions(&self) -> Vec<HitRegion> {
        self.hits.clone()
    }
    pub fn select_key(&mut self, key: &str) {
        if let Some(index) = self.rows.iter().position(|row| row.key == key) {
            self.selected = index;
            self.scroll = 0;
        }
    }
    pub fn refresh_with_profiles(
        &mut self,
        world: &World,
        office: Option<&Office>,
        memory: &ReviewMemory,
        baselines: &BTreeMap<String, Millis>,
        now: Millis,
        profiles: &BTreeMap<String, CharacterProfile>,
    ) {
        self.refresh(
            world,
            office.filter(|_| !self.scope_all),
            memory,
            baselines,
            now,
        );
        for row in &mut self.rows {
            if let Some(worker) = row.worker.as_ref().and_then(|id| world.worker(id)) {
                row.title = format!(
                    "{} · {}",
                    profile_for(&worker.id.0, profiles).name,
                    row.title
                );
            }
        }
    }
    pub fn refresh(
        &mut self,
        world: &World,
        office: Option<&Office>,
        memory: &ReviewMemory,
        baselines: &BTreeMap<String, Millis>,
        now: Millis,
    ) {
        self.human_requests.clear();
        if self.channel == Channel::Team {
            if let Some(id) = &self.pending_worker {
                for ancestor in world.ancestors(id) {
                    self.folded.remove(&format!("team:{}", ancestor.0));
                }
            }
        }
        self.scope = office.map_or_else(|| "All floors".into(), |o| o.name.clone());
        let selected = |id: &OfficeId| office.is_none_or(|o| o.id == *id);
        let workers: Vec<_> = world
            .offices()
            .flat_map(|office| &office.workers)
            .chain(world.retired_workers())
            .filter(|worker| selected(&worker.office))
            .collect();
        self.coverage = coverage_summary(&workers, now);
        let mut rows = Vec::new();
        match self.channel {
            Channel::Attention => {
                for office in world.offices().filter(|o| selected(&o.id)) {
                    for worker in &office.workers {
                        let covered = worker.coverage.observed_at > 0;
                        let unavailable = covered
                            && (!worker.coverage.available || worker.coverage.is_stale_at(now));
                        let label = if unavailable {
                            "SOURCE UNAVAILABLE"
                        } else {
                            match worker.wait_reason {
                                Some(WaitReason::HumanApproval) => "APPROVAL NEEDED",
                                Some(WaitReason::HumanInput) => "QUESTION FOR YOU",
                                Some(WaitReason::AutomaticReview) => "AUTOMATIC REVIEW",
                                Some(WaitReason::Child) => "WAITING FOR TEAM",
                                Some(WaitReason::Process) => "WAITING FOR PROCESS",
                                _ => match worker.status_at(now) {
                                    WorkerStatus::Failed => "ERROR",
                                    WorkerStatus::Blocked
                                        if matches!(worker.activity, Activity::Waiting { .. }) =>
                                    {
                                        "REQUEST RECORDED"
                                    }
                                    WorkerStatus::Blocked => "NO RECENT ACTIVITY",
                                    _ => continue,
                                },
                            }
                        };
                        let request = (!unavailable
                            && matches!(
                                worker.wait_reason,
                                Some(WaitReason::HumanApproval | WaitReason::HumanInput)
                            ))
                        .then(|| {
                            world.collaboration_for(&worker.id).rev().find(|event| {
                                event.actor == worker.id
                                    && event.kind == CollaborationKind::HumanRequest
                            })
                        })
                        .flatten();
                        let key = request.map_or_else(
                            || format!("attention:{}:{}:{label}", worker.id.0, worker.last_seen),
                            |event| format!("attention-request:{}", event_key(event)),
                        );
                        if !unavailable && presentation::human_request(worker) {
                            self.human_requests.insert(key.clone());
                        }
                        let detail = if unavailable {
                            format!("{}\nLast observation: {} ago. The source cannot confirm the current state.", worker.coverage.detail, super::duration_label(now.saturating_sub(worker.coverage.observed_at)))
                        } else {
                            format!("{}\n\n{}", worker.activity.detail().unwrap_or(worker.activity.label()), match label {
                                "AUTOMATIC REVIEW" => "An automatic reviewer is working. This is not a human approval request.",
                                "WAITING FOR TEAM" => "The source recorded a wait for another agent. Inspect Team for the available relationships.",
                                "NO RECENT ACTIVITY" => "Silence does not prove a pending approval. Check the original task.",
                                _ => "Open the task controls to inspect the live request or return to the original conversation.",
                            })
                        };
                        let detail = format!("{}\n\n{}\n\n{}\n\nMarking seen is a local reading marker. It never approves, answers, or completes this request.", request.and_then(|event| event.text.as_deref()).unwrap_or(&detail), if request.is_some() { "A human request was recorded by the source." } else { "Current observed state." }, coverage_detail(worker, now));
                        rows.push(Entry {
                            reviewed: memory.acknowledged.contains_key(&key),
                            key,
                            worker: Some(worker.id.clone()),
                            office: office.id.clone(),
                            title: format!("{} / {}", office.name, worker.name),
                            label: label.into(),
                            detail,
                            at: request.map_or(worker.last_seen, |event| event.at),
                            depth: 0,
                            branch: false,
                        });
                    }
                }
                rows.sort_by_key(|row| {
                    (
                        match row.label.as_str() {
                            "APPROVAL NEEDED" | "QUESTION FOR YOU" => 0,
                            "ERROR" => 1,
                            "SOURCE UNAVAILABLE" | "NO RECENT ACTIVITY" => 2,
                            _ => 3,
                        },
                        row.at,
                    )
                });
            }
            Channel::Deliveries | Channel::Changes => {
                for event in world.collaboration() {
                    // A returned child result can precede the child's transcript.
                    // The known recipient supplies project context, never actor identity.
                    let actor = world.worker(&event.actor);
                    let context =
                        actor.or_else(|| event.recipient.as_ref().and_then(|id| world.worker(id)));
                    let Some(worker) = context else {
                        continue;
                    };
                    if !selected(&worker.office) {
                        continue;
                    }
                    let baseline = baselines
                        .get(&worker.office.0)
                        .or_else(|| memory.last_visits.get(&worker.office.0))
                        .copied()
                        .unwrap_or(0);
                    if self.channel == Channel::Deliveries
                        && event.kind != CollaborationKind::Result
                    {
                        continue;
                    }
                    if self.channel == Channel::Changes && event.at <= baseline {
                        continue;
                    }
                    let key = event_key(event);
                    let target = event
                        .recipient
                        .as_ref()
                        .map(|id| task_label(world, id))
                        .unwrap_or_else(|| "Not recorded".into());
                    let actor_label = task_label(world, &event.actor);
                    let detail = format!("{}\n\n{}\n\nFrom: {}\nTo: {}\n\n{}{}\n\nRECORDED IDS\nTurn: {}\nItem: {}\nEvidence: {:?}", presentation::event_summary(event,&actor_label,event.recipient.as_ref().map(|_|target.as_str())),event.text.as_deref().unwrap_or("No message body recorded."),actor_label,target,coverage_detail(worker,now),if actor.is_none(){"\nCoverage belongs to the known recipient; the sender's transcript is unavailable."}else if !world.is_present(&event.actor){"\nThe sender is no longer in the current roster. Its recorded output is retained."}else{""},event.native_turn_id.as_deref().unwrap_or("not recorded"),event.native_item_id.as_deref().unwrap_or("not recorded"),event.evidence);
                    rows.push(Entry {
                        reviewed: memory.reviewed.contains_key(&key),
                        key,
                        worker: actor
                            .filter(|worker| world.is_present(&worker.id))
                            .map(|worker| worker.id.clone()),
                        office: worker.office.clone(),
                        title: actor_label,
                        label: format!(
                            "{} · {} ago",
                            presentation::event_label(event.kind),
                            super::duration_label(now.saturating_sub(event.at))
                        ),
                        detail,
                        at: event.at,
                        depth: 0,
                        branch: false,
                    });
                }
                if self.channel == Channel::Changes {
                    for worker in &workers {
                        let baseline = baselines
                            .get(&worker.office.0)
                            .or_else(|| memory.last_visits.get(&worker.office.0))
                            .copied()
                            .unwrap_or(0);
                        for beat in &worker.history {
                            if beat.at <= baseline
                                || matches!(beat.activity, Activity::Talking { .. })
                            {
                                continue;
                            }
                            rows.push(Entry {
                                key: format!(
                                    "beat:{}:{}:{:?}",
                                    worker.id.0, beat.at, beat.activity
                                ),
                                worker: world.is_present(&worker.id).then(|| worker.id.clone()),
                                office: worker.office.clone(),
                                title: worker.name.clone(),
                                label: format!(
                                    "{} · {} ago",
                                    beat.activity.label(),
                                    super::duration_label(now.saturating_sub(beat.at))
                                ),
                                detail: format!(
                                    "{}\n\nRecorded outcome: {:?}\n\n{}",
                                    beat.activity.detail().unwrap_or(beat.activity.label()),
                                    beat.outcome,
                                    coverage_detail(worker, now)
                                ),
                                at: beat.at,
                                reviewed: false,
                                depth: 0,
                                branch: false,
                            });
                        }
                    }
                    self.coverage.push_str(" · older changes may be missing");
                }
                rows.sort_by_key(|row| std::cmp::Reverse(row.at));
            }
            Channel::Team => {
                let mut included = BTreeSet::new();
                let mut projects: BTreeMap<OfficeId, BTreeSet<WorkerId>> = BTreeMap::new();
                for worker in &workers {
                    let family = projects.entry(worker.office.clone()).or_default();
                    if !family.contains(&worker.id) {
                        family.extend(world.family(&worker.id));
                    }
                }
                for (office_id, ids) in projects {
                    // Membership roots must precede their members even when the
                    // child's native ID sorts first. Cyclic membership records
                    // fall back to stable unique traversal, without new parentage.
                    let mut order: Vec<_> = ids
                        .iter()
                        .filter(|id| {
                            !world
                                .relationships()
                                .any(|link| &link.child == *id && ids.contains(&link.parent))
                        })
                        .cloned()
                        .collect();
                    order.extend(ids.iter().cloned());
                    for root in order {
                        if included.contains(&root) {
                            continue;
                        }
                        let mut hidden_depth = None;
                        for node in world.tree(&root) {
                            if hidden_depth.is_some_and(|depth| node.depth > depth) {
                                included.insert(node.worker);
                                continue;
                            }
                            hidden_depth = None;
                            if !included.insert(node.worker.clone()) {
                                continue;
                            }
                            let worker = world.worker(&node.worker);
                            let key = format!("team:{}", node.worker.0);
                            let branch =
                                world.relationships().any(|link| link.parent == node.worker);
                            let relationship = match node.via {
                                Some(RelationshipKind::Delegation) => "delegated",
                                Some(RelationshipKind::SessionMembership) => {
                                    "session member; immediate parent unknown"
                                }
                                Some(RelationshipKind::Fork) => "forked conversation",
                                None => "family root",
                            };
                            let recent = world
                                .collaboration_for(&node.worker)
                                .rev()
                                .take(12)
                                .map(|e| {
                                    format!(
                                        "{}: {}",
                                        presentation::event_label(e.kind),
                                        e.text.as_deref().unwrap_or("no detail")
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n\n");
                            let links = world
                                .relationships()
                                .filter(|link| link.child == node.worker)
                                .map(|link| {
                                    format!(
                                        "{:?} from {} ({:?})",
                                        link.kind,
                                        task_label(world, &link.parent),
                                        link.evidence
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            let source = worker.map_or_else(
                                || "Source: unknown; this task's transcript is unavailable.".into(),
                                |worker| coverage_detail(worker, now),
                            );
                            rows.push(Entry { key: key.clone(), worker: worker.filter(|worker| world.is_present(&worker.id)).map(|w| w.id.clone()), office: worker.map_or_else(|| office_id.clone(), |w| w.office.clone()), title: task_label(world, &node.worker), label: format!("{} · {}", relationship, worker.map_or("unknown", |w| if world.is_present(&w.id) { presentation::state_label(w, now) } else { "not in current roster" })), detail: format!("Relationship shown: {relationship}\n{links}\n\n{}\n\n{source}\n\n{recent}", worker.map_or("No current information; relationship preserved.", |w| w.activity.detail().unwrap_or(w.activity.label()))), at: worker.map_or(0, |w| w.last_seen), reviewed: false, depth: node.depth, branch });
                            if self.folded.contains(&key) {
                                hidden_depth = Some(node.depth);
                            }
                        }
                    }
                }
                self.coverage.push_str(" · ← collapse / → expand");
            }
        }
        if office.is_none() && self.channel != Channel::Attention {
            for row in &mut rows {
                let project = world
                    .office(&row.office)
                    .map(|office| office.name.as_str())
                    .unwrap_or_else(|| {
                        row.office
                            .0
                            .rsplit(['/', '\\'])
                            .find(|part| !part.is_empty())
                            .unwrap_or("project")
                    });
                row.title = format!("{project} / {}", row.title);
            }
        }
        if self.channel == Channel::Attention {
            for row in &mut rows {
                row.label = format!(
                    "{} · {}",
                    if self.human_requests.contains(&row.key) {
                        "FOR YOU"
                    } else {
                        "FOLLOW-UP"
                    },
                    row.label
                );
            }
        }
        self.replace(rows);
        if let Some(id) = self.pending_worker.take() {
            if let Some(index) = self.rows.iter().position(|row| {
                row.worker.as_ref() == Some(&id) || row.key == format!("team:{}", id.0)
            }) {
                self.selected = index;
                self.scroll = 0;
            }
        }
    }
    pub fn show(&mut self, channel: Channel) {
        self.open = true;
        self.channel = channel;
        self.selected = 0;
        self.scroll = 0;
        self.rows.clear();
        self.pending_worker = None;
    }

    pub fn replace(&mut self, rows: Vec<Entry>) {
        let selected = self.rows.get(self.selected).map(|row| row.key.clone());
        self.rows = rows;
        if let Some(index) = selected
            .as_ref()
            .and_then(|id| self.rows.iter().position(|row| &row.key == id))
        {
            self.selected = index;
        }
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        if self.rows.get(self.selected).map(|row| &row.key) != selected.as_ref() {
            self.scroll = 0;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.blocked_by_size && matches!(key.code, KeyCode::Enter | KeyCode::Char('r')) {
            return None;
        }
        match key.code {
            KeyCode::Char('f') => {
                self.scope_all = !self.scope_all;
                self.scroll = 0;
            }
            KeyCode::Char('i') => {
                self.details_expanded = !self.details_expanded;
                self.scroll = 0;
            }
            KeyCode::Esc | KeyCode::Char('b') | KeyCode::Char('q') => self.open = false,
            KeyCode::Char(digit @ '1'..='4') => {
                self.show(Channel::at((digit as u8 - b'1') as usize))
            }
            KeyCode::Tab => self.show(Channel::at(self.channel.index() + 1)),
            KeyCode::BackTab => self.show(Channel::at(self.channel.index() + 3)),
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.rows.len().saturating_sub(1));
                self.scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                self.scroll = 0;
            }
            KeyCode::Home => {
                self.selected = 0;
                self.scroll = 0;
            }
            KeyCode::End => {
                self.selected = self.rows.len().saturating_sub(1);
                self.scroll = 0;
            }
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(5),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(5),
            KeyCode::Left | KeyCode::Right if self.channel == Channel::Team => {
                if let Some(entry) = self.rows.get(self.selected).filter(|entry| entry.branch) {
                    if key.code == KeyCode::Left {
                        self.folded.insert(entry.key.clone());
                    } else {
                        self.folded.remove(&entry.key);
                    }
                }
            }
            KeyCode::Char('r')
                if matches!(self.channel, Channel::Deliveries | Channel::Attention) =>
            {
                return self
                    .rows
                    .get(self.selected)
                    .map(|entry| Action::Mark(entry.key.clone()));
            }
            KeyCode::Enter => {
                return self.rows.get(self.selected).and_then(|entry| {
                    entry.worker.clone().map(|worker| {
                        if self.human_requests.contains(&entry.key) {
                            Action::Review(worker)
                        } else {
                            Action::Open(worker)
                        }
                    })
                });
            }
            _ => {}
        }
        None
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        self.hits.clear();
        let screen = super::below_tab_bar(frame.area());
        let area = Rect::new(screen.x, screen.y, screen.width, screen.height);
        paint_opaque(frame, area, Style::default().bg(BACKGROUND).fg(INK));
        self.blocked_by_size = area.width < 28 || area.height < 10;
        if self.blocked_by_size {
            Paragraph::new("Notebook · enlarge terminal\nEsc return")
                .render(area, frame.buffer_mut());
            return;
        }
        let heading = format!(" PROJECT NOTEBOOK  /  {}", safe_display(&self.scope));
        Paragraph::new(heading)
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .render(Rect::new(area.x, area.y, area.width, 1), frame.buffer_mut());
        let filter = format!(
            "f {} · i {}",
            if !self.scope_all {
                "This floor"
            } else {
                "All floors"
            },
            if self.details_expanded {
                "Hide IDs"
            } else {
                "Show IDs"
            }
        );
        Paragraph::new(filter)
            .style(Style::default().fg(MUTED))
            .render(
                Rect::new(area.x, area.y + 1, area.width, 1),
                frame.buffer_mut(),
            );
        self.hits.push(HitRegion::new(
            Rect::new(area.x, area.y + 1, area.width.min(14), 1),
            HitAction::Key(KeyCode::Char('f')),
        ));
        if area.width > 15 {
            self.hits.push(HitRegion::new(
                Rect::new(area.x + 15, area.y + 1, area.width - 15, 1),
                HitAction::Key(KeyCode::Char('i')),
            ));
        }
        let tabs = (0..4)
            .flat_map(|index| {
                let channel = Channel::at(index);
                let label = if area.width < 72 {
                    ["Attn", "Out", "New", "Team"][index]
                } else {
                    channel.label()
                };
                [
                    Span::styled(
                        format!("{} {}", index + 1, label),
                        Style::default()
                            .fg(if channel == self.channel { INK } else { MUTED })
                            .bg(if channel == self.channel {
                                PANEL_HIGHLIGHT
                            } else {
                                BACKGROUND
                            }),
                    ),
                    Span::raw(" "),
                ]
            })
            .collect::<Vec<_>>();
        Paragraph::new(Line::from(tabs)).render(
            Rect::new(area.x, area.y + 2, area.width, 1),
            frame.buffer_mut(),
        );
        let mut tab_x = area.x;
        for index in 0..4 {
            let channel = Channel::at(index);
            let label = if area.width < 72 {
                ["Attn", "Out", "New", "Team"][index]
            } else {
                channel.label()
            };
            let width = (label.len() + 3) as u16;
            if tab_x + width <= area.right() {
                self.hits.push(HitRegion::new(
                    Rect::new(tab_x, area.y + 2, width, 1),
                    HitAction::Key(KeyCode::Char((b'1' + index as u8) as char)),
                ));
            }
            tab_x += width;
        }
        let footer = Rect::new(area.x, area.bottom() - 2, area.width, 2);
        let controls = match (area.width < 72, self.channel) {
            (true, Channel::Attention | Channel::Deliveries) => "Esc back · Enter · r seen",
            (true, Channel::Team) => "Esc · Enter · ←/→ fold",
            (true, Channel::Changes) => "Esc back · Enter · PgUp/Dn",
            (false, Channel::Attention | Channel::Deliveries) => {
                "Esc return · ↑↓ select · Enter inspect · r seen locally · PgUp/PgDn detail"
            }
            (false, Channel::Team) => {
                "Esc return · ↑↓ select · Enter inspect · ←/→ fold · PgUp/PgDn detail"
            }
            (false, Channel::Changes) => {
                "Esc return · ↑↓ select · Enter inspect · PgUp/PgDn detail"
            }
        };
        Paragraph::new(format!("{}\n{controls}", safe_display(&self.coverage)))
            .style(Style::default().fg(MUTED))
            .render(footer, frame.buffer_mut());
        let body = Rect::new(area.x + 1, area.y + 4, area.width - 2, area.height - 6);
        if self.rows.is_empty() {
            Paragraph::new(match self.channel {
                Channel::Attention => "No attention items in the available observations.",
                Channel::Deliveries => {
                    "No recorded deliveries yet. A quiet or finished turn alone is not a delivery."
                }
                Channel::Changes => {
                    "No recorded changes since the previous visit. History may be incomplete."
                }
                Channel::Team => "No tasks in this project yet.",
            })
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(MUTED))
            .render(body, frame.buffer_mut());
            return;
        }
        let list_height = (body.height / 2)
            .clamp(1, 12)
            .min(self.rows.len().min(u16::MAX as usize) as u16);
        let capacity = usize::from(list_height);
        let start = self.selected / capacity * capacity;
        for (offset, row) in self.rows.iter().skip(start).take(capacity).enumerate() {
            self.hits.push(HitRegion::new(
                Rect::new(body.x, body.y + offset as u16, body.width, 1),
                HitAction::NotebookSelect(row.key.clone()),
            ));
            let selected = self.selected == start + offset;
            let marker = if selected { ">" } else { " " };
            let indent = "  ".repeat(row.depth.min(6));
            let branch = if row.branch {
                if self.folded.contains(&row.key) {
                    "+ "
                } else {
                    "− "
                }
            } else {
                ""
            };
            let read = if row.reviewed { " [seen]" } else { "" };
            let text = format!(
                "{marker} {indent}{branch}{} · {}{read}",
                safe_display(&row.title),
                safe_display(&row.label)
            );
            Paragraph::new(text)
                .style(
                    Style::default()
                        .fg(if selected { INK } else { MUTED })
                        .bg(if selected {
                            PANEL_HIGHLIGHT
                        } else {
                            BACKGROUND
                        }),
                )
                .render(
                    Rect::new(body.x, body.y + offset as u16, body.width, 1),
                    frame.buffer_mut(),
                );
        }
        let row = &self.rows[self.selected];
        if let Some(worker) = &row.worker {
            let label = if self.human_requests.contains(&row.key) {
                "[ Review request ]"
            } else {
                "[ Inspect task ]"
            };
            let width = label.len() as u16;
            if width <= body.width {
                let button = Rect::new(body.right() - width, body.y + list_height, width, 1);
                Paragraph::new(label)
                    .style(Style::default().fg(INK).bg(PANEL_HIGHLIGHT))
                    .render(button, frame.buffer_mut());
                self.hits.push(HitRegion::new(
                    button,
                    if self.human_requests.contains(&row.key) {
                        HitAction::Review(worker.clone())
                    } else {
                        HitAction::Inspect(worker.clone())
                    },
                ));
            }
        }
        if matches!(self.channel, Channel::Attention | Channel::Deliveries) && body.width >= 36 {
            let button = Rect::new(body.x, body.y + list_height, 14, 1);
            Paragraph::new("[ Mark seen ]")
                .style(Style::default().fg(INK).bg(PANEL_HIGHLIGHT))
                .render(button, frame.buffer_mut());
            self.hits
                .push(HitRegion::new(button, HitAction::Mark(row.key.clone())));
        }
        let detail_area = Rect::new(
            body.x,
            body.y + list_height + 1,
            body.width,
            body.height.saturating_sub(list_height + 1),
        );
        if detail_area.height > 0 {
            paint_opaque(frame, detail_area, Style::default().bg(PANEL));
            let text = format!(
                "{}  ·  {}/{}\n{}\n\n{}",
                safe_display(&row.label),
                self.selected + 1,
                self.rows.len(),
                safe_display(&row.title),
                super::safe_multiline(if self.details_expanded {
                    &row.detail
                } else {
                    row.detail
                        .split("\n\nRECORDED IDS\n")
                        .next()
                        .unwrap_or(&row.detail)
                })
            );
            let lines = super::wrap_text(&text, detail_area.width);
            self.scroll = self.scroll.min(
                lines
                    .len()
                    .saturating_sub(detail_area.height as usize)
                    .min(u16::MAX as usize) as u16,
            );
            let paragraph =
                Paragraph::new(lines.join("\n")).style(Style::default().fg(INK).bg(PANEL));
            paragraph
                .scroll((self.scroll, 0))
                .render(detail_area, frame.buffer_mut());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use ratatui::{backend::TestBackend, Terminal};
    use theywork_core::{
        Agent, Beat, CoverageLevel, Event, EventKind, Evidence, Relationship, SourceCoverage,
        BLOCKED_AFTER_MS,
    };

    fn id(name: &str) -> WorkerId {
        WorkerId(name.into())
    }
    fn observe(world: &mut World, worker: &str, at: Millis, kind: EventKind) {
        world.apply(Event {
            at,
            office: OfficeId("/project".into()),
            office_path: "/project".into(),
            worker: id(worker),
            agent: Agent::Codex,
            kind,
        });
    }
    fn hire(world: &mut World, worker: &str) {
        observe(
            world,
            worker,
            10,
            EventKind::Seen {
                name: worker.into(),
                git_branch: None,
            },
        );
    }
    fn record(
        world: &mut World,
        actor: &str,
        recipient: Option<&str>,
        key: &str,
        kind: CollaborationKind,
        at: Millis,
    ) {
        observe(
            world,
            actor,
            at,
            EventKind::Collaboration(CollaborationEvent {
                id: key.into(),
                at,
                actor: id(actor),
                recipient: recipient.map(id),
                kind,
                text: Some(format!("Body of {key}")),
                correlation_id: Some(key.into()),
                native_turn_id: Some("turn-1".into()),
                native_item_id: Some(key.into()),
                evidence: Evidence::NativeEvent,
            }),
        );
    }
    fn link(world: &mut World, parent: &str, child: &str, kind: RelationshipKind) {
        observe(
            world,
            parent,
            20,
            EventKind::Relationship(Relationship {
                parent: id(parent),
                child: id(child),
                kind,
                evidence: Evidence::NativeMetadata,
                at: 20,
                correlation_id: None,
            }),
        );
    }
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn refresh(board: &mut Workboard, world: &World, memory: &ReviewMemory) {
        board.refresh(world, None, memory, &BTreeMap::new(), 1000);
    }
    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn local_read_marker_never_answers_approves_or_completes_a_request() {
        let mut world = World::new();
        hire(&mut world, "reviewer");
        observe(
            &mut world,
            "reviewer",
            100,
            EventKind::Turn { in_flight: true },
        );
        observe(
            &mut world,
            "reviewer",
            110,
            EventKind::Wait(Some(WaitReason::HumanApproval)),
        );
        record(
            &mut world,
            "reviewer",
            None,
            "approval-1",
            CollaborationKind::HumanRequest,
            110,
        );
        let before = serde_json::to_value(world.worker(&id("reviewer")).unwrap()).unwrap();
        let mut ui = crate::Ui::new();
        ui.tick(1000);
        ui.workboard.show(Channel::Attention);
        refresh(&mut ui.workboard, &world, &ReviewMemory::default());
        assert!(
            ui.handle_key(key(KeyCode::Char('r'))).is_none(),
            "Reading must not dispatch a control command"
        );
        let memory = ui.review_memory();
        assert_eq!(memory.acknowledged.len(), 1);
        assert!(memory.reviewed.is_empty());
        refresh(&mut ui.workboard, &world, &memory);
        assert!(ui.workboard.rows[0].reviewed);
        assert!(ui.workboard.rows[0].detail.contains("never approves"));
        assert_eq!(
            serde_json::to_value(world.worker(&id("reviewer")).unwrap()).unwrap(),
            before
        );
        assert_eq!(
            world.worker(&id("reviewer")).unwrap().status_at(1000),
            WorkerStatus::Blocked
        );
    }

    #[test]
    fn native_human_request_marker_survives_other_beats_and_changes_for_a_new_request() {
        let mut world = World::new();
        hire(&mut world, "lead");
        observe(
            &mut world,
            "lead",
            100,
            EventKind::Wait(Some(WaitReason::HumanInput)),
        );
        record(
            &mut world,
            "lead",
            None,
            "question-1",
            CollaborationKind::HumanRequest,
            100,
        );
        let mut board = Workboard::default();
        let mut memory = ReviewMemory::default();
        refresh(&mut board, &world, &memory);
        let original = board.rows[0].key.clone();
        memory.acknowledged.insert(original.clone(), 1000);
        observe(&mut world, "lead", 300, EventKind::Tokens(123));
        observe(
            &mut world,
            "lead",
            400,
            EventKind::Did(Beat {
                at: 400,
                activity: Activity::Thinking,
                outcome: None,
            }),
        );
        record(
            &mut world,
            "lead",
            None,
            "another-message",
            CollaborationKind::Message,
            450,
        );
        observe(
            &mut world,
            "lead",
            500,
            EventKind::Coverage(SourceCoverage {
                available: true,
                observed_at: 500,
                ..Default::default()
            }),
        );
        refresh(&mut board, &world, &memory);
        assert_eq!(board.rows[0].key, original);
        assert_eq!(board.rows[0].at, 100);
        assert!(board.rows[0].reviewed);
        assert!(board.rows[0].detail.contains("Body of question-1"));
        record(
            &mut world,
            "lead",
            None,
            "question-2",
            CollaborationKind::HumanRequest,
            600,
        );
        refresh(&mut board, &world, &memory);
        assert_ne!(board.rows[0].key, original);
        assert!(!board.rows[0].reviewed);
    }

    #[test]
    fn recorded_deliveries_keep_unknown_recipients_and_absent_senders_honest() {
        let mut world = World::new();
        hire(&mut world, "parent");
        record(
            &mut world,
            "parent",
            None,
            "root-output",
            CollaborationKind::Result,
            100,
        );
        record(
            &mut world,
            "child-missing",
            Some("parent"),
            "child-output",
            CollaborationKind::Result,
            200,
        );
        record(
            &mut world,
            "parent",
            None,
            "ordinary-comment",
            CollaborationKind::Message,
            300,
        );
        let mut board = Workboard::default();
        board.show(Channel::Deliveries);
        refresh(&mut board, &world, &ReviewMemory::default());
        assert_eq!(board.rows.len(), 2);
        assert!(board.rows[0].title.contains("Unavailable task"));
        assert!(board.rows[0].detail.contains("To: parent"));
        assert!(board.rows[0]
            .detail
            .contains("sender's transcript is unavailable"));
        assert!(board.rows[0].worker.is_none());
        assert!(board.rows[1].detail.contains("To: Not recorded"));
        assert!(!board.rows.iter().any(|row| row.detail.contains("To: user")));
        assert!(board.handle_key(key(KeyCode::Enter)).is_none());
        assert!(board.open);
    }

    #[test]
    fn retired_child_delivery_stays_readable_in_its_project() {
        let mut world = World::new();
        hire(&mut world, "parent");
        hire(&mut world, "child");
        link(&mut world, "parent", "child", RelationshipKind::Delegation);
        record(
            &mut world,
            "child",
            Some("parent"),
            "result",
            CollaborationKind::Result,
            100,
        );
        observe(&mut world, "child", 110, EventKind::Left);
        let mut board = Workboard::default();
        board.show(Channel::Deliveries);
        board.refresh(
            &world,
            world.office(&OfficeId("/project".into())),
            &ReviewMemory::default(),
            &BTreeMap::new(),
            1000,
        );
        assert_eq!(board.rows.len(), 1);
        assert_eq!(board.rows[0].title, "child");
        assert!(board.rows[0]
            .detail
            .contains("no longer in the current roster"));
        assert!(
            board.rows[0].worker.is_none(),
            "Enter cannot silently open a live seat that no longer exists"
        );
        let Action::Mark(marker) = board.handle_key(key(KeyCode::Char('r'))).unwrap() else {
            panic!("expected local marker");
        };
        let mut memory = ReviewMemory::default();
        memory.reviewed.insert(marker, 1000);
        refresh(&mut board, &world, &memory);
        assert!(board.rows[0].reviewed);
        assert!(world.worker(&id("child")).is_some());
        assert!(!world.is_present(&id("child")));
    }

    #[test]
    fn changes_use_entry_baseline_and_include_retired_history_without_talking_duplicates() {
        let mut world = World::new();
        hire(&mut world, "child");
        record(
            &mut world,
            "child",
            None,
            "old",
            CollaborationKind::Result,
            100,
        );
        record(
            &mut world,
            "child",
            None,
            "new",
            CollaborationKind::Result,
            200,
        );
        for (at, activity) in [
            (100, Activity::Thinking),
            (
                250,
                Activity::Reading {
                    detail: "result.rs".into(),
                },
            ),
            (
                200,
                Activity::Talking {
                    detail: "Body of new".into(),
                },
            ),
        ] {
            observe(
                &mut world,
                "child",
                at,
                EventKind::Did(Beat {
                    at,
                    activity,
                    outcome: None,
                }),
            );
        }
        observe(&mut world, "child", 300, EventKind::Left);
        let mut memory = ReviewMemory::default();
        memory.last_visits.insert("/project".into(), 500);
        let baselines = BTreeMap::from([("/project".into(), 100)]);
        let mut board = Workboard::default();
        board.show(Channel::Changes);
        board.refresh(&world, None, &memory, &baselines, 1000);
        assert_eq!(
            board.rows.iter().map(|row| row.at).collect::<Vec<_>>(),
            vec![250, 200]
        );
        memory.last_visits.insert("/project".into(), 1100);
        board.refresh(&world, None, &memory, &baselines, 1200);
        assert_eq!(
            board.rows.len(),
            2,
            "Drawing/revisiting the panel does not advance its entry baseline"
        );
        board.refresh(&world, None, &memory, &BTreeMap::new(), 1200);
        assert!(
            board.rows.is_empty(),
            "A restored visit is used when no entry baseline exists"
        );
    }

    #[test]
    fn team_preserves_nested_missing_member_and_fork_relationships_once() {
        let mut world = World::new();
        for name in [
            "z-root", "a-child", "b-leaf", "0-member", "c-fork", "d-orphan",
        ] {
            hire(&mut world, name);
        }
        for (parent, child, kind) in [
            ("z-root", "a-child", RelationshipKind::Delegation),
            ("a-child", "b-leaf", RelationshipKind::Delegation),
            ("z-root", "b-leaf", RelationshipKind::SessionMembership),
            ("z-root", "0-member", RelationshipKind::SessionMembership),
            ("z-root", "c-fork", RelationshipKind::Fork),
            ("missing", "d-orphan", RelationshipKind::Delegation),
            ("z-root", "a-child", RelationshipKind::Delegation),
        ] {
            link(&mut world, parent, child, kind);
        }
        observe(&mut world, "b-leaf", 100, EventKind::Left);
        let mut board = Workboard::default();
        board.show(Channel::Team);
        refresh(&mut board, &world, &ReviewMemory::default());
        assert_eq!(board.rows.len(), 7);
        assert_eq!(
            board
                .rows
                .iter()
                .map(|row| &row.key)
                .collect::<BTreeSet<_>>()
                .len(),
            7
        );
        let row = |name: &str| {
            board
                .rows
                .iter()
                .find(|row| row.key == format!("team:{name}"))
                .unwrap()
        };
        assert_eq!(row("z-root").depth, 0);
        assert_eq!(row("a-child").depth, 1);
        assert_eq!(row("b-leaf").depth, 2);
        assert!(row("b-leaf").label.contains("not in current roster"));
        assert_eq!(row("0-member").depth, 1);
        assert!(row("0-member").label.contains("immediate parent unknown"));
        assert_eq!(row("c-fork").depth, 1);
        assert!(row("c-fork").label.contains("forked conversation"));
        assert!(row("missing").worker.is_none());
        assert!(row("missing").detail.contains("transcript is unavailable"));
        assert_eq!(row("d-orphan").depth, 1);
    }

    #[test]
    fn session_only_family_can_collapse_even_when_child_id_sorts_first() {
        let mut world = World::new();
        hire(&mut world, "a-member");
        hire(&mut world, "z-session");
        link(
            &mut world,
            "z-session",
            "a-member",
            RelationshipKind::SessionMembership,
        );
        let mut board = Workboard::default();
        board.show(Channel::Team);
        refresh(&mut board, &world, &ReviewMemory::default());
        assert_eq!(board.rows[0].title, "project / z-session");
        assert!(board.rows[0].branch);
        assert_eq!(board.rows[1].depth, 1);
        board.handle_key(key(KeyCode::Left));
        refresh(&mut board, &world, &ReviewMemory::default());
        assert_eq!(board.rows.len(), 1);
        board.handle_key(key(KeyCode::Right));
        refresh(&mut board, &world, &ReviewMemory::default());
        assert_eq!(board.rows.len(), 2);
        assert!(world.ancestors(&id("a-member")).is_empty());
    }

    #[test]
    fn coverage_distinguishes_partial_stale_unavailable_and_unknown_sources() {
        let mut world = World::new();
        let now = BLOCKED_AFTER_MS + 100;
        for (name, available, observed_at) in [
            ("partial", true, now),
            ("stale", true, 1),
            ("offline", false, now),
        ] {
            hire(&mut world, name);
            observe(
                &mut world,
                name,
                now,
                EventKind::Coverage(SourceCoverage {
                    available,
                    incomplete: true,
                    observed_at,
                    relationships: CoverageLevel::Partial,
                    detail: format!("Coverage for {name}"),
                    ..Default::default()
                }),
            );
        }
        hire(&mut world, "unknown");
        let mut board = Workboard::default();
        board.refresh(
            &world,
            None,
            &ReviewMemory::default(),
            &BTreeMap::new(),
            now,
        );
        for label in ["1 partial", "1 stale", "1 unavailable", "1 unknown"] {
            assert!(board.coverage.contains(label));
        }
        assert_eq!(board.rows.len(), 2);
        assert!(board
            .rows
            .iter()
            .any(|row| row.detail.contains("Source needs refreshing")));
        assert!(board
            .rows
            .iter()
            .any(|row| row.detail.contains("Source unavailable")));
        board.show(Channel::Team);
        board.refresh(
            &world,
            None,
            &ReviewMemory::default(),
            &BTreeMap::new(),
            now,
        );
        assert!(board
            .rows
            .iter()
            .any(|row| row.detail.contains("Source not checked")));
        assert!(board
            .rows
            .iter()
            .any(|row| row.detail.contains("Partial local history")));
    }

    #[test]
    fn small_notebook_keeps_all_channels_escape_and_scrollable_safe_detail() {
        let mut world = World::new();
        hire(&mut world, "worker\u{1b}[31m");
        record(
            &mut world,
            "worker\u{1b}[31m",
            None,
            "result",
            CollaborationKind::Result,
            100,
        );
        for (width, height) in [(28, 12), (40, 16), (80, 24), (120, 36)] {
            let mut board = Workboard::default();
            board.show(Channel::Deliveries);
            refresh(&mut board, &world, &ReviewMemory::default());
            board.rows[0].detail = (0..60)
                .map(|n| format!("Line {n:02} 日本語 café\u{1b}[31m"))
                .collect::<Vec<_>>()
                .join("\n");
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| board.draw(frame)).unwrap();
            let before = buffer_text(&terminal);
            assert!(before.contains("4 Team"), "{width} columns lost a channel");
            assert!(before.contains("Esc"));
            assert!(!before.contains('\u{1b}'));
            for _ in 0..100 {
                board.handle_key(key(KeyCode::PageDown));
            }
            terminal.draw(|frame| board.draw(frame)).unwrap();
            let after = buffer_text(&terminal);
            assert_ne!(before, after);
            assert!(
                after.contains("Line 59"),
                "{width} columns cannot reach the last line"
            );
            assert!(
                board.scroll < 500,
                "rendering clamps scroll to the wrapped content"
            );
            board.handle_key(key(KeyCode::Home));
            assert_eq!(board.scroll, 0);
        }
    }

    #[test]
    fn one_delivery_gives_unused_list_space_to_its_body() {
        let mut world = World::new();
        hire(&mut world, "worker");
        record(
            &mut world,
            "worker",
            None,
            "result",
            CollaborationKind::Result,
            100,
        );
        let mut board = Workboard::default();
        board.show(Channel::Deliveries);
        refresh(&mut board, &world, &ReviewMemory::default());
        board.rows[0].detail = (0..10)
            .map(|n| format!("Delivery line {n:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| board.draw(frame)).unwrap();
        let text = buffer_text(&terminal);
        assert!(
            text.contains("Delivery line 09"),
            "An empty list must not hide a short delivery body"
        );
        assert_eq!(board.scroll, 0);
    }

    #[test]
    fn all_floor_titles_distinguish_projects_and_help_matches_the_channel() {
        let mut world = World::new();
        for path in ["/one", "/two"] {
            world.apply(Event {
                at: 10,
                office: OfficeId(path.into()),
                office_path: path.into(),
                worker: id(path),
                agent: Agent::Codex,
                kind: EventKind::Seen {
                    name: "Same title".into(),
                    git_branch: None,
                },
            });
        }
        let mut board = Workboard::default();
        board.show(Channel::Team);
        refresh(&mut board, &world, &ReviewMemory::default());
        let titles: BTreeSet<_> = board.rows.iter().map(|row| row.title.as_str()).collect();
        assert_eq!(
            titles,
            BTreeSet::from(["one / Same title", "two / Same title"])
        );
        for channel in [Channel::Team, Channel::Changes] {
            board.show(channel);
            refresh(&mut board, &world, &ReviewMemory::default());
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal.draw(|frame| board.draw(frame)).unwrap();
            assert!(!buffer_text(&terminal).contains("r seen"));
            assert!(board.handle_key(key(KeyCode::Char('r'))).is_none());
        }
        board.show(Channel::Team);
        board.refresh(
            &world,
            world.office(&OfficeId("/one".into())),
            &ReviewMemory::default(),
            &BTreeMap::new(),
            1000,
        );
        assert_eq!(board.rows.len(), 1);
        assert_eq!(board.rows[0].title, "Same title");
    }

    #[test]
    fn undersized_notice_cannot_mark_or_open_hidden_rows_until_resized() {
        let mut world = World::new();
        hire(&mut world, "worker");
        record(
            &mut world,
            "worker",
            None,
            "result",
            CollaborationKind::Result,
            100,
        );
        let mut board = Workboard::default();
        board.show(Channel::Deliveries);
        refresh(&mut board, &world, &ReviewMemory::default());
        let mut tiny = Terminal::new(TestBackend::new(20, 8)).unwrap();
        tiny.draw(|frame| board.draw(frame)).unwrap();
        assert!(board.handle_key(key(KeyCode::Char('r'))).is_none());
        assert!(board.handle_key(key(KeyCode::Enter)).is_none());
        assert!(board.open);
        let mut readable = Terminal::new(TestBackend::new(28, 12)).unwrap();
        readable.draw(|frame| board.draw(frame)).unwrap();
        assert!(buffer_text(&readable).contains("r seen"));
        assert!(matches!(
            board.handle_key(key(KeyCode::Char('r'))),
            Some(Action::Mark(_))
        ));
        assert!(matches!(
            board.handle_key(key(KeyCode::Enter)),
            Some(Action::Open(_))
        ));
        board.handle_key(key(KeyCode::Esc));
        assert!(!board.open);
    }

    #[test]
    fn global_attention_separates_human_requests_and_supports_visible_floor_filter() {
        let mut world = World::new();
        hire(&mut world, "human");
        observe(
            &mut world,
            "human",
            100,
            EventKind::Wait(Some(WaitReason::HumanInput)),
        );
        record(
            &mut world,
            "human",
            None,
            "question",
            CollaborationKind::HumanRequest,
            100,
        );
        world.apply(Event {
            at: 100,
            office: OfficeId("/other".into()),
            office_path: "/other".into(),
            worker: id("process"),
            agent: Agent::Codex,
            kind: EventKind::Wait(Some(WaitReason::Process)),
        });
        let profiles = BTreeMap::from([(
            "human".into(),
            CharacterProfile {
                name: "Avery".into(),
                ..Default::default()
            },
        )]);
        let mut board = Workboard::default();
        board.show(Channel::Attention);
        board.refresh_with_profiles(
            &world,
            world.office(&OfficeId("/project".into())),
            &ReviewMemory::default(),
            &BTreeMap::new(),
            1000,
            &profiles,
        );
        assert_eq!(board.rows.len(), 2);
        assert!(board.rows[0].label.starts_with("FOR YOU"));
        assert!(board.rows[1].label.starts_with("FOLLOW-UP"));
        assert!(board.rows[0].title.contains("Avery"));
        assert!(
            matches!(board.handle_key(key(KeyCode::Enter)),Some(Action::Review(worker)) if worker==id("human"))
        );
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| board.draw(frame)).unwrap();
        assert!(buffer_text(&terminal).contains("f All floors"));
        assert!(board
            .hit_regions()
            .iter()
            .any(|hit| hit.action == HitAction::Review(id("human"))));
        board.handle_key(key(KeyCode::Char('f')));
        board.refresh_with_profiles(
            &world,
            world.office(&OfficeId("/project".into())),
            &ReviewMemory::default(),
            &BTreeMap::new(),
            1000,
            &profiles,
        );
        assert_eq!(board.rows.len(), 1);
        assert!(!board.scope_all);
    }

    #[test]
    fn event_details_start_with_a_narrative_and_hide_native_ids_until_expanded() {
        let mut world = World::new();
        hire(&mut world, "sender");
        record(
            &mut world,
            "sender",
            None,
            "native-item-123",
            CollaborationKind::Result,
            100,
        );
        let mut board = Workboard::default();
        board.show(Channel::Deliveries);
        refresh(&mut board, &world, &ReviewMemory::default());
        let mut terminal = Terminal::new(TestBackend::new(120, 36)).unwrap();
        terminal.draw(|frame| board.draw(frame)).unwrap();
        let before = buffer_text(&terminal);
        assert!(before.contains("sender delivered a result"));
        // The fixture body intentionally contains its ID; test the metadata labels.
        assert!(!before.contains("Turn: turn-1"));
        assert!(!before.contains("Item: native-item-123"));
        board.handle_key(key(KeyCode::Char('i')));
        terminal.draw(|frame| board.draw(frame)).unwrap();
        let after = buffer_text(&terminal);
        assert!(after.contains("Turn: turn-1"));
        assert!(after.contains("Item: native-item-123"));
        let key = board.rows[0].key.clone();
        assert!(board
            .hit_regions()
            .iter()
            .any(|hit| hit.action == HitAction::Mark(key.clone())));
        board.select_key("no-longer-visible");
        assert_eq!(board.rows[board.selected].key, key);
    }

    #[test]
    fn selection_follows_delivery_identity_across_reordering() {
        let row = |key: &str| Entry {
            key: key.into(),
            worker: None,
            office: OfficeId("p".into()),
            title: key.into(),
            label: "delivery".into(),
            detail: String::new(),
            at: 1,
            reviewed: false,
            depth: 0,
            branch: false,
        };
        let mut board = Workboard::default();
        board.replace(vec![row("a"), row("b")]);
        board.selected = 1;
        board.scroll = 7;
        board.replace(vec![row("b"), row("a")]);
        assert_eq!(board.rows[board.selected].key, "b");
        assert_eq!(board.scroll, 7);
        board.replace(vec![row("new")]);
        assert_eq!(
            board.scroll, 0,
            "A replacement item must start at its own beginning"
        );
    }
    #[test]
    fn direct_team_focus_expands_ancestors_and_keeps_the_requested_worker_selected() {
        let mut world = World::new();
        for worker in ["a-root", "b-child", "c-grandchild", "z-other"] {
            observe(
                &mut world,
                worker,
                1,
                EventKind::Seen {
                    name: worker.into(),
                    git_branch: None,
                },
            );
        }
        link(
            &mut world,
            "a-root",
            "b-child",
            RelationshipKind::Delegation,
        );
        link(
            &mut world,
            "b-child",
            "c-grandchild",
            RelationshipKind::Delegation,
        );
        let mut board = Workboard::default();
        board.folded.insert(format!("team:{}", id("a-root").0));
        board.folded.insert(format!("team:{}", id("b-child").0));
        board.show(Channel::Team);
        board.focus_worker(id("c-grandchild"));
        refresh(&mut board, &world, &ReviewMemory::default());
        assert_eq!(board.rows[board.selected].worker, Some(id("c-grandchild")));
        assert!(board.folded.is_empty());
        refresh(&mut board, &world, &ReviewMemory::default());
        assert_eq!(board.rows[board.selected].worker, Some(id("c-grandchild")));
    }

    #[test]
    fn local_markers_round_trip_and_have_a_bound() {
        let mut state = ReviewMemory::default();
        for n in 0..5000 {
            state.reviewed.insert(format!("r{n}"), n);
        }
        state.prune();
        let decoded: ReviewMemory =
            serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(decoded.reviewed.len(), 4096);
        assert!(!decoded.reviewed.contains_key("r0"));
    }
}
