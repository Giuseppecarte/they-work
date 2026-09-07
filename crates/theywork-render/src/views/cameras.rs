//! The software tower: a floor directory and readable, paginated office feeds.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use theywork_core::{Millis, Office, WorkerStatus, World};

use crate::canvas::Canvas;
use crate::sprite::SpriteSet;

use super::{
    below_tab_bar, draw_footer, draw_header, draw_tiny, grid_rect, has_area, inset, paint_opaque,
    paint_scanlines, short_path, status_color, status_marker, status_style, timestamp,
    worker_status, ACCENT, BACKGROUND, HOT, INK, MUTED, PANEL, PANEL_HIGHLIGHT, WARNING,
};

/// The dimensions of a camera grid after taking count and terminal space into account.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GridLayout {
    pub columns: usize,
    pub rows: usize,
}

/// Pick a grid that fits the available space while retaining the broad CCTV-wall shape.
pub fn grid_layout(office_count: usize, width: u16, height: u16) -> GridLayout {
    if office_count == 0 || width == 0 || height == 0 {
        return GridLayout::default();
    }

    let max_rows = (height as usize / 10).max(1);
    let balanced_columns = if max_rows >= 2 && office_count >= 4 {
        office_count.div_ceil(2)
    } else {
        office_count
    };
    let max_columns = (width as usize / 26)
        .max(1)
        .min(office_count)
        .min(balanced_columns);
    let mut best = GridLayout {
        columns: 1,
        rows: office_count,
    };
    let mut best_score = u64::MAX;

    for columns in 1..=max_columns {
        let rows = office_count.div_ceil(columns);
        let aspect_error = (columns as i64 * 10 - rows as i64 * 26).unsigned_abs();
        let overflow = rows.saturating_sub(max_rows) as u64;
        let empty = (columns * rows).saturating_sub(office_count) as u64;
        // A fit beats an attractive shape that would make a tile too short;
        // after that, prefer the 26x10 camera aspect and fewer empty cells.
        let score = overflow * 1_000_000 + aspect_error * 10 + empty;
        if score < best_score {
            best_score = score;
            best = GridLayout { columns, rows };
        }
    }
    best
}

/// Keep floor numbers stable when a worker changes status. The world orders
/// projects by their canonical path; attention is exposed separately.
pub(crate) fn ordered_offices(world: &World, _now: Millis) -> Vec<&Office> {
    world.offices().collect()
}

#[derive(Default)]
struct StatusCounts {
    running: usize,
    idle: usize,
    blocked: usize,
    failed: usize,
}

impl StatusCounts {
    fn add_office(&mut self, office: &Office, now: Millis) {
        for worker in &office.workers {
            match worker_status(worker, now) {
                WorkerStatus::Running => self.running += 1,
                WorkerStatus::Idle => self.idle += 1,
                WorkerStatus::Blocked => self.blocked += 1,
                WorkerStatus::Failed => self.failed += 1,
            }
        }
    }

    fn label(&self) -> String {
        format!(
            "{} working · {} idle · {} waiting · {} failed",
            self.running, self.idle, self.blocked, self.failed
        )
    }
}

pub(crate) fn draw(
    frame: &mut Frame,
    world: &World,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    now: Millis,
    selected: usize,
    all_selected: bool,
) -> GridLayout {
    let area = below_tab_bar(frame.area());
    if area.width < 16 || area.height < 6 {
        draw_tiny(frame, "they-work • terminal too small for the camera wall");
        return GridLayout::default();
    }

    let offices = ordered_offices(world, now);
    let mut counts = StatusCounts::default();
    for office in &offices {
        counts.add_office(office, now);
    }
    let (header, body, footer) = super::vertical_bands(area, 2, 2);
    draw_header(
        frame,
        Rect::new(header.x, header.y, header.width, 1),
        "SOFTWARE TOWER",
        &format!(
            "{} floors · {} workers",
            offices.len(),
            world.worker_count()
        ),
    );
    if header.height > 1 {
        Paragraph::new(format!("  {}", counts.label()))
            .style(Style::default().fg(INK).bg(BACKGROUND))
            .render(
                Rect::new(header.x, header.y + 1, header.width, 1),
                frame.buffer_mut(),
            );
    }
    draw_footer(
        frame,
        footer,
        "arrows move · Enter floor · ! attention · c sources · ? help · q quit",
    );

    if !has_area(body) {
        return GridLayout::default();
    }
    if offices.is_empty() {
        Paragraph::new("No conversations found.\n\nPress c to choose local sources, then start a conversation in a project.\nEach project becomes a floor; each conversation becomes a worker.")
            .style(Style::default().fg(MUTED).bg(BACKGROUND))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .render(body, frame.buffer_mut());
        return GridLayout::default();
    }

    let feeds = if body.width >= 110 && offices.len() > 1 {
        let directory = Rect::new(body.x, body.y, 30, body.height);
        draw_directory(frame, directory, &offices, selected, now);
        Rect::new(body.x + 31, body.y, body.width - 31, body.height)
    } else {
        body
    };
    let capacity = (usize::from(feeds.width) / 26).max(1) * (usize::from(feeds.height) / 10).max(1);
    let layout = grid_layout(offices.len().min(capacity), feeds.width, feeds.height);
    let page_size = layout.columns.saturating_mul(layout.rows).max(1);
    let first = selected.min(offices.len() - 1) / page_size * page_size;
    for (index, office) in offices.iter().enumerate().skip(first).take(page_size) {
        let tile = grid_rect(feeds, index - first, layout.columns, layout.rows);
        if has_area(tile) {
            draw_tile(
                frame,
                canvas,
                sprites,
                office,
                tile,
                now,
                index == selected,
                index + 1,
            );
        }
    }
    if footer.height > 1 {
        let project = offices[selected.min(offices.len() - 1)];
        let detail = format!(
            "  Floor {}/{} · page {}/{} · PgUp/PgDn · {}",
            selected + 1,
            offices.len(),
            first / page_size + 1,
            offices.len().div_ceil(page_size),
            project.path
        );
        Paragraph::new(super::short_path(&detail, usize::from(footer.width)))
            .style(Style::default().fg(MUTED).bg(BACKGROUND))
            .render(
                Rect::new(footer.x, footer.y + 1, footer.width, 1),
                frame.buffer_mut(),
            );
    }
    let _ = all_selected;
    layout
}

