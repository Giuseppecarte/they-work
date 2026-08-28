//! Slide-up message app: a compact view of the company’s current signal.

use std::collections::{BTreeMap, VecDeque};

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};
use ratatui::Frame;
use theywork_core::{Millis, Worker, WorkerId, WorkerStatus, World};

use crate::canvas::Canvas;
use crate::sprite::SpriteSet;
use crate::ActivityRecord;

use super::{
    duration_label, elapsed_ms, has_area, human_tokens, inset, render_worker, short_path,
    status_style, timestamp, worker_status, PixelRect, ACCENT, INK, MUTED, PANEL, PANEL_HIGHLIGHT,
};

const SLIDE_MS: Millis = 260;
const MESSAGE_HEIGHT: u16 = 3;
const MIN_VISIBLE_HEIGHT: u16 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhoneChannel {
    Standup,
    Blocked,
    Shipping,
    Watercooler,
}

impl PhoneChannel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Standup => "#standup",
            Self::Blocked => "#blocked",
            Self::Shipping => "#shipping",
            Self::Watercooler => "#watercooler",
        }
    }

    pub(crate) fn next(self) -> Self {
        match self {
            Self::Standup => Self::Blocked,
            Self::Blocked => Self::Shipping,
            Self::Shipping => Self::Watercooler,
            Self::Watercooler => Self::Standup,
        }
    }

    pub(crate) fn previous(self) -> Self {
        match self {
            Self::Standup => Self::Watercooler,
            Self::Blocked => Self::Standup,
            Self::Shipping => Self::Blocked,
            Self::Watercooler => Self::Shipping,
        }
    }
}

#[derive(Debug, Clone)]
struct PhoneMessage {
    worker_id: Option<WorkerId>,
    name: String,
    agent: String,
    at: Millis,
    text: String,
    status: WorkerStatus,
}

pub(crate) struct PhoneDrawContext<'a> {
    pub(crate) world: &'a World,
    pub(crate) channel: PhoneChannel,
    pub(crate) history: &'a BTreeMap<WorkerId, VecDeque<ActivityRecord>>,
    pub(crate) now: Millis,
    pub(crate) transition_at: Millis,
    pub(crate) canvas: &'a mut Canvas,
    pub(crate) sprites: &'a SpriteSet,
}

pub(crate) fn draw(frame: &mut Frame, context: PhoneDrawContext<'_>) {
    let PhoneDrawContext {
        world,
        channel,
        history,
        now,
        transition_at,
        canvas,
        sprites,
    } = context;
    let area = frame.area();
    if !has_area(area) {
        return;
    }
    let panel_height = area.height.min(22);
    let minimum_visible_height = panel_height.clamp(1, MIN_VISIBLE_HEIGHT);
    let progress = elapsed_ms(now, transition_at).min(SLIDE_MS);
    let hidden = (panel_height as Millis)
        .saturating_mul(SLIDE_MS.saturating_sub(progress))
        .div_euclid(SLIDE_MS)
        .clamp(
            0,
            panel_height.saturating_sub(minimum_visible_height) as Millis,
        ) as u16;
    let horizontal_padding: u16 = if area.width > 2 { 1 } else { 0 };
    let panel_width = area
        .width
        .saturating_sub(horizontal_padding.saturating_mul(2));
    if panel_width == 0 || panel_height == 0 {
        return;
    }
    let panel_y = area.y.saturating_add(
        area.height
            .saturating_sub(panel_height)
            .saturating_add(hidden),
    );
    let panel_bottom = area.y.saturating_add(area.height);
    let visible_height = panel_bottom
        .saturating_sub(panel_y.min(panel_bottom))
        .min(panel_height);
    if visible_height == 0 {
        return;
    }
    let panel = Rect::new(
        area.x.saturating_add(horizontal_padding),
        panel_y.min(panel_bottom),
        panel_width,
        visible_height,
    );

    Block::default()
        .title(format!(" 📱 MESSAGES  {} ", channel.label()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL_HIGHLIGHT))
        .style(Style::default().bg(PANEL))
        .render(panel, frame.buffer_mut());

    let inner = inset(panel, 1);
    if !has_area(inner) {
        return;
    }
    let tab_area = Rect::new(inner.x, inner.y, inner.width, inner.height.min(1));
    let tab_line = channel_tabs(channel);
    Paragraph::new(tab_line)
        .style(Style::default().bg(PANEL))
        .render(tab_area, frame.buffer_mut());

    let body = Rect::new(
        inner.x,
        inner.y.saturating_add(tab_area.height),
        inner.width,
        inner.height.saturating_sub(tab_area.height),
    );
    if !has_area(body) {
        return;
    }
    let messages = messages_for(channel, world, history, now);
    if messages.is_empty() {
        Paragraph::new("No current messages.")
            .style(Style::default().fg(MUTED).bg(PANEL))
            .render(body, frame.buffer_mut());
        return;
    }

    let mut offset = 0;
    for message in messages {
        let remaining = body.height.saturating_sub(offset);
        if remaining == 0 {
            break;
        }
        let row_height = if message.worker_id.is_some() {
            MESSAGE_HEIGHT.min(remaining)
        } else {
            1.min(remaining)
        };
        let row = Rect::new(
            body.x,
            body.y.saturating_add(offset),
            body.width,
            row_height,
        );
        draw_message(frame, world, &message, canvas, sprites, now, row);
        offset = offset.saturating_add(row_height);
    }
}

