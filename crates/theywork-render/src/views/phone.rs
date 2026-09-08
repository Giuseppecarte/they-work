//! Native inbox for observed state and recorded history.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};
use ratatui::Frame;
use theywork_core::{
    Activity, Beat, Millis, Office, Outcome, Worker, WorkerId, WorkerStatus, World,
};

use super::{
    duration_label, elapsed_ms, has_area, inset, paint_opaque, safe_display, short_path,
    worker_status, ACCENT, BACKGROUND, INK, MUTED, PANEL, PANEL_HIGHLIGHT, WARNING,
};
use crate::interaction::{Action, HitRegion};
use crossterm::event::KeyCode;

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
}

/// A native inbox for recorded activity. It never answers a provider request.
pub(crate) fn draw(frame: &mut Frame, context: PhoneDrawContext<'_>) -> Vec<HitRegion> {
    let PhoneDrawContext {
        world,
        office,
        channel,
        selected,
        now,
    } = context;
    let full = frame.area();
    let area = Rect::new(
        full.x,
        full.y.saturating_add(2),
        full.width,
        full.height.saturating_sub(3),
    );
    let mut hits = Vec::new();
    if !has_area(area) {
        return hits;
    }
    let width = area.width.min(96);
    let panel = Rect::new(
        area.x + (area.width - width) / 2,
        area.y,
        width,
        area.height.min(42),
    );
    paint_opaque(frame, area, Style::default().fg(INK).bg(BACKGROUND));
    paint_opaque(frame, panel, Style::default().fg(INK).bg(PANEL));
    let messages = messages_for(channel, world, office, now);
    Block::default()
        .title(format!(
            " PHONE / {} · {} ",
            channel.label().to_uppercase(),
            messages.len()
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MUTED))
        .style(Style::default().bg(PANEL))
        .render(panel, frame.buffer_mut());
    let inner = inset(panel, 1);
    if !has_area(inner) {
        return hits;
    }
    Paragraph::new(short_path(
        &format!(
            "{} · {} conversations",
            safe_display(office.map_or("All projects", |office| office.name.as_str())),
            office.map_or_else(|| world.worker_count(), |office| office.workers.len())
        ),
        inner.width.into(),
    ))
    .style(Style::default().fg(MUTED))
    .render(
        Rect::new(inner.x, inner.y, inner.width, 1),
        frame.buffer_mut(),
    );
    let channels = [
        PhoneChannel::Standup,
        PhoneChannel::Blocked,
        PhoneChannel::Shipping,
        PhoneChannel::Watercooler,
    ];
    let columns = if inner.width >= 44 {
        4
    } else if inner.width >= 28 {
        2
    } else {
        1
    };
    let tab_rows = 4usize.div_ceil(columns) as u16;
    let mut x = inner.x;
    for (index, tab) in channels.into_iter().enumerate() {
        let row = index / columns;
        let col = index % columns;
        if col == 0 {
            x = inner.x;
        }
        let y = inner.y + 1 + row as u16;
        if y >= inner.bottom().saturating_sub(1) {
            break;
        }
        let slot = if columns == 4 {
            inner.right().saturating_sub(x)
        } else {
            inner.width / columns as u16
        };
        if let Some(hit) = crate::components::button(
            frame,
            Rect::new(x, y, slot, 1),
            tab.label(),
            Action::Key(KeyCode::Char(char::from(b'1' + index as u8))),
            if tab == channel {
                crate::components::ButtonKind::Primary
            } else {
                crate::components::ButtonKind::Quiet
            },
        ) {
            x = if columns == 4 {
                hit.area.right() + 1
            } else {
                x + slot
            };
            hits.push(hit);
        }
    }
    let top = (1 + tab_rows).min(inner.height);
    let body = Rect::new(
        inner.x,
        inner.y + top,
        inner.width,
        inner.height.saturating_sub(top + 1),
    );
    if !has_area(body) {
        return hits;
    }
    let selected = selected.min(messages.len().saturating_sub(1));
    let footer = Rect::new(inner.x, inner.bottom() - 1, inner.width, 1);
    let order = if matches!(channel, PhoneChannel::Shipping | PhoneChannel::Watercooler) {
        "recorded history"
    } else {
        "observed state"
    };
    Paragraph::new(short_path(
        &format!(
            "{}/{} · {order} · Enter inspect",
            if messages.is_empty() { 0 } else { selected + 1 },
            messages.len()
        ),
        footer.width.into(),
    ))
    .style(Style::default().fg(MUTED))
    .render(footer, frame.buffer_mut());
    if messages.is_empty() {
        let empty = match channel {
            PhoneChannel::Standup => "No conversations in this scope.",
            PhoneChannel::Blocked => "No attention items in the available observations.",
            PhoneChannel::Shipping => "No file edits recorded in the available history.",
            PhoneChannel::Watercooler => "No messages recorded in the available history.",
        };
        Paragraph::new(empty)
            .style(Style::default().fg(MUTED))
            .wrap(Wrap { trim: false })
            .render(body, frame.buffer_mut());
        return hits;
    }
    let selected_height = body.height.min(if inner.width < 44 { 7 } else { 6 });
    let visible = usize::from(body.height.saturating_sub(selected_height) / 4) + 1;
    let start = selected.saturating_sub(visible - 1);
    let mut y = body.y;
    for (index, message) in messages.iter().enumerate().skip(start) {
        let remaining = body.bottom().saturating_sub(y);
        let height = if index == selected {
            selected_height
        } else {
            4
        };
        if remaining == 0 || (remaining < height && index != selected) {
            break;
        }
        let row = Rect::new(body.x, y, body.width, height.min(remaining));
        draw_message(frame, world, message, now, row, index == selected);
        if let Some(id) = &message.worker_id {
            hits.push(HitRegion::new(row, Action::Inspect(id.clone())));
        }
        y += row.height;
    }
    hits
}