fn draw_directory(
    frame: &mut Frame,
    area: Rect,
    offices: &[&Office],
    selected: usize,
    now: Millis,
) {
    let inner = super::draw_panel(frame, area, "FLOORS / PROJECTS", false);
    if !has_area(inner) {
        return;
    }
    let visible = usize::from(inner.height).max(1);
    let first = selected
        .saturating_sub(visible / 2)
        .min(offices.len().saturating_sub(visible));
    for (index, office) in offices.iter().enumerate().skip(first).take(visible) {
        let marker = if super::office_dot_color(office, now) == WARNING {
            "!"
        } else if super::office_dot_color(office, now) == HOT {
            "×"
        } else {
            "·"
        };
        let style = Style::default()
            .fg(if index == selected { INK } else { MUTED })
            .bg(if index == selected {
                PANEL_HIGHLIGHT
            } else {
                PANEL
            });
        let line = Line::from(vec![
            Span::styled(
                format!(
                    "{}{:>2}",
                    if index == selected { ">" } else { " " },
                    index + 1
                ),
                style,
            ),
            Span::styled(marker, style.fg(super::office_dot_color(office, now))),
            Span::styled(
                format!(
                    " {}",
                    short_path(&office.name, usize::from(inner.width.saturating_sub(5)))
                ),
                style,
            ),
        ]);
        Paragraph::new(line).style(style).render(
            Rect::new(inner.x, inner.y + (index - first) as u16, inner.width, 1),
            frame.buffer_mut(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_tile(
    frame: &mut Frame,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    office: &theywork_core::Office,
    tile: Rect,
    now: Millis,
    selected: bool,
    floor_number: usize,
) {
    let blocked_count = office
        .workers
        .iter()
        .filter(|worker| worker_status(worker, now) == WorkerStatus::Blocked)
        .count();
    let failed_count = office
        .workers
        .iter()
        .filter(|worker| worker_status(worker, now) == WorkerStatus::Failed)
        .count();
    let border_color = if blocked_count > 0 {
        WARNING
    } else if failed_count > 0 {
        HOT
    } else if selected {
        ACCENT
    } else {
        MUTED
    };
    let border_style = Style::default().fg(border_color).bg(BACKGROUND);
    let title_prefix = format!("{}F{} ", if selected { "> " } else { "" }, floor_number);
    let title_width = tile.width.saturating_sub(4) as usize;
    let office_title = short_path(
        &office.name,
        title_width.saturating_sub(title_prefix.chars().count()),
    );
    Block::default()
        .title(format!(" {}{} ", title_prefix, office_title))
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(BACKGROUND))
        .render(tile, frame.buffer_mut());

    let inner = inset(tile, 1);
    if !has_area(inner) {
        return;
    }

    canvas.resize_for_cells(inner.width as usize, inner.height as usize);
    let markers = super::guard_scene::draw(canvas, office, sprites, now);
    canvas.render(frame.buffer_mut(), inner);
    paint_scanlines(frame.buffer_mut(), inner, now);

    for (index, worker) in office.workers.iter().enumerate() {
        let status = worker_status(worker, now);
        let Some(marker) = status_marker(status) else {
            continue;
        };
        let Some(&(marker_cell_x, marker_cell_y)) = markers.get(index) else {
            continue;
        };
        let marker_x =
            inner.x + marker_cell_x.clamp(0, inner.width.saturating_sub(1) as i32) as u16;
        let marker_y =
            inner.y + marker_cell_y.clamp(0, inner.height.saturating_sub(1) as i32) as u16;
        let marker_area = Rect::new(marker_x, marker_y, 1, 1);
        let marker_style = status_style(status).bg(BACKGROUND);
        paint_opaque(frame, marker_area, marker_style);
        Paragraph::new(marker)
            .style(marker_style)
            .render(marker_area, frame.buffer_mut());
    }

    let summary_status = if blocked_count > 0 {
        WorkerStatus::Blocked
    } else if failed_count > 0 {
        WorkerStatus::Failed
    } else if office
        .workers
        .iter()
        .any(|worker| worker_status(worker, now) == WorkerStatus::Running)
    {
        WorkerStatus::Running
    } else {
        WorkerStatus::Idle
    };
    let running = office
        .workers
        .iter()
        .filter(|worker| worker_status(worker, now) == WorkerStatus::Running)
        .count();
    let idle = office
        .workers
        .iter()
        .filter(|worker| worker_status(worker, now) == WorkerStatus::Idle)
        .count();
    let status = if blocked_count > 0 {
        format!("! {blocked_count} WAITING · {running} working")
    } else if failed_count > 0 {
        format!("× {failed_count} FAILED · {running} working")
    } else {
        format!("{running} working · {idle} idle")
    };

    let status_width = if inner.width >= 42 && inner.height >= 2 {
        inner.width.saturating_sub(8)
    } else {
        inner.width
    };
    let status_area = Rect::new(inner.x, inner.y, status_width, inner.height.min(1));
    let status_text_style = Style::default()
        .fg(status_color(summary_status))
        .bg(BACKGROUND);
    paint_opaque(frame, status_area, status_text_style);
    Paragraph::new(status)
        .style(status_text_style)
        .render(status_area, frame.buffer_mut());
    if inner.width >= 42 && inner.height >= 2 {
        let rec = if now.div_euclid(500) % 2 == 0 {
            "● REC"
        } else {
            "○ REC"
        };
        let rec_area = Rect::new(
            inner.x + inner.width.saturating_sub(7),
            inner.y,
            7.min(inner.width),
            1,
        );
        let rec_style = Style::default()
            .fg(status_color(summary_status))
            .bg(BACKGROUND);
        paint_opaque(frame, rec_area, rec_style);
        Paragraph::new(Line::from(vec![
            Span::styled(rec, rec_style),
            Span::styled(" ", rec_style),
        ]))
        .style(rec_style)
        .render(rec_area, frame.buffer_mut());
    }
    if inner.width >= 8 && inner.height >= 2 {
        let time_area = Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(1),
            inner.width,
            1,
        );
        let time_style = Style::default().fg(MUTED).bg(BACKGROUND);
        paint_opaque(frame, time_area, time_style);
        Paragraph::new(timestamp(now))
            .style(time_style)
            .render(time_area, frame.buffer_mut());
    }
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
            agent: Agent::Codex,
            kind,
        }
    }

    fn status_world() -> World {
        let mut world = World::new();
        world.apply(event(
            "/a-failed",
            "failed",
            0,
            EventKind::Acted(Activity::Error {
                detail: "compiler stopped".into(),
            }),
        ));
        world.apply(event(
            "/b-plain",
            "plain",
            0,
            EventKind::Seen {
                name: "Plain worker".into(),
                git_branch: None,
            },
        ));
        world.apply(event(
            "/y-blocked",
            "blocked",
            0,
            EventKind::Turn { in_flight: true },
        ));
        world.apply(event(
            "/y-blocked",
            "blocked",
            0,
            EventKind::Acted(Activity::Waiting {
                detail: "approve deploy".into(),
            }),
        ));
        world.apply(event(
            "/z-blocked-later",
            "blocked-later",
            0,
            EventKind::Turn { in_flight: true },
        ));
        world.apply(event(
            "/z-blocked-later",
            "blocked-later",
            0,
            EventKind::Acted(Activity::Typing {
                detail: "cargo test".into(),
            }),
        ));
        world
    }
    #[test]
    fn floor_numbers_do_not_change_when_a_worker_needs_attention() {
        let world = status_world();
        let offices = ordered_offices(&world, BLOCKED_AFTER_MS + 1);
        let names = offices
            .iter()
            .map(|office| office.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            ["a-failed", "b-plain", "y-blocked", "z-blocked-later"]
        );
        assert_eq!(
            worker_status(&offices[2].workers[0], BLOCKED_AFTER_MS + 1),
            WorkerStatus::Blocked
        );
        assert_eq!(
            worker_status(&offices[0].workers[0], BLOCKED_AFTER_MS + 1),
            WorkerStatus::Failed
        );
    }

    #[test]
    fn wide_guard_office_keeps_feeds_in_balanced_rows() {
        assert_eq!(
            grid_layout(6, 160, 44),
            GridLayout {
                columns: 3,
                rows: 2,
            }
        );
        assert_eq!(
            grid_layout(4, 160, 44),
            GridLayout {
                columns: 2,
                rows: 2,
            }
        );
    }
}
