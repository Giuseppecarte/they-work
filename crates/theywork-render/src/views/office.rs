//! One project's office floor, with a full-size employee at each desk.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use theywork_core::{Millis, Office, WorkerStatus};

use crate::canvas::Canvas;
use crate::sprite::SpriteSet;

use super::{
    draw_footer, draw_header, draw_tiny, fill_office_background, grid_rect, has_area, human_tokens,
    render_worker, short_path, status_color, status_style, token_bar, worker_status, PixelRect,
    ACCENT, BACKGROUND, MUTED,
};

const CARD_WIDTH: usize = 28;
const CARD_HEIGHT: usize = 16;
const MANAGER_APPROACH_MS: u64 = 2_400;
const MANAGER_HOLD_MS: u64 = 1_800;

/// The desk grid and pagination information for an office floor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OfficeLayout {
    pub columns: usize,
    pub rows: usize,
    pub page_size: usize,
    pub pages: usize,
}

/// Calculate how many full-size desk cards fit in a floor area.
pub fn desk_layout(worker_count: usize, width: u16, height: u16) -> OfficeLayout {
    if worker_count == 0 || width == 0 || height == 0 {
        return OfficeLayout::default();
    }
    let columns = (width as usize / CARD_WIDTH).max(1);
    let rows = (height as usize / CARD_HEIGHT).max(1);
    let page_size = columns.saturating_mul(rows).max(1);
    OfficeLayout {
        columns,
        rows,
        page_size,
        pages: worker_count.div_ceil(page_size),
    }
}

fn manager_approach(now: Millis, max_x: usize, target_x: usize) -> (usize, bool) {
    let target_x = target_x.min(max_x);
    let origin_x = if target_x <= max_x / 2 { max_x } else { 0 };
    if origin_x == target_x {
        return (target_x, false);
    }
    let cycle_ms = MANAGER_APPROACH_MS + MANAGER_HOLD_MS;
    let phase = now.max(0) as u64 % cycle_ms;
    if phase >= MANAGER_APPROACH_MS {
        return (target_x, false);
    }
    let distance = origin_x.abs_diff(target_x);
    let walked = (distance as u64 * phase / MANAGER_APPROACH_MS) as usize;
    let x = if origin_x < target_x {
        origin_x.saturating_add(walked)
    } else {
        origin_x.saturating_sub(walked)
    };
    (x, true)
}

