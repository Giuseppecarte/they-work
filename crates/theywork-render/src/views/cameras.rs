//! Security-camera grid: the building-wide headline view.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use theywork_core::{Millis, Office, WorkerStatus, World};

use super::office::{draw_room_scene, worker_marker_position, RoomScale};
use crate::canvas::Canvas;
use crate::sprite::{worker_looks, SpriteSet};

use super::{
    below_tab_bar, draw_footer, draw_header, draw_tiny, grid_rect, has_area, inset, paint_opaque,
    paint_scanlines, short_path, status_color, status_marker, status_style, timestamp,
    worker_status, ACCENT, BACKGROUND, HOT, MUTED, WARNING,
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

    let max_columns = (width as usize / 26).max(1).min(office_count);
    let max_rows = (height as usize / 10).max(1);
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

/// Return offices in attention-first order while preserving the world's stable
/// order within each attention tier.
pub(crate) fn ordered_offices(world: &World, now: Millis) -> Vec<&Office> {
    let mut indexed = world.offices().enumerate().collect::<Vec<_>>();
    indexed.sort_by_key(|(index, office)| (office_attention_rank(office, now), *index));
    indexed.into_iter().map(|(_, office)| office).collect()
}

fn office_attention_rank(office: &Office, now: Millis) -> u8 {
    if office
        .workers
        .iter()
        .any(|worker| worker_status(worker, now) == WorkerStatus::Blocked)
    {
        0
    } else if office
        .workers
        .iter()
        .any(|worker| worker_status(worker, now) == WorkerStatus::Failed)
    {
        1
    } else {
        2
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

    let (header, body, footer) = super::vertical_bands(area, 2, 2);
    draw_header(
        frame,
        header,
        if all_selected {
            "GUARD OFFICE"
        } else {
            "CAMERAS"
        },
        &format!(
            "{} rooms • {} workers • {}",
            world.office_count(),
            world.worker_count(),
            if all_selected {
                "all feeds"
            } else {
                "selected feed"
            }
        ),
    );
    draw_footer(
        frame,
        footer,
        if all_selected {
            "1-9 jump   Tab cycle   Enter open   s settings   q quit"
        } else {
            "←↑↓→ / hjkl move   Enter open   0 guard   s settings   q quit"
        },
    );

    if !has_area(body) {
        return GridLayout::default();
    }
    if world.office_count() == 0 {
        Paragraph::new("No active offices yet — waiting for an agent to arrive.")
            .style(Style::default().fg(MUTED).bg(BACKGROUND))
            .render(body, frame.buffer_mut());
        return GridLayout::default();
    }

    let layout = grid_layout(world.office_count(), body.width, body.height);
    for (index, office) in ordered_offices(world, now).into_iter().enumerate() {
        let tile = grid_rect(body, index, layout.columns, layout.rows);
        if !has_area(tile) {
            continue;
        }
        draw_tile(frame, canvas, sprites, office, tile, now, index == selected);
    }
    layout
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
    let title_prefix = if blocked_count > 0 {
        "! "
    } else if failed_count > 0 {
        "× "
    } else {
        "CAM "
    };
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
    let workers = office.workers.iter().collect::<Vec<_>>();
    let looks = worker_looks(&office.workers);
    let grid = draw_room_scene(
        canvas,
        &office.name,
        &workers,
        &looks,
        sprites,
        now,
        RoomScale::Feed,
    );
    canvas.render(frame.buffer_mut(), inner);
    paint_scanlines(frame.buffer_mut(), inner, now);

    for (index, worker) in office.workers.iter().enumerate() {
        let status = worker_status(worker, now);
        let Some(marker) = status_marker(status) else {
            continue;
        };
        let (marker_px, marker_py) = worker_marker_position(grid, index);
        let marker_x = inner.x + marker_px.clamp(0, inner.width.saturating_sub(1) as i32) as u16;
        let marker_y =
            inner.y + (marker_py / 2).clamp(0, inner.height.saturating_sub(1) as i32) as u16;
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
    let status = if blocked_count > 0 {
        format!(
            "! {} blocked • {} / {} busy",
            blocked_count,
            office.busy_count(),
            office.workers.len()
        )
    } else if failed_count > 0 {
        format!(
            "× {} failed • {} / {} busy",
            failed_count,
            office.busy_count(),
            office.workers.len()
        )
    } else {
        format!(
            "{} / {} busy • {}",
            office.busy_count(),
            office.workers.len(),
            summary_status.label()
        )
    };

    let status_area = Rect::new(inner.x, inner.y, inner.width, inner.height.min(1));
    let status_text_style = Style::default()
        .fg(status_color(summary_status))
        .bg(BACKGROUND);
    paint_opaque(frame, status_area, status_text_style);
    Paragraph::new(status)
        .style(status_text_style)
        .render(status_area, frame.buffer_mut());
    if inner.width >= 9 && inner.height >= 2 {
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
    fn attention_offices_sort_first_and_preserve_order_within_tiers() {
        let world = status_world();
        let offices = ordered_offices(&world, BLOCKED_AFTER_MS + 1);
        let names = offices
            .iter()
            .map(|office| office.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            ["y-blocked", "z-blocked-later", "a-failed", "b-plain"]
        );
        assert_eq!(
            worker_status(&offices[0].workers[0], BLOCKED_AFTER_MS + 1),
            WorkerStatus::Blocked
        );
        assert_eq!(
            worker_status(&offices[2].workers[0], BLOCKED_AFTER_MS + 1),
            WorkerStatus::Failed
        );
    }
}