fn channel_tabs(channel: PhoneChannel) -> Line<'static> {
    let tabs = [
        (PhoneChannel::Standup, "1 #standup"),
        (PhoneChannel::Blocked, "2 #blocked"),
        (PhoneChannel::Shipping, "3 #shipping"),
        (PhoneChannel::Watercooler, "4 #watercooler"),
    ];
    Line::from(
        tabs.into_iter()
            .flat_map(|(tab, label)| {
                let style = if tab == channel {
                    Style::default().fg(INK).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(MUTED)
                };
                [Span::styled(format!(" {label} "), style), Span::raw(" ")]
            })
            .collect::<Vec<_>>(),
    )
}

fn draw_message(
    frame: &mut Frame,
    world: &World,
    message: &PhoneMessage,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    now: Millis,
    row: Rect,
) {
    let Some(worker) = message
        .worker_id
        .as_ref()
        .and_then(|id| find_worker(world, id))
    else {
        let max_chars = row.width.saturating_sub(2) as usize;
        Paragraph::new(format!("  {}", short_path(&message.text, max_chars)))
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .render(row, frame.buffer_mut());
        return;
    };
    let avatar_width = row.width.min(5);
    let avatar_area = Rect::new(row.x, row.y, avatar_width, row.height);
    draw_avatar(frame, canvas, sprites, worker, now, avatar_area);
    let text_area = Rect::new(
        row.x.saturating_add(avatar_width),
        row.y,
        row.width.saturating_sub(avatar_width),
        row.height,
    );
    if !has_area(text_area) {
        return;
    }
    let max_chars = text_area.width.saturating_sub(1) as usize;
    let header = Line::from(vec![
        Span::styled(
            short_path(&message.name, max_chars),
            status_style(message.status).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}  {}", message.agent, timestamp(message.at)),
            Style::default().fg(MUTED),
        ),
    ]);
    let lines = Text::from(vec![
        header,
        Line::from(Span::styled(
            short_path(&message.text, max_chars),
            Style::default().fg(INK),
        )),
    ]);
    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .render(text_area, frame.buffer_mut());
}

fn draw_avatar(
    frame: &mut Frame,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    worker: &Worker,
    now: Millis,
    area: Rect,
) {
    if !has_area(area) {
        return;
    }
    let width = area.width as usize;
    let height = area.height as usize * 2;
    canvas.resize(width, height);
    canvas.clear();
    render_worker(
        canvas,
        sprites,
        worker,
        now,
        PixelRect {
            x: 0,
            y: 0,
            width,
            height,
        },
    );
    canvas.render(frame.buffer_mut(), area);
}

fn find_worker<'a>(world: &'a World, id: &WorkerId) -> Option<&'a Worker> {
    world
        .offices()
        .flat_map(|office| office.workers.iter())
        .find(|worker| &worker.id == id)
}