pub(crate) fn draw(
    frame: &mut Frame,
    office: Option<&Office>,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    now: Millis,
    selected: usize,
) -> OfficeLayout {
    let area = frame.area();
    if area.width < 16 || area.height < 7 {
        draw_tiny(frame, "they-work • terminal too small for the office floor");
        return OfficeLayout::default();
    }
    let Some(office) = office else {
        draw_tiny(frame, "No office selected.");
        return OfficeLayout::default();
    };

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

    let status_label = if blocked_count > 0 {
        "blocked"
    } else if failed_count > 0 {
        "failed"
    } else if office
        .workers
        .iter()
        .any(|worker| worker_status(worker, now) == WorkerStatus::Running)
    {
        "running"
    } else {
        "idle"
    };

    let branch = office
        .workers
        .iter()
        .find_map(|worker| worker.git_branch.as_deref())
        .unwrap_or("no branch");
    let max_tokens = office
        .workers
        .iter()
        .map(|worker| worker.tokens_used)
        .max()
        .unwrap_or(0);
    let office_title = short_path(&office.name, area.width.saturating_sub(14) as usize);
    let (header, body, footer) = super::vertical_bands(area, 2, 2);
    draw_header(
        frame,
        header,
        &format!("OFFICE / {}", office_title),
        &format!(
            "{} • branch {} • status {}",
            short_path(&office.path, 28),
            branch,
            status_label
        ),
    );

    let layout = desk_layout(office.workers.len(), body.width, body.height);
    let page = if layout.page_size == 0 {
        0
    } else {
        selected / layout.page_size
    };
    draw_footer(
        frame,
        footer,
        &format!(
            "←↑↓→ / hjkl move   Enter desk   Esc cameras   page {}/{}",
            page.saturating_add(1),
            layout.pages.max(1)
        ),
    );
    if !has_area(body) {
        return layout;
    }
    if office.workers.is_empty() {
        Paragraph::new("This floor is quiet.")
            .style(Style::default().fg(MUTED).bg(BACKGROUND))
            .render(body, frame.buffer_mut());
        return layout;
    }

    canvas.resize(body.width as usize, body.height as usize * 2);
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

    let start = page.saturating_mul(layout.page_size);
    let mut desk_anchors = vec![None; layout.page_size];
    for (visible_index, worker) in office
        .workers
        .iter()
        .skip(start)
        .take(layout.page_size)
        .enumerate()
    {
        let card = grid_rect(body, visible_index, layout.columns, layout.rows);
        if !has_area(card) {
            continue;
        }
        let card_x = card.x.saturating_sub(body.x) as usize;
        let card_y = card.y.saturating_sub(body.y) as usize * 2;
        let card_width = card.width as usize;
        let card_height = card.height as usize * 2;
        let sprite_width = card_width.saturating_sub(2).clamp(1, 24);
        let sprite_height = card_height.saturating_sub(9).clamp(1, 19);
        let worker_x = card_x + card_width.saturating_sub(sprite_width) / 2;
        let worker_y = card_y + 1;
        render_worker(
            canvas,
            sprites,
            worker,
            now,
            PixelRect {
                x: worker_x,
                y: worker_y,
                width: sprite_width,
                height: sprite_height,
            },
        );

        let desk_width = card_width.saturating_sub(2).clamp(1, 24);
        let desk_height = card_height.saturating_sub(sprite_height + 2).clamp(1, 8);
        let desk_x = card_x + card_width.saturating_sub(desk_width) / 2;
        let desk_y = card_y
            .saturating_add(sprite_height)
            .saturating_add(1)
            .min(canvas.height().saturating_sub(desk_height));
        if let Some(anchor) = desk_anchors.get_mut(visible_index) {
            *anchor = Some((desk_x, desk_y, desk_width));
        }
        canvas.blit_scaled(&sprites.desk, desk_x, desk_y, desk_width, desk_height);

        let monitor_width = desk_width.min(10);
        let monitor_height = desk_height.min(5);
        if monitor_width > 1 && monitor_height > 1 {
            let monitor_x = card_x + card_width.saturating_sub(monitor_width) / 2;
            let monitor_y = desk_y.saturating_sub(monitor_height.saturating_sub(1));
            canvas.blit_scaled(
                &sprites.monitor,
                monitor_x,
                monitor_y,
                monitor_width,
                monitor_height,
            );
        }
    }
    let blocked_visible_index = office
        .workers
        .iter()
        .enumerate()
        .find_map(|(index, worker)| {
            (worker_status(worker, now) == WorkerStatus::Blocked
                && index >= start
                && index < start.saturating_add(layout.page_size))
            .then_some(index.saturating_sub(start))
        });
    let manager_needs_attention = office
        .workers
        .iter()
        .any(|worker| worker_status(worker, now) == WorkerStatus::Blocked);
    let walking_sprite = sprites.manager_animation(false).frame_at(now);
    let manager_height = canvas
        .height()
        .saturating_sub(floor_start)
        .clamp(1, 12)
        .min(walking_sprite.height())
        .max(1);
    let manager_width = walking_sprite
        .width()
        .saturating_mul(manager_height)
        .div_ceil(walking_sprite.height().max(1))
        .clamp(1, canvas.width().max(1));
    let stop_index = if desk_anchors.is_empty() {
        None
    } else {
        let cycle = (now.max(0) as u64 / 2_800) as usize;
        (!cycle.is_multiple_of(4)).then_some(cycle % desk_anchors.len())
    };
    let manager_anchor_index = match (blocked_visible_index, manager_needs_attention) {
        (Some(index), _) => Some(index),
        (None, true) => desk_anchors.iter().position(|anchor| anchor.is_some()),
        (None, false) => stop_index,
    };
    let manager_anchor =
        manager_anchor_index.and_then(|index| desk_anchors.get(index).copied().flatten());
    let max_x = canvas.width().saturating_sub(manager_width);
    let target_position = manager_anchor.map(|(desk_x, desk_y, desk_width)| {
        let target_x = desk_x
            .saturating_add(desk_width / 2)
            .saturating_sub(manager_width / 2)
            .min(max_x);
        (target_x, desk_y.saturating_sub(manager_height))
    });
    let walking_x = if max_x == 0 {
        0
    } else {
        ((now.max(0) as u64 / 85) % (max_x as u64 + 1)) as usize
    };
    let (manager_x, manager_y, attention_pose) = if manager_needs_attention {
        if let Some((target_x, target_y)) = target_position {
            let (x, walking) = manager_approach(now, max_x, target_x);
            let y = if walking {
                canvas.height().saturating_sub(manager_height)
            } else {
                target_y
            };
            (x, y, !walking)
        } else {
            (
                walking_x,
                canvas.height().saturating_sub(manager_height),
                false,
            )
        }
    } else if let Some((target_x, target_y)) = target_position {
        (target_x, target_y, false)
    } else {
        (
            walking_x,
            canvas.height().saturating_sub(manager_height),
            false,
        )
    };
    let manager_sprite = sprites.manager_animation(attention_pose).frame_at(now);
    canvas.blit_scaled(
        manager_sprite,
        manager_x,
        manager_y,
        manager_width,
        manager_height,
    );
    canvas.render(frame.buffer_mut(), body);

    for (visible_index, worker) in office
        .workers
        .iter()
        .skip(start)
        .take(layout.page_size)
        .enumerate()
    {
        let card = grid_rect(body, visible_index, layout.columns, layout.rows);
        if !has_area(card) {
            continue;
        }
        let worker_index = start + visible_index;
        let selected_card = worker_index == selected;
        let status = worker_status(worker, now);
        let border_color = if status.needs_attention() {
            status_color(status)
        } else if selected_card {
            ACCENT
        } else {
            MUTED
        };
        let border_style = Style::default().fg(border_color);
        Block::default()
            .title(format!(
                " {} ",
                short_path(&worker.name, card.width.saturating_sub(4) as usize)
            ))
            .borders(Borders::ALL)
            .border_style(border_style)
            .render(card, frame.buffer_mut());

        let label_height = card.height.min(1);
        let label = Line::from(vec![
            Span::styled(
                format!("{}  {}", worker.agent.label(), worker.activity.label()),
                super::activity_style(&worker.activity),
            ),
            Span::styled(format!("  {}", status.label()), status_style(status)),
        ]);
        let label_area = Rect::new(
            card.x + 1,
            card.y + card.height.saturating_sub(2),
            card.width.saturating_sub(2),
            label_height,
        );
        Paragraph::new(label).render(label_area, frame.buffer_mut());
        if card.height >= 5 && card.width >= 4 {
            let token_area = Rect::new(
                card.x + 1,
                card.y + card.height.saturating_sub(4),
                card.width.saturating_sub(2),
                1,
            );
            Paragraph::new(format!("tokens {}", human_tokens(worker.tokens_used)))
                .style(Style::default().fg(MUTED))
                .render(token_area, frame.buffer_mut());
        }
        if card.height >= 3 && card.width >= 4 {
            let bar_area = Rect::new(
                card.x + 1,
                card.y + card.height.saturating_sub(3),
                card.width.saturating_sub(2),
                1,
            );
            Paragraph::new(token_bar(
                worker.tokens_used,
                max_tokens,
                bar_area.width as usize,
            ))
            .style(Style::default().fg(ACCENT))
            .render(bar_area, frame.buffer_mut());
        }
    }
    layout
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desk_layout_always_has_room_for_one_card() {
        let layout = desk_layout(1, 1, 1);
        assert_eq!(layout.columns, 1);
        assert_eq!(layout.rows, 1);
        assert_eq!(layout.page_size, 1);
        assert_eq!(layout.pages, 1);
    }

    #[test]
    fn desk_layout_paginates_workers() {
        let layout = desk_layout(9, 84, 32);
        assert_eq!(layout.columns, 3);
        assert_eq!(layout.rows, 2);
        assert_eq!(layout.page_size, 6);
        assert_eq!(layout.pages, 2);
    }

    #[test]
    fn blocked_manager_approaches_the_target_desk() {
        let target_x = 15;
        assert_eq!(manager_approach(0, 20, target_x), (0, true));
        assert!(manager_approach((MANAGER_APPROACH_MS / 2) as Millis, 20, target_x).0 > 0);
        assert_eq!(
            manager_approach(MANAGER_APPROACH_MS as Millis, 20, target_x),
            (target_x, false)
        );
    }
}