fn draw_message(
    frame: &mut Frame,
    world: &World,
    message: &PhoneMessage,
    now: Millis,
    row: Rect,
    selected: bool,
) {
    let worker = message.worker_id.as_ref().and_then(|id| world.worker(id));
    let current = worker.map_or("Source unavailable", |worker| {
        crate::presentation::state_label(worker, now)
    });
    let attention = !message.historical
        && matches!(
            current,
            "Approval needed" | "Question for you" | "Needs a follow-up" | "Error reported"
        );
    let background = if selected { PANEL_HIGHLIGHT } else { PANEL };
    paint_opaque(frame, row, Style::default().fg(INK).bg(background));
    let text = Rect::new(row.x + 1, row.y, row.width.saturating_sub(1), row.height);
    if !has_area(text) {
        return;
    }
    Paragraph::new(if selected { ">" } else { " " })
        .style(Style::default().fg(ACCENT))
        .render(Rect::new(row.x, row.y, 1, 1), frame.buffer_mut());
    let title_lines = super::desk::wrapped_lines(&message.name, text.width.into());
    let title_rows = if selected && row.height >= 6 {
        title_lines.len().clamp(1, 2) as u16
    } else {
        1
    };
    Paragraph::new(if title_rows == 1 {
        short_path(&message.name, text.width.into())
    } else {
        format!(
            "{}\n{}",
            title_lines[0],
            short_path(&title_lines[1..].join(" "), text.width.into())
        )
    })
    .style(
        Style::default()
            .fg(if attention { WARNING } else { INK })
            .add_modifier(Modifier::BOLD),
    )
    .wrap(Wrap { trim: false })
    .render(
        Rect::new(text.x, text.y, text.width, title_rows.min(text.height)),
        frame.buffer_mut(),
    );
    let mut y = text.y + title_rows;
    if y < text.bottom() {
        let state = if message.historical {
            "Recorded"
        } else {
            message.state_label
        };
        let line = format!(
            "{state} · {} ago · {}",
            duration_label(elapsed_ms(now, message.at)),
            message.context
        );
        Paragraph::new(short_path(&line, text.width.into()))
            .style(Style::default().fg(if attention { WARNING } else { MUTED }))
            .render(Rect::new(text.x, y, text.width, 1), frame.buffer_mut());
        y += 1;
    }
    if y < text.bottom() {
        let coverage = worker.map_or("Source unavailable", |worker| {
            crate::presentation::coverage_label(&worker.coverage, now)
        });
        Paragraph::new(short_path(coverage, text.width.into()))
            .style(Style::default().fg(MUTED))
            .render(Rect::new(text.x, y, text.width, 1), frame.buffer_mut());
        y += 1;
    }
    if y < text.bottom() {
        let height = usize::from(text.bottom() - y);
        let mut lines = super::safe_multiline(&message.text)
            .split('\n')
            .flat_map(|line| super::desk::wrapped_lines(line, text.width.into()))
            .collect::<Vec<_>>();
        if lines.len() > height {
            let remainder = lines[height - 1..].join(" ");
            lines.truncate(height);
            lines[height - 1] = short_path(&remainder, text.width.into());
        }
        Paragraph::new(lines.join("\n"))
            .style(Style::default().fg(INK))
            .wrap(Wrap { trim: false })
            .render(
                Rect::new(text.x, y, text.width, text.bottom() - y),
                frame.buffer_mut(),
            );
    }
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
                !message_needs_attention(message),
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
                let detail = if worker.status_at(now) != WorkerStatus::Failed
                    && worker.wait_reason.is_some()
                {
                    crate::presentation::wait_description(worker.wait_reason).to_string()
                } else {
                    summary.detail
                };
                worker_message(office, worker, worker.last_seen, detail, now, false)
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
    let text = if !historical
        && worker.coverage.observed_at > 0
        && (!worker.coverage.available || worker.coverage.is_stale_at(now))
    {
        format!(
            "Last recorded: {}",
            crate::work_brief::activity_text(&worker.activity)
        )
    } else {
        text
    };
    PhoneMessage {
        worker_id: Some(worker.id.clone()),
        name: worker.name.clone(),
        context: format!("{} · {}", worker.agent.label(), safe_display(&office.name)),
        historical,
        state_label: if historical {
            "recorded"
        } else {
            crate::presentation::state_label(worker, now)
        },
        at,
        text,
        status: worker_status(worker, now),
    }
}

