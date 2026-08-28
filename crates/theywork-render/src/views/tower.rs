//! Whole-building tower view: stacked offices, sky, lobby, and elevator.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use theywork_core::{Millis, Office, WorkerStatus, World};

use crate::canvas::Canvas;
use crate::sprite::SpriteSet;

use super::{
    draw_footer, draw_header, draw_tiny, fill_office_background, has_area, human_tokens, inset,
    render_worker, short_path, status_color, status_marker, status_style, worker_status, PixelRect,
    ACCENT, FLOOR, INK, PANEL, PANEL_HIGHLIGHT, WALL,
};

const FLOOR_HEIGHT: u16 = 7;
const ROOF_HEIGHT: u16 = 2;
const LOBBY_HEIGHT: u16 = 3;
const SHAFT_WIDTH: u16 = 12;
const MIN_WIDTH: u16 = 20;
const MIN_HEIGHT: u16 = 9;
const ELEVATOR_TRAVEL_MS: u64 = 900;
const ELEVATOR_HOLD_MS: u64 = 700;

/// The tower viewport after accounting for the roof, lobby, and floor height.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TowerLayout {
    pub floor_height: u16,
    pub visible_start: usize,
    pub visible_count: usize,
    pub total_floors: usize,
}

/// Keep a selected floor in the visible tower viewport.
pub fn viewport_start(total_floors: usize, selected: usize, visible_count: usize) -> usize {
    if total_floors <= visible_count || visible_count == 0 {
        return 0;
    }
    let selected = selected.min(total_floors - 1);
    selected
        .saturating_sub(visible_count / 2)
        .min(total_floors - visible_count)
}

/// Return a stable floor order. Reversing the world's stable order puts the
/// latest map entries toward the roof without re-sorting as activity changes.
pub(crate) fn ordered_offices(world: &World) -> Vec<&Office> {
    let mut offices = world.offices().collect::<Vec<_>>();
    offices.reverse();
    offices
}

/// Move the car from the opposite end of the shaft to the selected floor.
pub(crate) fn elevator_position(now: Millis, top: u16, bottom: u16, target: u16) -> u16 {
    let (top, bottom) = (top.min(bottom), top.max(bottom));
    let target = target.clamp(top, bottom);
    if top == bottom {
        return top;
    }

    let midpoint = top + (bottom - top) / 2;
    let origin = if target <= midpoint { bottom } else { top };
    if origin == target {
        return target;
    }

    let cycle_ms = ELEVATOR_TRAVEL_MS + ELEVATOR_HOLD_MS;
    let phase = now.max(0) as u64 % cycle_ms;
    if phase >= ELEVATOR_TRAVEL_MS {
        return target;
    }

    let distance = origin.abs_diff(target) as u64;
    let offset = distance * phase / ELEVATOR_TRAVEL_MS;
    if origin < target {
        origin.saturating_add(offset as u16)
    } else {
        origin.saturating_sub(offset as u16)
    }
}

