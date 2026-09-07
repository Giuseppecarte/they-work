//! Slide-up message app: a compact view of the company’s current signal.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};
use ratatui::Frame;
use theywork_core::{
    Activity, Beat, Millis, Office, Outcome, Worker, WorkerId, WorkerStatus, World,
};

use crate::canvas::Canvas;
use crate::sprite::{look_for_worker, SpriteSet, WorkerLook};

use super::{
    below_tab_bar, duration_label, elapsed_ms, has_area, inset, paint_opaque,
    render_worker_head_with_look, safe_display, short_path, status_style, worker_status, PixelRect,
    ACCENT, ATTENTION_PANEL, BACKGROUND, GOOD, INK, MUTED, PANEL, PANEL_HIGHLIGHT,
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
            Self::Standup => "Now",
            Self::Blocked => "Attention",
            Self::Shipping => "Edits",
            Self::Watercooler => "Messages",
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
    context: String,
    historical: bool,
    state_label: &'static str,
    at: Millis,
    text: String,
    status: WorkerStatus,
}

pub(crate) struct PhoneDrawContext<'a> {
    pub(crate) world: &'a World,
    pub(crate) office: Option<&'a Office>,
    pub(crate) channel: PhoneChannel,
    pub(crate) selected: usize,
    pub(crate) now: Millis,
    pub(crate) transition_at: Millis,
    pub(crate) canvas: &'a mut Canvas,
    pub(crate) sprites: &'a SpriteSet,
}

pub(crate) fn draw(frame: &mut Frame, context: PhoneDrawContext<'_>) {
    let PhoneDrawContext {
        world,
        office,
        channel,
        selected,
        now,
        transition_at,
        canvas,
        sprites,
    } = context;
    let area = below_tab_bar(frame.area());
    if !has_area(area) {
        return;
    }
    let slab_width = area.width.saturating_sub(4).clamp(28, 90).min(area.width);
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

    dim_backdrop(frame, area, canvas.is_light_mode());
    let shadow = Rect::new(
        panel.x.saturating_add(2),
        panel.y.saturating_add(1),
        panel.width.saturating_sub(2),
        panel.height.saturating_sub(1),
    );
    paint_opaque(frame, shadow, Style::default().bg(BACKGROUND));
    paint_opaque(frame, panel, Style::default().bg(PANEL));
    let messages = messages_for(channel, world, office, now);
    Block::default()
        .title(Line::styled(
            format!(
                " PHONE / {} · {} ",
                channel.label().to_uppercase(),
                messages.len()
            ),
            Style::default().fg(INK).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL_HIGHLIGHT))
        .style(Style::default().bg(PANEL))
        .render(panel, frame.buffer_mut());

    let inner = inset(panel, 1);
    if !has_area(inner) {
        return;
    }
    let top_rows = inner.height.min(2);
    Paragraph::new(format!(
        " {} · {} conversations",
        safe_display(office.map_or("All projects", |value| value.name.as_str())),
        office.map_or_else(|| world.worker_count(), |value| value.workers.len())
    ))
    .style(Style::default().fg(MUTED).bg(PANEL))
    .render(
        Rect::new(inner.x, inner.y, inner.width, 1),
        frame.buffer_mut(),
    );
    if top_rows > 1 {
        Paragraph::new(channel_tabs(channel, inner.width))
            .style(Style::default().bg(PANEL))
            .render(
                Rect::new(inner.x, inner.y + 1, inner.width, 1),
                frame.buffer_mut(),
            );
    }
    let footer_height = if inner.height >= 8 { 2 } else { 0 };
    let body = Rect::new(
        inner.x,
        inner.y + top_rows,
        inner.width,
        inner.height.saturating_sub(top_rows + footer_height),
    );
    if footer_height > 0 {
        let selected_position = if messages.is_empty() {
            0
        } else {
            selected.min(messages.len() - 1) + 1
        };
        let order = if matches!(channel, PhoneChannel::Shipping | PhoneChannel::Watercooler) {
            "newest first · recorded history"
        } else {
            "attention first · current state"
        };
        Paragraph::new(format!(" {selected_position}/{} · {order}", messages.len()))
            .style(Style::default().fg(MUTED).bg(PANEL))
            .render(
                Rect::new(inner.x, inner.bottom() - 2, inner.width, 1),
                frame.buffer_mut(),
            );
        let keys = if inner.width >= 48 {
            "↑↓ select  Enter inspect  ←→ channel  Esc close"
        } else if inner.width >= 32 {
            "↑↓ select · Enter inspect · Esc close"
        } else {
            "Enter inspect · Esc close"
        };
        Paragraph::new(keys)
            .style(Style::default().fg(MUTED).bg(PANEL))
            .render(
                Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
                frame.buffer_mut(),
            );
    }
    if !has_area(body) {
        return;
    }
    if messages.is_empty() {
        let empty = match channel {
            PhoneChannel::Standup => "No conversations in this project yet.",
            PhoneChannel::Blocked => "No conversations need attention.",
            PhoneChannel::Shipping => "No file edits recorded in the available history.",
            PhoneChannel::Watercooler => "No messages recorded in the available history.",
        };
        Paragraph::new(empty)
            .style(Style::default().fg(MUTED).bg(PANEL))
            .wrap(Wrap { trim: false })
            .render(body, frame.buffer_mut());
        return;
    }
    let selected = selected.min(messages.len() - 1);
    let selected_height = selected_height(&messages[selected], body.width);
    let visible = usize::from(
        body.height
            .saturating_sub(selected_height.saturating_sub(MESSAGE_HEIGHT))
            / MESSAGE_HEIGHT,
    )
    .max(1);
    let start = selected.saturating_sub(visible.saturating_sub(1));
    let mut offset = 0;
    for (index, message) in messages.iter().enumerate().skip(start) {
        let remaining = body.height.saturating_sub(offset);
        let wanted = if index == selected {
            selected_height
        } else {
            MESSAGE_HEIGHT
        };
        if remaining == 0 || (remaining < wanted && index != selected) {
            break;
        }
        let row = Rect::new(body.x, body.y + offset, body.width, wanted.min(remaining));
        draw_message(
            frame,
            world,
            message,
            canvas,
            sprites,
            now,
            row,
            index == selected,
        );
        offset += row.height;
    }
}

