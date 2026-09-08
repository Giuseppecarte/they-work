//! Contextual work reading. Controls execute only through the host's validated actions.
use super::{
    control::ControlStatus, paint_opaque, safe_display, safe_multiline, wrap_text, ACCENT, INK,
    MUTED, PANEL, WARNING,
};
use crate::{
    components::{self, ButtonKind},
    design::CharacterProfile,
    interaction::{Action, HitRegion},
    presentation,
    work_brief::{ReadingAnchor, WorkBrief, WorkTab},
};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Paragraph, Widget},
    Frame,
};
use std::collections::BTreeMap;
use theywork_core::{WorkerId, World};

pub struct InspectorContext<'a> {
    pub world: &'a World,
    pub worker_id: &'a WorkerId,
    pub profiles: &'a BTreeMap<String, CharacterProfile>,
    pub status: &'a ControlStatus,
    pub now: i64,
    pub scroll: usize,
}

#[derive(Default)]
pub struct InspectorState {
    worker: Option<WorkerId>,
    pub tab: WorkTab,
    pub results_only: bool,
    pub show_actions: bool,
    pub expanded: bool,
    reading: ReadingAnchor,
    pending_scroll: i32,
    offset: usize,
}
impl InspectorState {
    pub fn select_worker(&mut self, id: &WorkerId) {
        if self.worker.as_ref() != Some(id) {
            let expanded = self.expanded;
            *self = Self::default();
            self.expanded = expanded;
            self.worker = Some(id.clone());
            self.reading.latest();
        }
    }
    pub fn select_tab(&mut self, tab: WorkTab) {
        self.tab = tab;
        self.results_only = false;
        self.reading = ReadingAnchor::default();
        self.latest();
    }
    pub fn scroll(&mut self, delta: i32) {
        self.pending_scroll = self.pending_scroll.saturating_add(delta);
    }
    pub fn latest(&mut self) {
        self.reading.latest();
        self.offset = 0;
        self.pending_scroll = 0;
    }
    pub fn focus_record(&mut self, key: String) {
        self.tab = WorkTab::Activity;
        self.results_only = false;
        self.reading = ReadingAnchor::default();
        self.reading.key = Some(key);
        self.reading.line = 0;
        self.reading.following = false;
        self.pending_scroll = 0;
    }
    pub fn results(&mut self) {
        self.tab = WorkTab::Activity;
        self.results_only = true;
        self.latest();
    }
    pub fn toggle_actions(&mut self) {
        self.show_actions = !self.show_actions;
    }
    pub fn new_count(&self) -> usize {
        self.reading.new_count()
    }
}

struct Block {
    key: String,
    heading: String,
    text: String,
    action: Option<Action>,
}
impl Block {
    fn new(key: impl Into<String>, heading: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            heading: heading.into(),
            text: text.into(),
            action: None,
        }
    }
    fn action(mut self, action: Action) -> Self {
        self.action = Some(action);
        self
    }
}

/// Compatibility for consumers that have not yet persisted panel reading state.
pub fn draw(frame: &mut Frame, area: Rect, context: &InspectorContext<'_>) -> Vec<HitRegion> {
    let mut state = InspectorState::default();
    state.select_worker(context.worker_id);
    state.scroll(context.scroll.min(i32::MAX as usize) as i32);
    draw_with_state(frame, area, context, &mut state)
}