fn messages_for(
    channel: PhoneChannel,
    world: &World,
    history: &BTreeMap<WorkerId, VecDeque<ActivityRecord>>,
    now: Millis,
) -> Vec<PhoneMessage> {
    match channel {
        PhoneChannel::Standup => standup_messages(world, now),
        PhoneChannel::Blocked => blocked_messages(world, history, now),
        PhoneChannel::Shipping => shipping_messages(world, history, now),
        PhoneChannel::Watercooler => watercooler_messages(world, history, now),
    }
}

fn standup_messages(world: &World, now: Millis) -> Vec<PhoneMessage> {
    let mut messages = Vec::new();
    for office in world.offices() {
        for worker in &office.workers {
            let branch = worker.git_branch.as_deref().unwrap_or("no branch");
            messages.push(worker_message(
                worker,
                worker.last_seen,
                format!(
                    "{} • {} • branch {} • {} tokens",
                    worker_status(worker, now).label(),
                    current_activity(worker),
                    branch,
                    human_tokens(worker.tokens_used)
                ),
                now,
            ));
        }
    }
    messages
}

fn current_activity(worker: &Worker) -> String {
    match worker.activity.detail() {
        Some(detail) if !detail.is_empty() => {
            format!("{} • {}", worker.activity.label(), detail)
        }
        _ => worker.activity.label().to_string(),
    }
}

fn worker_message(worker: &Worker, at: Millis, text: String, now: Millis) -> PhoneMessage {
    PhoneMessage {
        worker_id: Some(worker.id.clone()),
        name: worker.name.clone(),
        agent: worker.agent.label().to_string(),
        at,
        text,
        status: worker_status(worker, now),
    }
}

fn blocked_messages(
    world: &World,
    history: &BTreeMap<WorkerId, VecDeque<ActivityRecord>>,
    now: Millis,
) -> Vec<PhoneMessage> {
    let mut messages = Vec::new();
    for office in world.offices() {
        for worker in &office.workers {
            let status = worker_status(worker, now);
            if !status.needs_attention() {
                continue;
            }
            let duration = if status == WorkerStatus::Blocked {
                format!(
                    "stuck {}",
                    duration_label(elapsed_ms(now, worker.last_seen))
                )
            } else {
                format!(
                    "failed {}",
                    duration_label(elapsed_ms(now, worker.last_seen))
                )
            };
            let last = last_command_or_question(history, worker)
                .unwrap_or_else(|| "no command or question captured".to_string());
            messages.push(worker_message(
                worker,
                worker.last_seen,
                format!("{} • {} • last {}", status.label(), duration, last),
                now,
            ));
        }
    }
    messages
}

fn last_command_or_question(
    history: &BTreeMap<WorkerId, VecDeque<ActivityRecord>>,
    worker: &Worker,
) -> Option<String> {
    let Some(records) = history.get(&worker.id) else {
        return worker.activity.detail().map(str::to_string);
    };
    records
        .iter()
        .rev()
        .find_map(|record| {
            let is_prompt = matches!(record.label.as_str(), "typing" | "waiting");
            is_prompt.then(|| record.display())
        })
        .or_else(|| worker.activity.detail().map(str::to_string))
}

fn shipping_messages(
    world: &World,
    history: &BTreeMap<WorkerId, VecDeque<ActivityRecord>>,
    now: Millis,
) -> Vec<PhoneMessage> {
    let mut messages = Vec::new();
    for office in world.offices() {
        let mut wrote_office = false;
        for worker in &office.workers {
            let Some(records) = history.get(&worker.id) else {
                continue;
            };
            for record in records.iter().filter(|record| record.label == "editing") {
                if !wrote_office {
                    messages.push(PhoneMessage {
                        worker_id: None,
                        name: office.name.clone(),
                        agent: String::new(),
                        at: record.at,
                        text: format!("{}  /  shipping", office.name),
                        status: WorkerStatus::Idle,
                    });
                    wrote_office = true;
                }
                let branch = record
                    .branch
                    .as_deref()
                    .or(worker.git_branch.as_deref())
                    .unwrap_or("no branch");
                let file = record.detail.as_deref().unwrap_or("file change");
                messages.push(worker_message(
                    worker,
                    record.at,
                    format!("{} • {}", branch, short_path(file, 60)),
                    now,
                ));
            }
        }
    }
    messages
}