fn dim_backdrop(frame: &mut Frame, area: Rect, light: bool) {
    let base = if light {
        (244u16, 239u16, 228u16)
    } else {
        (13, 11, 20)
    };
    let dim = |color| {
        let color = if light {
            super::light_color(color)
        } else {
            color
        };
        match color {
            Color::Rgb(r, g, b) => Color::Rgb(
                ((u16::from(r) + base.0 * 7) / 8) as u8,
                ((u16::from(g) + base.1 * 7) / 8) as u8,
                ((u16::from(b) + base.2 * 7) / 8) as u8,
            ),
            _ => {
                if light {
                    super::LIGHT_BACKGROUND
                } else {
                    BACKGROUND
                }
            }
        }
    };
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if let Some(cell) = frame.buffer_mut().cell_mut((x, y)) {
                let foreground = dim(cell.fg);
                let background = dim(cell.bg);
                cell.set_fg(foreground).set_bg(background);
            }
        }
    }
}

fn channel_tabs(channel: PhoneChannel, width: u16) -> Line<'static> {
    let tabs = [
        PhoneChannel::Standup,
        PhoneChannel::Blocked,
        PhoneChannel::Shipping,
        PhoneChannel::Watercooler,
    ];
    Line::from(
        tabs.into_iter()
            .enumerate()
            .map(|(index, tab)| {
                let label = if width < 28 {
                    ""
                } else if width < 34 {
                    ["Now", "!", "Ed", "Msg"][index]
                } else if width < 44 {
                    ["Now", "Help", "Edits", "Chat"][index]
                } else {
                    tab.label()
                };
                let style = if tab == channel {
                    Style::default()
                        .fg(BACKGROUND)
                        .bg(GOOD)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(MUTED).bg(PANEL)
                };
                Span::styled(format!(" {} {label} ", index + 1), style)
            })
            .collect::<Vec<_>>(),
    )
}

fn selected_height(message: &PhoneMessage, width: u16) -> u16 {
    let text_width = width.saturating_sub(if width >= 44 { 8 } else { 3 }) as usize;
    let title = super::desk::wrapped_lines(&message.name, text_width)
        .len()
        .clamp(1, 2);
    let detail = super::desk::wrapped_lines(&message.text, text_width)
        .len()
        .clamp(1, 3);
    (title + detail + 3) as u16
}

