//! Compact rooms for the guard wall, with geometry sized for miniature staff.

use ratatui::style::Color;
use theywork_core::{Millis, Office};

use crate::canvas::Canvas;
use crate::sprite::{worker_looks, SpriteSet};

fn polygon(canvas: &mut Canvas, points: &[(i32, i32)], color: Color) {
    let min_y = points.iter().map(|point| point.1).min().unwrap_or(0).max(0);
    let max_y = points
        .iter()
        .map(|point| point.1)
        .max()
        .unwrap_or(0)
        .min(canvas.height() as i32 - 1);
    for y in min_y..=max_y {
        let mut crossings = Vec::with_capacity(points.len());
        for index in 0..points.len() {
            let (x0, y0) = points[index];
            let (x1, y1) = points[(index + 1) % points.len()];
            if (y0 <= y && y < y1) || (y1 <= y && y < y0) {
                crossings.push(x0 + (y - y0) * (x1 - x0) / (y1 - y0));
            }
        }
        crossings.sort_unstable();
        for pair in crossings.chunks_exact(2) {
            for x in pair[0].max(0)..=pair[1].min(canvas.width() as i32 - 1) {
                canvas.set(x as usize, y as usize, color);
            }
        }
    }
}

fn palette(office: &Office, light: bool) -> [Color; 4] {
    let theme = office.id.0.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % 4;
    if light {
        let colors = match theme {
            0 => [
                (189, 178, 212),
                (207, 198, 224),
                (230, 217, 184),
                (162, 112, 63),
            ],
            1 => [
                (149, 173, 197),
                (178, 196, 214),
                (199, 213, 224),
                (82, 112, 142),
            ],
            2 => [
                (147, 179, 148),
                (174, 201, 170),
                (216, 224, 181),
                (132, 117, 70),
            ],
            _ => [
                (176, 145, 191),
                (198, 171, 213),
                (214, 204, 232),
                (115, 86, 143),
            ],
        };
        return colors.map(|(r, g, b)| Color::Rgb(r, g, b));
    }
    let colors = match theme {
        0 => [(43, 37, 66), (58, 51, 88), (220, 201, 164), (138, 90, 56)],
        1 => [(22, 31, 51), (30, 42, 68), (65, 80, 107), (47, 61, 87)],
        2 => [(36, 59, 44), (47, 74, 56), (203, 211, 168), (122, 106, 69)],
        _ => [(46, 23, 64), (61, 31, 82), (34, 32, 61), (58, 47, 102)],
    };
    colors.map(|(r, g, b)| Color::Rgb(r, g, b))
}

pub(super) fn draw(
    canvas: &mut Canvas,
    office: &Office,
    sprites: &SpriteSet,
    now: Millis,
) -> Vec<(i32, i32)> {
    canvas.fill(super::BACKGROUND);
    let w = canvas.width() as i32;
    let h = canvas.height() as i32;
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let point = |x: i32, y: i32| {
        (
            w * (50 + (x - 50) * 2 / 3) / 100,
            h * (50 + (y - 50) * 3 / 5) / 100,
        )
    };
    let [left_wall, right_wall, floor, wood] = palette(office, canvas.is_light_mode());
    let back = point(50, 25);
    let right = point(86, 55);
    let front = point(50, 88);
    let left = point(14, 55);
    let wall_height = (h / 10).max(1);
    polygon(
        canvas,
        &[
            back,
            left,
            (left.0, left.1 - wall_height),
            (back.0, back.1 - wall_height),
        ],
        left_wall,
    );
    polygon(
        canvas,
        &[
            back,
            right,
            (right.0, right.1 - wall_height),
            (back.0, back.1 - wall_height),
        ],
        right_wall,
    );
    polygon(canvas, &[back, right, front, left], floor);

    let count = office.workers.len();
    let positions = if count <= 5 {
        [(48, 35), (61, 46), (72, 59), (33, 49), (47, 67)]
            .into_iter()
            .take(count)
            .map(|(x, y)| point(x, y))
            .collect::<Vec<_>>()
    } else {
        let columns = count.isqrt().max(1);
        let rows = count.div_ceil(columns);
        (0..count)
            .map(|index| {
                let u = (index % columns + 1) as i32 * 100 / (columns + 1) as i32;
                let v = (index / columns + 1) as i32 * 100 / (rows + 1) as i32;
                point(50 + (u - v) * 32 / 100, 27 + (u + v) * 28 / 100)
            })
            .collect()
    };
    let looks = worker_looks(&office.workers);
    let mut order = (0..count).collect::<Vec<_>>();
    order.sort_by_key(|&index| positions[index].1);
    let desk_half_width = (w / 18).max(2);
    let desk_half_height = (h / 32).max(1);
    for index in order {
        let (x, y) = positions[index];
        polygon(
            canvas,
            &[
                (x, y - desk_half_height),
                (x + desk_half_width, y),
                (x, y + desk_half_height),
                (x - desk_half_width, y),
            ],
            wood,
        );
        let sprite = sprites.worker_frame(&office.workers[index], looks[index], now);
        let width = (w / 14).max(3) as usize;
        let height = (h / 5).max(3) as usize;
        canvas.blit_scaled(
            &sprite,
            (x - width as i32 / 2).max(0) as usize,
            (y - height as i32).max(0) as usize,
            width,
            height,
        );
    }
    positions
        .into_iter()
        .map(|(x, y)| {
            (
                x / canvas.encoding().width_per_cell() as i32,
                y / canvas.encoding().height_per_cell() as i32,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{ColorDepth, PixelEncoding};
    use theywork_core::{Agent, OfficeId, Worker, WorkerId};

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
                assert_eq!(markers.len(), office.workers.len());
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
}