fn message_needs_attention(message: &PhoneMessage) -> bool {
    !message.historical
        && matches!(
            message.state_label,
            "Approval needed" | "Question for you" | "Needs a follow-up" | "Error reported"
        )
}

fn blocked_messages(world: &World, selected: Option<&Office>, now: Millis) -> Vec<PhoneMessage> {
    standup_messages(world, selected, now)
        .into_iter()
        .filter(message_needs_attention)
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
    fn native_phone_exposes_every_channel_and_real_record_at_compact_sizes() {
        use ratatui::{backend::TestBackend, Terminal};
        let (mut world, id) = blocked_world();
        world.apply(event(
            "/workspace/app",
            &id.0,
            1,
            EventKind::Coverage(theywork_core::SourceCoverage {
                observed_at: 1,
                available: false,
                ..Default::default()
            }),
        ));
        for (width, height) in [(32, 14), (80, 24), (192, 58)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut hits = Vec::new();
            terminal
                .draw(|frame| {
                    hits = draw(
                        frame,
                        PhoneDrawContext {
                            world: &world,
                            office: None,
                            channel: PhoneChannel::Standup,
                            selected: 0,
                            now: 20,
                        },
                    );
                })
                .unwrap();
            for key in ['1', '2', '3', '4'] {
                assert!(
                    hits.iter()
                        .any(|hit| hit.action == Action::Key(KeyCode::Char(key))),
                    "missing {key} at {width}x{height}"
                );
            }
            assert!(hits
                .iter()
                .any(|hit| hit.action == Action::Inspect(id.clone())));
            let body = Rect::new(0, 2, width, height - 3);
            assert!(hits.iter().all(|hit| hit.area.width > 0
                && hit.area.height > 0
                && body.intersection(hit.area) == hit.area));
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(text.contains("Source unavailable"));
            assert!(text.contains("Dev Phone"));
            assert!(text.contains("Last recorded:"));
            assert!(!text.contains("Approval needed"));
        }
    }

    #[test]
    fn unavailable_or_stale_source_is_not_a_current_attention_request() {
        let (mut world, id) = blocked_world();
        for available in [false, true] {
            world.apply(event(
                "/workspace/app",
                &id.0,
                1,
                EventKind::Coverage(theywork_core::SourceCoverage {
                    observed_at: 1,
                    available,
                    ..Default::default()
                }),
            ));
            let now = if available { i64::MAX } else { 20 };
            assert!(messages_for(PhoneChannel::Blocked, &world, None, now).is_empty());
            assert!(messages_for(PhoneChannel::Standup, &world, None, now)[0]
                .text
                .starts_with("Last recorded:"));
        }
    }

    #[test]
    fn an_explicit_question_never_uses_an_inferred_silence_explanation() {
        let (mut world, id) = blocked_world();
        world.apply(event(
            "/workspace/app",
            &id.0,
            1,
            EventKind::Wait(Some(theywork_core::WaitReason::HumanInput)),
        ));
        let records = messages_for(PhoneChannel::Blocked, &world, None, BLOCKED_AFTER_MS + 20);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].state_label, "Question for you");
        assert!(records[0].text.contains("recorded a question"));
        assert!(!records[0].text.contains("No recent activity"));
        assert!(!records[0].text.contains("No approval was identified"));
    }

    #[test]
    fn compact_message_truncation_is_explicit_without_changing_recorded_text() {
        use ratatui::{backend::TestBackend, Terminal};
        let (world, id) = blocked_world();
        let original = "First recorded line has a detailed explanation. A second line contains more source evidence than can fit in this compact row.";
        let message = PhoneMessage {
            worker_id: Some(id),
            name: "Recorded task".into(),
            context: "codex".into(),
            historical: true,
            state_label: "recorded",
            at: 1,
            text: original.into(),
            status: WorkerStatus::Idle,
        };
        let mut terminal = Terminal::new(TestBackend::new(32, 4)).unwrap();
        terminal
            .draw(|frame| draw_message(frame, &world, &message, 20, Rect::new(0, 0, 32, 4), true))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains('…'));
        assert_eq!(message.text, original);
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
                    },
                );
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