#[allow(clippy::too_many_arguments)]
fn draw_message(
    frame: &mut Frame,
    world: &World,
    message: &PhoneMessage,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    now: Millis,
    row: Rect,
    selected: bool,
) {
    let Some((office, worker)) = message
        .worker_id
        .as_ref()
        .and_then(|id| find_worker(world, id))
    else {
        return;
    };
    let look = look_for_worker(&office.workers, worker);
    let attention = message.status.needs_attention() && !message.historical;
    let background = if selected {
        PANEL_HIGHLIGHT
    } else if attention {
        ATTENTION_PANEL
    } else {
        PANEL
    };
    let accent = if attention {
        status_style(message.status)
    } else {
        Style::default().fg(ACCENT)
    };
    paint_opaque(frame, row, Style::default().bg(background));
    Paragraph::new(if selected {
        ">"
    } else if attention {
        "!"
    } else {
        " "
    })
    .style(accent.bg(background))
    .render(Rect::new(row.x, row.y, 1, 1), frame.buffer_mut());
    let avatar_width = if row.width >= 44 { 7 } else { 2 };
    if avatar_width > 2 {
        draw_avatar(
            frame,
            canvas,
            sprites,
            worker,
            &look,
            now,
            Rect::new(row.x + 1, row.y, 5, row.height.min(3)),
            background,
        );
    }
    let text = Rect::new(
        row.x + avatar_width,
        row.y,
        row.width.saturating_sub(avatar_width + 1),
        row.height,
    );
    if !has_area(text) {
        return;
    }
    let title_height = if selected {
        (super::desk::wrapped_lines(&message.name, text.width as usize)
            .len()
            .clamp(1, 2) as u16)
            .min(text.height)
    } else {
        1
    };
    Paragraph::new(if selected {
        safe_display(&message.name)
    } else {
        short_path(&message.name, text.width as usize)
    })
    .style(
        Style::default()
            .fg(INK)
            .bg(background)
            .add_modifier(Modifier::BOLD),
    )
    .wrap(Wrap { trim: false })
    .render(
        Rect::new(text.x, text.y, text.width, title_height),
        frame.buffer_mut(),
    );
    let mut y = text.y + title_height;
    if y < text.bottom() {
        let age = duration_label(elapsed_ms(now, message.at));
        let prefix = format!("{} · {age} ago · ", message.state_label);
        Paragraph::new(Line::from(vec![
            Span::styled(prefix.clone(), accent.bg(background)),
            Span::styled(
                short_path(
                    &message.context,
                    text.width.saturating_sub(Span::raw(prefix).width() as u16) as usize,
                ),
                Style::default().fg(MUTED).bg(background),
            ),
        ]))
        .render(Rect::new(text.x, y, text.width, 1), frame.buffer_mut());
        y += 1;
    }
    let detail_height = text
        .bottom()
        .saturating_sub(y + if selected { 2 } else { 1 });
    if detail_height > 0 {
        Paragraph::new(if selected {
            message.text.clone()
        } else {
            short_path(&message.text, text.width as usize)
        })
        .style(Style::default().fg(INK).bg(background))
        .wrap(Wrap { trim: false })
        .render(
            Rect::new(text.x, y, text.width, detail_height),
            frame.buffer_mut(),
        );
    }
    if selected && text.height >= 5 {
        let hint = if message.historical {
            "Enter: full history and current state"
        } else if attention && matches!(worker.activity, Activity::Waiting { .. }) {
            "Enter: inspect · review in the original conversation"
        } else if attention {
            "Enter: inspect · check the original conversation"
        } else {
            "Enter: full title, activity and conversation details"
        };
        Paragraph::new(hint)
            .style(Style::default().fg(MUTED).bg(background))
            .render(
                Rect::new(text.x, text.bottom() - 2, text.width, 1),
                frame.buffer_mut(),
            );
    }
}

