//! Furnished cross-section of the selected tower floor.

use ratatui::style::Color;
use theywork_core::{Millis, Office};

use crate::canvas::Canvas;
use crate::sprite::SpriteSet;

pub(super) fn palette(theme: usize, light: bool) -> [Color; 4] {
    let accents = [
        (113, 137, 133),
        (110, 130, 153),
        (124, 141, 112),
        (138, 125, 145),
    ];
    let (r, g, b) = accents[theme % accents.len()];
    if light {
        [
            Color::Rgb(196, 201, 201),
            Color::Rgb(243, 243, 239),
            [
                Color::Rgb(226, 214, 193),
                Color::Rgb(217, 215, 207),
                Color::Rgb(221, 218, 198),
                Color::Rgb(223, 213, 209),
            ][theme % 4],
            Color::Rgb(r, g, b),
        ]
    } else {
        [
            Color::Rgb(43, 48, 53),
            Color::Rgb(65, 69, 73),
            [
                Color::Rgb(104, 96, 82),
                Color::Rgb(91, 97, 104),
                Color::Rgb(95, 102, 86),
                Color::Rgb(101, 93, 100),
            ][theme % 4],
            Color::Rgb(r, g, b),
        ]
    }
}

/// Attention is visible first without changing project or conversation identity.
pub(super) fn ranked_workers(office: &Office, now: Millis) -> Vec<&theywork_core::Worker> {
    let mut workers = office.workers.iter().collect::<Vec<_>>();
    workers.sort_by_key(|worker| match super::worker_status(worker, now) {
        theywork_core::WorkerStatus::Blocked => 0,
        theywork_core::WorkerStatus::Failed => 1,
        theywork_core::WorkerStatus::Running => 2,
        theywork_core::WorkerStatus::Idle => 3,
    });
    workers
}

fn rect(canvas: &mut Canvas, x: usize, y: usize, w: usize, h: usize, color: Color) {
    for yy in y..y.saturating_add(h).min(canvas.height()) {
        for xx in x..x.saturating_add(w).min(canvas.width()) {
            canvas.set(xx, yy, color);
        }
    }
}

