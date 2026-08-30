//! Slide-up message app: a compact view of the company’s current signal.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};
use ratatui::Frame;
use theywork_core::{
    Activity, Beat, Millis, Office, Outcome, Worker, WorkerId, WorkerStatus, World,
};

use crate::canvas::Canvas;
use crate::sprite::{look_for_worker, SpriteSet, WorkerLook};

use super::{
    below_tab_bar, duration_label, elapsed_ms, has_area, human_tokens, inset, paint_opaque,
    render_worker_with_look, safe_display, short_path, status_style, timestamp, worker_status,
    PixelRect, ACCENT, BACKGROUND, GOOD, INK, MUTED, PANEL, PANEL_HIGHLIGHT,
};

const SLIDE_MS: Millis = 260;
const MESSAGE_HEIGHT: u16 = 4;
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
    pub(crate) now: Millis,
    pub(crate) transition_at: Millis,
    pub(crate) canvas: &'a mut Canvas,
    pub(crate) sprites: &'a SpriteSet,
}

pub(crate) fn draw(frame: &mut Frame, context: PhoneDrawContext<'_>) {
    let PhoneDrawContext {
        world,
        channel,
        now,
        transition_at,
        canvas,
        sprites,
    } = context;
    let area = below_tab_bar(frame.area());
    if !has_area(area) {
        return;
    }
    let slab_width = area
        .width
        .saturating_mul(2)
        .div_euclid(3)
        .clamp(28, 72)
        .min(area.width);
    let slab_height = area.height.saturating_sub(2).min(40);
    if slab_width == 0 || slab_height == 0 {
        return;
    }
    let minimum_visible_height = slab_height.clamp(1, MIN_VISIBLE_HEIGHT);
    let progress = elapsed_ms(now, transition_at).min(SLIDE_MS);
    let hidden = (slab_height as Millis)
        .saturating_mul(SLIDE_MS.saturating_sub(progress))
        .div_euclid(SLIDE_MS)
        .clamp(
            0,
            slab_height.saturating_sub(minimum_visible_height) as Millis,
        ) as u16;
    let panel_x = area
        .x
        .saturating_add(area.width.saturating_sub(slab_width) / 2);
    let panel_y = area
        .y
        .saturating_add(area.height.saturating_sub(slab_height))
        .saturating_add(hidden);
    let panel_bottom = area.y.saturating_add(area.height);
    let visible_top = panel_y.min(panel_bottom);
    let visible_height = panel_bottom.saturating_sub(visible_top).min(slab_height);
    if visible_height == 0 {
        return;
    }
    let panel = Rect::new(panel_x, visible_top, slab_width, visible_height);

    paint_opaque(frame, area, Style::default().bg(BACKGROUND));
    let shadow = Rect::new(
        panel.x.saturating_add(2),
        panel.y.saturating_add(1),
        panel.width.saturating_sub(2),
        panel.height.saturating_sub(1),
    );
    paint_opaque(frame, shadow, Style::default().bg(BACKGROUND));
    paint_opaque(frame, panel, Style::default().bg(PANEL));
    Block::default()
        .title(format!(" MESSAGES  {} ", channel.label()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL_HIGHLIGHT))
        .style(Style::default().bg(PANEL))
        .render(panel, frame.buffer_mut());

    let inner = inset(panel, 1);
    if !has_area(inner) {
        return;
    }
    let top_rows = inner.height.min(3);
    if inner.height >= 1 {
        let speaker_width = inner.width.min(10);
        let speaker = Rect::new(
            inner
                .x
                .saturating_add(inner.width.saturating_sub(speaker_width) / 2),
            inner.y,
            speaker_width,
            1,
        );
        let speaker_style = Style::default().fg(MUTED).bg(PANEL);
        paint_opaque(frame, speaker, speaker_style);
        Paragraph::new("──────")
            .style(speaker_style)
            .render(speaker, frame.buffer_mut());
    }
    if inner.height >= 2 {
        let title_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
        let title_style = Style::default().fg(GOOD).bg(PANEL);
        paint_opaque(frame, title_area, title_style);
        Paragraph::new(Line::from(vec![
            Span::styled(" STANDUP", title_style),
            Span::styled(
                format!("  {} workers", world.worker_count()),
                Style::default().fg(MUTED).bg(PANEL),
            ),
        ]))
        .style(title_style)
        .render(title_area, frame.buffer_mut());
    }
    if inner.height >= 3 {
        let tab_area = Rect::new(inner.x, inner.y + 2, inner.width, 1);
        paint_opaque(frame, tab_area, Style::default().bg(PANEL));
        Paragraph::new(channel_tabs(channel))
            .style(Style::default().bg(PANEL))
            .render(tab_area, frame.buffer_mut());
    }
    let footer_height = if inner.height >= top_rows.saturating_add(3) {
        2
    } else {
        0
    };
    let body = Rect::new(
        inner.x,
        inner.y.saturating_add(top_rows),
        inner.width,
        inner
            .height
            .saturating_sub(top_rows.saturating_add(footer_height)),
    );
    if footer_height > 0 {
        let footer_y = inner.y + inner.height - footer_height;
        let summary_area = Rect::new(inner.x, footer_y, inner.width, 1);
        let home_area = Rect::new(inner.x, footer_y + 1, inner.width, 1);
        let (running, idle, needs_attention) = world
            .offices()
            .flat_map(|office| office.workers.iter())
            .fold(
                (0, 0, 0),
                |(running, idle, attention), worker| match worker_status(worker, now) {
                    WorkerStatus::Running => (running + 1, idle, attention),
                    WorkerStatus::Idle => (running, idle + 1, attention),
                    WorkerStatus::Blocked | WorkerStatus::Failed => (running, idle, attention + 1),
                },
            );
        let summary_style = Style::default().bg(PANEL);
        paint_opaque(frame, summary_area, summary_style);
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {running} running"),
                status_style(WorkerStatus::Running).bg(PANEL),
            ),
            Span::styled(
                format!("  {idle} idle"),
                status_style(WorkerStatus::Idle).bg(PANEL),
            ),
            Span::styled(
                format!("  {needs_attention} needs you"),
                status_style(WorkerStatus::Blocked).bg(PANEL),
            ),
        ]))
        .style(summary_style)
        .render(summary_area, frame.buffer_mut());
        paint_opaque(frame, home_area, summary_style);
        Paragraph::new("                 ━━━━━━━━")
            .style(Style::default().fg(MUTED).bg(PANEL))
            .render(home_area, frame.buffer_mut());
    }
    if !has_area(body) {
        return;
    }
    let messages = messages_for(channel, world, now);
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
    let Some((office, worker)) = message
        .worker_id
        .as_ref()
        .and_then(|id| find_worker(world, id))
    else {
        let max_chars = row.width.saturating_sub(2) as usize;
        let text_style = Style::default()
            .fg(ACCENT)
            .bg(PANEL)
            .add_modifier(Modifier::BOLD);
        paint_opaque(frame, row, text_style);
        Paragraph::new(format!("  {}", short_path(&message.text, max_chars)))
            .style(text_style)
            .render(row, frame.buffer_mut());
        return;
    };
    let look = look_for_worker(&office.workers, worker);
    let avatar_width = row.width.min(7);
    let avatar_area = Rect::new(row.x, row.y, avatar_width, row.height);
    draw_avatar(frame, canvas, sprites, worker, &look, now, avatar_area);
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
    let text_style = Style::default().bg(PANEL);
    paint_opaque(frame, text_area, text_style);
    let header = Line::from(vec![
        Span::styled(
            short_path(&message.name, max_chars),
            status_style(message.status)
                .add_modifier(Modifier::BOLD)
                .bg(PANEL),
        ),
        Span::styled(
            format!("  {}  {}", message.agent, timestamp(message.at)),
            Style::default().fg(MUTED).bg(PANEL),
        ),
    ]);
    let lines = Text::from(vec![
        header,
        Line::from(Span::styled(
            short_path(&message.text, max_chars),
            Style::default().fg(INK).bg(PANEL),
        )),
    ]);
    Paragraph::new(lines)
        .style(text_style)
        .wrap(Wrap { trim: false })
        .render(text_area, frame.buffer_mut());
}

