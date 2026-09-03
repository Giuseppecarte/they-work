//! Detailed employee view: a large sprite and the useful live context.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::Frame;
use theywork_core::{Activity, Beat, Millis, Office, Outcome, Worker};

use crate::canvas::Canvas;
use crate::sprite::{look_for_worker, SpriteSet};

use super::{
    below_tab_bar, draw_footer, draw_header, draw_panel, draw_tiny, has_area, human_tokens,
    paint_opaque, render_worker_with_look, safe_display, short_path, status_style, token_bar,
    worker_status, PixelRect, ACCENT, BACKGROUND, GOOD, INK, MUTED, PANEL, PANEL_HIGHLIGHT,
};
const TIMELINE_LIMIT: usize = 8;

fn timeline_time(at: Millis) -> String {
    let minutes = at.max(0).div_euclid(60_000).rem_euclid(1_440) as u64;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn timeline_label(activity: &Activity) -> &'static str {
    match activity {
        Activity::Typing { .. } => "RAN",
        Activity::Reading { .. } => "READ",
        Activity::Editing { .. } => "EDITED",
        Activity::Searching { .. } => "SEARCH",
        Activity::Thinking => "THOUGHT",
        Activity::Talking { .. } => "SAID",
        Activity::Waiting { .. } => "ASKED",
        Activity::Idle => "IDLE",
        Activity::Error { .. } => "FAILED",
    }
}

fn timeline_outcome(outcome: Option<Outcome>) -> String {
    match outcome {
        Some(Outcome::Exited(status)) => format!("exit {status}"),
        Some(Outcome::Changed { added, removed }) => format!("+{added} −{removed}"),
        None => String::new(),
    }
}

fn timeline_color(activity: &Activity, outcome: Option<Outcome>) -> Color {
    match outcome {
        Some(Outcome::Exited(status)) if status != 0 => super::HOT,
        Some(Outcome::Exited(_)) => GOOD,
        Some(Outcome::Changed { .. }) => ACCENT,
        None => match activity {
            Activity::Error { .. } => super::HOT,
            Activity::Waiting { .. } => super::WARNING,
            Activity::Thinking => ACCENT,
            Activity::Idle => MUTED,
            _ => GOOD,
        },
    }
}

fn timeline_line(beat: &Beat, width: usize) -> Line<'static> {
    let label = timeline_label(&beat.activity);
    let outcome = timeline_outcome(beat.outcome);
    let prefix_width = 6 + 7;
    let outcome_width = if outcome.is_empty() {
        0
    } else {
        outcome.chars().count() + 2
    };
    let detail_width = width.saturating_sub(prefix_width + outcome_width);
    let detail = beat
        .activity
        .detail()
        .filter(|detail| !detail.is_empty())
        .map(safe_display)
        .unwrap_or_else(|| "-".to_string());
    let detail = short_path(&detail, detail_width);
    let label_style = Style::default().fg(timeline_color(&beat.activity, beat.outcome));
    let mut spans = vec![
        Span::styled(
            format!("{} ", timeline_time(beat.at)),
            Style::default().fg(MUTED),
        ),
        Span::styled(format!("{label:<7}"), label_style),
        Span::styled(detail, Style::default().fg(INK)),
    ];
    if !outcome.is_empty() {
        spans.push(Span::styled(
            format!("  {outcome}"),
            Style::default().fg(
                if matches!(
                    beat.outcome,
                    Some(Outcome::Exited(status)) if status != 0
                ) {
                    super::HOT
                } else {
                    timeline_color(&beat.activity, beat.outcome)
                },
            ),
        ));
    }
    Line::from(spans)
}

