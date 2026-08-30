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
    below_tab_bar, draw_footer, draw_header, draw_panel, draw_tiny, fill_office_background,
    has_area, human_tokens, render_worker_with_look, safe_display, short_path, status_style,
    token_bar, worker_status, PixelRect, ACCENT, GOOD, INK, MUTED,
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

    let left_width = if body.width >= 44 {
        body.width.saturating_mul(5) / 11
    } else {
        body.width / 2
    }
    .max(1);
    let left = Rect::new(body.x, body.y, left_width, body.height);
    let right = Rect::new(
        body.x.saturating_add(left_width),
        body.y,
        body.width.saturating_sub(left_width),
        body.height,
    );
    let left_inner = draw_panel(frame, left, "LIVE FEED", true);
    let right_inner = draw_panel(frame, right, "WORKER DETAILS", false);

    if has_area(left_inner) {
        canvas.resize(left_inner.width as usize, left_inner.height as usize * 2);
        let floor_start = fill_office_background(canvas, sprites);
        if canvas.width() >= sprites.plant.width() + 2 {
            canvas.blit(&sprites.plant, 1, 1);
        }
        if canvas.width() >= sprites.water_cooler.width() + 2 {
            canvas.blit(
                &sprites.water_cooler,
                canvas
                    .width()
                    .saturating_sub(sprites.water_cooler.width() + 1),
                1,
            );
        }

        let look = look_for_worker(&office.workers, worker);
        let worker_sprite = sprites.worker_frame(worker, look, now);
        let desk_height = canvas.height().clamp(1, 7);
        let desk_y = canvas.height().saturating_sub(desk_height);
        let worker_width = worker_sprite
            .width()
            .min(canvas.width().saturating_sub(2))
            .max(1);
        let worker_height = worker_sprite
            .height()
            .min(desk_y.saturating_sub(1).max(1))
            .max(1);
        let worker_x = canvas.width().saturating_sub(worker_width) / 2;
        let worker_y = floor_start.min(desk_y).saturating_sub(worker_height);
        render_worker_with_look(
            canvas,
            sprites,
            worker,
            &look,
            now,
            PixelRect {
                x: worker_x,
                y: worker_y,
                width: worker_width,
                height: worker_height,
            },
        );
        let desk_width = canvas.width().min(sprites.desk.width()).max(1);
        canvas.blit_scaled(
            &sprites.desk,
            canvas.width().saturating_sub(desk_width) / 2,
            desk_y,
            desk_width,
            desk_height,
        );
        canvas.render(frame.buffer_mut(), left_inner);
    }

    if has_area(right_inner) {
        let detail = worker.activity.detail().unwrap_or("no detail");
        let bar = token_bar(
            worker.tokens_used,
            max_tokens,
            right_inner.width.saturating_sub(2) as usize,
        );
        let mut lines = vec![
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
                short_path(detail, right_inner.width.saturating_sub(2) as usize),
                Style::default().fg(INK),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("agent     {}", worker.agent.label()),
                Style::default().fg(MUTED),
            )),
            Line::from(Span::styled(
                format!("status    {}", status.label()),
                status_style(status),
            )),
            Line::from(Span::styled(
                format!("branch    {}", branch),
                Style::default().fg(MUTED),
            )),
            Line::from(Span::styled(
                format!(
                    "tokens    {} / {} max",
                    human_tokens(worker.tokens_used),
                    human_tokens(max_tokens)
                ),
                Style::default().fg(MUTED),
            )),
        ];
        if !bar.is_empty() {
            lines.push(Line::from(Span::styled(bar, Style::default().fg(GOOD))));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "RECENT ACTIVITY",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )));
        if worker.history.is_empty() {
            lines.push(Line::from(Span::styled(
                "  no recorded beats yet",
                Style::default().fg(MUTED),
            )));
        } else {
            let start = worker.history.len().saturating_sub(TIMELINE_LIMIT);
            for beat in worker.history.iter().skip(start) {
                lines.push(timeline_line(beat, right_inner.width as usize));
            }
        }
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .render(right_inner, frame.buffer_mut());
    }
}
