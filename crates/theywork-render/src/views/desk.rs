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
    paint_opaque, render_worker_with_look, safe_display, short_path, status_style, worker_status,
    PixelRect, ACCENT, ATTENTION_PANEL, BACKGROUND, GOOD, INK, MUTED, PANEL,
};

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
        Activity::Thinking => "THINKING",
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
        Some(Outcome::Exited(_)) => ACCENT,
        Some(Outcome::Changed { .. }) => GOOD,
        None => match activity {
            Activity::Error { .. } => super::HOT,
            Activity::Waiting { .. } => super::WARNING,
            Activity::Thinking => MUTED,
            Activity::Typing { .. } | Activity::Reading { .. } => ACCENT,
            Activity::Talking { .. } => Color::Rgb(232, 131, 74),
            Activity::Idle => MUTED,
            _ => GOOD,
        },
    }
}

fn timeline_lines(beat: &Beat, width: usize) -> Vec<Line<'static>> {
    let detail_width = width.saturating_sub(15).max(1);
    let detail = beat
        .activity
        .detail()
        .filter(|value| !value.is_empty())
        .map(safe_display)
        .unwrap_or_else(|| beat.activity.label().to_string());
    let mut chunks = vec![String::new()];
    let mut used = 0;
    for character in detail.chars() {
        let value = character.to_string();
        let size = Span::raw(value.clone()).width();
        if used + size > detail_width && used > 0 {
            chunks.push(String::new());
            used = 0;
        }
        chunks.last_mut().expect("one chunk").push_str(&value);
        used += size;
    }
    let background = if matches!(beat.activity, Activity::Waiting { .. }) {
        ATTENTION_PANEL
    } else {
        BACKGROUND
    };
    let color = timeline_color(&beat.activity, beat.outcome);
    let mut lines = chunks
        .into_iter()
        .enumerate()
        .map(|(index, detail)| {
            let prefix = if index == 0 {
                format!("{} ", timeline_time(beat.at))
            } else {
                "      ".to_string()
            };
            let label = if index == 0 {
                format!("{:<9}", timeline_label(&beat.activity))
            } else {
                "         ".to_string()
            };
            Line::from(vec![
                Span::styled(prefix, Style::default().fg(MUTED)),
                Span::styled(label, Style::default().fg(color)),
                Span::styled(
                    detail,
                    Style::default().fg(if matches!(beat.activity, Activity::Thinking) {
                        MUTED
                    } else {
                        INK
                    }),
                ),
            ])
            .style(Style::default().bg(background))
        })
        .collect::<Vec<_>>();
    let outcome = timeline_outcome(beat.outcome);
    if !outcome.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("               {outcome}"),
            Style::default().fg(color),
        )));
    }
    lines.push(Line::from(""));
    lines
}