pub(crate) fn draw(
    frame: &mut Frame,
    office: Option<&Office>,
    worker: Option<&Worker>,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    now: Millis,
) {
    let area = below_tab_bar(frame.area());
    if area.width < 16 || area.height < 8 {
        draw_tiny(frame, "they-work • terminal too small for the desk view");
        return;
    }
    let (Some(office), Some(worker)) = (office, worker) else {
        draw_tiny(frame, "No desk selected.");
        return;
    };

    let branch = worker
        .git_branch
        .as_deref()
        .map(safe_display)
        .unwrap_or_else(|| "no branch".to_string());
    let status = worker_status(worker, now);
    let max_tokens = office
        .workers
        .iter()
        .map(|worker| worker.tokens_used)
        .max()
        .unwrap_or(0);
    let worker_title = short_path(&worker.name, area.width.saturating_sub(11) as usize);
    let office_title = short_path(&office.name, area.width.saturating_sub(20) as usize);
    let (header, body, footer) = super::vertical_bands(area, 2, 2);
    draw_header(
        frame,
        header,
        &format!("DESK / {}", worker_title),
        &format!(
            "{} • {} • {} • branch {}",
            office_title,
            worker.agent.label(),
            status.label(),
            branch
        ),
    );
    draw_footer(
        frame,
        footer,
        "←↑↓→ / hjkl switch worker   Esc office   q quit",
    );
    if !has_area(body) {
        return;
    }

    paint_opaque(frame, body, Style::default().bg(BACKGROUND));
    let profile_height = body
        .height
        .min(10)
        .min(body.height.saturating_sub(3).max(1));
    let profile = Rect::new(body.x, body.y, body.width, profile_height);
    let avatar_width = profile.width.min(11);
    let avatar = Rect::new(profile.x, profile.y, avatar_width, profile.height.min(7));
    paint_opaque(frame, avatar, Style::default().bg(PANEL));
    if has_area(avatar) {
        canvas.resize_for_cells(avatar.width as usize, avatar.height as usize);
        canvas.clear();
        let look = look_for_worker(&office.workers, worker);
        let width = canvas.encoding().scale_width(9).min(canvas.width()).max(1);
        let height = canvas
            .encoding()
            .scale_half_height(12)
            .min(canvas.height())
            .max(1);
        render_worker_with_look(
            canvas,
            sprites,
            worker,
            &look,
            now,
            PixelRect {
                x: canvas.width().saturating_sub(width) / 2,
                y: canvas.height().saturating_sub(height) / 2,
                width,
                height,
            },
        );
        canvas.render(frame.buffer_mut(), avatar);
    }

    let info = Rect::new(
        profile.x.saturating_add(avatar_width).saturating_add(2),
        profile.y,
        profile.width.saturating_sub(avatar_width.saturating_add(2)),
        profile.height,
    );
    if has_area(info) {
        let detail = worker.activity.detail().unwrap_or("no detail");
        let bar = token_bar(
            worker.tokens_used,
            max_tokens,
            info.width.saturating_sub(2) as usize,
        );
        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    worker.name.to_ascii_uppercase(),
                    Style::default().fg(INK).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", worker.agent.label()),
                    Style::default().fg(ACCENT),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    status.label().to_ascii_uppercase(),
                    status_style(status).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", worker.activity.label().to_ascii_uppercase()),
                    super::activity_style(&worker.activity),
                ),
            ]),
            Line::from(Span::styled(
                format!(
                    "branch {branch}  ·  {} tokens",
                    human_tokens(worker.tokens_used)
                ),
                Style::default().fg(MUTED),
            )),
        ];
        if !bar.is_empty() {
            lines.push(Line::from(Span::styled(bar, Style::default().fg(GOOD))));
        }
        let attention_style = if status.needs_attention() {
            Style::default().fg(INK).bg(PANEL_HIGHLIGHT)
        } else {
            Style::default().fg(INK).bg(PANEL)
        };
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            if status.needs_attention() {
                " WAITING ON YOU "
            } else {
                " CURRENT WORK "
            },
            status_style(status)
                .bg(if status.needs_attention() {
                    PANEL_HIGHLIGHT
                } else {
                    PANEL
                })
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!(
                " {}",
                short_path(detail, info.width.saturating_sub(2) as usize)
            ),
            attention_style,
        )));
        Paragraph::new(Text::from(lines))
            .style(Style::default().bg(BACKGROUND))
            .wrap(Wrap { trim: false })
            .render(info, frame.buffer_mut());
    }

    let thread = Rect::new(
        body.x,
        body.y.saturating_add(profile_height),
        body.width,
        body.height.saturating_sub(profile_height),
    );
    let thread_inner = draw_panel(frame, thread, "THIS THREAD · NEWEST LAST", false);
    if has_area(thread_inner) {
        let available = thread_inner.height as usize;
        let start = worker
            .history
            .len()
            .saturating_sub(available.min(TIMELINE_LIMIT));
        let mut lines = worker
            .history
            .iter()
            .skip(start)
            .map(|beat| timeline_line(beat, thread_inner.width as usize))
            .collect::<Vec<_>>();
        if lines.is_empty() && lines.len() < available {
            let current = Beat {
                at: worker.last_seen,
                activity: worker.activity.clone(),
                outcome: None,
            };
            lines.push(timeline_line(&current, thread_inner.width as usize));
        }
        Paragraph::new(Text::from(lines))
            .style(Style::default().bg(BACKGROUND))
            .wrap(Wrap { trim: false })
            .render(thread_inner, frame.buffer_mut());
    }
}