pub(crate) fn draw(
    frame: &mut Frame,
    world: &World,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    now: Millis,
    selected: usize,
) -> TowerLayout {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_tiny(frame, "they-work • terminal too small for the tower");
        return TowerLayout::default();
    }

    let (header, body, footer) = super::vertical_bands(area, 2, 2);
    draw_header(
        frame,
        header,
        "TOWER",
        &format!(
            "{} floors • {} workers • whole-building view",
            world.office_count(),
            world.worker_count()
        ),
    );
    draw_footer(
        frame,
        footer,
        "↑↓ / jk move floor   Tab cameras   Enter open   p phone   ? help   q quit",
    );
    if !has_area(body) {
        return TowerLayout::default();
    }

    Block::default()
        .style(Style::default().bg(Color::Rgb(32, 49, 83)))
        .render(body, frame.buffer_mut());

    let shaft_width = SHAFT_WIDTH.min(body.width).max(1);
    let shaft = Rect::new(body.x, body.y, shaft_width, body.height);
    let building_x = body.x.saturating_add(shaft_width).saturating_add(1);
    let building_width = body
        .width
        .saturating_sub(shaft_width.saturating_add(2))
        .max(1);
    let building = Rect::new(building_x, body.y, building_width, body.height);

    Block::default()
        .title(" ELEVATOR ")
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(PANEL_HIGHLIGHT))
        .style(Style::default().bg(PANEL))
        .render(shaft, frame.buffer_mut());

    let roof_height = ROOF_HEIGHT.min(building.height);
    let lobby_height = LOBBY_HEIGHT.min(building.height.saturating_sub(roof_height));
    let floor_zone = Rect::new(
        building.x,
        building.y.saturating_add(roof_height),
        building.width,
        building
            .height
            .saturating_sub(roof_height)
            .saturating_sub(lobby_height),
    );
    let offices = ordered_offices(world);
    let total_floors = offices.len();
    let visible_count = if total_floors == 0 || floor_zone.height == 0 {
        0
    } else {
        ((floor_zone.height / FLOOR_HEIGHT).max(1) as usize).min(total_floors)
    };
    let floor_height = if visible_count == 0 {
        0
    } else {
        (floor_zone.height / visible_count as u16).max(1)
    };
    let visible_start = viewport_start(total_floors, selected, visible_count);

    draw_roof(frame, building, roof_height);
    draw_lobby(
        frame,
        building,
        lobby_height,
        total_floors,
        world.worker_count(),
    );

    for (display_index, office) in offices
        .iter()
        .skip(visible_start)
        .take(visible_count)
        .enumerate()
    {
        let floor = Rect::new(
            floor_zone.x,
            floor_zone
                .y
                .saturating_add(display_index as u16 * floor_height),
            floor_zone.width,
            floor_height,
        );
        if !has_area(floor) {
            continue;
        }
        draw_floor(
            frame,
            canvas,
            sprites,
            office,
            floor,
            now,
            visible_start + display_index == selected,
        );
    }

    let selected_floor = selected.min(total_floors.saturating_sub(1));
    let display_index = selected_floor.saturating_sub(visible_start);
    let target_floor = if visible_count == 0 {
        None
    } else {
        Some(Rect::new(
            floor_zone.x,
            floor_zone
                .y
                .saturating_add(display_index as u16 * floor_height),
            floor_zone.width,
            floor_height,
        ))
    };
    draw_elevator(frame, shaft, target_floor, now);
    TowerLayout {
        floor_height,
        visible_start,
        visible_count,
        total_floors,
    }
}

fn draw_roof(frame: &mut Frame, building: Rect, height: u16) {
    if !has_area(building) || height == 0 {
        return;
    }
    let roof = Rect::new(building.x, building.y, building.width, height);
    Paragraph::new(Line::from(vec![
        Span::styled(
            " ▲ ROOF ",
            Style::default().fg(INK).add_modifier(Modifier::BOLD),
        ),
        Span::styled("•", Style::default().fg(ACCENT)),
    ]))
    .style(Style::default().fg(INK).bg(WALL))
    .block(Block::default().borders(Borders::TOP | Borders::LEFT | Borders::RIGHT))
    .render(roof, frame.buffer_mut());
}

fn draw_lobby(
    frame: &mut Frame,
    building: Rect,
    height: u16,
    floor_count: usize,
    worker_count: usize,
) {
    if !has_area(building) || height == 0 {
        return;
    }
    let y = building
        .y
        .saturating_add(building.height.saturating_sub(height));
    let lobby = Rect::new(building.x, y, building.width, height);
    let message = format!(" LOBBY  •  {floor_count} floors  •  {worker_count} workers ");
    Paragraph::new(message)
        .style(Style::default().fg(INK).bg(FLOOR))
        .block(Block::default().borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT))
        .render(lobby, frame.buffer_mut());
}