// An avatar needs the canvas, where it goes, how big it is and whose it is;
// a struct would only move the same list somewhere else.
#[allow(clippy::too_many_arguments)]
fn draw_avatar(
    frame: &mut Frame,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    worker: &Worker,
    look: &WorkerLook,
    now: Millis,
    area: Rect,
    background: Color,
) {
    if !has_area(area) {
        return;
    }
    paint_opaque(frame, area, Style::default().bg(background));
    let width = area.width as usize;
    let height = area.height as usize;
    canvas.resize_for_cells(width, height);
    canvas.fill(background);
    render_worker_head_with_look(
        canvas,
        sprites,
        worker,
        look,
        now,
        PixelRect {
            x: 0,
            y: 0,
            width: canvas.width(),
            height: canvas.height(),
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

fn messages_for(
    channel: PhoneChannel,
    world: &World,
    office: Option<&Office>,
    now: Millis,
) -> Vec<PhoneMessage> {
    let mut messages = match channel {
        PhoneChannel::Standup => standup_messages(world, office, now),
        PhoneChannel::Blocked => blocked_messages(world, office, now),
        PhoneChannel::Shipping => shipping_messages(world, office, now),
        PhoneChannel::Watercooler => watercooler_messages(world, office, now),
    };
    if matches!(channel, PhoneChannel::Shipping | PhoneChannel::Watercooler) {
        messages.sort_by_key(|message| std::cmp::Reverse(message.at));
    } else {
        messages.sort_by_key(|message| {
            (
                !message.status.needs_attention(),
                message.status == WorkerStatus::Idle,
                std::cmp::Reverse(message.at),
            )
        });
    }
    messages
}

/// Stable identities for preserving focus while new activity reorders the inbox.
pub(crate) fn message_keys(
    channel: PhoneChannel,
    world: &World,
    office: Option<&Office>,
    now: Millis,
) -> Vec<String> {
    messages_for(channel, world, office, now)
        .into_iter()
        .map(|message| {
            let id = message.worker_id.map(|id| id.0).unwrap_or_default();
            if message.historical {
                format!("history:{}:{id}:{}:{}", id.len(), message.at, message.text)
            } else {
                format!("current:{id}")
            }
        })
        .collect()
}

pub(crate) fn message_workers(
    channel: PhoneChannel,
    world: &World,
    office: Option<&Office>,
    now: Millis,
) -> Vec<Option<WorkerId>> {
    messages_for(channel, world, office, now)
        .into_iter()
        .map(|message| message.worker_id)
        .collect()
}

fn standup_messages(world: &World, selected: Option<&Office>, now: Millis) -> Vec<PhoneMessage> {
    world
        .offices()
        .filter(|office| selected.is_none_or(|selected| office.id == selected.id))
        .flat_map(|office| {
            office.workers.iter().map(move |worker| {
                let summary = super::desk::inspection_summary(worker, now);
                worker_message(office, worker, worker.last_seen, summary.detail, now, false)
            })
        })
        .collect()
}

fn worker_message(
    office: &Office,
    worker: &Worker,
    at: Millis,
    text: String,
    now: Millis,
    historical: bool,
) -> PhoneMessage {
    PhoneMessage {
        worker_id: Some(worker.id.clone()),
        name: worker.name.clone(),
        context: format!("{} · {}", worker.agent.label(), safe_display(&office.name)),
        historical,
        state_label: if historical {
            "recorded"
        } else {
            match worker_status(worker, now) {
                WorkerStatus::Blocked if matches!(worker.activity, Activity::Waiting { .. }) => {
                    "needs review"
                }
                WorkerStatus::Blocked => "silent",
                WorkerStatus::Failed => "error",
                WorkerStatus::Idle => "idle",
                WorkerStatus::Running => "working",
            }
        },
        at,
        text,
        status: worker_status(worker, now),
    }
}

fn blocked_messages(world: &World, selected: Option<&Office>, now: Millis) -> Vec<PhoneMessage> {
    standup_messages(world, selected, now)
        .into_iter()
        .filter(|message| message.status.needs_attention())
        .collect()
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

fn history_messages(
    world: &World,
    selected: Option<&Office>,
    now: Millis,
    edits: bool,
) -> Vec<PhoneMessage> {
    world
        .offices()
        .filter(|office| selected.is_none_or(|selected| office.id == selected.id))
        .flat_map(|office| {
            office.workers.iter().flat_map(move |worker| {
                worker
                    .history
                    .iter()
                    .filter(move |beat| {
                        if edits {
                            matches!(beat.activity, Activity::Editing { .. })
                        } else {
                            matches!(beat.activity, Activity::Talking { .. })
                        }
                    })
                    .map(move |beat| {
                        worker_message(office, worker, beat.at, beat_display(beat), now, true)
                    })
            })
        })
        .collect()
}

fn shipping_messages(world: &World, selected: Option<&Office>, now: Millis) -> Vec<PhoneMessage> {
    history_messages(world, selected, now, true)
}

fn watercooler_messages(
    world: &World,
    selected: Option<&Office>,
    now: Millis,
) -> Vec<PhoneMessage> {
    history_messages(world, selected, now, false)
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
    fn current_message_identity_survives_new_activity_and_reordering() {
        let (mut world, id) = blocked_world();
        let before = message_keys(PhoneChannel::Standup, &world, None, 10)[0].clone();
        world.apply(event(
            "/workspace/app",
            "new-error",
            15,
            EventKind::Acted(Activity::Error {
                detail: "Error".into(),
            }),
        ));
        world.apply(event(
            "/workspace/app",
            &id.0,
            20,
            EventKind::Turn { in_flight: false },
        ));
        let after = message_keys(PhoneChannel::Standup, &world, None, 25);
        assert_eq!(after[1], before);
    }

    #[test]
    fn phone_tabs_fit_the_smallest_supported_panel() {
        for width in [22, 26, 30, 40, 72] {
            for channel in [
                PhoneChannel::Standup,
                PhoneChannel::Blocked,
                PhoneChannel::Shipping,
                PhoneChannel::Watercooler,
            ] {
                assert!(channel_tabs(channel, width).width() <= width as usize);
            }
        }
    }

    #[test]
    fn historical_edit_keeps_its_evidence_when_worker_later_needs_approval() {
        use ratatui::{backend::TestBackend, Terminal};
        let (mut world, id) = blocked_world();
        world.apply(event(
            "/workspace/app",
            &id.0,
            10,
            EventKind::Did(Beat {
                at: 10,
                activity: Activity::Editing {
                    detail: "src/payment.rs".into(),
                },
                outcome: Some(Outcome::Changed {
                    added: 12,
                    removed: 3,
                }),
            }),
        ));
        world.apply(event(
            "/workspace/app",
            &id.0,
            20,
            EventKind::Acted(Activity::Waiting {
                detail: "approve deploy".into(),
            }),
        ));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut canvas = Canvas::new(0, 0);
        let sprites = SpriteSet::new();
        terminal
            .draw(|frame| {
                draw(
                    frame,
                    PhoneDrawContext {
                        world: &world,
                        office: None,
                        channel: PhoneChannel::Shipping,
                        selected: 0,
                        now: 500,
                        transition_at: 0,
                        canvas: &mut canvas,
                        sprites: &sprites,
                    },
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
        assert!(text.contains("PHONE / EDITS"));
        assert!(text.contains("src/payment.rs"));
        assert!(text.contains("+12 −3"));
        assert!(!text.contains("approve deploy"));
        assert!(text.contains("recorded"));
    }

    #[test]
    fn attention_precedes_idle_and_errors_are_included_without_old_prompts() {
        let (mut world, id) = blocked_world();
        world.apply(event(
            "/workspace/app",
            &id.0,
            10,
            EventKind::Turn { in_flight: false },
        ));
        world.apply(event(
            "/workspace/app",
            "failed",
            20,
            EventKind::Acted(Activity::Error {
                detail: "Connection refused".into(),
            }),
        ));
        let messages = messages_for(PhoneChannel::Standup, &world, None, 100);
        assert_eq!(messages[0].status, WorkerStatus::Failed);
        assert!(messages[0].text.contains("Connection refused"));
        assert_eq!(messages[1].status, WorkerStatus::Idle);
        assert!(!messages[1].text.contains("approve deploy"));
        let messages = messages_for(PhoneChannel::Blocked, &world, None, 100);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn recorded_events_are_newest_first_across_workers() {
        let (mut world, id) = blocked_world();
        for (at, worker) in [(10, id.0.as_str()), (30, "other"), (20, id.0.as_str())] {
            world.apply(event(
                "/workspace/app",
                worker,
                at,
                EventKind::Did(Beat {
                    at,
                    activity: Activity::Talking {
                        detail: format!("message {at}"),
                    },
                    outcome: None,
                }),
            ));
        }
        let messages = messages_for(PhoneChannel::Watercooler, &world, None, 50);
        assert_eq!(
            messages
                .iter()
                .map(|message| message.at)
                .collect::<Vec<_>>(),
            [30, 20, 10]
        );
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

        let standup = messages_for(PhoneChannel::Standup, &world, None, now);
        assert_eq!(standup.len(), 1);
        assert_eq!(standup[0].status, WorkerStatus::Blocked);
        assert!(standup[0].text.contains("No approval was identified"));
        assert!(standup[0].context.contains("claude"));

        let blocked = messages_for(PhoneChannel::Blocked, &world, None, now);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].status, WorkerStatus::Blocked);
        assert!(blocked[0].text.contains("No recent activity"));
        assert!(!blocked[0].text.contains("cargo test"));

        let shipping = messages_for(PhoneChannel::Shipping, &world, None, now);
        assert_eq!(shipping.len(), 1);
        assert!(shipping[0].historical);
        assert!(shipping[0].text.contains("editing • src/phone.rs"));
        assert!(shipping[0].text.contains("+12 −3"));

        let watercooler = messages_for(PhoneChannel::Watercooler, &world, None, now);
        assert_eq!(watercooler.len(), 1);
        assert!(watercooler[0].text.contains("I found the issue"));
    }
}