pub(super) fn draw(
    canvas: &mut Canvas,
    office: &Office,
    sprites: &SpriteSet,
    now: Millis,
) -> Vec<(i32, i32)> {
    let (w, h) = (canvas.width(), canvas.height());
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let (px, py) = canvas.pixels_per_cell();
    let [_, wall, floor, wood] =
        palette(sprites.office_palette_index(office), canvas.is_light_mode());
    canvas.fill(wall);
    let ground = h.saturating_sub((py * 2).max(h / 7));
    rect(canvas, 0, ground, w, h - ground, floor);
    let seam = super::INK;
    rect(canvas, 0, ground, w, 1, seam);
    let capacity = (w / px / 18).clamp(1, 5);
    let workers = ranked_workers(office, now);
    let shown = workers.len().min(capacity);
    let slots = shown.max(1);
    let slot_width = w / slots;
    let mut markers = Vec::with_capacity(shown);
    for (slot, worker) in workers.into_iter().take(shown).enumerate() {
        let center = slot_width * slot + slot_width / 2;
        let available_height = ground.saturating_sub(py * 3).max(1);
        let stretch = super::sprite_pixel_width(canvas);
        let budget_width = (slot_width * 3 / 5).max(1);
        let source = sprites.worker_frame_fitting(
            worker,
            crate::sprite::look_for_worker(&office.workers, worker),
            now,
            budget_width / stretch,
            available_height,
        );
        let scale = (budget_width / (source.width() * stretch))
            .min(available_height / source.height())
            .max(1);
        let figure_w = source.width() * stretch * scale;
        let figure_h = source.height() * scale;
        let desk_w = (figure_w * 2)
            .min(slot_width.saturating_sub(px * 2))
            .max(px * 4);
        let desk_h = (figure_h / 3).max(py * 2);
        let desk_y = ground.saturating_sub(desk_h);
        let worker_y = desk_y.saturating_sub(figure_h * 2 / 3);
        let window_y = py;
        let window_h = (ground * 2 / 5).clamp(py * 3, py * 7);
        let window_w = (slot_width * 3 / 5).min(window_h * 3);
        let window_x = center.saturating_sub(window_w / 2);
        if ground > py * 7 {
            rect(
                canvas,
                window_x,
                window_y,
                window_w,
                window_h,
                super::BACKGROUND,
            );
            rect(
                canvas,
                window_x + px,
                window_y + 1,
                window_w.saturating_sub(px * 2),
                window_h.saturating_sub(2),
                Color::Rgb(71, 136, 168),
            );
            rect(
                canvas,
                window_x + window_w / 2,
                window_y,
                px,
                window_h,
                wall,
            );
            rect(canvas, window_x, window_y + window_h / 2, window_w, 1, wall);
        }
        let chair_x = center.saturating_sub(figure_w / 2);
        rect(
            canvas,
            chair_x,
            worker_y + figure_h / 3,
            figure_w,
            figure_h * 2 / 3,
            super::PANEL_HIGHLIGHT,
        );
        canvas.blit_scaled(&source, chair_x, worker_y, figure_w, figure_h);
        let desk_x = center.saturating_sub(desk_w / 2);
        rect(canvas, desk_x, desk_y, desk_w, (desk_h / 3).max(1), wood);
        for x in [desk_x + px, desk_x + desk_w.saturating_sub(px * 2)] {
            rect(canvas, x, desk_y, px.max(1), desk_h, wood);
        }
        let mw = (figure_w * 3 / 4).max(px * 3);
        let mh = (figure_h / 4).max(py);
        rect(
            canvas,
            desk_x + px,
            desk_y.saturating_sub(mh),
            mw,
            mh,
            super::BACKGROUND,
        );
        rect(
            canvas,
            desk_x + px * 2,
            desk_y.saturating_sub(mh) + 1,
            mw.saturating_sub(px * 2),
            mh.saturating_sub(2),
            Color::Rgb(107, 190, 204),
        );
        rect(
            canvas,
            center + desk_w / 4,
            desk_y.saturating_sub(py),
            px,
            py,
            super::INK,
        );
        markers.push((
            (center / px).min(w / px - 1) as i32,
            (ground / py).min(h / py - 1) as i32,
        ));
    }
    // The room also reads as a room without staff: a shared floor and windows,
    // never silhouettes standing on bare diamonds.
    if shown == 0 && w >= px * 10 && h >= py * 6 {
        rect(canvas, w / 3, py, w / 3, h / 3, Color::Rgb(71, 136, 168));
        rect(canvas, w / 2, py, px, h / 3, wall);
    }
    markers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{ColorDepth, PixelEncoding};
    use theywork_core::{Agent, OfficeId, Worker, WorkerId};

    #[test]
    fn room_representatives_prioritize_attention_without_reordering_the_office() {
        let id = OfficeId("/project".into());
        let mut office = Office::new(id.clone(), id.0.clone());
        office.workers = (0..10)
            .map(|index| {
                Worker::new(
                    WorkerId(index.to_string()),
                    id.clone(),
                    Agent::Codex,
                    index.to_string(),
                    0,
                )
            })
            .collect();
        office.workers[8].activity = theywork_core::Activity::Error {
            detail: "test failed".into(),
        };
        office.workers[9].activity = theywork_core::Activity::Waiting {
            detail: "approve command".into(),
        };
        office.workers[9].turn_in_flight = true;
        let ranked = ranked_workers(&office, 0);
        assert_eq!(ranked[0].id.0, "9");
        assert_eq!(ranked[1].id.0, "8");
        assert_eq!(office.workers[0].id.0, "0");
        let mut canvas = Canvas::with_color_depth_and_encoding(
            0,
            0,
            ColorDepth::TrueColor,
            PixelEncoding::Quadrants,
        );
        canvas.resize_for_cells(80, 16);
        let sprites = SpriteSet::new();
        sprites.set_animation_time(Some(0));
        assert_eq!(draw(&mut canvas, &office, &sprites, 0).len(), 4);
        let before = canvas.pixel_frame().rgba().to_vec();
        draw(&mut canvas, &office, &sprites, 1000);
        assert_eq!(
            before,
            canvas.pixel_frame().rgba(),
            "reduced motion must freeze the furnished room"
        );
    }

    #[test]
    fn dense_guard_rooms_keep_markers_bounded_and_edges_opaque() {
        let id = OfficeId("/project".into());
        let mut office = Office::new(id.clone(), id.0.clone());
        office.workers = (0..17)
            .map(|index| {
                Worker::new(
                    WorkerId(index.to_string()),
                    id.clone(),
                    Agent::Codex,
                    index.to_string(),
                    0,
                )
            })
            .collect();
        let sprites = SpriteSet::new();
        for encoding in PixelEncoding::ALL {
            for (width, height) in [(1, 1), (17, 8), (53, 19)] {
                let mut canvas =
                    Canvas::with_color_depth_and_encoding(0, 0, ColorDepth::TrueColor, encoding);
                canvas.resize_for_cells(width, height);
                let markers = draw(&mut canvas, &office, &sprites, 0);
                assert_eq!(
                    markers.len(),
                    office.workers.len().min((width / 18).clamp(1, 5))
                );
                assert!(markers
                    .iter()
                    .all(|&(x, y)| x >= 0 && y >= 0 && x < width as i32 && y < height as i32));
                assert!(canvas
                    .pixel_frame()
                    .rgba()
                    .chunks_exact(4)
                    .all(|pixel| pixel[3] == 255));
            }
        }
    }

    #[test]
    fn native_image_markers_stay_in_terminal_cell_coordinates() {
        let id = OfficeId("/native-project".into());
        let mut office = Office::new(id.clone(), id.0.clone());
        office.workers.push(Worker::new(
            WorkerId("worker".into()),
            id,
            Agent::Codex,
            "worker".into(),
            0,
        ));
        let mut canvas = Canvas::with_color_depth_and_encoding(
            0,
            0,
            ColorDepth::TrueColor,
            PixelEncoding::Sextants,
        );
        canvas.set_cell_pixel_size(Some((10, 20)));
        canvas.resize_for_cells(53, 19);
        let markers = draw(&mut canvas, &office, &SpriteSet::new(), 0);
        assert_eq!(markers.len(), 1);
        assert!(markers
            .iter()
            .all(|&(x, y)| (0..53).contains(&x) && (0..19).contains(&y)));
    }
}