fn draw_floor(
    frame: &mut Frame,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    office: &Office,
    floor: Rect,
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
    let border_color = if selected {
        ACCENT
    } else {
        status_color(summary_status)
    };
    let marker = status_marker(summary_status).unwrap_or("•");
    let floor_title = short_path(&office.name, floor.width.saturating_sub(8) as usize);
    Block::default()
        .title(format!(" {marker} {floor_title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(PANEL))
        .render(floor, frame.buffer_mut());

    let inner = inset(floor, 1);
    if !has_area(inner) {
        return;
    }

    canvas.resize(inner.width as usize, inner.height as usize * 2);
    let floor_start = fill_office_background(canvas, sprites);
    let worker_count = office.workers.len().max(1);
    let slot_width = (inner.width as usize / worker_count).max(1);
    let worker_width = slot_width.min(6);
    let worker_height = floor_start.saturating_sub(1).clamp(1, 8);
    for (index, worker) in office.workers.iter().enumerate() {
        let slot_x = index.saturating_mul(inner.width as usize) / worker_count;
        let worker_x = slot_x + slot_width.saturating_sub(worker_width) / 2;
        let worker_y = floor_start.saturating_sub(worker_height + 1);
        render_worker(
            canvas,
            sprites,
            worker,
            now,
            PixelRect {
                x: worker_x,
                y: worker_y,
                width: worker_width,
                height: worker_height,
            },
        );
        let desk_width = slot_width.clamp(1, 7);
        let desk_x = slot_x + slot_width.saturating_sub(desk_width) / 2;
        let desk_y = floor_start
            .saturating_sub(2)
            .min(canvas.height().saturating_sub(1));
        canvas.blit_scaled(&sprites.desk, desk_x, desk_y, desk_width, 2);
    }
    canvas.render(frame.buffer_mut(), inner);

    let status = if blocked_count > 0 {
        format!("! blocked • {blocked_count} waiting")
    } else if failed_count > 0 {
        format!("× failed • {failed_count} worker")
    } else {
        let maximum_tokens = office
            .workers
            .iter()
            .map(|worker| worker.tokens_used)
            .max()
            .unwrap_or_default();
        format!(
            "{} / {} busy • max {}",
            office.busy_count(),
            office.workers.len(),
            human_tokens(maximum_tokens)
        )
    };
    Paragraph::new(status)
        .style(status_style(summary_status))
        .render(
            Rect::new(inner.x, inner.y, inner.width, inner.height.min(1)),
            frame.buffer_mut(),
        );
}

fn draw_elevator(frame: &mut Frame, shaft: Rect, target_floor: Option<Rect>, now: Millis) {
    let inner = inset(shaft, 1);
    if !has_area(inner) {
        return;
    }
    let car_width = inner.width.clamp(1, 3);
    let car_height = inner.height.clamp(1, 3);
    let top = inner.y;
    let bottom = inner
        .y
        .saturating_add(inner.height.saturating_sub(car_height));
    let target = target_floor
        .map(|floor| {
            floor
                .y
                .saturating_add(floor.height / 2)
                .saturating_sub(car_height / 2)
        })
        .unwrap_or(bottom)
        .clamp(top, bottom);
    let car_y = elevator_position(now, top, bottom, target);
    let car = Rect::new(inner.x, car_y, car_width, car_height);
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(PANEL_HIGHLIGHT))
        .render(car, frame.buffer_mut());
    if car.width > 2 && car.height > 2 {
        Paragraph::new("╫")
            .style(Style::default().fg(INK).bg(PANEL_HIGHLIGHT))
            .render(inset(car, 1), frame.buffer_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_keeps_selected_floor_visible() {
        assert_eq!(viewport_start(6, 0, 3), 0);
        assert_eq!(viewport_start(6, 3, 3), 2);
        assert_eq!(viewport_start(6, 5, 3), 3);
    }

    #[test]
    fn elevator_moves_to_top_and_bottom_targets() {
        assert_eq!(elevator_position(0, 0, 20, 0), 20);
        assert_eq!(elevator_position(ELEVATOR_TRAVEL_MS as Millis, 0, 20, 0), 0);
        assert_eq!(elevator_position(0, 0, 20, 20), 0);
        assert_eq!(
            elevator_position(ELEVATOR_TRAVEL_MS as Millis, 0, 20, 20),
            20
        );
    }
}