fn watercooler_messages(
    world: &World,
    history: &BTreeMap<WorkerId, VecDeque<ActivityRecord>>,
    now: Millis,
) -> Vec<PhoneMessage> {
    let mut messages = Vec::new();
    for office in world.offices() {
        for worker in &office.workers {
            let Some(records) = history.get(&worker.id) else {
                continue;
            };
            for record in records.iter().filter(|record| record.label == "talking") {
                let detail = record.detail.as_deref().unwrap_or("said something");
                messages.push(worker_message(
                    worker,
                    record.at,
                    format!("{} • {}", office.name, detail),
                    now,
                ));
            }
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use theywork_core::{Activity, Agent, Event, EventKind, OfficeId, WorkerId, BLOCKED_AFTER_MS};

    fn event(office: &str, worker: &str, at: Millis, kind: EventKind) -> Event {
        Event {
            at,
            office: OfficeId(office.to_string()),
            office_path: office.to_string(),
            worker: WorkerId(worker.to_string()),
            agent: Agent::Claude,
            kind,
        }
    }

    fn blocked_world() -> (World, WorkerId) {
        let office = "/workspace/app";
        let worker = WorkerId("/workspace/app#dev".into());
        let mut world = World::new();
        world.apply(event(
            office,
            &worker.0,
            0,
            EventKind::Seen {
                name: "Dev Phone".into(),
                git_branch: Some("codex/phone".into()),
            },
        ));
        world.apply(event(office, &worker.0, 0, EventKind::Tokens(42)));
        world.apply(event(
            office,
            &worker.0,
            0,
            EventKind::Turn { in_flight: true },
        ));
        world.apply(event(
            office,
            &worker.0,
            0,
            EventKind::Acted(Activity::Waiting {
                detail: "approve deploy".into(),
            }),
        ));
        (world, worker)
    }

    fn record(at: Millis, label: &str, detail: &str, branch: &str) -> ActivityRecord {
        ActivityRecord {
            at,
            label: label.into(),
            detail: Some(detail.into()),
            branch: Some(branch.into()),
        }
    }

    #[test]
    fn channels_cycle_in_both_directions() {
        assert_eq!(PhoneChannel::Standup.next(), PhoneChannel::Blocked);
        assert_eq!(PhoneChannel::Blocked.next(), PhoneChannel::Shipping);
        assert_eq!(PhoneChannel::Shipping.next(), PhoneChannel::Watercooler);
        assert_eq!(PhoneChannel::Watercooler.next(), PhoneChannel::Standup);
        assert_eq!(PhoneChannel::Standup.previous(), PhoneChannel::Watercooler);
        assert_eq!(PhoneChannel::Blocked.previous(), PhoneChannel::Standup);
    }

    #[test]
    fn channels_include_live_status_and_observed_activity_history() {
        let (world, worker_id) = blocked_world();
        let now = BLOCKED_AFTER_MS + 1;
        let mut history = BTreeMap::new();
        history.insert(
            worker_id,
            VecDeque::from([
                record(10, "typing", "cargo test", "codex/phone"),
                record(20, "editing", "src/phone.rs", "codex/phone"),
                record(30, "talking", "I found the issue", "codex/phone"),
            ]),
        );

        let standup = messages_for(PhoneChannel::Standup, &world, &history, now);
        assert_eq!(standup.len(), 1);
        assert!(standup[0].text.contains("blocked"));
        assert!(standup[0].text.contains("waiting"));
        assert!(standup[0].text.contains("codex/phone"));
        assert!(standup[0].text.contains("42 tokens"));

        let blocked = messages_for(PhoneChannel::Blocked, &world, &history, now);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].status, WorkerStatus::Blocked);
        assert!(blocked[0].text.contains("stuck 3m 00s"));
        assert!(blocked[0].text.contains("typing • cargo test"));
        let shipping = messages_for(PhoneChannel::Shipping, &world, &history, now);
        assert_eq!(shipping.len(), 2);
        assert!(shipping[0].text.contains("shipping"));
        assert!(shipping[1].text.contains("codex/phone"));
        assert!(shipping[1].text.contains("src/phone.rs"));

        let watercooler = messages_for(PhoneChannel::Watercooler, &world, &history, now);
        assert_eq!(watercooler.len(), 1);
        assert!(watercooler[0].text.contains("I found the issue"));
    }
}
