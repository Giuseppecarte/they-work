//! Detailed employee view: a large sprite and the useful live context.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::Frame;
use theywork_core::{Activity, Beat, Millis, Office, Outcome, Worker, WorkerStatus};

use crate::canvas::Canvas;
use crate::sprite::{look_for_worker, SpriteSet};

use super::{
    below_tab_bar, draw_footer, draw_header, draw_panel, draw_tiny, duration_label, elapsed_ms,
    has_area, human_tokens, paint_opaque, render_worker_with_look, safe_display, short_path,
    status_style, worker_status, PixelRect, ACCENT, ATTENTION_PANEL, BACKGROUND, GOOD, INK, MUTED,
    PANEL,
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
    // Historical requests are evidence, not an outstanding approval badge.
    let background = BACKGROUND;
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

/// A current observation, kept separate from historical events in both views.
pub(super) struct InspectionSummary {
    pub label: &'static str,
    pub detail: String,
    pub next_step: &'static str,
}

pub(super) fn inspection_summary(worker: &Worker, now: Millis) -> InspectionSummary {
    let detail = worker
        .activity
        .detail()
        .filter(|value| !value.trim().is_empty());
    if worker.coverage.observed_at > 0
        && (!worker.coverage.available || worker.coverage.is_stale_at(now))
    {
        return InspectionSummary {
            label: "SOURCE UNAVAILABLE",
            detail: safe_display(&worker.coverage.detail),
            next_step: "Current state is unknown. Check Connections or the original client.",
        };
    }
    if let Some(reason) = worker.wait_reason.filter(|reason| {
        matches!(
            reason,
            theywork_core::WaitReason::AutomaticReview
                | theywork_core::WaitReason::Child
                | theywork_core::WaitReason::Process
        )
    }) {
        return InspectionSummary { label:match reason { theywork_core::WaitReason::AutomaticReview=>"AUTOMATIC REVIEW",theywork_core::WaitReason::Child=>"WAITING FOR TEAM",_=>"WAITING FOR PROCESS" }, detail:safe_display(detail.unwrap_or("The provider recorded this wait.")),next_step:"No human response is requested by this event. g shows available team relationships." };
    }
    match worker_status(worker, now) {
        WorkerStatus::Blocked if matches!(worker.activity, Activity::Waiting { .. }) => {
            InspectionSummary {
                label: "WAITING ON YOU",
                detail: safe_display(
                    detail.unwrap_or("The source recorded a request without details."),
                ),
                next_step: "m opens available task controls; external requests stay in their original client.",
            }
        }
        WorkerStatus::Blocked => InspectionSummary {
            label: "NEEDS ATTENTION · SILENT",
            detail: "No recent activity. No approval was identified.".into(),
            next_step: "Check the original conversation for progress or a problem.",
        },
        WorkerStatus::Failed => InspectionSummary {
            label: "NEEDS ATTENTION · ERROR",
            detail: safe_display(detail.unwrap_or("The source reported an error without details.")),
            next_step: "Review the error in the original conversation.",
        },
        WorkerStatus::Idle => InspectionSummary {
            label: "IDLE · LAST UPDATE",
            detail: safe_display(detail.unwrap_or("No task is currently running.")),
            next_step: "Continue in the original conversation when you are ready.",
        },
        WorkerStatus::Running => InspectionSummary {
            label: "WORKING · LATEST ACTIVITY",
            detail: detail
                .map(safe_display)
                .unwrap_or_else(|| worker.activity.label().into()),
            next_step: "Follow the recorded activity below.",
        },
    }
}

// Keep every character available when text wraps, including command whitespace.
pub(super) fn wrapped_lines(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut used = 0;
    for character in safe_display(text).chars() {
        let size = Span::raw(character.to_string()).width();
        if used + size > width.max(1) && used > 0 {
            let current = lines.last_mut().expect("one line");
            let tail = current
                .rfind(' ')
                .filter(|index| *index > 0)
                .map(|index| current.split_off(index + 1))
                .unwrap_or_default();
            used = Span::raw(tail.clone()).width();
            lines.push(tail);
        }
        lines.last_mut().expect("one line").push(character);
        used += size;
    }
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
    if area.width < 24 || area.height < 12 {
        draw_tiny(
            frame,
            "Desk needs 24 columns × 13 rows. Esc returns to the floor.",
        );
        return;
    }
    let (Some(office), Some(worker)) = (office, worker) else {
        draw_tiny(frame, "No desk selected.");
        return;
    };
    let status = worker_status(worker, now);
    let summary = inspection_summary(worker, now);
    let (header, mut body, footer) = super::vertical_bands(area, 2, 2);
    let content_width = body.width.min(124);
    body.x += body.width.saturating_sub(content_width) / 2;
    body.width = content_width;
    draw_header(
        frame,
        header,
        &format!(
            "DESK / {}",
            short_path(&office.name, area.width.saturating_sub(10) as usize)
        ),
        &format!(
            "{} conversation · observed {} ago · recorded trail",
            worker.agent.label(),
            duration_label(elapsed_ms(now, worker.last_seen))
        ),
    );
    draw_footer(
        frame,
        Rect::new(footer.x, footer.bottom().saturating_sub(1), footer.width, 1),
        "↑↓ history  PgUp/PgDn page  ←→ desks  / find  p phone  Esc floor",
    );
    if footer.height > 1 {
        let (persona, _) = sprites.persona_label(worker);
        Paragraph::new(format!(
            " {persona} · w character   {} tokens · branch {}",
            human_tokens(worker.tokens_used),
            safe_display(worker.git_branch.as_deref().unwrap_or("none"))
        ))
        .style(Style::default().fg(MUTED).bg(PANEL))
        .render(
            Rect::new(footer.x, footer.y, footer.width, 1),
            frame.buffer_mut(),
        );
    }
    if !has_area(body) {
        return;
    }
    paint_opaque(frame, body, Style::default().bg(BACKGROUND));
    let avatar_width = if body.width >= 60 { 11 } else { 0 };
    let gap = if avatar_width > 0 { 2 } else { 0 };
    let info_width = body.width.saturating_sub(avatar_width + gap);
    let title_lines = wrapped_lines(&worker.name, info_width as usize);
    let title_height = title_lines.len().min(3) as u16;
    // The request has priority over decorative metadata; leave a usable log.
    let profile_height = (title_height + 7).min(body.height.saturating_sub(5)).max(1);
    let profile = Rect::new(body.x, body.y, body.width, profile_height);
    if avatar_width > 0 {
        let avatar = Rect::new(profile.x, profile.y, avatar_width, profile.height.min(9));
        canvas.resize_for_cells(avatar.width as usize, avatar.height as usize);
        canvas.fill(PANEL);
        let look = look_for_worker(&office.workers, worker);
        render_worker_with_look(
            canvas,
            sprites,
            worker,
            &look,
            now,
            PixelRect {
                x: 0,
                y: 0,
                width: canvas.width(),
                height: canvas.height(),
            },
        );
        canvas.render(frame.buffer_mut(), avatar);
    }
    let info = Rect::new(
        profile.x + avatar_width + gap,
        profile.y,
        info_width,
        profile.height,
    );
    let title_height = title_height.min(info.height);
    let mut title = title_lines
        .iter()
        .take(title_height as usize)
        .cloned()
        .collect::<Vec<_>>();
    if title_lines.len() > title.len() {
        if let Some(last) = title.last_mut() {
            *last = short_path(&format!("{last}…"), info.width as usize);
        }
    }
    Paragraph::new(title.into_iter().map(Line::from).collect::<Vec<_>>())
        .style(Style::default().fg(INK).add_modifier(Modifier::BOLD))
        .render(
            Rect::new(info.x, info.y, info.width, title_height),
            frame.buffer_mut(),
        );
    let notice = Rect::new(
        info.x,
        info.y + title_height,
        info.width,
        info.height.saturating_sub(title_height + 1),
    );
    if has_area(notice) {
        let background = if status.needs_attention() {
            ATTENTION_PANEL
        } else {
            PANEL
        };
        let accent = if status == WorkerStatus::Failed {
            super::HOT
        } else if status.needs_attention() {
            super::WARNING
        } else {
            ACCENT
        };
        paint_opaque(frame, notice, Style::default().bg(background));
        Paragraph::new(format!(" {}", summary.label))
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
        let detail_height = notice
            .height
            .saturating_sub(3)
            .max(1)
            .min(notice.height.saturating_sub(1));
        let detail = Rect::new(
            notice.x + 1,
            notice.y + 1,
            notice.width.saturating_sub(2),
            detail_height,
        );
        let details = wrapped_lines(&summary.detail, detail.width as usize);
        let mut preview = details
            .iter()
            .take(detail.height as usize)
            .cloned()
            .collect::<Vec<_>>();
        if details.len() > preview.len() {
            if let Some(last) = preview.last_mut() {
                *last = short_path(&format!("{last}…"), detail.width as usize);
            }
        }
        Paragraph::new(preview.into_iter().map(Line::from).collect::<Vec<_>>())
            .style(Style::default().fg(INK).bg(background))
            .render(detail, frame.buffer_mut());
        let action = Rect::new(
            notice.x + 1,
            detail.bottom(),
            notice.width.saturating_sub(2),
            notice.bottom().saturating_sub(detail.bottom()),
        );
        Paragraph::new(summary.next_step)
            .style(Style::default().fg(accent).bg(background))
            .wrap(Wrap { trim: false })
            .render(action, frame.buffer_mut());
    }
    let thread = Rect::new(
        body.x,
        body.y + profile_height,
        body.width,
        body.height.saturating_sub(profile_height),
    );
    // Context is scrollable too: very long titles and IDs remain recoverable.
    let width = thread.width.saturating_sub(2) as usize;
    let mut lines = Vec::new();
    for (label, value) in [
        ("Conversation", worker.name.as_str()),
        ("Thread", worker.id.0.as_str()),
        ("Branch", worker.git_branch.as_deref().unwrap_or("none")),
    ] {
        lines.extend(
            wrapped_lines(&format!("{label}: {value}"), width)
                .into_iter()
                .map(|line| Line::styled(line, Style::default().fg(MUTED))),
        );
    }
    lines.push(Line::from(""));
    if worker.history.is_empty() {
        lines.push(Line::styled(
            "No recorded history for this conversation.",
            Style::default().fg(MUTED),
        ));
        lines.push(Line::from(""));
    }
    let mut day = None;
    for beat in &worker.history {
        let recorded_day = beat.at.div_euclid(86_400_000);
        if day != Some(recorded_day) {
            let days_ago = now.div_euclid(86_400_000).saturating_sub(recorded_day);
            let label = match days_ago {
                0 => "Today (UTC)".into(),
                1 => "Yesterday (UTC)".into(),
                value if value > 1 => format!("{value} days ago (UTC)"),
                _ => "Future timestamp (UTC)".into(),
            };
            lines.push(Line::styled(label, Style::default().fg(MUTED)));
            day = Some(recorded_day);
        }
        lines.extend(timeline_lines(beat, width));
    }
    // Current state is distinct from the recorded timeline. An Acted event may
    // carry the current request without ever producing a historical Beat.
    lines.push(Line::styled(
        format!("NOW · {}", summary.label),
        status_style(status).add_modifier(Modifier::BOLD),
    ));
    lines.extend(
        wrapped_lines(&summary.detail, width)
            .into_iter()
            .map(|line| Line::styled(line, Style::default().fg(INK))),
    );
    let available = thread.height.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(available);
    *scroll = (*scroll).min(max_scroll);
    let start = max_scroll.saturating_sub(*scroll);
    let range = format!(
        "HISTORY · UTC · {}–{}/{}{}",
        start + 1,
        (start + available).min(lines.len()),
        lines.len(),
        if *scroll == 0 {
            " · latest"
        } else {
            " · End latest"
        }
    );
    let thread_inner = draw_panel(frame, thread, &range, false);
    if has_area(thread_inner) {
        paint_opaque(frame, thread_inner, Style::default().bg(BACKGROUND));
        Paragraph::new(Text::from(lines))
            .style(Style::default().fg(INK).bg(BACKGROUND))
            .scroll((start.min(u16::MAX as usize) as u16, 0))
            .render(thread_inner, frame.buffer_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_work_is_attention_without_an_invented_approval_request() {
        use ratatui::{backend::TestBackend, Terminal};
        use theywork_core::{Agent, OfficeId, WorkerId, BLOCKED_AFTER_MS};
        let office = Office::new(OfficeId("/quiet".into()), "Quiet".into());
        let mut worker = Worker::new(
            WorkerId("quiet-worker".into()),
            office.id.clone(),
            Agent::Codex,
            "Quiet worker".into(),
            0,
        );
        worker.activity = Activity::Typing {
            detail: "an earlier command".into(),
        };
        worker.turn_in_flight = true;
        let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
        let mut canvas = Canvas::new(0, 0);
        let mut scroll = 0;
        terminal
            .draw(|frame| {
                draw(
                    frame,
                    Some(&office),
                    Some(&worker),
                    &mut canvas,
                    &SpriteSet::new(),
                    BLOCKED_AFTER_MS + 1,
                    &mut scroll,
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
        assert!(text.contains("NEEDS ATTENTION"));
        assert!(text.contains("No approval was identified."));
        assert!(!text.contains("WAITING ON YOU"));
        let profile = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .take(120 * 12)
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(!profile.contains("an earlier command"));
    }

    #[test]
    fn current_request_remains_complete_after_recorded_work() {
        use ratatui::{backend::TestBackend, Terminal};
        use theywork_core::{Agent, OfficeId, WorkerId};
        let office = Office::new(OfficeId("/project".into()), "Project".into());
        let mut worker = Worker::new(
            WorkerId("thread".into()),
            office.id.clone(),
            Agent::Codex,
            "Review the release plan for database migration and production cutover".into(),
            100,
        );
        worker.history.push_back(Beat {
            at: 1,
            activity: Activity::Typing {
                detail: "cargo test".into(),
            },
            outcome: Some(Outcome::Exited(0)),
        });
        let request = "Approve the migration only after verifying the backup exists and confirming the exact production account: project-production-eu-west.";
        worker.activity = Activity::Waiting {
            detail: request.into(),
        };
        worker.turn_in_flight = true;
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut canvas = Canvas::new(0, 0);
        let mut scroll = 0;
        terminal
            .draw(|frame| {
                draw(
                    frame,
                    Some(&office),
                    Some(&worker),
                    &mut canvas,
                    &SpriteSet::new(),
                    100,
                    &mut scroll,
                )
            })
            .unwrap();
        let rows = terminal
            .backend()
            .buffer()
            .content
            .chunks(80)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>();
        let text = rows.join("\n");
        assert!(text.contains("WAITING ON YOU"));
        assert!(
            text.contains("cutover"),
            "title must retain its distinguishing suffix"
        );
        assert!(
            text.contains("project-production-eu-west."),
            "full current request must be reachable at the latest position"
        );
        assert!(text.contains("HISTORY · UTC"));
    }

    #[test]
    fn historical_request_is_not_styled_as_a_current_alert() {
        let beat = Beat {
            at: 0,
            activity: Activity::Waiting {
                detail: "old approval".into(),
            },
            outcome: None,
        };
        assert!(timeline_lines(&beat, 80)
            .iter()
            .all(|line| line.style.bg != Some(ATTENTION_PANEL)));
    }

    #[test]
    fn wrapped_context_preserves_title_and_unicode_at_narrow_widths() {
        let title = "Review producción café 東京 with the final distinguishing suffix";
        for width in [1, 8, 25, 60] {
            let lines = wrapped_lines(title, width);
            assert_eq!(lines.concat(), title);
            assert!(lines
                .iter()
                .all(|line| Span::raw(line).width() <= width.max(2)));
        }
    }

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