pub fn draw_with_state(
    frame: &mut Frame,
    area: Rect,
    context: &InspectorContext<'_>,
    state: &mut InspectorState,
) -> Vec<HitRegion> {
    let mut hits = Vec::new();
    paint_opaque(frame, area, Style::default().fg(INK).bg(PANEL));
    if area.width < 20 || area.height < 6 {
        Paragraph::new("Task panel\nEnlarge to read work\nEsc back")
            .render(area, frame.buffer_mut());
        return hits;
    }
    state.select_worker(context.worker_id);
    let Some(brief) = WorkBrief::new(
        context.world,
        context.worker_id,
        context.profiles,
        context.now,
    ) else {
        Paragraph::new(
            "Task unavailable\nIts source no longer provides this conversation.\nEsc back",
        )
        .render(area, frame.buffer_mut());
        return hits;
    };
    if area.width < 28 || area.height < 12 {
        let request = context
            .status
            .requests
            .iter()
            .find(|request| request.worker == brief.worker);
        let header = format!(
            "{} · {}\n{}\n{}\n{}",
            safe_display(&brief.alias),
            brief.provider,
            safe_display(&brief.title),
            if request.is_some() {
                "Response requested"
            } else {
                brief.state
            },
            safe_display(&brief.coverage)
        );
        Paragraph::new(header)
            .style(Style::default().fg(INK))
            .render(Rect::new(area.x, area.y, area.width, 4), frame.buffer_mut());
        let body = request.map_or_else(
            || brief.observed.clone(),
            |request| format!("{}: {}", request.title, request.detail),
        );
        Paragraph::new(wrap_text(&safe_multiline(&body), area.width).join("\n")).render(
            Rect::new(
                area.x,
                area.y + 4,
                area.width,
                area.height.saturating_sub(5),
            ),
            frame.buffer_mut(),
        );
        let y = area.bottom() - 1;
        let mut x = area.x;
        if request.is_some() {
            if let Some(hit) = components::button(
                frame,
                Rect::new(x, y, 10, 1),
                "Review",
                Action::Review(brief.worker.clone()),
                ButtonKind::Primary,
            ) {
                hits.push(hit);
                x += 11;
            }
        }
        if let Some(hit) = components::button(
            frame,
            Rect::new(x, y, 8, 1),
            "Back",
            Action::Close,
            ButtonKind::Quiet,
        ) {
            hits.push(hit);
        }
        return hits;
    }
    let inner = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
    let compact = inner.height < 17;
    components::heading(
        frame,
        Rect::new(inner.x, inner.y, inner.width, 1),
        &safe_display(&brief.alias),
    );
    let title_lines = wrap_text(&safe_display(&brief.title), inner.width);
    let title_height = title_lines.len().min(if compact { 1 } else { 2 }) as u16;
    let mut visible_title = title_lines
        .iter()
        .take(title_height as usize)
        .cloned()
        .collect::<Vec<_>>();
    if title_lines.len() > visible_title.len() {
        if let Some(last) = visible_title.last_mut() {
            *last = super::short_path(&format!("{last} …"), inner.width as usize);
        }
    }
    Paragraph::new(visible_title.join("\n")).render(
        Rect::new(inner.x, inner.y + 1, inner.width, title_height),
        frame.buffer_mut(),
    );
    let status_y = inner.y + 1 + title_height;
    let pending = context
        .status
        .requests
        .iter()
        .filter(|request| request.worker == brief.worker)
        .collect::<Vec<_>>();
    let status = if pending.is_empty() {
        brief.state
    } else {
        "Response requested"
    };
    Paragraph::new(format!(
        "{} · {} · {}",
        brief.provider,
        status,
        super::duration_label(context.now.saturating_sub(brief.observed_at))
    ))
    .style(Style::default().fg(MUTED))
    .render(
        Rect::new(inner.x, status_y, inner.width, 1),
        frame.buffer_mut(),
    );
    let coverage = brief
        .coverage
        .lines()
        .next()
        .unwrap_or("Source not checked");
    components::notice(
        frame,
        Rect::new(inner.x, status_y + 1, inner.width, 1),
        coverage,
        brief.warning,
    );
    let tabs_y = status_y + 2;
    let mut x = inner.x;
    let mut tab_y = tabs_y;
    for tab in WorkTab::ALL {
        let width = (tab.label().len() + 4) as u16;
        if x + width > inner.right() {
            x = inner.x;
            tab_y += 1;
        }
        if let Some(hit) = components::button(
            frame,
            Rect::new(x, tab_y, width, 1),
            tab.label(),
            Action::WorkTab(tab),
            if tab == state.tab {
                ButtonKind::Primary
            } else {
                ButtonKind::Quiet
            },
        ) {
            hits.push(hit);
        }
        x += width;
    }
    let access = context.status.tasks.get(&brief.worker.0);
    let mut actions = Vec::new();
    if !pending.is_empty() {
        actions.push((
            "Review request",
            Action::Review(brief.worker.clone()),
            ButtonKind::Primary,
        ));
    } else if access.is_some_and(|access| access.send) {
        actions.push((
            "Instruct",
            Action::Controls(brief.worker.clone()),
            ButtonKind::Primary,
        ));
    } else if access.is_some_and(|access| access.native) {
        actions.push((
            "Open conversation",
            Action::Control(super::control::Command::OpenNative {
                worker: brief.worker.clone(),
            }),
            ButtonKind::Primary,
        ));
    }
    if !compact {
        actions.push(("Task actions", Action::WorkActions, ButtonKind::Quiet));
    }
    actions.push((
        if state.expanded { "Collapse" } else { "Expand" },
        Action::WorkExpand,
        ButtonKind::Quiet,
    ));
    let mut secondary = Vec::new();
    if state.show_actions && !compact {
        secondary.push(("Character", Action::Character(brief.worker.clone())));
        if access.is_some_and(|a| a.send) && !pending.is_empty() {
            secondary.insert(0, ("Instruct", Action::Controls(brief.worker.clone())));
        }
        if access.is_some_and(|a| a.native) && !pending.is_empty() {
            secondary.insert(
                0,
                (
                    "Open conversation",
                    Action::Control(super::control::Command::OpenNative {
                        worker: brief.worker.clone(),
                    }),
                ),
            );
        }
        if access.is_some_and(|a| a.interrupt) {
            secondary.push((
                "Stop task",
                Action::Control(super::control::Command::Interrupt {
                    worker: brief.worker.clone(),
                }),
            ));
        }
        if access.is_some_and(|a| a.reconnect) {
            secondary.push((
                "Reconnect",
                Action::Control(super::control::Command::Reconnect {
                    worker: brief.worker.clone(),
                }),
            ));
        }
    }
    // Measure all action rows before reserving the reading area. The side panel
    // must not drop its last button when a long primary action wraps.
    let mut layout = Vec::new();
    let mut bx = 0u16;
    let mut row = 0u16;
    for (label, action, kind) in actions.into_iter().chain(
        secondary
            .into_iter()
            .map(|(label, action)| (label, action, ButtonKind::Secondary)),
    ) {
        let width = label.len() as u16 + 4;
        if bx + width > inner.width {
            bx = 0;
            row += 1;
        }
        layout.push((bx, row, width, label, action, kind));
        bx += width + 1;
    }
    let action_y = inner.bottom().saturating_sub(row + 2);
    for (x, row, width, label, action, kind) in layout {
        if let Some(hit) = components::button(
            frame,
            Rect::new(inner.x + x, action_y + row, width, 1),
            label,
            action,
            kind,
        ) {
            hits.push(hit);
        }
    }
    let body_y = tab_y + 2;
    let body = Rect::new(
        inner.x,
        body_y,
        inner.width,
        action_y.saturating_sub(body_y),
    );
    let mut blocks = Vec::new();
    match state.tab {
        WorkTab::Now => {
            if let Some(request) = pending.first() {
                blocks.push(
                    Block::new(
                        format!("request:{}", request.id),
                        format!("FOR YOU · {}", request.title),
                        request.detail.clone(),
                    )
                    .action(Action::Review(brief.worker.clone())),
                );
            }
            if pending.is_empty()
                || context
                    .world
                    .worker(&brief.worker)
                    .is_some_and(|worker| worker.activity.detail().is_some())
            {
                blocks.push(Block::new(
                    "now",
                    format!(
                        "OBSERVED · {} ago",
                        super::duration_label(context.now.saturating_sub(brief.observed_at))
                    ),
                    brief.observed.clone(),
                ));
                if let Some(worker) = context.world.worker(&brief.worker) {
                    if worker.wait_reason.is_some() && pending.is_empty() {
                        blocks.last_mut().unwrap().text.push_str(&format!(
                            "\n{}",
                            presentation::wait_description(worker.wait_reason)
                        ));
                    }
                }
            }
            if let Some(record) = brief.latest_result() {
                blocks.push(
                    Block::new(
                        record.key.clone(),
                        format!("LATEST RESULT · {}", record.author),
                        record.text.clone(),
                    )
                    .action(Action::WorkRecord(record.key.clone())),
                );
            } else if let Some(record) = brief.records.first() {
                blocks.push(
                    Block::new(record.key.clone(), "LATEST UPDATE", record.detail())
                        .action(Action::WorkRecord(record.key.clone())),
                );
            } else if pending.is_empty() {
                blocks.push(Block::new(
                    "no-records",
                    "RECORDED UPDATES",
                    "No retained activity or result from this source yet.",
                ));
            }
            if !brief.team.is_empty() {
                blocks.push(
                    Block::new(
                        "team-count",
                        format!("RECORDED TEAM · {} related tasks", brief.team.len()),
                        brief
                            .team
                            .iter()
                            .map(|member| format!("{} · {}", member.name, member.state))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                    .action(Action::WorkTab(WorkTab::Team)),
                );
            }
        }
        WorkTab::Activity => {
            for record in brief
                .records
                .iter()
                .filter(|record| !state.results_only || record.result)
            {
                let suffix = if record.human_request {
                    " · historical record"
                } else {
                    ""
                };
                blocks.push(Block::new(
                    record.key.clone(),
                    format!(
                        "{} ago · {}{}",
                        super::duration_label(context.now.saturating_sub(record.at)),
                        record.heading,
                        suffix
                    ),
                    record.text.clone(),
                ));
            }
            if blocks.is_empty() {
                blocks.push(Block::new("no-records","NO RECORDED RESULTS",if state.results_only{"No retained result was delivered. A completed turn alone is not a result."}else{"This source has no retained activity. Open its original conversation for more context."}));
            }
        }
        WorkTab::Team => {
            for member in &brief.team {
                let block = Block::new(
                    format!("team:{}", member.id.0),
                    format!("{} · {}", member.name, member.state),
                    format!("{}\n{}", member.title, member.relation),
                );
                blocks.push(if member.present {
                    block.action(Action::Inspect(member.id.clone()))
                } else {
                    block
                });
            }
            if blocks.is_empty() {
                blocks.push(Block::new("no-team","NO RECORDED TEAM","No family relationship is available. This does not prove the task worked alone."));
            }
        }
        WorkTab::Details => {
            let connection=access.map_or("Observation only. No available control connection was provided; use the original app to give instructions or respond.",|access|access.description.as_str());
            blocks.push(Block::new("connection", "CONNECTION", connection));
            blocks.push(Block::new("details", "TASK DETAILS", &brief.details));
            if !brief.present {
                blocks.push(Block::new("retired","RETAINED HISTORY","This task is no longer in the current roster. Its recorded history remains available."));
            }
            if let Some(record) = brief.records.first() {
                blocks.push(Block::new(
                    "provenance",
                    "LATEST RECORD PROVENANCE",
                    &record.provenance,
                ));
            }
        }
    }
    // Keep a stable record/line pair while new records prepend or width changes.
    let mut lines: Vec<(String, usize, String, bool, Option<Action>)> = Vec::new();
    for block in &blocks {
        let mut block_lines = wrap_text(&safe_multiline(&block.heading), body.width);
        let heading_len = block_lines.len();
        block_lines.extend(wrap_text(&safe_multiline(&block.text), body.width));
        block_lines.push(String::new());
        for (line, text) in block_lines.into_iter().enumerate() {
            lines.push((
                block.key.clone(),
                line,
                text,
                line < heading_len,
                block.action.clone(),
            ));
        }
    }
    state
        .reading
        .observe(blocks.iter().map(|block| block.key.clone()));
    let base = if state.reading.following {
        0
    } else {
        state
            .reading
            .key
            .as_ref()
            .and_then(|key| {
                lines.iter().position(|(candidate, line, _, _, _)| {
                    candidate == key && *line == state.reading.line
                })
            })
            .or_else(|| {
                state.reading.key.as_ref().and_then(|key| {
                    lines
                        .iter()
                        .position(|(candidate, _, _, _, _)| candidate == key)
                })
            })
            .unwrap_or(state.offset)
    };
    let maximum = lines.len().saturating_sub(body.height as usize);
    let offset = base
        .saturating_add_signed(state.pending_scroll as isize)
        .min(maximum);
    state.pending_scroll = 0;
    state.offset = offset;
    if let Some((key, line, _, _, _)) = lines.get(offset) {
        state.reading.key = Some(key.clone());
        state.reading.line = *line;
    }
    if offset > 0 {
        state.reading.following = false;
    }
    for (row, (_, _, text, heading, action)) in lines
        .iter()
        .skip(offset)
        .take(body.height as usize)
        .enumerate()
    {
        let rect = Rect::new(body.x, body.y + row as u16, body.width, 1);
        Paragraph::new(text.as_str())
            .style(
                Style::default()
                    .fg(if *heading { ACCENT } else { INK })
                    .add_modifier(if *heading {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            )
            .render(rect, frame.buffer_mut());
        if *heading {
            if let Some(action) = action {
                hits.push(HitRegion::new(rect, action.clone()));
            }
        }
    }
    let hint = if state.reading.expired {
        "Older record left retained history".into()
    } else if state.new_count() > 0 {
        format!(
            "{} new · Latest · line {}/{}",
            state.new_count(),
            offset + 1,
            lines.len()
        )
    } else if state.tab == WorkTab::Activity && offset > 0 {
        format!(
            "Latest · lines {}–{}/{}",
            offset + 1,
            (offset + body.height as usize).min(lines.len()),
            lines.len()
        )
    } else if maximum == 0 {
        "All shown · Esc back".into()
    } else {
        format!(
            "PgUp/Dn · line {}/{} · Esc",
            if lines.is_empty() { 0 } else { offset + 1 },
            lines.len()
        )
    };
    let footer = Rect::new(inner.x, inner.bottom() - 1, inner.width, 1);
    Paragraph::new(hint)
        .style(Style::default().fg(if state.reading.expired {
            WARNING
        } else {
            MUTED
        }))
        .render(footer, frame.buffer_mut());
    if state.tab == WorkTab::Activity {
        let label = if state.results_only {
            "All activity"
        } else {
            "Results"
        };
        let width = label.len() as u16 + 4;
        if let Some(hit) = components::button(
            frame,
            Rect::new(inner.x, tab_y + 1, width, 1),
            label,
            if state.results_only {
                Action::WorkTab(WorkTab::Activity)
            } else {
                Action::WorkResults
            },
            ButtonKind::Quiet,
        ) {
            hits.push(hit);
        }
        if state.offset > 0 || state.new_count() > 0 {
            hits.push(HitRegion::new(footer, Action::WorkLatest));
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use theywork_core::{
        Activity, Agent, Beat, CollaborationEvent, CollaborationKind, Event, EventKind, Evidence,
        OfficeId, Outcome, Relationship, RelationshipKind, SourceCoverage, WaitReason,
    };
    fn event(worker: &str, at: i64, kind: EventKind) -> Event {
        Event {
            at,
            office: OfficeId("/checkout".into()),
            office_path: "/checkout".into(),
            worker: WorkerId(worker.into()),
            agent: Agent::Codex,
            kind,
        }
    }
    fn fixture() -> (World, BTreeMap<String, CharacterProfile>, ControlStatus) {
        let mut world = World::new();
        let mut profiles = BTreeMap::new();
        for (id, alias, title) in [
            ("lead", "Avery", "Implement the checkout retry policy"),
            ("tests", "Morgan", "Validate timeout behavior"),
            ("docs", "Casey", "Document API behavior"),
        ] {
            profiles.insert(
                id.into(),
                CharacterProfile {
                    name: alias.into(),
                    ..Default::default()
                },
            );
            world.apply(event(
                id,
                100,
                EventKind::Seen {
                    name: title.into(),
                    git_branch: Some("feature/retry".into()),
                },
            ));
            world.apply(event(
                id,
                1000,
                EventKind::Coverage(SourceCoverage {
                    available: true,
                    incomplete: false,
                    observed_at: 1000,
                    ..Default::default()
                }),
            ));
        }
        world.apply(event("lead", 800, EventKind::Turn { in_flight: true }));
        world.apply(event(
            "lead",
            900,
            EventKind::Did(Beat {
                at: 900,
                activity: Activity::Editing {
                    detail: "src/checkout/retry.rs".into(),
                },
                outcome: Some(Outcome::Changed {
                    added: 8,
                    removed: 2,
                }),
            }),
        ));
        world.apply(event(
            "tests",
            910,
            EventKind::Wait(Some(WaitReason::HumanInput)),
        ));
        for child in ["tests", "docs"] {
            world.apply(event(
                "lead",
                200,
                EventKind::Relationship(Relationship {
                    parent: WorkerId("lead".into()),
                    child: WorkerId(child.into()),
                    kind: RelationshipKind::Delegation,
                    evidence: Evidence::NativeEvent,
                    at: 200,
                    correlation_id: None,
                }),
            ));
        }
        world.apply(event("docs",950,EventKind::Collaboration(CollaborationEvent {id:"result-one".into(),at:950,actor:WorkerId("docs".into()),recipient:Some(WorkerId("lead".into())),kind:CollaborationKind::Result,text:Some("Retry behavior documented.\nChanged docs/api.md.\nValidation: examples match the current endpoint.".into()),correlation_id:None,native_turn_id:Some("turn-docs".into()),native_item_id:Some("result-one".into()),evidence:Evidence::NativeEvent})));
        let status = ControlStatus {
            tasks: BTreeMap::from([(
                "lead".into(),
                super::super::control::TaskAccess {
                    send: true,
                    interrupt: true,
                    description: "Verified local connection".into(),
                    ..Default::default()
                },
            )]),
            requests: vec![super::super::control::Request {
                id: "question-current".into(),
                worker: WorkerId("tests".into()),
                origin: "checkout / Validate timeout behavior".into(),
                title: "Choose the public timeout".into(),
                detail:
                    "Keep 30 seconds or increase it to 60 seconds? Existing clients rely on 30."
                        .into(),
                choices: vec![],
                questions: vec![super::super::control::Question {
                    id: "timeout".into(),
                    prompt: "Public timeout".into(),
                    options: vec!["30 seconds".into(), "60 seconds".into()],
                    secret: false,
                }],
            }],
            ..Default::default()
        };
        (world, profiles, status)
    }
    fn text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
    #[test]
    fn pilot_prioritizes_observed_work_result_and_current_request_at_80_columns() {
        let (world, profiles, status) = fixture();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        for id in ["lead", "tests", "docs"] {
            let id = WorkerId(id.into());
            let context = InspectorContext {
                world: &world,
                worker_id: &id,
                profiles: &profiles,
                status: &status,
                now: 1000,
                scroll: 0,
            };
            let mut state = InspectorState::default();
            let mut hits = vec![];
            terminal
                .draw(|frame| hits = draw_with_state(frame, frame.area(), &context, &mut state))
                .unwrap();
            let text = text(&terminal);
            assert!(text.contains("Available history"));
            assert!(!text.contains("tokens reported"));
            if id.0 == "lead" {
                assert!(text.contains("Editing src/checkout/retry.rs"));
                assert!(text.contains("Retry behavior documented."));
            }
            if id.0 == "tests" {
                assert!(text.contains("Keep 30 seconds or increase it to 60 seconds?"));
                assert!(hits
                    .iter()
                    .any(|hit| hit.action == Action::Review(id.clone())));
            }
            for tab in WorkTab::ALL {
                assert!(hits.iter().any(|hit| hit.action == Action::WorkTab(tab)));
            }
        }
    }
    #[test]
    fn activity_keeps_all_retained_beats_and_anchors_while_updates_arrive() {
        let (mut world, profiles, status) = fixture();
        for n in 0..20 {
            world.apply(event(
                "lead",
                1001 + n,
                EventKind::Did(Beat {
                    at: 1001 + n,
                    activity: Activity::Reading {
                        detail: format!("file-{n}.rs"),
                    },
                    outcome: None,
                }),
            ));
        }
        let id = WorkerId("lead".into());
        let mut state = InspectorState::default();
        state.select_worker(&id);
        state.select_tab(WorkTab::Activity);
        let mut terminal = Terminal::new(TestBackend::new(40, 24)).unwrap();
        let render =
            |terminal: &mut Terminal<TestBackend>, world: &World, state: &mut InspectorState| {
                terminal
                    .draw(|frame| {
                        draw_with_state(
                            frame,
                            frame.area(),
                            &InspectorContext {
                                world,
                                worker_id: &id,
                                profiles: &profiles,
                                status: &status,
                                now: 1100,
                                scroll: 0,
                            },
                            state,
                        );
                    })
                    .unwrap();
            };
        render(&mut terminal, &world, &mut state);
        state.scroll(13);
        render(&mut terminal, &world, &mut state);
        let anchor = state.reading.key.clone();
        let line = state.reading.line;
        world.apply(event(
            "lead",
            1050,
            EventKind::Did(Beat {
                at: 1050,
                activity: Activity::Reading {
                    detail: "newest.rs".into(),
                },
                outcome: None,
            }),
        ));
        render(&mut terminal, &world, &mut state);
        assert_eq!(state.reading.key, anchor);
        assert_eq!(state.reading.line, line);
        assert_eq!(state.new_count(), 1);
        state.scroll(i32::MAX);
        render(&mut terminal, &world, &mut state);
        assert!(text(&terminal).contains("8 lines added, 2 removed"));
        state.latest();
        render(&mut terminal, &world, &mut state);
        assert!(text(&terminal).contains("newest.rs"));
        assert_eq!(state.new_count(), 0);
    }
    #[test]
    fn source_warning_and_current_request_action_survive_a_short_panel() {
        let (mut world, profiles, status) = fixture();
        let id = WorkerId("tests".into());
        world.apply(event(
            "tests",
            1010,
            EventKind::Coverage(SourceCoverage {
                available: false,
                incomplete: true,
                observed_at: 1010,
                ..Default::default()
            }),
        ));
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        let mut state = InspectorState::default();
        let mut hits = vec![];
        terminal
            .draw(|frame| {
                hits = draw_with_state(
                    frame,
                    frame.area(),
                    &InspectorContext {
                        world: &world,
                        worker_id: &id,
                        profiles: &profiles,
                        status: &status,
                        now: 1100,
                        scroll: 0,
                    },
                    &mut state,
                )
            })
            .unwrap();
        let text = text(&terminal);
        assert!(text.contains("Source unavailable"));
        assert!(hits
            .iter()
            .any(|hit| hit.action == Action::Review(id.clone())));
        assert!(hits.iter().all(|hit| hit.area.bottom() <= 12));
    }

    #[test]
    fn historical_requests_never_offer_a_current_approval_action() {
        let (mut world, profiles, mut status) = fixture();
        status.requests.clear();
        world.apply(event(
            "tests",
            950,
            EventKind::Collaboration(CollaborationEvent {
                id: "old-request".into(),
                at: 950,
                actor: WorkerId("tests".into()),
                recipient: None,
                kind: CollaborationKind::HumanRequest,
                text: Some("Choose the old timeout".into()),
                correlation_id: None,
                native_turn_id: None,
                native_item_id: Some("old-request".into()),
                evidence: Evidence::NativeEvent,
            }),
        ));
        let id = WorkerId("tests".into());
        let mut state = InspectorState::default();
        state.select_worker(&id);
        state.select_tab(WorkTab::Activity);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut hits = vec![];
        terminal
            .draw(|frame| {
                hits = draw_with_state(
                    frame,
                    frame.area(),
                    &InspectorContext {
                        world: &world,
                        worker_id: &id,
                        profiles: &profiles,
                        status: &status,
                        now: 1000,
                        scroll: 0,
                    },
                    &mut state,
                )
            })
            .unwrap();
        assert!(text(&terminal).contains("historical record"));
        assert!(!hits.iter().any(|hit| matches!(
            hit.action,
            Action::Review(_) | Action::Control(super::super::control::Command::Reply { .. })
        )));
    }

    #[test]
    fn emergency_panel_keeps_current_work_and_exact_request_access() {
        let (world, profiles, status) = fixture();
        let mut terminal = Terminal::new(TestBackend::new(32, 11)).unwrap();
        let id = WorkerId("tests".into());
        let mut state = InspectorState::default();
        let mut hits = Vec::new();
        terminal
            .draw(|frame| {
                hits = draw_with_state(
                    frame,
                    frame.area(),
                    &InspectorContext {
                        world: &world,
                        worker_id: &id,
                        profiles: &profiles,
                        status: &status,
                        now: 1000,
                        scroll: 0,
                    },
                    &mut state,
                )
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Morgan"));
        assert!(text.contains("Response requested"));
        assert!(text.contains("Available history"));
        assert!(hits
            .iter()
            .any(|hit| hit.action == Action::Review(id.clone())));
        assert!(hits.iter().any(|hit| hit.action == Action::Close));
        assert!(hits
            .iter()
            .all(|hit| hit.area.right() <= 32 && hit.area.bottom() <= 11));
    }

    #[test]
    fn side_panel_wraps_primary_expand_and_all_available_secondary_actions() {
        let (world, profiles, mut status) = fixture();
        status.tasks.insert(
            "docs".into(),
            super::super::control::TaskAccess {
                native: true,
                interrupt: true,
                reconnect: true,
                ..Default::default()
            },
        );
        let id = WorkerId("docs".into());
        let mut state = InspectorState::default();
        state.select_worker(&id);
        state.show_actions = true;
        let mut terminal = Terminal::new(TestBackend::new(40, 24)).unwrap();
        let mut hits = Vec::new();
        terminal
            .draw(|frame| {
                hits = draw_with_state(
                    frame,
                    frame.area(),
                    &InspectorContext {
                        world: &world,
                        worker_id: &id,
                        profiles: &profiles,
                        status: &status,
                        now: 1000,
                        scroll: 0,
                    },
                    &mut state,
                )
            })
            .unwrap();
        assert!(hits.iter().any(|hit| hit.action == Action::WorkExpand));
        assert!(hits.iter().any(|hit| hit.action == Action::WorkActions));
        for command in [
            super::super::control::Command::OpenNative { worker: id.clone() },
            super::super::control::Command::Interrupt { worker: id.clone() },
            super::super::control::Command::Reconnect { worker: id.clone() },
        ] {
            assert!(hits
                .iter()
                .any(|hit| hit.action == Action::Control(command.clone())));
        }
        assert!(hits
            .iter()
            .all(|hit| hit.area.right() <= 40 && hit.area.bottom() <= 24));
        assert!(text(&terminal).contains("Task actions"));
    }

    #[test]
    #[ignore = "Manual pilot capture; writes only to an explicit audit output directory"]
    fn export_work_panel_pilot() {
        let output = std::path::PathBuf::from(
            std::env::var("THEYWORK_PANEL_PILOT_DIR").expect("explicit audit output path"),
        );
        assert!(output
            .components()
            .any(|component| component.as_os_str() == "design-audit"));
        std::fs::create_dir_all(&output).unwrap();
        let (world, profiles, status) = fixture();
        for (width, height) in [(80, 24), (40, 36), (64, 36)] {
            for id in ["lead", "tests", "docs"] {
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let id = WorkerId(id.into());
                let mut state = InspectorState::default();
                terminal
                    .draw(|frame| {
                        draw_with_state(
                            frame,
                            frame.area(),
                            &InspectorContext {
                                world: &world,
                                worker_id: &id,
                                profiles: &profiles,
                                status: &status,
                                now: 1000,
                                scroll: 0,
                            },
                            &mut state,
                        );
                    })
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let color = |color: ratatui::style::Color| match color {
                    ratatui::style::Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
                    _ => "#d3d7cf".into(),
                };
                let rows=(0..height).map(|y|(0..width).map(|x|{let cell=&buffer[(x,y)];serde_json::json!({"text":cell.symbol(),"fg":color(cell.fg),"bg":color(cell.bg),"bold":cell.modifier.contains(Modifier::BOLD)})}).collect::<Vec<_>>()).collect::<Vec<_>>();
                let path = output.join(format!("{}-{width}x{height}", id.0));
                std::fs::write(
                    path.with_extension("json"),
                    serde_json::to_vec(
                        &serde_json::json!({"columns":width,"rows":height,"cells":rows}),
                    )
                    .unwrap(),
                )
                .unwrap();
                std::fs::write(
                    path.with_extension("txt"),
                    (0..height)
                        .map(|y| {
                            (0..width)
                                .map(|x| buffer[(x, y)].symbol())
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap();
            }
        }
    }
}