fn draw_avatar(
    frame: &mut Frame,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    worker: &Worker,
    look: &WorkerLook,
    now: Millis,
    area: Rect,
) {
    if !has_area(area) {
        return;
    }
    paint_opaque(frame, area, Style::default().bg(PANEL));
    let width = area.width as usize;
    let height = area.height as usize;
    canvas.resize_for_cells(width, height);
    canvas.clear();
    render_worker_with_look(
        canvas,
        sprites,
        worker,
        look,
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

fn find_worker<'a>(world: &'a World, id: &WorkerId) -> Option<(&'a Office, &'a Worker)> {
    world.offices().find_map(|office| {
        office
            .workers
            .iter()
            .find(|worker| &worker.id == id)
            .map(|worker| (office, worker))
    })
}

fn messages_for(channel: PhoneChannel, world: &World, now: Millis) -> Vec<PhoneMessage> {
    match channel {
        PhoneChannel::Standup => standup_messages(world, now),
        PhoneChannel::Blocked => blocked_messages(world, now),
        PhoneChannel::Shipping => shipping_messages(world, now),
        PhoneChannel::Watercooler => watercooler_messages(world, now),
    }
}

fn standup_messages(world: &World, now: Millis) -> Vec<PhoneMessage> {
    let mut messages = Vec::new();
    for office in world.offices() {
        for worker in &office.workers {
            let branch = worker
                .git_branch
                .as_deref()
                .map(safe_display)
                .unwrap_or_else(|| "no branch".to_string());
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
            format!("{} • {}", worker.activity.label(), safe_display(detail))
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

fn blocked_messages(world: &World, now: Millis) -> Vec<PhoneMessage> {
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
            let last = last_command_or_question(worker)
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

fn beat_display(beat: &Beat) -> String {
    let base = match beat.activity.detail() {
        Some(detail) if !detail.is_empty() => {
            format!("{} • {}", beat.activity.label(), safe_display(detail))
        }
        _ => beat.activity.label().to_string(),
    };
    match beat.outcome {
        Some(Outcome::Exited(status)) => format!("{base} • exit {status}"),
        Some(Outcome::Changed { added, removed }) => {
            format!("{base} • +{added} −{removed}")
        }
        None => base,
    }
}

fn last_command_or_question(worker: &Worker) -> Option<String> {
    worker
        .history
        .iter()
        .rev()
        .find_map(|beat| {
            let is_prompt = matches!(
                &beat.activity,
                Activity::Typing { .. } | Activity::Waiting { .. }
            );
            is_prompt.then(|| beat_display(beat))
        })
        .or_else(|| worker.activity.detail().map(str::to_string))
}

fn shipping_messages(world: &World, now: Millis) -> Vec<PhoneMessage> {
    let mut messages = Vec::new();
    for office in world.offices() {
        let mut wrote_office = false;
        for worker in &office.workers {
            for beat in worker
                .history
                .iter()
                .filter(|beat| matches!(&beat.activity, Activity::Editing { .. }))
            {
                if !wrote_office {
                    messages.push(PhoneMessage {
                        worker_id: None,
                        name: office.name.clone(),
                        agent: String::new(),
                        at: beat.at,
                        text: format!("{}  /  shipping", office.name),
                        status: WorkerStatus::Idle,
                    });
                    wrote_office = true;
                }
                let branch = worker.git_branch.as_deref().unwrap_or("no branch");
                messages.push(worker_message(
                    worker,
                    beat.at,
                    format!("{} • {}", safe_display(branch), beat_display(beat)),
                    now,
                ));
            }
        }
    }
    messages
}

fn watercooler_messages(world: &World, now: Millis) -> Vec<PhoneMessage> {
    let mut messages = Vec::new();
    for office in world.offices() {
        for worker in &office.workers {
            for beat in worker
                .history
                .iter()
                .filter(|beat| matches!(&beat.activity, Activity::Talking { .. }))
            {
                messages.push(worker_message(
                    worker,
                    beat.at,
                    format!("{} • {}", office.name, beat_display(beat)),
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
    use theywork_core::{
        Activity, Agent, Beat, Event, EventKind, OfficeId, Outcome, WorkerId, BLOCKED_AFTER_MS,
    };

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
    fn channels_include_live_status_and_core_beat_history() {
        let (mut world, worker_id) = blocked_world();
        let office = "/workspace/app";
        world.apply(event(
            office,
            &worker_id.0,
            10,
            EventKind::Did(Beat {
                at: 10,
                activity: Activity::Typing {
                    detail: "cargo test".into(),
                },
                outcome: Some(Outcome::Exited(0)),
            }),
        ));
        world.apply(event(
            office,
            &worker_id.0,
            20,
            EventKind::Did(Beat {
                at: 20,
                activity: Activity::Editing {
                    detail: "src/phone.rs".into(),
                },
                outcome: Some(Outcome::Changed {
                    added: 12,
                    removed: 3,
                }),
            }),
        ));
        world.apply(event(
            office,
            &worker_id.0,
            30,
            EventKind::Did(Beat {
                at: 30,
                activity: Activity::Talking {
                    detail: "I found the issue".into(),
                },
                outcome: None,
            }),
        ));
        let now = BLOCKED_AFTER_MS + 31;

        let standup = messages_for(PhoneChannel::Standup, &world, now);
        assert_eq!(standup.len(), 1);
        assert!(standup[0].text.contains("blocked"));
        assert!(standup[0].text.contains("talking"));
        assert!(standup[0].text.contains("codex/phone"));
        assert!(standup[0].text.contains("42 tokens"));

        let blocked = messages_for(PhoneChannel::Blocked, &world, now);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].status, WorkerStatus::Blocked);
        assert!(blocked[0].text.contains("stuck 3m 00s"));
        assert!(blocked[0].text.contains("typing • cargo test • exit 0"));

        let shipping = messages_for(PhoneChannel::Shipping, &world, now);
        assert_eq!(shipping.len(), 2);
        assert!(shipping[0].text.contains("shipping"));
        assert!(shipping[1].text.contains("codex/phone"));
        assert!(shipping[1].text.contains("editing • src/phone.rs"));
        assert!(shipping[1].text.contains("+12 −3"));

        let watercooler = messages_for(PhoneChannel::Watercooler, &world, now);
        assert_eq!(watercooler.len(), 1);
        assert!(watercooler[0].text.contains("I found the issue"));
    }
}