pub(crate) fn draw(
    frame: &mut Frame,
    office: Option<&Office>,
    worker: Option<&Worker>,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    now: Millis,
    scroll: &mut usize,
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
        Rect::new(
            footer.x,
            footer.y + footer.height.saturating_sub(1),
            footer.width,
            footer.height.min(1),
        ),
        "↑↓ scroll  ←→ desks  p phone  Esc floor  q quit · read-only",
    );
    if footer.height > 1 {
        Paragraph::new(Line::from(vec![
            Span::styled(" THINKING ", Style::default().fg(MUTED)),
            Span::styled(" RAN / READ ", Style::default().fg(ACCENT)),
            Span::styled(" EDITED ", Style::default().fg(GOOD)),
            Span::styled(" SAID ", Style::default().fg(Color::Rgb(232, 131, 74))),
            Span::styled(" ASKED ", Style::default().fg(super::WARNING)),
        ]))
        .style(Style::default().bg(PANEL))
        .render(
            Rect::new(footer.x, footer.y, footer.width, 1),
            frame.buffer_mut(),
        );
    }
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
        canvas.fill(PANEL);
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
        let metadata = vec![
            Line::from(vec![
                Span::styled(
                    safe_display(&worker.name).to_uppercase(),
                    Style::default().fg(INK).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", worker.agent.label()),
                    Style::default().fg(ACCENT),
                ),
                Span::styled(
                    format!("  {}", status.label().to_uppercase()),
                    status_style(status),
                ),
            ]),
            Line::from(Span::styled(
                format!(
                    "branch {branch} · thread {}",
                    short_path(
                        &worker.id.0,
                        usize::from(info.width).saturating_sub(branch.len().saturating_add(18))
                    )
                ),
                Style::default().fg(MUTED),
            )),
            Line::from(Span::styled(
                format!("{} tokens", human_tokens(worker.tokens_used)),
                Style::default().fg(MUTED),
            )),
        ];
        Paragraph::new(metadata)
            .style(Style::default().bg(BACKGROUND))
            .render(
                Rect::new(info.x, info.y, info.width, info.height.min(3)),
                frame.buffer_mut(),
            );
        if info.height > 4 {
            let notice = Rect::new(
                info.x,
                info.y + 4,
                info.width,
                info.height.saturating_sub(4).min(5),
            );
            let background = if status.needs_attention() {
                ATTENTION_PANEL
            } else {
                PANEL
            };
            let accent = if status.needs_attention() {
                super::WARNING
            } else {
                ACCENT
            };
            paint_opaque(frame, notice, Style::default().bg(background));
            let label = match status {
                theywork_core::WorkerStatus::Blocked => " WAITING ON YOU",
                theywork_core::WorkerStatus::Failed => " NEEDS ATTENTION",
                _ => " CURRENT WORK",
            };
            Paragraph::new(label)
                .style(
                    Style::default()
                        .fg(accent)
                        .bg(background)
                        .add_modifier(Modifier::BOLD),
                )
                .render(
                    Rect::new(notice.x, notice.y, notice.width, 1),
                    frame.buffer_mut(),
                );
            if notice.height >= 2 {
                let instruction = if status == theywork_core::WorkerStatus::Blocked {
                    " Review in the original thread; this view is read-only."
                } else {
                    " Latest activity"
                };
                Paragraph::new(instruction)
                    .style(Style::default().fg(INK).bg(background))
                    .render(
                        Rect::new(notice.x, notice.y + 1, notice.width, 1),
                        frame.buffer_mut(),
                    );
            }
            if notice.height >= 3 && notice.width > 2 {
                let detail = Rect::new(
                    notice.x + 1,
                    notice.y + 2,
                    notice.width - 2,
                    notice.height - 2,
                );
                paint_opaque(frame, detail, Style::default().bg(BACKGROUND));
                Paragraph::new(safe_display(
                    worker.activity.detail().unwrap_or("No detail available"),
                ))
                .style(Style::default().fg(accent).bg(BACKGROUND))
                .wrap(Wrap { trim: false })
                .render(detail, frame.buffer_mut());
            }
            for y in notice.y..notice.y + notice.height {
                Paragraph::new("▌")
                    .style(Style::default().fg(accent).bg(background))
                    .render(Rect::new(notice.x, y, 1, 1), frame.buffer_mut());
            }
        }
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
        let mut lines = worker
            .history
            .iter()
            .flat_map(|beat| timeline_lines(beat, thread_inner.width as usize))
            .collect::<Vec<_>>();
        if lines.is_empty() && lines.len() < available {
            let current = Beat {
                at: worker.last_seen,
                activity: worker.activity.clone(),
                outcome: None,
            };
            lines.extend(timeline_lines(&current, thread_inner.width as usize));
        }
        let max_scroll = lines.len().saturating_sub(available);
        *scroll = (*scroll).min(max_scroll);
        let start = max_scroll.saturating_sub(*scroll);
        paint_opaque(frame, thread_inner, Style::default().bg(BACKGROUND));
        for (index, line) in lines.iter().skip(start).take(available).enumerate() {
            if line.style.bg == Some(ATTENTION_PANEL) {
                paint_opaque(
                    frame,
                    Rect::new(
                        thread_inner.x,
                        thread_inner.y + index as u16,
                        thread_inner.width,
                        1,
                    ),
                    Style::default().bg(ATTENTION_PANEL),
                );
            }
        }
        Paragraph::new(Text::from(lines))
            .scroll((start.min(u16::MAX as usize) as u16, 0))
            .render(thread_inner, frame.buffer_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_wraps_complete_detail_and_retains_outcome() {
        let detail = "a long command with a distinguishing suffix";
        let beat = Beat {
            at: 60_000,
            activity: Activity::Typing {
                detail: detail.into(),
            },
            outcome: Some(Outcome::Exited(0)),
        };
        let lines = timeline_lines(&beat, 27);
        let recovered: String = lines
            .iter()
            .filter_map(|line| line.spans.get(2))
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(recovered, detail);
        assert!(lines.len() > 3);
        assert!(lines
            .iter()
            .flat_map(|line| &line.spans)
            .any(|span| span.content.contains("exit 0")));
    }
}
