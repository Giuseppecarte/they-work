//! One project's dense isometric office floor.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Frame;
use theywork_core::{Millis, Office, Worker, WorkerStatus};

use crate::canvas::{Canvas, PixelEncoding};
use crate::sprite::{worker_looks, Sprite, SpriteSet, WorkerLook};

use super::{
    draw_footer, draw_header, draw_tiny, has_area, paint_opaque, short_path, status_color,
    worker_status, ACCENT, FLOOR, INK, MUTED, PANEL, PANEL_HIGHLIGHT, WALL, WARNING,
};

const MAX_DESKS: usize = 10;
const MIN_WIDTH: u16 = 16;
const MIN_HEIGHT: u16 = 7;
// The terminal height passed here is the body height after the title/footer
// bands. These cutoffs leave enough pixels for the room silhouette before the
// renderer asks a smaller window to carry the same information more plainly.
const ISO_MIN_WIDTH: u16 = 128;
const ISO_MIN_HEIGHT: u16 = 36;
const TOP_DOWN_MIN_WIDTH: u16 = 80;
const TOP_DOWN_MIN_HEIGHT: u16 = 16;
const MANAGER_TRAVEL_MS: u64 = 2_400;
const MANAGER_HOLD_MS: u64 = 1_800;
const SKY_CYCLE_MS: u64 = 90_000;
const CLOUD_DRIFT_MS: u64 = 220;

const OUTLINE: Color = Color::Rgb(13, 11, 20);
const FLOOR_LIGHT: Color = Color::Rgb(220, 201, 164);
const FLOOR_DARK: Color = Color::Rgb(156, 135, 99);
const FLOOR_DITHER: Color = Color::Rgb(192, 170, 130);
const WALL_LIGHT: Color = Color::Rgb(58, 51, 88);
const WINDOW_FRAME: Color = Color::Rgb(43, 37, 66);
const WINDOW_LIGHT: Color = Color::Rgb(90, 169, 201);
const TABLE_TOP: Color = Color::Rgb(107, 68, 41);
const TABLE_LIGHT: Color = Color::Rgb(138, 90, 56);
const CHAIR_BACK: Color = Color::Rgb(65, 58, 91);
const RUG: Color = Color::Rgb(194, 90, 74);
const RUG_BORDER: Color = Color::Rgb(142, 58, 46);
#[cfg(test)]
const TITLE_SIGN: Color = Color::Rgb(92, 15, 12);
#[cfg(test)]
const TITLE_BODY: Color = Color::Rgb(142, 26, 21);
const TITLE_COLOR: Color = Color::Rgb(184, 231, 222);
#[cfg(test)]
const SIGN_EXTRUSION_STEPS: usize = 6;
#[cfg(test)]
const ROOM_SIGN_EXTRUSION_STEPS: usize = 2;
#[cfg(test)]
const SIGN_EXTRUSION_STEP: i32 = 1;
#[cfg(test)]
const SIGN_GLYPH_RISE: i32 = 0;
#[cfg(test)]
const SIGN_MAX_WIDTH_NUMERATOR: usize = 9;
#[cfg(test)]
const SIGN_MAX_WIDTH_DENOMINATOR: usize = 20;
#[cfg(test)]
const SIGN_STATUS_RESERVE_CELLS: usize = 38;
const NIGHT_SKY_TOP: Color = Color::Rgb(13, 11, 20);
const NIGHT_SKY_BOTTOM: Color = Color::Rgb(58, 51, 88);
const DAY_SKY_TOP: Color = Color::Rgb(88, 214, 232);
const DAY_SKY_BOTTOM: Color = Color::Rgb(90, 169, 201);

// The sixth column is a composition gutter: the outer occupied tile still has
// room for a full-width worker and desk instead of ending at the floor edge.
const ISO_ROOM_COLUMNS: usize = 6;
const ISO_ROOM_ROWS: usize = 4;
const ISO_DESK_TILES: [(usize, usize); 10] = [
    (2, 0),
    (3, 2),
    (4, 0),
    (0, 2),
    (2, 3),
    (0, 1),
    (1, 3),
    (2, 1),
    (2, 2),
    (1, 2),
];
const ISO_RUG_TILE: (usize, usize) = (2, 3);
const ISO_PLANT_TILE: (usize, usize) = (0, 0);
const ISO_COOLER_TILE: (usize, usize) = (1, 0);
#[cfg(test)]
const ISO_MEETING_TABLE_TILE: (usize, usize) = (4, 2);

fn outline_color(canvas: &Canvas) -> Color {
    if canvas.is_light_mode() {
        super::LIGHT_INK
    } else {
        OUTLINE
    }
}

pub(super) fn apply_room_palette(canvas: &mut Canvas, palette: usize) {
    let [_, wall, floor, _] = super::guard_scene::palette(palette, canvas.is_light_mode());
    let dark = blend_color(floor, Color::Rgb(26, 22, 38), 65);
    // Dark room palettes need light seams. Darkening both samples collapses
    // them into one ANSI-256 colour, erasing the room's floor on native shells.
    let mid = match floor {
        Color::Rgb(r, g, b) if u16::from(r) + u16::from(g) + u16::from(b) < 360 => {
            blend_color(floor, Color::Rgb(170, 190, 218), 90)
        }
        _ => blend_color(floor, dark, 120),
    };
    canvas.remap_materials(&[
        (FLOOR_LIGHT, floor),
        (FLOOR_DARK, dark),
        (FLOOR_DITHER, mid),
        (WALL, wall),
        (Color::Rgb(188, 145, 93), floor),
        (Color::Rgb(224, 181, 115), mid),
        (Color::Rgb(91, 82, 112), wall),
        (Color::Rgb(117, 103, 139), blend_color(wall, floor, 32)),
    ]);
}

/// The desk grid and pagination information for an office floor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OfficeLayout {
    pub columns: usize,
    pub rows: usize,
    pub page_size: usize,
    pub pages: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Projection {
    Auto,
    Iso,
    TopDown,
    Side,
    List,
}

impl Projection {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Auto => Self::Iso,
            Self::Iso => Self::TopDown,
            Self::TopDown => Self::Side,
            Self::Side => Self::List,
            Self::List => Self::Auto,
        }
    }

    pub(crate) fn previous(self) -> Self {
        match self {
            Self::Auto => Self::List,
            Self::Iso => Self::Auto,
            Self::TopDown => Self::Iso,
            Self::Side => Self::TopDown,
            Self::List => Self::Side,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Iso => "isometric",
            Self::TopDown => "top-down",
            Self::Side => "side",
            Self::List => "list",
        }
    }
}

pub(crate) fn effective_projection(projection: Projection, width: u16, height: u16) -> Projection {
    match projection {
        Projection::Auto if width >= ISO_MIN_WIDTH && height >= ISO_MIN_HEIGHT => Projection::Iso,
        Projection::Auto if width >= TOP_DOWN_MIN_WIDTH && height >= TOP_DOWN_MIN_HEIGHT => {
            Projection::TopDown
        }
        Projection::Auto if width >= 60 && height >= 13 => Projection::Side,
        Projection::Auto => Projection::List,
        other => other,
    }
}
/// Calculate a bounded desk layout. The main floor shows at most ten desks;
/// additional workers are reachable on a clearly labelled overflow page.
pub fn desk_layout(worker_count: usize, width: u16, height: u16) -> OfficeLayout {
    if worker_count == 0 || width == 0 || height == 0 {
        return OfficeLayout::default();
    }
    let width = usize::from(width);
    let height = usize::from(height);
    let preferred_columns = if worker_count <= 5 {
        3
    } else if width >= 64 {
        5
    } else if width >= 42 {
        4
    } else {
        3
    };
    let columns = preferred_columns.min(worker_count).max(1);
    let rows = worker_count
        .div_ceil(columns)
        .min(if height >= 14 { 2 } else { 1 });
    let page_size = columns.saturating_mul(rows).clamp(1, MAX_DESKS);
    OfficeLayout {
        columns,
        rows,
        page_size,
        pages: worker_count.div_ceil(page_size),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IsoFootprint {
    width: u8,
    depth: u8,
    height: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IsoKind {
    Plant,
    Cooler,
    MeetingTable,
    Chair(usize),
    Worker(usize),
    Manager,
    Desk(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IsoItem {
    tile_x: usize,
    tile_y: usize,
    footprint: IsoFootprint,
    kind: IsoKind,
}

fn kind_rank(kind: IsoKind) -> u8 {
    match kind {
        IsoKind::Plant | IsoKind::Cooler => 1,
        IsoKind::MeetingTable | IsoKind::Chair(_) => 3,
        IsoKind::Worker(_) => 4,
        IsoKind::Manager => 5,
        IsoKind::Desk(_) => 6,
    }
}

fn painter_key(item: IsoItem) -> (usize, usize, u8) {
    (
        item.tile_x
            .saturating_add(item.tile_y)
            .saturating_add(usize::from(item.footprint.depth)),
        item.tile_y,
        kind_rank(item.kind),
    )
}

fn painter_order(items: &mut [IsoItem]) {
    items.sort_by_key(|item| painter_key(*item));
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct IsoGrid {
    columns: usize,
    rows: usize,
    tile_width: i32,
    tile_height: i32,
    origin_x: i32,
    origin_y: i32,
    encoding: PixelEncoding,
    pixels_per_cell: (usize, usize),
    desk_tiles: [(usize, usize); MAX_DESKS],
    wall_height: i32,
    animation_now: Option<Millis>,
}

impl IsoGrid {
    fn scale_half_height(self, value: usize) -> usize {
        value
            .saturating_mul(self.pixels_per_cell.1)
            .saturating_add(1)
            / 2
    }
}

impl IsoGrid {
    fn center(self, tile_x: usize, tile_y: usize) -> (i32, i32) {
        (
            self.origin_x + (tile_x as i32 - tile_y as i32) * self.tile_width / 2,
            self.origin_y + (tile_x as i32 + tile_y as i32) * self.tile_height / 2,
        )
    }

    fn desk_tile(self, slot: usize) -> (usize, usize) {
        let (tile_x, tile_y) = self
            .desk_tiles
            .get(slot)
            .copied()
            .unwrap_or((slot % self.columns, slot / self.columns));
        (
            tile_x.min(self.columns.saturating_sub(1)),
            tile_y.min(self.rows.saturating_sub(1)),
        )
    }
}

fn manager_tile(grid: IsoGrid, worker_count: usize, blocked_slot: usize) -> (usize, usize) {
    let blocked = grid.desk_tile(blocked_slot);
    let preferred = (
        blocked
            .0
            .saturating_add(1)
            .min(grid.columns.saturating_sub(1)),
        grid.rows.saturating_sub(1),
    );
    let occupied = (0..worker_count)
        .map(|slot| grid.desk_tile(slot))
        .collect::<Vec<_>>();

    (0..grid.rows)
        .flat_map(|tile_y| (0..grid.columns).map(move |tile_x| (tile_x, tile_y)))
        .filter(|tile| !occupied.contains(tile))
        .min_by_key(|&(tile_x, tile_y)| {
            (
                tile_x.abs_diff(preferred.0) + tile_y.abs_diff(preferred.1),
                tile_x.abs_diff(blocked.0) + tile_y.abs_diff(blocked.1),
            )
        })
        .unwrap_or(blocked)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RoomScale {
    Floor,
}

impl RoomScale {
    fn worker_size(self, grid: IsoGrid) -> (usize, usize) {
        figure_dimensions(
            grid.encoding,
            grid.pixels_per_cell,
            (grid.tile_width * 9 / 20).max(7) as usize,
            (grid.tile_height * 2).max(10) as usize,
        )
    }

    fn desk_size(self, grid: IsoGrid) -> (usize, usize) {
        let (width, height) = self.worker_size(grid);
        (
            (width * 2).min(grid.tile_width as usize * 4 / 5).max(5),
            (height / 3 + grid.pixels_per_cell.1).max(3),
        )
    }

    fn monitor_size(self, grid: IsoGrid) -> (usize, usize) {
        let (width, height) = self.worker_size(grid);
        ((width * 2 / 3).max(4), (height / 4).max(4))
    }

    fn plant_size(self, grid: IsoGrid) -> (usize, usize) {
        let (width, height) = self.worker_size(grid);
        ((width * 3 / 4).max(4), (height * 4 / 5).max(5))
    }

    fn cooler_size(self, grid: IsoGrid) -> (usize, usize) {
        let (width, height) = self.worker_size(grid);
        ((width * 2 / 3).max(4), (height * 3 / 4).max(5))
    }

    fn manager_size(self, grid: IsoGrid) -> (usize, usize) {
        self.worker_size(grid)
    }
}

#[cfg(test)]
fn make_grid(width: usize, height: usize, columns: usize, rows: usize) -> IsoGrid {
    make_grid_with_encoding(
        width,
        height,
        columns,
        rows,
        PixelEncoding::HalfBlocks,
        (1, 2),
    )
}

fn make_grid_with_encoding(
    width: usize,
    height: usize,
    columns: usize,
    rows: usize,
    encoding: PixelEncoding,
    pixels_per_cell: (usize, usize),
) -> IsoGrid {
    let columns = columns.max(1);
    let rows = rows.max(1);
    let floor_span = columns.saturating_add(rows).max(2);
    let (px, py) = pixels_per_cell;
    let title_band = py * 5;
    let wall_height = (height / 5).max(py * 4).min(py * 12) as i32;
    let available_height = height.saturating_sub(title_band + wall_height as usize + py * 2);
    let tile_width = (width * 18 / (floor_span * 10)).max(px * 5) as i32;
    let ideal_height = tile_width as usize * py / (px * 5).max(1);
    let tile_height = ideal_height
        .min(available_height * 2 / floor_span)
        .max(py * 2) as i32;
    let floor_depth = floor_span as i32 * tile_height / 2;
    let room_height = wall_height + floor_depth;
    let origin_y = title_band as i32
        + (height as i32 - title_band as i32 - room_height).max(0) / 2
        + wall_height
        + tile_height / 2;
    let origin_x = width as i32 / 2 - (columns as i32 - rows as i32) * tile_width / 4;
    IsoGrid {
        columns,
        rows,
        tile_width,
        tile_height,
        origin_x,
        origin_y,
        encoding,
        pixels_per_cell,
        desk_tiles: ISO_DESK_TILES,
        wall_height,
        animation_now: None,
    }
}

fn make_room_grid(canvas: &Canvas, count: usize) -> IsoGrid {
    let (columns, rows) = if count <= 1 {
        (4, 3)
    } else if count <= 3 {
        (5, 4)
    } else {
        (ISO_ROOM_COLUMNS, ISO_ROOM_ROWS)
    };
    let mut grid = make_grid_with_encoding(
        canvas.width(),
        canvas.height(),
        columns,
        rows,
        canvas.encoding(),
        canvas.pixels_per_cell(),
    );
    if count <= 1 {
        grid.desk_tiles[0] = (2, 1);
    } else if count <= 3 {
        grid.desk_tiles[..3].copy_from_slice(&[(1, 0), (3, 0), (2, 2)]);
    }
    grid
}

fn phase_ms(now: Millis, period: u64) -> u64 {
    if period == 0 {
        return 0;
    }
    now.max(0) as u64 % period
}

fn desk_bounds(grid: IsoGrid, scale: RoomScale, slot: usize) -> (i32, i32, usize, usize) {
    let (center_x, center_y) = grid.center(grid.desk_tile(slot).0, grid.desk_tile(slot).1);
    let (width, height) = scale.desk_size(grid);
    let x = center_x - width as i32 / 2;
    let y = center_y - scale.worker_size(grid).1 as i32 / 6;
    (x, y, width, height)
}

fn worker_bounds(grid: IsoGrid, scale: RoomScale, slot: usize) -> (i32, i32, usize, usize) {
    let (center_x, center_y) = grid.center(grid.desk_tile(slot).0, grid.desk_tile(slot).1);
    let (width, height) = scale.worker_size(grid);
    let seated_drop = height / 6;
    (
        center_x - width as i32 / 2,
        center_y - height as i32 + seated_drop as i32 + 1,
        width,
        height,
    )
}

fn manager_position(grid: IsoGrid, tile: (usize, usize), now: Millis) -> (i32, i32, bool) {
    let origin = grid.center(0, grid.rows.saturating_sub(1));
    let target = grid.center(tile.0, tile.1);
    let cycle = MANAGER_TRAVEL_MS + MANAGER_HOLD_MS;
    let phase = phase_ms(now, cycle);
    let travel = phase.min(MANAGER_TRAVEL_MS);
    (
        origin.0 + (target.0 - origin.0) * travel as i32 / MANAGER_TRAVEL_MS as i32,
        origin.1 + (target.1 - origin.1) * travel as i32 / MANAGER_TRAVEL_MS as i32,
        phase >= MANAGER_TRAVEL_MS,
    )
}

fn manager_bounds(
    grid: IsoGrid,
    scale: RoomScale,
    tile: (usize, usize),
    now: Millis,
) -> (i32, i32, usize, usize, bool) {
    let (manager_x, manager_y, attention) =
        manager_position(grid, tile, grid.animation_now.unwrap_or(now));
    let (width, height) = scale.manager_size(grid);
    (
        manager_x - width as i32 / 2,
        manager_y - height as i32 + 1,
        width,
        height,
        attention,
    )
}

fn daylight_amount(now: Millis) -> u16 {
    let half = SKY_CYCLE_MS / 2;
    let phase = phase_ms(now, SKY_CYCLE_MS);
    let distance = if phase <= half {
        phase
    } else {
        SKY_CYCLE_MS - phase
    };
    (distance.saturating_mul(255) / half.max(1)) as u16
}

fn blend_channel(from: u8, to: u8, amount: u16) -> u8 {
    let weighted = u32::from(from) * u32::from(255 - amount) + u32::from(to) * u32::from(amount);
    ((weighted + 127) / 255) as u8
}

fn blend_color(from: Color, to: Color, amount: u16) -> Color {
    let amount = amount.min(255);
    match (from, to) {
        (Color::Rgb(fr, fg, fb), Color::Rgb(tr, tg, tb)) => Color::Rgb(
            blend_channel(fr, tr, amount),
            blend_channel(fg, tg, amount),
            blend_channel(fb, tb, amount),
        ),
        _ if amount < 128 => from,
        _ => to,
    }
}

fn sky_color(now: Millis, row: usize, height: usize) -> Color {
    let daylight = daylight_amount(now);
    let top = blend_color(NIGHT_SKY_TOP, DAY_SKY_TOP, daylight);
    let bottom = blend_color(NIGHT_SKY_BOTTOM, DAY_SKY_BOTTOM, daylight);
    let row_amount = if height <= 1 {
        0
    } else {
        (row.min(height - 1) as u32 * 255 / (height - 1) as u32) as u16
    };
    blend_color(top, bottom, row_amount)
}

fn set_pixel(canvas: &mut Canvas, x: i32, y: i32, color: Color) {
    if x >= 0 && y >= 0 {
        canvas.set(x as usize, y as usize, color);
    }
}

#[cfg(test)]
fn set_sign_pixel(canvas: &mut Canvas, x: i32, y: i32, color: Color, floor_top: usize) {
    if y >= 0 && (y as usize) < floor_top {
        set_pixel(canvas, x, y, color);
    }
}

fn draw_line(canvas: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
    let steps = (x1 - x0).abs().max((y1 - y0).abs());
    if steps == 0 {
        set_pixel(canvas, x0, y0, color);
        return;
    }
    for step in 0..=steps {
        let x = x0 + (x1 - x0) * step / steps;
        let y = y0 + (y1 - y0) * step / steps;
        set_pixel(canvas, x, y, color);
    }
}

fn fill_polygon(canvas: &mut Canvas, points: &[(i32, i32)], color: Color) {
    if points.len() < 3 {
        return;
    }
    let min_y = points.iter().map(|point| point.1).min().unwrap_or(0);
    let max_y = points.iter().map(|point| point.1).max().unwrap_or(0);
    for y in min_y..=max_y {
        let mut intersections = Vec::with_capacity(points.len());
        for index in 0..points.len() {
            let (x0, y0) = points[index];
            let (x1, y1) = points[(index + 1) % points.len()];
            if y0 == y1 {
                continue;
            }
            let lower = y0.min(y1);
            let upper = y0.max(y1);
            if y < lower || y > upper {
                continue;
            }
            let numerator = i64::from(x1 - x0) * i64::from(y - y0);
            let denominator = i64::from(y1 - y0);
            let x = i64::from(x0) + numerator / denominator;
            if let Ok(x) = i32::try_from(x) {
                intersections.push(x);
            }
        }
        intersections.sort_unstable();
        intersections.dedup();
        for pair in intersections.chunks_exact(2) {
            let start = pair[0].min(pair[1]);
            let end = pair[0].max(pair[1]);
            for x in start..=end {
                set_pixel(canvas, x, y, color);
            }
        }
    }
}

fn fill_rect(canvas: &mut Canvas, x: i32, y: i32, width: i32, height: i32, color: Color) {
    for row in 0..height.max(0) {
        for column in 0..width.max(0) {
            set_pixel(canvas, x + column, y + row, color);
        }
    }
}

fn draw_rect_outline(canvas: &mut Canvas, x: i32, y: i32, width: i32, height: i32, color: Color) {
    if width <= 0 || height <= 0 {
        return;
    }
    draw_line(canvas, x, y, x + width - 1, y, color);
    draw_line(canvas, x, y, x, y + height - 1, color);
    draw_line(
        canvas,
        x + width - 1,
        y,
        x + width - 1,
        y + height - 1,
        color,
    );
    draw_line(
        canvas,
        x,
        y + height - 1,
        x + width - 1,
        y + height - 1,
        color,
    );
}

fn diamond_contains(dx: i32, dy: i32, half_width: i32, half_height: i32) -> bool {
    if half_width <= 0 || half_height <= 0 {
        return false;
    }
    dx.abs() * half_height + dy.abs() * half_width <= half_width * half_height
}

fn fill_diamond(
    canvas: &mut Canvas,
    center_x: i32,
    center_y: i32,
    half_width: i32,
    half_height: i32,
    base: Color,
    dither: Color,
) {
    for dy in -half_height..=half_height {
        for dx in -half_width..=half_width {
            if !diamond_contains(dx, dy, half_width, half_height) {
                continue;
            }
            let color = if (dx + dy).rem_euclid(5) == 0 {
                dither
            } else {
                base
            };
            set_pixel(canvas, center_x + dx, center_y + dy, color);
        }
    }
}

fn draw_diamond(
    canvas: &mut Canvas,
    center_x: i32,
    center_y: i32,
    half_width: i32,
    half_height: i32,
    base: Color,
    dither: Color,
) {
    fill_diamond(
        canvas,
        center_x,
        center_y,
        half_width,
        half_height,
        base,
        dither,
    );
    let top = (center_x, center_y - half_height);
    let right = (center_x + half_width, center_y);
    let bottom = (center_x, center_y + half_height);
    let left = (center_x - half_width, center_y);
    draw_line(canvas, top.0, top.1, left.0, left.1, FLOOR_DARK);
    draw_line(canvas, top.0, top.1, right.0, right.1, FLOOR_DITHER);
    draw_line(
        canvas,
        left.0,
        left.1,
        bottom.0,
        bottom.1,
        outline_color(canvas),
    );
    draw_line(canvas, right.0, right.1, bottom.0, bottom.1, FLOOR_DARK);
}

fn draw_sky_area(canvas: &mut Canvas, x: i32, y: i32, width: i32, height: i32, now: Millis) {
    if width <= 0 || height <= 0 {
        return;
    }
    let daylight = daylight_amount(now);
    for row in 0..height {
        let color = sky_color(now, row as usize, height as usize);
        for column in 0..width {
            set_pixel(canvas, x + column, y + row, color);
        }
    }

    let span = u64::from(width.max(1) as u32) + 10;
    let elapsed = now.max(0) as u64;
    let cloud_x = x - 7 + (elapsed / CLOUD_DRIFT_MS % span) as i32;
    let cloud_y = y + height / 3;
    for offset in 0..8 {
        if cloud_x + offset < x || cloud_x + offset >= x + width {
            continue;
        }
        set_pixel(
            canvas,
            cloud_x + offset,
            cloud_y,
            blend_color(WALL, INK, daylight),
        );
    }
    for offset in [1, 2, 4, 5, 6] {
        if cloud_x + offset < x || cloud_x + offset >= x + width || cloud_y - 1 < y {
            continue;
        }
        set_pixel(
            canvas,
            cloud_x + offset,
            cloud_y - 1,
            blend_color(WALL, INK, daylight),
        );
    }

    if daylight < 150 {
        let star_color = blend_color(INK, DAY_SKY_TOP, daylight);
        let width = width.max(1) as u64;
        let height = height.max(1) as u64;
        for index in 0..4_u64 {
            let star_x = x + ((index * 17 + elapsed / 900) % width) as i32;
            let star_y = y + ((index * 5 + elapsed / 1_600) % height) as i32;
            set_pixel(canvas, star_x, star_y, star_color);
        }
    }
}

fn draw_window(canvas: &mut Canvas, x: i32, y: i32, width: i32, height: i32, now: Millis) {
    if width < 5 || height < 4 {
        return;
    }
    fill_rect(canvas, x, y, width, height, WINDOW_FRAME);
    draw_sky_area(canvas, x + 1, y + 1, width - 2, height - 2, now);
    draw_rect_outline(canvas, x, y, width, height, outline_color(canvas));
    let middle = x + width / 2;
    draw_line(canvas, middle, y + 1, middle, y + height - 2, WINDOW_FRAME);
    draw_line(
        canvas,
        x + 1,
        y + height - 2,
        x + width - 2,
        y + height - 2,
        WINDOW_LIGHT,
    );
}

fn draw_polygon(canvas: &mut Canvas, points: &[(i32, i32)], color: Color) {
    if points.len() < 2 {
        return;
    }
    for index in 0..points.len() {
        let start = points[index];
        let end = points[(index + 1) % points.len()];
        draw_line(canvas, start.0, start.1, end.0, end.1, color);
    }
}

fn wall_point(start: (i32, i32), end: (i32, i32), along: i32, rise: i32) -> (i32, i32) {
    let x = start.0 + (end.0 - start.0) * along / 10;
    let y = start.1 + (end.1 - start.1) * along / 10 - rise;
    (x, y)
}

#[allow(clippy::too_many_arguments)]
fn draw_wall_window(
    canvas: &mut Canvas,
    start: (i32, i32),
    end: (i32, i32),
    wall_height: i32,
    start_along: i32,
    end_along: i32,
    now: Millis,
    window_color: Color,
) {
    if wall_height < 6 || end_along <= start_along {
        return;
    }
    let frame_top = wall_height.saturating_sub(1);
    let frame = [
        wall_point(start, end, start_along, frame_top),
        wall_point(start, end, end_along, frame_top),
        wall_point(start, end, end_along, 2),
        wall_point(start, end, start_along, 2),
    ];
    fill_polygon(canvas, &frame, WINDOW_FRAME);
    draw_polygon(canvas, &frame, outline_color(canvas));
    let inner_top = wall_height.saturating_sub(2);
    let inner_bottom = 3;
    let inner = [
        wall_point(start, end, start_along + 1, inner_top),
        wall_point(start, end, end_along - 1, inner_top),
        wall_point(start, end, end_along - 1, inner_bottom),
        wall_point(start, end, start_along + 1, inner_bottom),
    ];
    let sky = blend_color(window_color, ACCENT, daylight_amount(now) / 3);
    fill_polygon(canvas, &inner, sky);
    draw_polygon(canvas, &inner, window_color);
    let middle = (start_along + end_along) / 2;
    let divider_top = wall_point(start, end, middle, inner_top);
    let divider_bottom = wall_point(start, end, middle, inner_bottom);
    draw_line(
        canvas,
        divider_top.0,
        divider_top.1,
        divider_bottom.0,
        divider_bottom.1,
        WINDOW_FRAME,
    );
}

#[cfg(test)]
fn iso_wall_height(canvas: &Canvas) -> i32 {
    let base_height = canvas.half_space_height(canvas.height());
    canvas.scale_half_height((base_height / 4).clamp(16, 24)) as i32
}
fn draw_isometric_backdrop(canvas: &mut Canvas, grid: IsoGrid, now: Millis) {
    canvas.fill(super::BACKGROUND);
    let [back, right, _, left] = floor_corners(grid);
    let wall_height = grid.wall_height;
    let back_top = (back.0, back.1 - wall_height);
    let left_top = (left.0, left.1 - wall_height);
    let right_top = (right.0, right.1 - wall_height);
    fill_polygon(canvas, &[back_top, back, left, left_top], WALL);
    fill_polygon(
        canvas,
        &[back_top, back, right, right_top],
        Color::Rgb(43, 37, 66),
    );
    let outline = outline_color(canvas);
    draw_line(
        canvas, back_top.0, back_top.1, left_top.0, left_top.1, outline,
    );
    draw_line(
        canvas,
        back_top.0,
        back_top.1,
        right_top.0,
        right_top.1,
        outline,
    );
    draw_line(canvas, back_top.0, back_top.1, back.0, back.1, outline);
    draw_line(canvas, left_top.0, left_top.1, left.0, left.1, outline);
    draw_line(canvas, right_top.0, right_top.1, right.0, right.1, outline);
    draw_wall_window(canvas, back, left, wall_height, 2, 8, now, TITLE_COLOR);
    draw_wall_window(
        canvas,
        back,
        right,
        wall_height,
        2,
        5,
        now + 1_700,
        WINDOW_LIGHT,
    );
    draw_wall_window(
        canvas,
        back,
        right,
        wall_height,
        6,
        9,
        now + 3_400,
        WINDOW_LIGHT,
    );
}
fn compact_glyph(character: char) -> [u8; 5] {
    match character.to_ascii_uppercase() {
        'A' => return [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => return [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => return [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => return [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => return [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => return [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => return [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => return [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => return [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => return [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => return [0b101, 0b110, 0b100, 0b110, 0b101],
        'L' => return [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => return [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => return [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => return [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => return [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => return [0b010, 0b101, 0b101, 0b011, 0b001],
        'R' => return [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => return [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => return [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => return [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => return [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => return [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => return [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => return [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => return [0b111, 0b001, 0b010, 0b100, 0b111],
        _ => {}
    }
    let glyph = glyph_5x7(character);
    [0, 1, 3, 5, 6].map(|row| {
        let source = glyph[row];
        let mut compact = 0;
        if source & 0b11_000 != 0 {
            compact |= 0b100;
        }
        if source & 0b00_100 != 0 {
            compact |= 0b010;
        }
        if source & 0b00_011 != 0 {
            compact |= 0b001;
        }
        compact
    })
}

#[cfg(test)]
fn compact_sign_width(line: &str) -> usize {
    line.chars().count().saturating_mul(4).saturating_sub(1)
}

#[cfg(test)]
fn compact_sign_lines(label: &str, width: usize) -> Vec<String> {
    let max_chars = width.saturating_add(1).div_euclid(4).max(1);
    label
        .chars()
        .collect::<Vec<_>>()
        .chunks(max_chars)
        .map(|line| line.iter().collect::<String>())
        .collect()
}

#[cfg(test)]
fn draw_compact_sign(canvas: &mut Canvas, label: &str, wall_top: i32) {
    let x_scale = canvas.pixels_per_cell().0;
    let y_scale = canvas.scale_half_height(1);
    let width = room_sign_max_width(canvas);
    let lines = compact_sign_lines(label, width / x_scale.max(1));
    let line_height = 6_i32.saturating_mul(y_scale as i32);
    let total_height = lines.len().saturating_mul(line_height as usize) as i32;
    let floor_limit = wall_top.saturating_sub(1).max(0) as usize;
    let top = wall_top
        .saturating_sub(1)
        .saturating_sub(total_height)
        .max(0);
    for (line_index, line) in lines.iter().enumerate() {
        let line_width = compact_sign_width(line).saturating_mul(x_scale);
        let x0 = room_sign_x(canvas, line_width) as i32;
        let y0 = top + line_index as i32 * line_height;
        for (character_index, character) in line.chars().enumerate() {
            let glyph = compact_glyph(character);
            let glyph_x = x0 + character_index as i32 * 4 * x_scale as i32;
            for (row_index, bits) in glyph.iter().enumerate() {
                for column_index in 0..3 {
                    if bits & (1 << (2 - column_index)) == 0 {
                        continue;
                    }
                    set_sign_block(
                        canvas,
                        glyph_x + column_index * x_scale as i32,
                        y0 + row_index as i32 * y_scale as i32,
                        x_scale,
                        y_scale,
                        TITLE_COLOR,
                        floor_limit,
                    );
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn draw_extruded_glyph(
    canvas: &mut Canvas,
    glyph: [u8; 7],
    glyph_x: i32,
    glyph_y: i32,
    floor_limit: usize,
    x_scale: usize,
    y_scale: usize,
    depth_steps: usize,
) {
    let mut face_pixels = Vec::new();
    for (row_index, bits) in glyph.iter().enumerate() {
        for column_index in 0..5 {
            if bits & (1 << (4 - column_index)) == 0 {
                continue;
            }
            let x = glyph_x + column_index * x_scale as i32;
            let y = glyph_y + row_index as i32 * y_scale as i32;
            face_pixels.push((x, y));
        }
    }

    // Each glyph owns its complete shadow. Keeping depth local prevents one
    // letter's extrusion from becoming the next letter's tail.
    let depth_step_x = sign_depth_step(canvas, x_scale);
    let depth_step_y = sign_depth_step(canvas, y_scale);
    for depth in (1..=depth_steps).rev() {
        let offset_x = depth as i32 * SIGN_EXTRUSION_STEP * depth_step_x as i32;
        let offset_y = depth as i32 * SIGN_EXTRUSION_STEP * depth_step_y as i32;
        let color = if depth >= depth_steps.saturating_sub(1) {
            TITLE_SIGN
        } else {
            TITLE_BODY
        };
        for &(x, y) in &face_pixels {
            set_sign_block(
                canvas,
                x + offset_x,
                y + offset_y,
                x_scale,
                y_scale,
                color,
                floor_limit,
            );
        }
    }

    let outline_x = sign_outline_radius(canvas, x_scale);
    let outline_y = sign_outline_radius(canvas, y_scale);
    for &(x, y) in &face_pixels {
        for y_offset in -outline_y..=outline_y {
            for x_offset in -outline_x..=outline_x {
                set_sign_block(
                    canvas,
                    x + x_offset,
                    y + y_offset,
                    x_scale,
                    y_scale,
                    outline_color(canvas),
                    floor_limit,
                );
            }
        }
    }
    for (x, y) in face_pixels {
        set_sign_block(canvas, x, y, x_scale, y_scale, TITLE_COLOR, floor_limit);
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn draw_extruded_line(
    canvas: &mut Canvas,
    line: &str,
    x0: i32,
    y0: i32,
    floor_limit: usize,
    x_scale: usize,
    y_scale: usize,
    glyph_pitch: usize,
    rise_per_glyph: usize,
    depth_steps: usize,
) {
    for (character_index, character) in line.chars().enumerate() {
        draw_extruded_glyph(
            canvas,
            glyph_5x7(character),
            x0 + character_index as i32 * glyph_pitch as i32,
            y0 + character_index as i32 * rise_per_glyph as i32,
            floor_limit,
            x_scale,
            y_scale,
            depth_steps,
        );
    }
    // Compose depth first, then restore all faces as one line. A later glyph's
    // extrusion may cross an earlier glyph, but may never erase its face.
    for (character_index, character) in line.chars().enumerate() {
        let glyph_x = x0 + character_index as i32 * glyph_pitch as i32;
        let glyph_y = y0 + character_index as i32 * rise_per_glyph as i32;
        for (row_index, bits) in glyph_5x7(character).iter().enumerate() {
            for column_index in 0..5 {
                if bits & (1 << (4 - column_index)) != 0 {
                    set_sign_block(
                        canvas,
                        glyph_x + column_index * x_scale as i32,
                        glyph_y + row_index as i32 * y_scale as i32,
                        x_scale,
                        y_scale,
                        TITLE_COLOR,
                        floor_limit,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn draw_sign_glyph_sheet(canvas: &mut Canvas) {
    canvas.fill(super::BACKGROUND);
    let x_scale = canvas.pixels_per_cell().0;
    let y_scale = canvas.scale_half_height(1);
    let glyph_pitch = sign_glyph_pitch(canvas, x_scale, SIGN_EXTRUSION_STEPS);
    let extrusion_y = SIGN_EXTRUSION_STEPS * sign_depth_step(canvas, y_scale);
    let row_pitch = 7 * y_scale + extrusion_y + 3 * y_scale;
    for (row, line) in ["ABCDEFGHIJKL", "MNOPQRSTUVWX", "YZ0123456789"]
        .into_iter()
        .enumerate()
    {
        draw_extruded_line(
            canvas,
            line,
            (2 * x_scale) as i32,
            (2 * y_scale + row * row_pitch) as i32,
            canvas.height(),
            x_scale,
            y_scale,
            glyph_pitch,
            0,
            SIGN_EXTRUSION_STEPS,
        );
    }
}

#[cfg(test)]
fn sign_depth_step(canvas: &Canvas, scale: usize) -> usize {
    if canvas.has_image_density() {
        (scale / 4).max(1)
    } else {
        1
    }
}

#[cfg(test)]
fn sign_glyph_pitch(canvas: &Canvas, x_scale: usize, depth_steps: usize) -> usize {
    let extrusion = depth_steps
        .saturating_mul(SIGN_EXTRUSION_STEP as usize)
        .saturating_mul(sign_depth_step(canvas, x_scale));
    6usize.saturating_mul(x_scale).saturating_add(extrusion)
}

#[cfg(test)]
fn room_sign_max_width(canvas: &Canvas) -> usize {
    canvas.width().saturating_mul(SIGN_MAX_WIDTH_NUMERATOR) / SIGN_MAX_WIDTH_DENOMINATOR
}

#[cfg(test)]
fn room_sign_x(canvas: &Canvas, width: usize) -> usize {
    let centered = canvas.width().saturating_sub(width) / 2;
    let reserved = SIGN_STATUS_RESERVE_CELLS.saturating_mul(canvas.pixels_per_cell().0);
    centered
        .max(reserved)
        .min(canvas.width().saturating_sub(width))
}

#[cfg(test)]
fn sign_outline_radius(canvas: &Canvas, scale: usize) -> i32 {
    if canvas.has_image_density() {
        (scale / 5).max(1) as i32
    } else {
        0
    }
}

#[cfg(test)]
fn set_sign_block(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    color: Color,
    floor_top: usize,
) {
    for row in 0..height {
        for column in 0..width {
            set_sign_pixel(canvas, x + column as i32, y + row as i32, color, floor_top);
        }
    }
}

#[cfg(test)]
fn draw_isometric_sign(canvas: &mut Canvas, label: &str, wall_top: i32) {
    if canvas.width() == 0 || canvas.height() == 0 {
        return;
    }
    let lines = sign_lines(label);
    let Some(line) = (lines.len() == 1).then(|| lines[0].as_str()) else {
        draw_compact_sign(canvas, label, wall_top);
        return;
    };
    let x_scale = canvas.pixels_per_cell().0;
    let y_scale = canvas.scale_half_height(1);
    let depth_step_x = sign_depth_step(canvas, x_scale);
    let depth_step_y = sign_depth_step(canvas, y_scale);
    let extrusion_x = ROOM_SIGN_EXTRUSION_STEPS
        .saturating_mul(SIGN_EXTRUSION_STEP as usize)
        .saturating_mul(depth_step_x);
    let extrusion_y = ROOM_SIGN_EXTRUSION_STEPS
        .saturating_mul(SIGN_EXTRUSION_STEP as usize)
        .saturating_mul(depth_step_y);
    let glyph_pitch = sign_glyph_pitch(canvas, x_scale, ROOM_SIGN_EXTRUSION_STEPS);
    let face_width = line
        .chars()
        .count()
        .saturating_sub(1)
        .saturating_mul(glyph_pitch)
        .saturating_add(5usize.saturating_mul(x_scale));
    let rise_per_glyph = (SIGN_GLYPH_RISE as usize).saturating_mul(y_scale);
    let sign_height = 7usize
        .saturating_mul(y_scale)
        .saturating_add(line.chars().count().saturating_sub(1) * rise_per_glyph)
        .saturating_add(extrusion_y);
    let floor_limit = wall_top.saturating_sub(1).max(0) as usize;
    if face_width == 0
        || face_width.saturating_add(extrusion_x) > room_sign_max_width(canvas)
        || sign_height > floor_limit
    {
        draw_compact_sign(canvas, label, wall_top);
        return;
    }

    let x0 = room_sign_x(canvas, face_width.saturating_add(extrusion_x)) as i32;
    let top = wall_top - 1 - sign_height as i32;
    draw_extruded_line(
        canvas,
        line,
        x0,
        top,
        floor_limit,
        x_scale,
        y_scale,
        glyph_pitch,
        rise_per_glyph,
        ROOM_SIGN_EXTRUSION_STEPS,
    );
}

fn glyph_5x7(character: char) -> [u8; 7] {
    match character {
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x1e, 0x11, 0x11, 0x11, 0x1e],
        'C' => [0x0f, 0x10, 0x10, 0x10, 0x10, 0x10, 0x0f],
        'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0f, 0x10, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x1f],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x12, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1b, 0x11],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1e, 0x01, 0x01, 0x0e, 0x01, 0x01, 0x1e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x10, 0x1e, 0x01, 0x01, 0x1e],
        '6' => [0x0e, 0x10, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x01, 0x0e],
        '-' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f],
        '·' => [0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00],
        _ => [0x00; 7],
    }
}

fn project_label(name: &str) -> String {
    let source = name
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(name);
    let mut label = source
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else if matches!(character, '-' | '_' | '.') {
                character
            } else {
                '·'
            }
        })
        .collect::<String>();
    if label.is_empty() {
        label = "OFFICE".to_string();
    }
    label
}
fn worker_plate_name(worker_name: &str, office_name: &str) -> String {
    let worker_name = worker_name.trim();
    let prefixes = [
        office_name,
        office_name.trim_matches('/'),
        office_name
            .rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or(office_name),
    ];
    for prefix in prefixes {
        if prefix.is_empty() {
            continue;
        }
        let Some(suffix) = worker_name.strip_prefix(prefix) else {
            continue;
        };
        let suffix = suffix.trim_start_matches(|character: char| {
            matches!(character, '/' | '#' | ':' | '-' | '_' | ' ' | '·')
        });
        if !suffix.is_empty() {
            return suffix.to_string();
        }
    }
    worker_name.to_string()
}

fn unique_plate_name(workers: &[&Worker], slot: usize, office_name: &str, width: usize) -> String {
    let names = workers
        .iter()
        .map(|worker| worker_plate_name(&worker.name, office_name))
        .collect::<Vec<_>>();
    let Some(name) = names.get(slot) else {
        return String::new();
    };
    let shortened = short_path(name, width);
    let collisions = names
        .iter()
        .enumerate()
        .filter(|(_, other)| short_path(other, width) == shortened)
        .collect::<Vec<_>>();
    if collisions.len() < 2 {
        return shortened;
    }
    let common = name
        .chars()
        .enumerate()
        .take_while(|(index, character)| {
            collisions
                .iter()
                .all(|(_, other)| other.chars().nth(*index) == Some(*character))
        })
        .count();
    let suffix = name.chars().skip(common).collect::<String>();
    let suffix = suffix.trim_start_matches([' ', '-', '/', ':', '_']);
    if !suffix.is_empty() {
        let chars = suffix.chars().collect::<Vec<_>>();
        let result = if chars.len() + 5 < width {
            format!(
                "{} {}",
                short_path(name, width.saturating_sub(chars.len() + 1)),
                suffix
            )
        } else if chars.len() <= width {
            suffix.to_owned()
        } else {
            format!(
                "…{}",
                chars[chars.len().saturating_sub(width.saturating_sub(1))..]
                    .iter()
                    .collect::<String>()
            )
        };
        if collisions
            .iter()
            .filter(|(_, other)| *other == name)
            .count()
            == 1
        {
            return result;
        }
    }
    let hash = workers[slot]
        .id
        .0
        .bytes()
        .fold(2166136261u32, |hash, byte| {
            (hash ^ byte as u32).wrapping_mul(16777619)
        });
    let tag = format!("#{:06x}", hash & 0xffffff);
    format!(
        "{}{}",
        short_path(name, width.saturating_sub(tag.len())),
        tag
    )
}

/// Idle workers occasionally shift along their own desk's aisle. The excursion
/// is bounded by that station, deterministic by identity, and returns to zero
/// when reduced motion is enabled. It never implies an active model turn.
fn idle_step(worker: &Worker, now: Millis, animation_now: Millis, width: usize) -> (i32, bool) {
    if animation_now == 0 || worker_status(worker, now) != WorkerStatus::Idle {
        return (0, false);
    }
    let seed = worker
        .id
        .0
        .bytes()
        .fold(14695981039346656037u64, |hash, byte| {
            (hash ^ byte as u64).wrapping_mul(1099511628211)
        });
    let phase = (animation_now.max(0) as u64 + seed % 26000) % 26000;
    let fraction = match phase {
        8000..=11999 => phase - 8000,
        12000..=15999 => 4000,
        16000..=19999 => 20000 - phase,
        _ => 0,
    };
    let offset =
        (width as u64 * fraction / 12000) as i32 * if seed.is_multiple_of(2) { 1 } else { -1 };
    (
        offset,
        (8000..12000).contains(&phase) || (16000..20000).contains(&phase),
    )
}

#[cfg(test)]
fn sign_lines(label: &str) -> Vec<String> {
    let chars = label.chars().collect::<Vec<_>>();
    if chars.len() <= 16 {
        return vec![label.to_string()];
    }
    let preferred = chars.len() / 2;
    let split = (1..chars.len())
        .filter(|index| matches!(chars[index - 1], '-' | '_' | '.'))
        .min_by_key(|index| index.abs_diff(preferred))
        .unwrap_or(preferred);
    let first = chars[..split].iter().collect::<String>();
    let second = chars[split..].iter().collect::<String>();
    vec![first, second]
}

fn blit_scaled_signed(
    canvas: &mut Canvas,
    sprite: &Sprite,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
) {
    if width == 0 || height == 0 || sprite.width() == 0 || sprite.height() == 0 {
        return;
    }
    for dy in 0..height {
        let py = y + dy as i32;
        if py < 0 || py >= canvas.height() as i32 {
            continue;
        }
        let sy = dy.saturating_mul(sprite.height()) / height;
        for dx in 0..width {
            let px = x + dx as i32;
            if px < 0 || px >= canvas.width() as i32 {
                continue;
            }
            let sx = dx.saturating_mul(sprite.width()) / width;
            if let Some(color) = sprite.pixel(sx, sy) {
                canvas.set(px as usize, py as usize, color);
            }
        }
    }
}

fn blit_floor_worker(
    canvas: &mut Canvas,
    sprite: &Sprite,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
) {
    debug_assert!(width.is_multiple_of(sprite.width()));
    debug_assert!(height.is_multiple_of(sprite.height()));
    blit_scaled_signed(canvas, sprite, x, y, width, height);
}

fn lerp_point(
    start: (i32, i32),
    end: (i32, i32),
    numerator: usize,
    denominator: usize,
) -> (i32, i32) {
    if denominator == 0 {
        return start;
    }
    let numerator = numerator as i32;
    let denominator = denominator as i32;
    (
        start.0 + (end.0 - start.0) * numerator / denominator,
        start.1 + (end.1 - start.1) * numerator / denominator,
    )
}
fn draw_floor_grid(canvas: &mut Canvas, grid: IsoGrid) {
    // At room scale these seams make the floor read as a constructed surface;
    // compact views omit them so each remaining pixel can carry a signal.
    if grid.tile_width < 18 {
        return;
    }
    let [back, right, front, left] = floor_corners(grid);
    for index in 1..grid.columns {
        let start = lerp_point(back, right, index, grid.columns);
        let end = lerp_point(left, front, index, grid.columns);
        draw_line(canvas, start.0, start.1, end.0, end.1, FLOOR_DITHER);
    }
    for index in 1..grid.rows {
        let start = lerp_point(back, left, index, grid.rows);
        let end = lerp_point(right, front, index, grid.rows);
        draw_line(canvas, start.0, start.1, end.0, end.1, FLOOR_DITHER);
    }
}
fn draw_floor_plate(canvas: &mut Canvas, grid: IsoGrid) {
    // Fill the complete quadrilateral once. Separately rasterized diamonds
    // leave one-pixel cracks when an odd tile width meets an even tile height.
    fill_polygon(canvas, &floor_corners(grid), FLOOR);
    draw_floor_grid(canvas, grid);
    draw_floor_edges(canvas, grid);
}

fn draw_table(canvas: &mut Canvas, grid: IsoGrid, center_x: i32, center_y: i32) {
    let half_width = (grid.tile_width / 5).max(2);
    let half_height = (grid.tile_height / 2).max(1);
    draw_diamond(
        canvas,
        center_x,
        center_y - 1,
        half_width,
        half_height,
        TABLE_TOP,
        TABLE_LIGHT,
    );
    draw_line(
        canvas,
        center_x - half_width / 2,
        center_y,
        center_x - half_width / 2,
        center_y + grid.tile_height / 2 + 1,
        outline_color(canvas),
    );
    draw_line(
        canvas,
        center_x + half_width / 2,
        center_y,
        center_x + half_width / 2,
        center_y + grid.tile_height / 2 + 1,
        outline_color(canvas),
    );
}

fn draw_rug(canvas: &mut Canvas, grid: IsoGrid, center_x: i32, center_y: i32) {
    let half_width = (grid.tile_width * 9 / 20).max(3);
    let half_height = (grid.tile_height * 9 / 20).max(2);
    fill_diamond(
        canvas,
        center_x,
        center_y,
        half_width,
        half_height,
        RUG,
        RUG,
    );
    let top = (center_x, center_y - half_height);
    let right = (center_x + half_width, center_y);
    let bottom = (center_x, center_y + half_height);
    let left = (center_x - half_width, center_y);
    draw_line(canvas, top.0, top.1, left.0, left.1, RUG_BORDER);
    draw_line(canvas, top.0, top.1, right.0, right.1, RUG_BORDER);
    draw_line(canvas, left.0, left.1, bottom.0, bottom.1, RUG_BORDER);
    draw_line(canvas, right.0, right.1, bottom.0, bottom.1, RUG_BORDER);
}

fn draw_chair(canvas: &mut Canvas, grid: IsoGrid, scale: RoomScale, slot: usize) {
    let (tile_x, tile_y) = grid.desk_tile(slot);
    let (center_x, center_y) = grid.center(tile_x, tile_y);
    let (worker_width, worker_height) = scale.worker_size(grid);
    let width = (worker_width * 2 / 3).max(3) as i32;
    let height = (worker_height / 3).max(3) as i32;
    let x = center_x - width / 2;
    let y = center_y - worker_height as i32 * 2 / 3;
    fill_rect(canvas, x, y, width, height, CHAIR_BACK);
    draw_rect_outline(canvas, x, y, width, height, outline_color(canvas));
    draw_line(
        canvas,
        x + 1,
        y + height,
        x + 1,
        center_y + grid.tile_height / 4,
        outline_color(canvas),
    );
    draw_line(
        canvas,
        x + width - 2,
        y + height,
        x + width - 2,
        center_y + grid.tile_height / 4,
        outline_color(canvas),
    );
}

// Drawing one item needs the canvas, where it sits, how big the room is, and
// who is in it; grouping those into a struct would only move the same list.
#[allow(clippy::too_many_arguments)]
fn draw_item(
    canvas: &mut Canvas,
    grid: IsoGrid,
    scale: RoomScale,
    item: IsoItem,
    workers: &[&Worker],
    looks: &[WorkerLook],
    sprites: &SpriteSet,
    now: Millis,
    selected_slot: Option<usize>,
) {
    let (center_x, center_y) = grid.center(item.tile_x, item.tile_y);
    match item.kind {
        IsoKind::Plant => {
            let (width, height) = scale.plant_size(grid);
            blit_scaled_signed(
                canvas,
                &sprites.plant,
                center_x - width as i32 / 2,
                center_y - height as i32,
                width,
                height,
            );
        }
        IsoKind::Cooler => {
            let (width, height) = scale.cooler_size(grid);
            blit_scaled_signed(
                canvas,
                &sprites.water_cooler,
                center_x - width as i32 / 2,
                center_y - height as i32,
                width,
                height,
            );
        }
        IsoKind::MeetingTable => draw_table(canvas, grid, center_x, center_y),
        IsoKind::Chair(slot) => draw_chair(canvas, grid, scale, slot),
        IsoKind::Worker(index) => {
            let Some(worker) = workers.get(index) else {
                return;
            };
            let Some(look) = looks.get(index) else {
                return;
            };
            let (x, y, width, height) = worker_bounds(grid, scale, index);
            let stretch =
                if grid.encoding == PixelEncoding::Quadrants && grid.pixels_per_cell == (2, 2) {
                    2
                } else {
                    1
                };
            let (step, walking) = idle_step(worker, now, grid.animation_now.unwrap_or(now), width);
            let x = x + step;
            let sprite = if !walking {
                sprites.worker_frame_fitting(worker, *look, now, width / stretch, height)
            } else {
                sprites.worker_stroll_frame_fitting(worker, *look, now, width / stretch, height)
            };
            if scale == RoomScale::Floor {
                blit_floor_worker(canvas, &sprite, x, y, width, height);
            } else {
                blit_scaled_signed(canvas, &sprite, x, y, width, height);
            }
        }
        IsoKind::Manager => {
            let (x, y, width, height, attention) =
                manager_bounds(grid, scale, (item.tile_x, item.tile_y), now);
            let stretch =
                if grid.encoding == PixelEncoding::Quadrants && grid.pixels_per_cell == (2, 2) {
                    2
                } else {
                    1
                };
            let sprite = sprites.manager_frame_fitting(attention, now, width / stretch, height);
            blit_floor_worker(canvas, &sprite, x, y, width, height);
            if attention {
                set_pixel(canvas, x + width as i32 + 1, y, WARNING);
            }
        }
        IsoKind::Desk(slot) => {
            let (desk_x, desk_y, width, height) = desk_bounds(grid, scale, slot);
            blit_scaled_signed(canvas, &sprites.desk, desk_x, desk_y, width, height);
            let (monitor_width, monitor_height) = scale.monitor_size(grid);
            let monitor_x = center_x - width as i32 / 4 - monitor_width as i32 / 2;
            let monitor_y = desk_y - monitor_height as i32 + grid.scale_half_height(2) as i32;
            blit_scaled_signed(
                canvas,
                &sprites.monitor,
                monitor_x,
                monitor_y,
                monitor_width,
                monitor_height,
            );
            draw_nameplate_chip(
                canvas,
                grid,
                slot,
                workers,
                now,
                selected_slot == Some(slot),
            );
        }
    }
}

pub(crate) fn draw_room_scene(
    canvas: &mut Canvas,
    office_name: &str,
    visible_workers: &[&Worker],
    looks: &[WorkerLook],
    sprites: &SpriteSet,
    now: Millis,
    selected_slot: Option<usize>,
) -> IsoGrid {
    canvas.clear();
    let scale = RoomScale::Floor;
    let mut grid = make_room_grid(canvas, visible_workers.len());
    grid.animation_now = Some(sprites.animation_now(now));
    draw_isometric_backdrop(canvas, grid, grid.animation_now.unwrap_or(now));
    let [back, ..] = floor_corners(grid);
    let wall_top = back.1 - grid.wall_height;
    draw_flat_project_sign(
        canvas,
        &project_label(office_name),
        wall_top.max(0) as usize,
    );
    draw_floor_plate(canvas, grid);

    let lounge_tile = if visible_workers.len() <= 3 {
        (0, grid.rows - 1)
    } else {
        ISO_RUG_TILE
    };
    let (rug_x, rug_y) = grid.center(lounge_tile.0, lounge_tile.1);
    draw_rug(canvas, grid, rug_x, rug_y);

    let mut items = Vec::with_capacity(visible_workers.len().saturating_mul(3) + 3);
    for (slot, _) in visible_workers.iter().enumerate() {
        let (tile_x, tile_y) = grid.desk_tile(slot);
        items.push(IsoItem {
            tile_x,
            tile_y,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 1,
            },
            kind: IsoKind::Chair(slot),
        });
        items.push(IsoItem {
            tile_x,
            tile_y,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 2,
            },
            kind: IsoKind::Worker(slot),
        });
        items.push(IsoItem {
            tile_x,
            tile_y,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 1,
            },
            kind: IsoKind::Desk(slot),
        });
    }

    items.extend([
        IsoItem {
            tile_x: ISO_PLANT_TILE.0,
            tile_y: ISO_PLANT_TILE.1,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 2,
            },
            kind: IsoKind::Plant,
        },
        IsoItem {
            tile_x: ISO_COOLER_TILE.0,
            tile_y: ISO_COOLER_TILE.1,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 2,
            },
            kind: IsoKind::Cooler,
        },
        IsoItem {
            tile_x: lounge_tile.0,
            tile_y: lounge_tile.1,
            footprint: IsoFootprint {
                width: 2,
                depth: 1,
                height: 1,
            },
            kind: IsoKind::MeetingTable,
        },
    ]);

    if let Some((slot, _)) = visible_workers
        .iter()
        .enumerate()
        .find(|(_, worker)| worker_status(worker, now) == WorkerStatus::Blocked)
    {
        let (tile_x, tile_y) = manager_tile(grid, visible_workers.len(), slot);
        items.push(IsoItem {
            tile_x,
            tile_y,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 2,
            },
            kind: IsoKind::Manager,
        });
    }

    painter_order(&mut items);
    for item in items {
        draw_item(
            canvas,
            grid,
            scale,
            item,
            visible_workers,
            looks,
            sprites,
            now,
            selected_slot,
        );
    }
    grid
}

/// Geometry is planned in terminal cells before converting to the selected
/// pixel density. A larger window adds room around furniture, not empty desks.
#[derive(Clone, Copy, Debug)]
struct Station {
    center_x: usize,
    worker_y: usize,
    worker_width: usize,
    worker_height: usize,
    desk_y: usize,
    desk_width: usize,
    desk_height: usize,
    label_y: usize,
    label_width: usize,
}

#[derive(Debug)]
struct FlatRoom {
    top: usize,
    bottom: usize,
    wall_height: usize,
    floor_line: usize,
    lounge_y: Option<usize>,
    stations: Vec<Station>,
}

fn character_dimensions(canvas: &Canvas, width: usize, height: usize) -> (usize, usize) {
    figure_dimensions(canvas.encoding(), canvas.pixels_per_cell(), width, height)
}

fn figure_dimensions(
    encoding: PixelEncoding,
    density: (usize, usize),
    width: usize,
    height: usize,
) -> (usize, usize) {
    let stretch_x = if encoding == PixelEncoding::Quadrants && density == (2, 2) {
        2
    } else {
        1
    };
    let (source_width, source_height) = [(24, 34), (14, 20), (7, 10)]
        .into_iter()
        .find(|(w, h)| w * stretch_x <= width && *h <= height)
        .unwrap_or((7, 10));
    let zoom = (width / (source_width * stretch_x))
        .min(height / source_height)
        .max(1);
    (source_width * stretch_x * zoom, source_height * zoom)
}

impl FlatRoom {
    fn new(canvas: &Canvas, count: usize, layout: OfficeLayout, projection: Projection) -> Self {
        let (px, py) = canvas.pixels_per_cell();
        let width = canvas.width();
        let height = canvas
            .height()
            .min((width / px / 3 + 16).clamp(28, 48) * py);
        let top = canvas.height().saturating_sub(height) / 2;
        let side = projection == Projection::Side;
        let columns = if side {
            count.max(1)
        } else {
            layout.columns.max(1).min(count.max(1))
        };
        let rows = count.max(1).div_ceil(columns);
        // A shallow wall cap leaves the top view's floor usable even at 80x24.
        let wall_height = (height / 7).clamp(py * 2, py * 6).min(height / 3);
        let floor_line = top
            + if side {
                height.saturating_sub((height / 8).max(py * 2))
            } else {
                wall_height
            };
        let lounge = !side && rows == 1 && height / py >= 36;
        let floor_height = height.saturating_sub(wall_height + py);
        let station_height = if side {
            floor_line.saturating_sub(top + wall_height)
        } else if lounge {
            floor_height * 3 / 5
        } else {
            floor_height / rows
        };
        let margin = if width / px >= 110 { px * 5 } else { px };
        let slot_width = width.saturating_sub(margin * 2) / columns;
        let mut stations = Vec::with_capacity(count);
        for slot in 0..count {
            let row = slot / columns;
            let row_count = (count - row * columns).min(columns);
            let row_width = slot_width * row_count;
            let center_x = width.saturating_sub(row_width) / 2
                + (slot % columns) * slot_width
                + slot_width / 2;
            let budget_height = if side {
                station_height.min(height * 2 / 3) * 2 / 3
            } else {
                station_height.saturating_sub(py * 2) * 4 / 5
            };
            let (worker_width, worker_height) =
                character_dimensions(canvas, slot_width.saturating_sub(px * 4) / 2, budget_height);
            let desk_width = (worker_width * 5 / 2)
                .min(slot_width.saturating_sub(px * 2))
                .max(px * 6);
            let desk_height = (worker_height / 3).max(py * 2);
            let label_y = if side {
                floor_line + py / 2
            } else {
                top + wall_height + (row + 1) * station_height - py
            }
            .min((top + height).saturating_sub(py));
            let desk_y = label_y.saturating_sub(desk_height + py);
            let worker_y = desk_y.saturating_sub(worker_height * 2 / 3);
            stations.push(Station {
                center_x,
                worker_y,
                worker_width,
                worker_height,
                desk_y,
                desk_width,
                desk_height,
                label_y,
                label_width: (slot_width / px).saturating_sub(2).min(28),
            });
        }
        Self {
            top,
            bottom: top + height,
            wall_height,
            floor_line,
            lounge_y: lounge.then_some(top + wall_height + station_height + py * 2),
            stations,
        }
    }
}

fn draw_flat_project_sign(canvas: &mut Canvas, label: &str, wall_height: usize) {
    draw_room_project_sign(canvas, label, 0, wall_height);
}

fn draw_room_project_sign(canvas: &mut Canvas, label: &str, top: usize, wall_height: usize) {
    let (px, py) = canvas.pixels_per_cell();
    // Fixed lettering tiers: never scale type into the room's unused height.
    let available_height = wall_height
        .saturating_sub(py)
        .min(py * 4)
        .min((canvas.height() * 12 / 100).max(5));
    let available_width = canvas.width().saturating_sub(px * 8).min(px * 64);
    if available_height < 5 || available_width < 3 {
        return;
    }
    let compact = available_height < 7 || label.chars().count() * 6 > available_width;
    let (gw, gh) = if compact { (3, 5) } else { (5, 7) };
    let letters = label
        .chars()
        .take((available_width + 1) / (gw + 1))
        .collect::<Vec<_>>();
    let face_width = letters.len().saturating_mul(gw + 1).saturating_sub(1);
    if face_width == 0 {
        return;
    }
    let scale = (available_height / gh)
        .min(available_width / face_width)
        .max(1);
    let x = (canvas.width() - face_width * scale) / 2;
    let y = top + wall_height.saturating_sub(gh * scale) / 2;
    // The native attention banner owns row 1 and its first 36 cells. Keep
    // this space reserved even when no alert is present, so a state change
    // cannot erase part of the lettering. The native FLOOR header already
    // identifies the project when the artwork title cannot fit beside it.
    let sign_left = x.saturating_sub(px * 2);
    let sign_top = y.saturating_sub(1);
    let sign_bottom = y.saturating_add(gh * scale).saturating_add(1);
    if sign_left < px * 36 && sign_top < py * 2 && sign_bottom > py {
        return;
    }
    fill_rect(
        canvas,
        x.saturating_sub(px * 2) as i32,
        y.saturating_sub(1) as i32,
        (face_width * scale + px * 4) as i32,
        (gh * scale + 2) as i32,
        PANEL,
    );
    for (index, letter) in letters.into_iter().enumerate() {
        let glyph = if compact {
            let short = compact_glyph(letter);
            [short[0], short[1], short[2], short[3], short[4], 0, 0]
        } else {
            glyph_5x7(letter)
        };
        for (row, bits) in glyph.iter().take(gh).enumerate() {
            for column in 0..gw {
                if bits & (1 << (gw - 1 - column)) != 0 {
                    fill_rect(
                        canvas,
                        (x + (index * (gw + 1) + column) * scale) as i32,
                        (y + row * scale) as i32,
                        scale as i32,
                        scale as i32,
                        TITLE_COLOR,
                    );
                }
            }
        }
    }
}

fn draw_flat_floor(canvas: &mut Canvas, start: usize, bottom: usize) {
    let (px, py) = canvas.pixels_per_cell();
    fill_rect(
        canvas,
        0,
        start as i32,
        canvas.width() as i32,
        bottom.saturating_sub(start) as i32,
        FLOOR_LIGHT,
    );
    let spacing = (py * 5).max(4);
    for y in (start..bottom).step_by(spacing) {
        draw_line(
            canvas,
            0,
            y as i32,
            canvas.width() as i32 - 1,
            y as i32,
            FLOOR_DITHER,
        );
        for x in ((y / spacing % 2) * px * 12..canvas.width()).step_by((px * 24).max(1)) {
            draw_line(
                canvas,
                x as i32,
                y as i32,
                x as i32,
                (y + spacing).min(bottom - 1) as i32,
                FLOOR_DITHER,
            );
        }
    }
    fill_rect(
        canvas,
        0,
        start as i32,
        canvas.width() as i32,
        py.max(1) as i32,
        FLOOR_DARK,
    );
}

fn draw_bookcase(canvas: &mut Canvas, x: usize, y: usize, width: usize, height: usize) {
    if width < 5 || height < 6 {
        return;
    }
    fill_rect(
        canvas,
        x as i32,
        y as i32,
        width as i32,
        height as i32,
        TABLE_TOP,
    );
    let shelf = (height / 3).max(2);
    for row in 0..3 {
        let top = y + row * shelf + 1;
        for book in 0..5 {
            let bw = (width.saturating_sub(4) / 5).max(1);
            let color = [RUG, WINDOW_LIGHT, FLOOR_DITHER, ACCENT, CHAIR_BACK][book];
            fill_rect(
                canvas,
                (x + 2 + book * bw) as i32,
                (top + book % 2) as i32,
                bw.saturating_sub(1).max(1) as i32,
                shelf.saturating_sub(2 + book % 2).max(1) as i32,
                color,
            );
        }
    }
}

fn draw_potted_plant(canvas: &mut Canvas, x: usize, y: usize, width: usize, height: usize) {
    let stem = (width / 6).max(1);
    let center = x + width / 2;
    let green = Color::Rgb(91, 185, 123);
    fill_rect(
        canvas,
        center as i32,
        y as i32,
        stem as i32,
        (height * 3 / 4) as i32,
        green,
    );
    for (side, level) in [(false, 1), (true, 2), (false, 3)] {
        let leaf_x = if side {
            center
        } else {
            center.saturating_sub(width / 3)
        };
        fill_rect(
            canvas,
            leaf_x as i32,
            (y + height * level / 6) as i32,
            (width / 3 + stem) as i32,
            (height / 6).max(1) as i32,
            green,
        );
    }
    fill_rect(
        canvas,
        (x + width / 4) as i32,
        (y + height * 3 / 4) as i32,
        (width / 2).max(2) as i32,
        (height / 4).max(2) as i32,
        TABLE_LIGHT,
    );
}

fn draw_lounge(canvas: &mut Canvas, top: usize, bottom: usize) {
    let (px, py) = canvas.pixels_per_cell();
    let height = bottom.saturating_sub(top + py * 2);
    if height < py * 5 {
        return;
    }
    let width = (canvas.width() / 2).min(px * 62);
    let left = (canvas.width() - width) / 2;
    let sofa_w = width / 3;
    let sofa_h = (height / 2).min(py * 7);
    let y = top + (height - sofa_h) / 2;
    fill_rect(
        canvas,
        left as i32,
        top as i32,
        width as i32,
        height as i32,
        RUG_BORDER,
    );
    fill_rect(
        canvas,
        (left + px) as i32,
        (top + py / 2) as i32,
        width.saturating_sub(px * 2) as i32,
        height.saturating_sub(py) as i32,
        RUG,
    );
    for x in [left + px * 2, left + width - sofa_w - px * 2] {
        fill_rect(
            canvas,
            x as i32,
            y as i32,
            sofa_w as i32,
            sofa_h as i32,
            CHAIR_BACK,
        );
        fill_rect(
            canvas,
            (x + px) as i32,
            (y + py) as i32,
            sofa_w.saturating_sub(px * 2) as i32,
            sofa_h.saturating_sub(py * 2) as i32,
            WALL_LIGHT,
        );
    }
    let table_w = width / 5;
    fill_rect(
        canvas,
        (canvas.width() / 2 - table_w / 2) as i32,
        (y + sofa_h / 3) as i32,
        table_w as i32,
        (sofa_h / 2).max(2) as i32,
        TABLE_LIGHT,
    );
    draw_potted_plant(
        canvas,
        left.saturating_sub(px * 9),
        y,
        px * 6,
        sofa_h.max(py * 5),
    );
    draw_bookcase(
        canvas,
        (left + width + px * 3).min(canvas.width().saturating_sub(px * 9)),
        y,
        px * 7,
        sofa_h,
    );
}

fn draw_station(
    canvas: &mut Canvas,
    station: Station,
    worker: &Worker,
    look: WorkerLook,
    sprites: &SpriteSet,
    now: Millis,
) {
    let (px, py) = canvas.pixels_per_cell();
    let Station {
        center_x,
        worker_y,
        worker_width,
        worker_height,
        desk_y,
        desk_width,
        desk_height,
        ..
    } = station;
    let chair_x = center_x.saturating_sub(worker_width / 2);
    fill_rect(
        canvas,
        chair_x as i32,
        (worker_y + worker_height / 3) as i32,
        worker_width as i32,
        (worker_height * 2 / 3) as i32,
        CHAIR_BACK,
    );
    let stretch = if canvas.encoding() == PixelEncoding::Quadrants && !canvas.has_image_density() {
        2
    } else {
        1
    };
    let (step, walking) = idle_step(worker, now, sprites.animation_now(now), worker_width);
    let sprite = if !walking {
        sprites.worker_frame_fitting(worker, look, now, worker_width / stretch, worker_height)
    } else {
        sprites.worker_stroll_frame_fitting(
            worker,
            look,
            now,
            worker_width / stretch,
            worker_height,
        )
    };
    blit_scaled_signed(
        canvas,
        &sprite,
        chair_x as i32 + step,
        worker_y as i32,
        worker_width,
        worker_height,
    );
    let desk_x = center_x.saturating_sub(desk_width / 2);
    let top_h = (desk_height / 3).max(1);
    fill_rect(
        canvas,
        desk_x as i32,
        desk_y as i32,
        desk_width as i32,
        top_h as i32,
        TABLE_LIGHT,
    );
    fill_rect(
        canvas,
        desk_x as i32,
        (desk_y + top_h) as i32,
        desk_width as i32,
        py.max(1) as i32,
        TABLE_TOP,
    );
    for x in [desk_x + px, desk_x + desk_width.saturating_sub(px * 2)] {
        fill_rect(
            canvas,
            x as i32,
            (desk_y + top_h + py) as i32,
            px.max(1) as i32,
            desk_height.saturating_sub(top_h + py).max(1) as i32,
            TABLE_TOP,
        );
    }
    let mw = (worker_width * 3 / 4).max(px * 3);
    let mh = (worker_height / 4).max(py * 2);
    let mx = desk_x + px;
    let my = desk_y.saturating_sub(mh);
    fill_rect(
        canvas,
        mx as i32,
        my as i32,
        mw as i32,
        mh as i32,
        WINDOW_FRAME,
    );
    fill_rect(
        canvas,
        (mx + px.max(1)) as i32,
        (my + 1) as i32,
        mw.saturating_sub(px * 2).max(1) as i32,
        mh.saturating_sub(2).max(1) as i32,
        WINDOW_LIGHT,
    );
    draw_line(
        canvas,
        (mx + px) as i32,
        (my + mh / 2) as i32,
        (mx + mw * 2 / 3) as i32,
        (my + mh / 2) as i32,
        INK,
    );
    fill_rect(
        canvas,
        (center_x + desk_width / 4) as i32,
        desk_y.saturating_sub(py) as i32,
        (px * 2) as i32,
        py as i32,
        FLOOR_LIGHT,
    );
}

fn draw_top_down_scene(
    canvas: &mut Canvas,
    office: &Office,
    workers: &[&Worker],
    looks: &[WorkerLook],
    sprites: &SpriteSet,
    now: Millis,
    layout: OfficeLayout,
) {
    canvas.fill(super::BACKGROUND);
    let plan = FlatRoom::new(canvas, workers.len(), layout, Projection::TopDown);
    fill_rect(
        canvas,
        0,
        plan.top as i32,
        canvas.width() as i32,
        (plan.bottom - plan.top) as i32,
        WALL,
    );
    draw_flat_floor(canvas, plan.top + plan.wall_height, plan.bottom);
    draw_room_project_sign(
        canvas,
        &project_label(&office.name),
        plan.top,
        plan.wall_height,
    );
    let (px, py) = canvas.pixels_per_cell();
    // Perimeter walls and a shared floor establish one room, never a cell per person.
    fill_rect(
        canvas,
        0,
        (plan.top + plan.wall_height) as i32,
        px.max(1) as i32,
        plan.bottom.saturating_sub(plan.top + plan.wall_height) as i32,
        WALL,
    );
    fill_rect(
        canvas,
        canvas.width().saturating_sub(px) as i32,
        (plan.top + plan.wall_height) as i32,
        px.max(1) as i32,
        plan.bottom.saturating_sub(plan.top + plan.wall_height) as i32,
        WALL,
    );
    if let Some(top) = plan.lounge_y {
        draw_lounge(canvas, top, plan.bottom);
    }
    for ((station, worker), look) in plan.stations.iter().zip(workers).zip(looks) {
        draw_station(canvas, *station, worker, *look, sprites, now);
    }
    if canvas.width() / px >= 100 {
        let prop_h = (canvas.height() / 7).max(py * 4).min(py * 9);
        draw_potted_plant(
            canvas,
            px * 2,
            plan.top + plan.wall_height + py,
            px * 6,
            prop_h,
        );
        canvas.blit_scaled(
            &sprites.water_cooler,
            canvas.width().saturating_sub(px * 9),
            plan.top + plan.wall_height + py,
            px * 5,
            prop_h,
        );
    }
}

fn draw_side_scene(
    canvas: &mut Canvas,
    office: &Office,
    workers: &[&Worker],
    looks: &[WorkerLook],
    sprites: &SpriteSet,
    now: Millis,
) {
    let layout = OfficeLayout {
        columns: workers.len().max(1),
        rows: 1,
        page_size: workers.len(),
        pages: 1,
    };
    let plan = FlatRoom::new(canvas, workers.len(), layout, Projection::Side);
    canvas.fill(super::BACKGROUND);
    fill_rect(
        canvas,
        0,
        plan.top as i32,
        canvas.width() as i32,
        (plan.bottom - plan.top) as i32,
        WALL,
    );
    let (px, py) = canvas.pixels_per_cell();
    let window_top = plan.top + plan.wall_height + py;
    let worker_top = plan
        .stations
        .iter()
        .map(|s| s.worker_y)
        .min()
        .unwrap_or(plan.floor_line / 2);
    let window_height = worker_top
        .saturating_sub(window_top)
        .max(py * 3)
        .min(py * 16);
    let window_count = if canvas.width() / px >= 110 { 3 } else { 2 };
    let window_width = canvas.width() * 2 / 3 / window_count;
    let total = window_width * window_count;
    let left = (canvas.width() - total) / 2;
    for index in 0..window_count {
        draw_window(
            canvas,
            (left + index * window_width + px) as i32,
            window_top as i32,
            window_width.saturating_sub(px * 2) as i32,
            window_height as i32,
            sprites.animation_now(now) + index as Millis * 1700,
        );
    }
    draw_room_project_sign(
        canvas,
        &project_label(&office.name),
        plan.top,
        plan.wall_height,
    );
    draw_flat_floor(canvas, plan.floor_line.saturating_sub(py), plan.bottom);
    let prop_h = plan
        .stations
        .first()
        .map(|s| s.worker_height)
        .unwrap_or(py * 8);
    let prop_y = plan.floor_line.saturating_sub(prop_h);
    if canvas.width() / px >= 100 {
        draw_bookcase(canvas, px * 2, prop_y, px * 9, prop_h);
        draw_potted_plant(
            canvas,
            canvas.width().saturating_sub(px * 10),
            prop_y,
            px * 7,
            prop_h,
        );
    }
    for ((station, worker), look) in plan.stations.iter().zip(workers).zip(looks) {
        draw_station(canvas, *station, worker, *look, sprites, now);
    }
}
fn draw_list_scene(canvas: &mut Canvas) {
    // The list view is intentionally quiet: the text rows carry identity and
    // status, while the pixel canvas simply clears the room behind them.
    canvas.fill(super::BACKGROUND);
}
fn status_label(office: &Office, now: Millis) -> String {
    let blocked = office
        .workers
        .iter()
        .filter(|worker| worker_status(worker, now) == WorkerStatus::Blocked)
        .count();
    let failed = office
        .workers
        .iter()
        .filter(|worker| worker_status(worker, now) == WorkerStatus::Failed)
        .count();
    if blocked > 0 && failed > 0 {
        format!("! {blocked} blocked • × {failed} failed • manager responding")
    } else if blocked > 0 {
        format!("! {blocked} blocked • manager responding")
    } else if failed > 0 {
        format!("× {failed} failed • floor needs attention")
    } else if office
        .workers
        .iter()
        .any(|worker| worker_status(worker, now) == WorkerStatus::Running)
    {
        "running • all desks live".to_string()
    } else {
        "idle • floor quiet".to_string()
    }
}
fn draw_list_rows(
    frame: &mut Frame,
    body: Rect,
    office_name: &str,
    workers: &[&Worker],
    start: usize,
    selected: usize,
    now: Millis,
) {
    if !has_area(body) {
        return;
    }
    for (slot, worker) in workers.iter().enumerate() {
        if slot >= usize::from(body.height) {
            break;
        }
        let row = Rect::new(body.x, body.y.saturating_add(slot as u16), body.width, 1);
        let status = worker_status(worker, now);
        let status_word = match status {
            WorkerStatus::Running => "RUNNING",
            WorkerStatus::Idle => "IDLE",
            WorkerStatus::Blocked => "BLOCKED",
            WorkerStatus::Failed => "FAILED",
        };
        let marker = if start + slot == selected {
            ">"
        } else if status == WorkerStatus::Blocked {
            "!"
        } else if status == WorkerStatus::Failed {
            "×"
        } else {
            " "
        };
        let name_width = usize::from(body.width)
            .saturating_sub(status_word.len().saturating_add(4))
            .max(1);
        let name = short_path(&worker_plate_name(&worker.name, office_name), name_width);
        let label = format!("{marker} {name}  {status_word}");
        let style = if start + slot == selected {
            Style::default().fg(INK).bg(PANEL_HIGHLIGHT)
        } else {
            Style::default().fg(status_color(status)).bg(PANEL)
        };
        paint_opaque(frame, row, style);
        Paragraph::new(Line::from(Span::styled(label, style)))
            .style(style)
            .render(row, frame.buffer_mut());
    }
}

fn physical_to_cell(value: i32, scale: usize) -> i32 {
    value.div_euclid(scale.max(1) as i32)
}

fn worker_plate_position(grid: IsoGrid, slot: usize) -> (i32, i32) {
    let (center_x, _) = grid.center(grid.desk_tile(slot).0, grid.desk_tile(slot).1);
    let (_, desk_y, _, desk_height) = desk_bounds(grid, RoomScale::Floor, slot);
    let plate_y = desk_y
        .saturating_add(desk_height as i32)
        .saturating_add(grid.scale_half_height(1) as i32);
    (
        physical_to_cell(center_x, grid.pixels_per_cell.0),
        physical_to_cell(plate_y, grid.pixels_per_cell.1),
    )
}

fn worker_plate_width(body: Rect, grid: IsoGrid) -> u16 {
    let tile_width_cells = physical_to_cell(grid.tile_width, grid.pixels_per_cell.0).max(12) as u16;
    (tile_width_cells / 2).max(12).min(body.width)
}

fn scene_cell_rect(body: Rect, grid: IsoGrid, x: i32, y: i32, width: usize, height: usize) -> Rect {
    let x_scale = grid.pixels_per_cell.0.max(1) as i32;
    let y_scale = grid.pixels_per_cell.1.max(1) as i32;
    let left = x.div_euclid(x_scale).max(0) as u16;
    let top = y.div_euclid(y_scale).max(0) as u16;
    let right = (x + width as i32)
        .max(0)
        .saturating_add(x_scale - 1)
        .div_euclid(x_scale) as u16;
    let bottom = (y + height as i32)
        .max(0)
        .saturating_add(y_scale - 1)
        .div_euclid(y_scale) as u16;
    Rect::new(
        body.x.saturating_add(left),
        body.y.saturating_add(top),
        right.saturating_sub(left),
        bottom.saturating_sub(top),
    )
}

fn rects_overlap(first: Rect, second: Rect) -> bool {
    first.x < second.x.saturating_add(second.width)
        && second.x < first.x.saturating_add(first.width)
        && first.y < second.y.saturating_add(second.height)
        && second.y < first.y.saturating_add(first.height)
}

fn expanded_rect(rect: Rect, body: Rect, margin: u16) -> Rect {
    let left = rect.x.saturating_sub(margin).max(body.x);
    let top = rect.y.saturating_sub(margin).max(body.y);
    let right = rect
        .x
        .saturating_add(rect.width)
        .saturating_add(margin)
        .min(body.x.saturating_add(body.width));
    let bottom = rect
        .y
        .saturating_add(rect.height)
        .saturating_add(margin)
        .min(body.y.saturating_add(body.height));
    Rect::new(
        left,
        top,
        right.saturating_sub(left),
        bottom.saturating_sub(top),
    )
}

fn blocked_slot(workers: &[&Worker], now: Millis) -> Option<usize> {
    workers
        .iter()
        .position(|worker| worker_status(worker, now) == WorkerStatus::Blocked)
}

fn plate_obstacles(
    body: Rect,
    grid: IsoGrid,
    owner: usize,
    worker_count: usize,
    manager_for: Option<usize>,
    now: Millis,
) -> Vec<Rect> {
    let owner_tile = grid.desk_tile(owner);
    let owner_key = painter_key(IsoItem {
        tile_x: owner_tile.0,
        tile_y: owner_tile.1,
        footprint: IsoFootprint {
            width: 1,
            depth: 1,
            height: 1,
        },
        kind: IsoKind::Desk(owner),
    });
    let mut obstacles = Vec::with_capacity(worker_count.saturating_add(1));
    for front in 0..worker_count {
        if front == owner {
            continue;
        }
        let front_tile = grid.desk_tile(front);
        let front_item = IsoItem {
            tile_x: front_tile.0,
            tile_y: front_tile.1,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 2,
            },
            kind: IsoKind::Worker(front),
        };
        if painter_key(front_item) > owner_key {
            let (x, y, width, height) = worker_bounds(grid, RoomScale::Floor, front);
            obstacles.push(expanded_rect(
                scene_cell_rect(body, grid, x, y, width, height),
                body,
                1,
            ));
        }
    }
    if let Some(blocked) = manager_for {
        let tile = manager_tile(grid, worker_count, blocked);
        let manager_item = IsoItem {
            tile_x: tile.0,
            tile_y: tile.1,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 2,
            },
            kind: IsoKind::Manager,
        };
        if painter_key(manager_item) > owner_key {
            let (x, y, width, height, _) = manager_bounds(grid, RoomScale::Floor, tile, now);
            obstacles.push(expanded_rect(
                scene_cell_rect(body, grid, x, y, width, height),
                body,
                1,
            ));
        }
    }
    obstacles
}

fn worker_plate_rect(
    body: Rect,
    grid: IsoGrid,
    slot: usize,
    worker_count: usize,
    manager_for: Option<usize>,
    now: Millis,
) -> Rect {
    let label_width = worker_plate_width(body, grid);
    let (center_x, y) = worker_plate_position(grid, slot);
    let max_x = body.width.saturating_sub(label_width);
    let preferred_x = (center_x - i32::from(label_width) / 2).clamp(0, i32::from(max_x)) as u16;
    let y = y.clamp(0, i32::from(body.height.saturating_sub(1)));
    let obstacles = plate_obstacles(body, grid, slot, worker_count, manager_for, now);
    let x = (0..=max_x)
        .filter(|candidate| {
            let rect = Rect::new(
                body.x.saturating_add(*candidate),
                body.y.saturating_add(y as u16),
                label_width,
                1,
            );
            obstacles
                .iter()
                .all(|obstacle| !rects_overlap(rect, *obstacle))
        })
        .min_by_key(|candidate| candidate.abs_diff(preferred_x))
        .unwrap_or(preferred_x);
    Rect::new(
        body.x.saturating_add(x),
        body.y.saturating_add(y as u16),
        label_width,
        1,
    )
}

fn draw_nameplate_chip(
    canvas: &mut Canvas,
    grid: IsoGrid,
    slot: usize,
    workers: &[&Worker],
    now: Millis,
    selected: bool,
) {
    let cell_width = canvas.width() / grid.pixels_per_cell.0.max(1);
    let cell_height = canvas.height() / grid.pixels_per_cell.1.max(1);
    let body = Rect::new(
        0,
        0,
        u16::try_from(cell_width).unwrap_or(u16::MAX),
        u16::try_from(cell_height).unwrap_or(u16::MAX),
    );
    let rect = worker_plate_rect(
        body,
        grid,
        slot,
        workers.len(),
        blocked_slot(workers, now),
        now,
    );
    fill_rect(
        canvas,
        i32::from(rect.x) * grid.pixels_per_cell.0 as i32,
        i32::from(rect.y) * grid.pixels_per_cell.1 as i32,
        i32::from(rect.width) * grid.pixels_per_cell.0 as i32,
        i32::from(rect.height) * grid.pixels_per_cell.1 as i32,
        if selected { PANEL_HIGHLIGHT } else { PANEL },
    );
}

#[cfg(test)]
fn worker_cell_rect(grid: IsoGrid, slot: usize) -> Rect {
    let (x, y, width, height) = worker_bounds(grid, RoomScale::Floor, slot);
    scene_cell_rect(
        Rect::new(0, 0, u16::MAX, u16::MAX),
        grid,
        x,
        y,
        width,
        height,
    )
}

#[allow(clippy::too_many_arguments)]
fn draw_nameplates(
    frame: &mut Frame,
    body: Rect,
    grid: IsoGrid,
    office_name: &str,
    workers: &[&Worker],
    start: usize,
    selected: usize,
    now: Millis,
) {
    if body.width == 0 || body.height == 0 {
        return;
    }
    let mut slots = (0..workers.len()).collect::<Vec<_>>();
    slots.sort_by_key(|slot| {
        let (tile_x, tile_y) = grid.desk_tile(*slot);
        painter_key(IsoItem {
            tile_x,
            tile_y,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 1,
            },
            kind: IsoKind::Desk(*slot),
        })
    });
    for slot in slots {
        let worker = workers[slot];
        let rect = worker_plate_rect(
            body,
            grid,
            slot,
            workers.len(),
            blocked_slot(workers, now),
            now,
        );
        let mut prefix = " ";
        let status = worker_status(worker, now);
        if start + slot == selected {
            prefix = ">";
        } else if status == WorkerStatus::Blocked {
            prefix = "!";
        } else if status == WorkerStatus::Failed {
            prefix = "×";
        }
        let name = unique_plate_name(
            workers,
            slot,
            office_name,
            usize::from(rect.width.saturating_sub(2)),
        );
        let label = format!("{prefix} {name}");
        let label = format!("{label:<width$}", width = usize::from(rect.width));
        let style = if start + slot == selected {
            Style::default().fg(INK).bg(PANEL_HIGHLIGHT)
        } else {
            Style::default().fg(status_color(status)).bg(PANEL)
        };
        paint_opaque(frame, rect, style);
        Paragraph::new(Line::from(Span::styled(label, style)))
            .style(style)
            .render(rect, frame.buffer_mut());
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_projection_nameplates(
    frame: &mut Frame,
    canvas: &Canvas,
    body: Rect,
    office_name: &str,
    workers: &[&Worker],
    start: usize,
    selected: usize,
    now: Millis,
    projection: Projection,
    layout: OfficeLayout,
) {
    if body.width == 0 || body.height == 0 || workers.is_empty() {
        return;
    }
    let plan = FlatRoom::new(canvas, workers.len(), layout, projection);
    let (px, py) = canvas.pixels_per_cell();
    for (slot, worker) in workers.iter().enumerate() {
        let station = plan.stations[slot];
        let label_width = station.label_width.max(1);
        let status = worker_status(worker, now);
        let prefix = if start + slot == selected {
            match status {
                WorkerStatus::Blocked => "> !",
                WorkerStatus::Failed => "> ×",
                _ => ">",
            }
        } else if status == WorkerStatus::Blocked {
            "!"
        } else if status == WorkerStatus::Failed {
            "×"
        } else {
            " "
        };
        let name = unique_plate_name(
            workers,
            slot,
            office_name,
            label_width.saturating_sub(prefix.chars().count() + 1),
        );
        let style = if start + slot == selected {
            Style::default().fg(INK).bg(PANEL_HIGHLIGHT)
        } else {
            Style::default().fg(status_color(status)).bg(PANEL)
        };
        let x = body
            .x
            .saturating_add((station.center_x / px).saturating_sub(label_width / 2) as u16)
            .min(body.x.saturating_add(body.width.saturating_sub(1)));
        let y = body
            .y
            .saturating_add((station.label_y / py) as u16)
            .min(body.y.saturating_add(body.height.saturating_sub(1)));
        let available = body.x.saturating_add(body.width).saturating_sub(x);
        paint_opaque(
            frame,
            Rect::new(x, y, available.min(label_width as u16).max(1), 1),
            style,
        );
        Paragraph::new(Line::from(Span::styled(format!("{prefix} {name}"), style)))
            .style(style)
            .render(
                Rect::new(x, y, available.min(label_width as u16).max(1), 1),
                frame.buffer_mut(),
            );
    }
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    frame: &mut Frame,
    office: Option<&Office>,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    now: Millis,
    selected: usize,
    projection: Projection,
    name_plates: bool,
) -> OfficeLayout {
    let area = super::below_tab_bar(frame.area());
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_tiny(
            frame,
            "they-work • terminal too small for the isometric floor",
        );
        return OfficeLayout::default();
    }
    let Some(office) = office else {
        draw_tiny(frame, "No project floor selected.");
        return OfficeLayout::default();
    };

    let body_height = area.height.saturating_sub(4);
    let effective = effective_projection(projection, area.width, body_height);
    let mut layout = desk_layout(office.workers.len(), area.width, body_height);
    if effective == Projection::Side && layout.columns > 0 {
        layout.rows = 1;
        layout.page_size = layout.columns;
        layout.pages = office.workers.len().div_ceil(layout.page_size);
    }
    if effective == Projection::Iso && layout.columns > 0 {
        let capacity = if area.width < 110 || body_height < 26 {
            3
        } else if area.width < ISO_MIN_WIDTH || body_height < ISO_MIN_HEIGHT {
            5
        } else {
            MAX_DESKS
        };
        if layout.page_size > capacity {
            layout.page_size = capacity;
            layout.columns = capacity;
            layout.rows = 1;
            layout.pages = office.workers.len().div_ceil(capacity);
        }
    }
    let page = if layout.page_size == 0 {
        0
    } else {
        selected / layout.page_size
    };
    let start = page.saturating_mul(layout.page_size);
    let visible_workers = office
        .workers
        .iter()
        .skip(start)
        .take(layout.page_size)
        .collect::<Vec<_>>();
    let looks = worker_looks(&office.workers)
        .into_iter()
        .skip(start)
        .take(layout.page_size)
        .collect::<Vec<_>>();
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
    let overflow = office.workers.len().saturating_sub(MAX_DESKS);
    let title = short_path(&office.name, area.width.saturating_sub(14) as usize);
    let subtitle = format!(
        "{} • {} workers • {} • {}",
        short_path(&office.path, 24),
        office.workers.len(),
        status_label(office, now),
        effective.label()
    );
    let (header, body, footer) = super::vertical_bands(area, 2, 2);
    draw_header(frame, header, &format!("FLOOR / {title}"), &subtitle);
    let footer_text = if overflow > 0 {
        format!(
            "←↑↓→ / hjkl desks   Enter open   Tab floor   Esc tower   v view   page {}/{}   +{} overflow   p phone   o palette   c sources   ? help",
            page.saturating_add(1),
            layout.pages.max(1),
            overflow
        )
    } else {
        format!(
            "←↑↓→ / hjkl desks   Enter open   Tab floor   Esc tower   v view   page {}/{}   p phone   o palette   c sources   ? help",
            page.saturating_add(1),
            layout.pages.max(1)
        )
    };
    draw_footer(frame, footer, &footer_text);
    if footer.height > 1 && footer.width >= 50 {
        Paragraph::new("  PgUp/PgDn pages · Home first worker · End last worker")
            .style(Style::default().fg(MUTED).bg(super::BACKGROUND))
            .render(
                Rect::new(footer.x, footer.y + 1, footer.width, 1),
                frame.buffer_mut(),
            );
    }
    if !has_area(body) {
        return layout;
    }

    canvas.resize_for_cells(body.width as usize, body.height as usize);
    let iso_grid = match effective {
        Projection::Iso | Projection::Auto => Some(draw_room_scene(
            canvas,
            &office.name,
            &visible_workers,
            &looks,
            sprites,
            now,
            selected.checked_sub(start),
        )),
        Projection::TopDown => {
            draw_top_down_scene(
                canvas,
                office,
                &visible_workers,
                &looks,
                sprites,
                now,
                layout,
            );
            None
        }
        Projection::Side => {
            draw_side_scene(canvas, office, &visible_workers, &looks, sprites, now);
            None
        }
        Projection::List => {
            draw_list_scene(canvas);
            None
        }
    };
    if effective != Projection::List {
        apply_room_palette(canvas, sprites.office_palette_index(office));
    }
    canvas.render(frame.buffer_mut(), body);
    if effective == Projection::List {
        draw_list_rows(
            frame,
            body,
            &office.name,
            &visible_workers,
            start,
            selected,
            now,
        );
    } else if name_plates {
        if let Some(grid) = iso_grid {
            draw_nameplates(
                frame,
                body,
                grid,
                &office.name,
                &visible_workers,
                start,
                selected,
                now,
            );
        } else {
            draw_projection_nameplates(
                frame,
                canvas,
                body,
                &office.name,
                &visible_workers,
                start,
                selected,
                now,
                effective,
                layout,
            );
        }
    }
    if (blocked_count > 0 || failed_count > 0)
        && body.width >= 24
        && body.height >= 2
        && effective != Projection::List
    {
        let (alert, color) = if blocked_count > 0 {
            let alert = if failed_count > 0 {
                format!("! {blocked_count} BLOCKED • × {failed_count} FAILED • MANAGER")
            } else {
                format!("! {blocked_count} BLOCKED  •  MANAGER ON FLOOR")
            };
            (alert, WARNING)
        } else {
            (
                format!("× {failed_count} FAILED  •  CHECK DESK"),
                super::HOT,
            )
        };
        paint_opaque(
            frame,
            Rect::new(body.x, body.y.saturating_add(1), body.width.min(36), 1),
            Style::default().bg(super::BACKGROUND),
        );
        Paragraph::new(alert)
            .style(
                Style::default()
                    .fg(color)
                    .bg(super::BACKGROUND)
                    .add_modifier(Modifier::BOLD),
            )
            .render(
                Rect::new(body.x, body.y.saturating_add(1), body.width.min(36), 1),
                frame.buffer_mut(),
            );
    }
    if office.workers.is_empty() && body.width >= 20 && body.height >= 2 {
        paint_opaque(
            frame,
            Rect::new(
                body.x,
                body.y.saturating_add(body.height / 2),
                body.width.min(34),
                1,
            ),
            Style::default().bg(PANEL),
        );
        Paragraph::new("QUIET FLOOR  •  waiting for a developer")
            .style(Style::default().fg(MUTED).bg(PANEL))
            .render(
                Rect::new(
                    body.x,
                    body.y.saturating_add(body.height / 2),
                    body.width.min(34),
                    1,
                ),
                frame.buffer_mut(),
            );
    }
    layout
}

fn floor_corners(grid: IsoGrid) -> [(i32, i32); 4] {
    let half_width = grid.tile_width / 2;
    let half_height = grid.tile_height / 2;
    let (back_x, back_y) = grid.center(0, 0);
    let (right_x, right_y) = grid.center(grid.columns.saturating_sub(1), 0);
    let (front_x, front_y) =
        grid.center(grid.columns.saturating_sub(1), grid.rows.saturating_sub(1));
    let (left_x, left_y) = grid.center(0, grid.rows.saturating_sub(1));
    [
        (back_x, back_y - half_height),
        (right_x + half_width, right_y),
        (front_x, front_y + half_height),
        (left_x - half_width, left_y),
    ]
}

#[cfg(test)]
fn worker_marker_position(grid: IsoGrid, slot: usize) -> (i32, i32) {
    let (tile_x, tile_y) = grid.desk_tile(slot);
    let (center_x, center_y) = grid.center(tile_x, tile_y);
    (
        physical_to_cell(center_x, grid.encoding.width_per_cell()),
        physical_to_cell(
            center_y + grid.tile_height / 2,
            grid.encoding.height_per_cell(),
        ),
    )
}

fn draw_floor_edges(canvas: &mut Canvas, grid: IsoGrid) {
    let [back, right, front, left] = floor_corners(grid);
    let outline = outline_color(canvas);
    draw_line(canvas, back.0, back.1, left.0, left.1, FLOOR_LIGHT);
    draw_line(canvas, back.0, back.1, right.0, right.1, FLOOR_LIGHT);
    draw_line(canvas, left.0, left.1, front.0, front.1, outline);
    draw_line(canvas, front.0, front.1, right.0, right.1, outline);
    for offset in 1..=2 {
        draw_line(
            canvas,
            left.0,
            left.1 + offset,
            front.0,
            front.1 + offset,
            FLOOR_DARK,
        );
        draw_line(
            canvas,
            front.0,
            front.1 + offset,
            right.0,
            right.1 + offset,
            FLOOR_DARK,
        );
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use theywork_core::{Agent, OfficeId, WorkerId};

    use super::*;

    #[test]
    fn projection_cycle_is_reversible_for_every_view() {
        for projection in [
            Projection::Auto,
            Projection::Iso,
            Projection::TopDown,
            Projection::Side,
            Projection::List,
        ] {
            assert_eq!(projection.next().previous(), projection);
            assert_eq!(projection.previous().next(), projection);
        }
    }

    #[test]
    fn desk_layout_caps_the_main_floor_and_pages_overflow() {
        let layout = desk_layout(11, 100, 26);
        assert_eq!(layout.columns, 5);
        assert_eq!(layout.rows, 2);
        assert_eq!(layout.page_size, 10);
        assert_eq!(layout.pages, 2);
    }

    #[test]
    fn common_five_worker_floor_uses_balanced_rows() {
        let layout = desk_layout(5, 100, 26);
        assert_eq!(layout.columns, 3);
        assert_eq!(layout.rows, 2);
        assert_eq!(layout.page_size, 6);
        assert_eq!(layout.pages, 1);
    }

    #[test]
    fn occupied_desk_tiles_are_unique_and_spread_across_the_plate() {
        let grid = make_grid(160, 88, ISO_ROOM_COLUMNS, ISO_ROOM_ROWS);
        let tiles = (0..5).map(|slot| grid.desk_tile(slot)).collect::<Vec<_>>();
        let occupied = tiles
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(occupied.len(), tiles.len(), "occupied desk tiles overlap");

        let min_x = tiles.iter().map(|tile| tile.0).min().unwrap_or(0);
        let max_x = tiles.iter().map(|tile| tile.0).max().unwrap_or(0);
        let min_y = tiles.iter().map(|tile| tile.1).min().unwrap_or(0);
        let max_y = tiles.iter().map(|tile| tile.1).max().unwrap_or(0);
        assert!(max_x - min_x >= 3, "desks do not span the plate width");
        assert!(max_y - min_y >= 3, "desks do not span the plate depth");

        let manager = manager_tile(grid, tiles.len(), 0);
        assert!(
            !occupied.contains(&manager),
            "manager shares occupied desk tile {manager:?}"
        );
    }

    #[test]
    fn five_workers_fit_the_plate_without_clipping_or_each_other() {
        let modes = [
            (PixelEncoding::HalfBlocks, None),
            (PixelEncoding::Quadrants, None),
            (PixelEncoding::Sextants, None),
            (PixelEncoding::Sextants, Some((10, 20))),
        ];
        for (encoding, cell_size) in modes {
            let mut canvas = Canvas::with_color_depth_and_encoding(
                0,
                0,
                crate::canvas::ColorDepth::TrueColor,
                encoding,
            );
            canvas.set_cell_pixel_size(cell_size);
            canvas.resize_for_cells(160, 44);
            let grid = make_grid_with_encoding(
                canvas.width(),
                canvas.height(),
                ISO_ROOM_COLUMNS,
                ISO_ROOM_ROWS,
                encoding,
                canvas.pixels_per_cell(),
            );
            let [back, _, front, _] = floor_corners(grid);
            let plate_height = front.1.saturating_sub(back.1) as usize;
            let mut bounds = Vec::new();
            for slot in 0..5 {
                let (x, y, width, height) = worker_bounds(grid, RoomScale::Floor, slot);
                assert!(
                    x >= 0 && y >= 0,
                    "{encoding:?}/{cell_size:?} worker {slot} clips top/left"
                );
                assert!(
                    x + width as i32 <= canvas.width() as i32
                        && y + height as i32 <= canvas.height() as i32,
                    "{encoding:?}/{cell_size:?} worker {slot} clips bottom/right"
                );
                if slot == 2 {
                    assert!(
                        canvas.width() as i32 - (x + width as i32) >= width as i32 / 2,
                        "{encoding:?}/{cell_size:?} outer worker has no figure-width gutter"
                    );
                }
                assert!(
                    height * 2 <= plate_height,
                    "{encoding:?}/{cell_size:?} worker height {height} overwhelms plate height {plate_height}"
                );
                bounds.push((x, y, x + width as i32, y + height as i32));

                let (desk_x, desk_y, desk_width, desk_height) =
                    desk_bounds(grid, RoomScale::Floor, slot);
                let (tile_x, tile_y) = grid.desk_tile(slot);
                let (center_x, _) = grid.center(tile_x, tile_y);
                let (monitor_width, monitor_height) = RoomScale::Floor.monitor_size(grid);
                let monitor_x = center_x - desk_width as i32 / 4 - monitor_width as i32 / 2;
                let monitor_y = desk_y - monitor_height as i32 + grid.scale_half_height(2) as i32;
                assert!(
                    desk_x >= 0
                        && desk_y >= 0
                        && desk_x + desk_width as i32 <= canvas.width() as i32
                        && desk_y + desk_height as i32 <= canvas.height() as i32,
                    "{encoding:?}/{cell_size:?} desk {slot} clips the frame"
                );
                assert!(
                    monitor_x >= 0
                        && monitor_y >= 0
                        && monitor_x + monitor_width as i32 <= canvas.width() as i32
                        && monitor_y + monitor_height as i32 <= canvas.height() as i32,
                    "{encoding:?}/{cell_size:?} monitor {slot} clips the frame"
                );
            }
            for (index, first) in bounds.iter().enumerate() {
                for (other_index, second) in bounds.iter().enumerate().skip(index + 1) {
                    assert_eq!(
                        overlap_area(*first, *second),
                        0,
                        "{encoding:?}/{cell_size:?} workers {index} and {other_index} overlap"
                    );
                }
            }
        }
    }

    #[test]
    fn chairs_workers_and_desks_form_a_seated_stack_at_one_scale() {
        let modes = [
            (PixelEncoding::HalfBlocks, None),
            (PixelEncoding::Quadrants, None),
            (PixelEncoding::Sextants, None),
            (PixelEncoding::Sextants, Some((10, 20))),
        ];
        for (encoding, cell_size) in modes {
            let pixels_per_cell =
                cell_size.unwrap_or((encoding.width_per_cell(), encoding.height_per_cell()));
            let grid = make_grid_with_encoding(
                160 * pixels_per_cell.0,
                44 * pixels_per_cell.1,
                ISO_ROOM_COLUMNS,
                ISO_ROOM_ROWS,
                encoding,
                pixels_per_cell,
            );
            let (_, worker_y, worker_width, worker_height) =
                worker_bounds(grid, RoomScale::Floor, 0);
            let (_, desk_y, _, _) = desk_bounds(grid, RoomScale::Floor, 0);
            let covered = (worker_y + worker_height as i32 - desk_y).max(0) as usize;
            assert!(
                covered >= worker_height / 6 && covered <= worker_height / 2,
                "{encoding:?}/{cell_size:?} desk covers {covered} of {worker_height} worker pixels"
            );
            assert_eq!(
                RoomScale::Floor.manager_size(grid),
                (worker_width, worker_height)
            );

            let mut stack = [
                IsoItem {
                    tile_x: 2,
                    tile_y: 0,
                    footprint: IsoFootprint {
                        width: 1,
                        depth: 1,
                        height: 1,
                    },
                    kind: IsoKind::Desk(0),
                },
                IsoItem {
                    tile_x: 2,
                    tile_y: 0,
                    footprint: IsoFootprint {
                        width: 1,
                        depth: 1,
                        height: 2,
                    },
                    kind: IsoKind::Worker(0),
                },
                IsoItem {
                    tile_x: 2,
                    tile_y: 0,
                    footprint: IsoFootprint {
                        width: 1,
                        depth: 1,
                        height: 1,
                    },
                    kind: IsoKind::Chair(0),
                },
            ];
            painter_order(&mut stack);
            assert_eq!(
                stack.map(|item| item.kind),
                [IsoKind::Chair(0), IsoKind::Worker(0), IsoKind::Desk(0)]
            );
        }
    }

    #[test]
    fn floor_worker_plates_stay_with_their_desks_at_every_encoding() {
        let workers = (0..5)
            .map(|slot| {
                Worker::new(
                    WorkerId(format!("/golden/they-work#worker-{slot}")),
                    OfficeId("/golden/they-work".to_string()),
                    if slot % 2 == 0 {
                        Agent::Claude
                    } else {
                        Agent::Codex
                    },
                    format!("they-work worker {slot}"),
                    0,
                )
            })
            .collect::<Vec<_>>();
        let visible_workers = workers.iter().collect::<Vec<_>>();
        let body = Rect::new(0, 0, 160, 44);

        let modes = [
            (PixelEncoding::HalfBlocks, None),
            (PixelEncoding::Quadrants, None),
            (PixelEncoding::Sextants, None),
            (PixelEncoding::Sextants, Some((10, 20))),
        ];
        for (encoding, cell_size) in modes {
            let mut canvas = Canvas::with_color_depth_and_encoding(
                0,
                0,
                crate::canvas::ColorDepth::TrueColor,
                encoding,
            );
            canvas.set_cell_pixel_size(cell_size);
            canvas.resize_for_cells(body.width as usize, body.height as usize);
            let sprites = SpriteSet::new();
            let looks = worker_looks(&workers);
            let grid = draw_room_scene(
                &mut canvas,
                "/golden/they-work",
                &visible_workers,
                &looks,
                &sprites,
                0,
                None,
            );
            let mut terminal =
                Terminal::new(TestBackend::new(body.width, body.height)).expect("test terminal");
            terminal
                .draw(|frame| {
                    canvas.render(frame.buffer_mut(), body);
                    draw_nameplates(
                        frame,
                        body,
                        grid,
                        "/golden/they-work",
                        &visible_workers,
                        0,
                        0,
                        0,
                    );
                })
                .expect("floor with worker plates");

            let buffer = terminal.backend().buffer();
            let mut plate_rects = Vec::new();
            for slot in 0..workers.len() {
                let needle = format!("worker {slot}");
                let mut matches = Vec::new();
                for y in body.y..body.y.saturating_add(body.height) {
                    let needle_width = needle.chars().count();
                    for x in body.x..body.x.saturating_add(body.width) {
                        let mut candidate = String::new();
                        for offset in 0..needle_width {
                            let Some(cell) = buffer.cell((x.saturating_add(offset as u16), y))
                            else {
                                break;
                            };
                            candidate.push_str(cell.symbol());
                        }
                        if candidate == needle {
                            matches.push((x, y));
                        }
                    }
                }
                assert_eq!(
                    matches.len(),
                    1,
                    "{encoding:?}/{cell_size:?} should render exactly one plate for worker {slot}"
                );

                let rect = worker_plate_rect(
                    body,
                    grid,
                    slot,
                    workers.len(),
                    blocked_slot(&visible_workers, 0),
                    0,
                );
                plate_rects.push(rect);
                let tile_width_cells =
                    physical_to_cell(grid.tile_width, grid.pixels_per_cell.0).max(12) as u16;
                assert_eq!(
                    rect.width,
                    (tile_width_cells / 2).max(12).min(body.width),
                    "{encoding:?}/{cell_size:?} plate width did not use actual cell density"
                );
                assert_eq!(matches[0], (rect.x.saturating_add(2), rect.y));

                let (_, desk_y, _, desk_height) = desk_bounds(grid, RoomScale::Floor, slot);
                let desk_bottom =
                    physical_to_cell(desk_y + desk_height as i32 - 1, grid.pixels_per_cell.1)
                        as u16;
                assert!(
                    rect.y >= body.y.saturating_add(desk_bottom)
                        && rect.y <= body.y.saturating_add(desk_bottom).saturating_add(1),
                    "{encoding:?}/{cell_size:?} plate {slot} at row {} is detached from desk bottom {}",
                    rect.y,
                    body.y.saturating_add(desk_bottom)
                );
            }

            for (index, first) in plate_rects.iter().enumerate() {
                for second in plate_rects.iter().skip(index + 1) {
                    let separated = first.x.saturating_add(first.width) <= second.x
                        || second.x.saturating_add(second.width) <= first.x
                        || first.y.saturating_add(first.height) <= second.y
                        || second.y.saturating_add(second.height) <= first.y;
                    assert!(
                        separated,
                        "{encoding:?}/{cell_size:?} worker plates {index} and {} overlap",
                        index + 1
                    );
                }
            }

            for (owner, plate) in plate_rects.iter().copied().enumerate() {
                let owner_tile = grid.desk_tile(owner);
                let owner_key = painter_key(IsoItem {
                    tile_x: owner_tile.0,
                    tile_y: owner_tile.1,
                    footprint: IsoFootprint {
                        width: 1,
                        depth: 1,
                        height: 1,
                    },
                    kind: IsoKind::Desk(owner),
                });
                for front in 0..workers.len() {
                    let front_tile = grid.desk_tile(front);
                    let front_key = painter_key(IsoItem {
                        tile_x: front_tile.0,
                        tile_y: front_tile.1,
                        footprint: IsoFootprint {
                            width: 1,
                            depth: 1,
                            height: 2,
                        },
                        kind: IsoKind::Worker(front),
                    });
                    if front == owner || front_key <= owner_key {
                        continue;
                    }
                    assert!(
                        !rects_overlap(
                            plate,
                            expanded_rect(worker_cell_rect(grid, front), body, 1)
                        ),
                        "{encoding:?}/{cell_size:?} plate {owner} enters the safety gap around foreground worker {front}"
                    );
                }
            }

            // No manager is present on an idle floor. Manager avoidance is
            // checked below using actual blocked state and its own frame time.
        }
    }

    #[test]
    fn worker_marker_positions_use_encoding_cell_density() {
        for encoding in PixelEncoding::ALL {
            let grid = make_grid_with_encoding(
                160usize.saturating_mul(encoding.width_per_cell()),
                encoding.height_per_cell() * 44,
                ISO_ROOM_COLUMNS,
                ISO_ROOM_ROWS,
                encoding,
                (encoding.width_per_cell(), encoding.height_per_cell()),
            );
            let (tile_x, tile_y) = grid.desk_tile(0);
            let (center_x, center_y) = grid.center(tile_x, tile_y);
            assert_eq!(
                worker_marker_position(grid, 0),
                (
                    physical_to_cell(center_x, encoding.width_per_cell()),
                    physical_to_cell(center_y + grid.tile_height / 2, encoding.height_per_cell(),),
                ),
                "{encoding:?} marker should be returned in terminal-cell coordinates"
            );
        }
    }

    fn room_items(worker_count: usize) -> Vec<IsoItem> {
        let grid = make_grid(80, 38, ISO_ROOM_COLUMNS, ISO_ROOM_ROWS);
        let mut items = Vec::with_capacity(worker_count.saturating_mul(3) + 4);
        for slot in 0..worker_count {
            let (tile_x, tile_y) = grid.desk_tile(slot);
            items.push(IsoItem {
                tile_x,
                tile_y,
                footprint: IsoFootprint {
                    width: 1,
                    depth: 1,
                    height: 1,
                },
                kind: IsoKind::Chair(slot),
            });
            items.push(IsoItem {
                tile_x,
                tile_y,
                footprint: IsoFootprint {
                    width: 1,
                    depth: 1,
                    height: 2,
                },
                kind: IsoKind::Worker(slot),
            });
            items.push(IsoItem {
                tile_x,
                tile_y,
                footprint: IsoFootprint {
                    width: 1,
                    depth: 1,
                    height: 1,
                },
                kind: IsoKind::Desk(slot),
            });
        }
        items.extend([
            IsoItem {
                tile_x: ISO_PLANT_TILE.0,
                tile_y: ISO_PLANT_TILE.1,
                footprint: IsoFootprint {
                    width: 1,
                    depth: 1,
                    height: 2,
                },
                kind: IsoKind::Plant,
            },
            IsoItem {
                tile_x: ISO_COOLER_TILE.0,
                tile_y: ISO_COOLER_TILE.1,
                footprint: IsoFootprint {
                    width: 1,
                    depth: 1,
                    height: 2,
                },
                kind: IsoKind::Cooler,
            },
            IsoItem {
                tile_x: ISO_MEETING_TABLE_TILE.0,
                tile_y: ISO_MEETING_TABLE_TILE.1,
                footprint: IsoFootprint {
                    width: 2,
                    depth: 1,
                    height: 1,
                },
                kind: IsoKind::MeetingTable,
            },
        ]);
        items
    }

    fn sprite_bounds(grid: IsoGrid, item: IsoItem) -> Option<(i32, i32, i32, i32)> {
        let (center_x, center_y) = grid.center(item.tile_x, item.tile_y);
        let (width, height) = match item.kind {
            IsoKind::Manager | IsoKind::Chair(_) => return None,
            IsoKind::Plant => (
                (grid.tile_width / 2).clamp(4, 10),
                (grid.tile_height + 2).clamp(5, 10),
            ),
            IsoKind::Cooler => (
                (grid.tile_width / 3).clamp(4, 8),
                (grid.tile_height + 1).clamp(5, 9),
            ),
            IsoKind::Worker(_) => (
                (grid.tile_width / 3).clamp(4, 8),
                (grid.tile_height + 1).clamp(5, 9),
            ),
            IsoKind::Desk(_) => (
                (grid.tile_width * 2 / 3).clamp(5, 14),
                grid.tile_height.saturating_sub(1).clamp(3, 6),
            ),
            IsoKind::MeetingTable => {
                let half_width = (grid.tile_width / 5).max(2);
                let half_height = (grid.tile_height / 2).max(1);
                return Some((
                    center_x - half_width,
                    center_y - 1 - half_height,
                    center_x + half_width + 1,
                    center_y + grid.tile_height / 2 + 2,
                ));
            }
        };
        let (top, y_offset) = match item.kind {
            IsoKind::Worker(_) => (center_y - height + 1, 0),
            IsoKind::Desk(_) => (center_y + grid.tile_height / 3 - height / 2, 0),
            _ => (center_y - height, 0),
        };
        Some((
            center_x - width / 2,
            top + y_offset,
            center_x - width / 2 + width,
            top + y_offset + height,
        ))
    }

    fn overlap_area(first: (i32, i32, i32, i32), second: (i32, i32, i32, i32)) -> i32 {
        let width = (first.2.min(second.2) - first.0.max(second.0)).max(0);
        let height = (first.3.min(second.3) - first.1.max(second.1)).max(0);
        width * height
    }

    fn polygon_area(points: &[(i32, i32)]) -> usize {
        let twice_area = points
            .iter()
            .enumerate()
            .map(|(index, &(x0, y0))| {
                let (x1, y1) = points[(index + 1) % points.len()];
                i64::from(x0) * i64::from(y1) - i64::from(y0) * i64::from(x1)
            })
            .sum::<i64>()
            .unsigned_abs();
        usize::try_from(twice_area / 2).expect("room polygon fits in usize")
    }

    #[test]
    fn isometric_room_has_a_floor_plate_with_breathing_space() {
        let width = 80;
        let height = 38;
        let grid = make_grid(width, height, ISO_ROOM_COLUMNS, ISO_ROOM_ROWS);
        let plate_area = polygon_area(&floor_corners(grid));
        let frame_area = width * height;

        // A room should anchor the scene without swallowing its wall and margin.
        assert!(
            plate_area >= frame_area / 5 && plate_area <= frame_area * 3 / 5,
            "floor plate area {plate_area} is not a sane share of frame area {frame_area}"
        );

        let mut canvas = crate::canvas::Canvas::with_color_depth(
            width,
            height,
            crate::canvas::ColorDepth::TrueColor,
        );
        draw_floor_plate(&mut canvas, grid);
        let floor_colors = [FLOOR, FLOOR_DARK, FLOOR_DITHER];
        let floor_pixels = (0..width)
            .flat_map(|x| (0..height).map(move |y| (x, y)))
            .filter(|(x, y)| {
                canvas
                    .pixel(*x, *y)
                    .is_some_and(|color| floor_colors.contains(&color))
            })
            .count();
        assert!(
            floor_pixels >= frame_area / 5,
            "floor plate drew only {floor_pixels} pixels in a {frame_area}-pixel frame"
        );
    }

    #[test]
    fn native_room_uses_the_available_image_frame() {
        let width = 1_600;
        let height = 860;
        let grid = make_grid_with_encoding(
            width,
            height,
            ISO_ROOM_COLUMNS,
            ISO_ROOM_ROWS,
            PixelEncoding::Sextants,
            (10, 20),
        );
        let [back, right, front, left] = floor_corners(grid);
        let mut canvas = Canvas::with_color_depth_and_encoding(
            0,
            0,
            crate::canvas::ColorDepth::TrueColor,
            PixelEncoding::Sextants,
        );
        canvas.set_cell_pixel_size(Some((10, 20)));
        canvas.resize(width, height);
        let wall_top = back.1 - iso_wall_height(&canvas);

        assert!(right.0 - left.0 >= width as i32 * 4 / 5);
        assert!(wall_top <= height as i32 / 6);
        assert!(front.1 >= height as i32 * 5 / 6);
    }

    #[test]
    fn every_letter_and_digit_keeps_its_five_by_seven_face_at_every_density() {
        let modes = [
            (PixelEncoding::HalfBlocks, None),
            (PixelEncoding::Quadrants, None),
            (PixelEncoding::Sextants, None),
            (PixelEncoding::Sextants, Some((10, 20))),
        ];
        for (encoding, cell_size) in modes {
            for character in ('A'..='Z').chain('0'..='9') {
                let mut canvas = Canvas::with_color_depth_and_encoding(
                    100,
                    100,
                    crate::canvas::ColorDepth::TrueColor,
                    encoding,
                );
                canvas.set_cell_pixel_size(cell_size);
                canvas.resize(100, 100);
                let x_scale = canvas.pixels_per_cell().0;
                let y_scale = canvas.scale_half_height(1);
                let glyph = glyph_5x7(character);
                draw_extruded_glyph(
                    &mut canvas,
                    glyph,
                    10,
                    10,
                    100,
                    x_scale,
                    y_scale,
                    SIGN_EXTRUSION_STEPS,
                );
                for (row, bits) in glyph.iter().enumerate() {
                    for column in 0..5 {
                        let pixel = canvas.pixel(
                            10 + column * x_scale + x_scale / 2,
                            10 + row * y_scale + y_scale / 2,
                        );
                        if bits & (1 << (4 - column)) != 0 {
                            assert_eq!(
                                pixel,
                                Some(TITLE_COLOR),
                                "{encoding:?} {character} lost ({column}, {row})"
                            );
                        } else if cell_size.is_some() {
                            assert_ne!(
                                pixel,
                                Some(OUTLINE),
                                "{character} outline filled ({column}, {row})"
                            );
                        }
                    }
                }
            }
        }
        assert_ne!(compact_glyph('K'), compact_glyph('I'));
        assert_ne!(compact_glyph('K'), compact_glyph(':'));
        assert_ne!(compact_glyph('W'), compact_glyph('H'));
        assert_ne!(compact_glyph('R'), compact_glyph('A'));
        let compact_letters = ('A'..='Z')
            .map(compact_glyph)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(compact_letters.len(), 26);
    }

    #[test]
    fn complete_they_work_sign_keeps_every_letter_face_at_every_density() {
        let modes = [
            (PixelEncoding::HalfBlocks, None),
            (PixelEncoding::Quadrants, None),
            (PixelEncoding::Sextants, None),
            (PixelEncoding::Sextants, Some((10, 20))),
        ];
        let label = "THEY-WORK";
        for (encoding, cell_size) in modes {
            let mut canvas = Canvas::with_color_depth_and_encoding(
                0,
                0,
                crate::canvas::ColorDepth::TrueColor,
                encoding,
            );
            canvas.set_cell_pixel_size(cell_size);
            canvas.resize_for_cells(160, 44);
            let wall_top = canvas.height() as i32 - 1;
            draw_isometric_sign(&mut canvas, label, wall_top);

            let x_scale = canvas.pixels_per_cell().0;
            let y_scale = canvas.scale_half_height(1);
            let glyph_pitch = sign_glyph_pitch(&canvas, x_scale, ROOM_SIGN_EXTRUSION_STEPS);
            let face_width =
                (label.chars().count() - 1) * glyph_pitch + 5usize.saturating_mul(x_scale);
            let extrusion_x = ROOM_SIGN_EXTRUSION_STEPS * sign_depth_step(&canvas, x_scale);
            let extrusion_y = ROOM_SIGN_EXTRUSION_STEPS * sign_depth_step(&canvas, y_scale);
            let rise = SIGN_GLYPH_RISE as usize * y_scale;
            let sign_height = 7 * y_scale + (label.chars().count() - 1) * rise + extrusion_y;
            let x0 = room_sign_x(&canvas, face_width.saturating_add(extrusion_x));
            let top = wall_top as usize - 1 - sign_height;

            for (character_index, character) in label.chars().enumerate() {
                for (row, bits) in glyph_5x7(character).iter().enumerate() {
                    for column in 0..5 {
                        if bits & (1 << (4 - column)) == 0 {
                            continue;
                        }
                        let pixel = canvas.pixel(
                            x0 + character_index * glyph_pitch + column * x_scale + x_scale / 2,
                            top + character_index * rise + row * y_scale + y_scale / 2,
                        );
                        assert_eq!(
                            pixel,
                            Some(TITLE_COLOR),
                            "{encoding:?} {character} lost ({column}, {row})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn worker_wardrobe_variants_have_distinct_pixels_at_every_density() {
        let worker = Worker::new(
            WorkerId("/office#worker".into()),
            OfficeId("/office".into()),
            Agent::Codex,
            "worker".into(),
            0,
        );
        let sprites = SpriteSet::new();
        let modes = [
            (PixelEncoding::HalfBlocks, 7, 10, None),
            (PixelEncoding::Quadrants, 14, 10, None),
            (PixelEncoding::Sextants, 14, 20, None),
            (PixelEncoding::Sextants, 96, 136, Some((10, 20))),
        ];
        for (encoding, width, height, cell_size) in modes {
            let checksums = (0..6_u8)
                .map(|variant| {
                    let look = WorkerLook {
                        head: variant,
                        face: variant % 5,
                        top: variant,
                        desk_prop: variant,
                        skin: 0,
                        hair: variant,
                        contractor: false,
                    };
                    let sprite = sprites.worker_frame_fitting(&worker, look, 0, width, height);
                    let mut canvas = Canvas::with_color_depth_and_encoding(
                        width,
                        height,
                        crate::canvas::ColorDepth::TrueColor,
                        encoding,
                    );
                    canvas.set_cell_pixel_size(cell_size);
                    canvas.resize(width, height);
                    blit_floor_worker(&mut canvas, &sprite, 0, 0, width, height);
                    canvas
                        .pixel_frame()
                        .rgba()
                        .iter()
                        .fold(0xcbf29ce484222325_u64, |hash, byte| {
                            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
                        })
                })
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                checksums.len(),
                6,
                "{encoding:?} collapsed wardrobe variants"
            );
        }
    }

    #[test]
    fn every_floor_density_uses_native_pixels_or_integer_enlargement() {
        let worker = Worker::new(
            WorkerId("/office#face".into()),
            OfficeId("/office".into()),
            Agent::Codex,
            "face".into(),
            0,
        );
        let sprites = SpriteSet::new();
        let look = crate::sprite::worker_look(&worker);
        for (encoding, cell_size) in [
            (PixelEncoding::HalfBlocks, None),
            (PixelEncoding::Quadrants, None),
            (PixelEncoding::Sextants, None),
            (PixelEncoding::Sextants, Some((10, 20))),
        ] {
            let pixels_per_cell =
                cell_size.unwrap_or((encoding.width_per_cell(), encoding.height_per_cell()));
            let grid = make_grid_with_encoding(
                160 * pixels_per_cell.0,
                44 * pixels_per_cell.1,
                ISO_ROOM_COLUMNS,
                ISO_ROOM_ROWS,
                encoding,
                pixels_per_cell,
            );
            let (width, height) = RoomScale::Floor.worker_size(grid);
            let sprite = sprites.worker_frame_fitting(&worker, look, 0, width, height);
            assert_eq!(width % sprite.width(), 0, "{encoding:?} squeezes columns");
            assert_eq!(height % sprite.height(), 0, "{encoding:?} squeezes rows");
            let mut canvas = Canvas::with_color_depth_and_encoding(
                width,
                height,
                crate::canvas::ColorDepth::TrueColor,
                encoding,
            );
            blit_floor_worker(&mut canvas, &sprite, 0, 0, width, height);
            for y in 0..sprite.height() {
                for x in 0..sprite.width() {
                    assert_eq!(
                        canvas.pixel(x * width / sprite.width(), y * height / sprite.height()),
                        sprite.pixel(x, y)
                    );
                }
            }
        }
    }

    #[test]
    fn isometric_room_keeps_objects_separate_and_ground_open() {
        let grid = make_grid(80, 38, ISO_ROOM_COLUMNS, ISO_ROOM_ROWS);
        let items = room_items(5);
        let bounds = items
            .iter()
            .filter_map(|item| sprite_bounds(grid, *item).map(|bounds| (item.kind, bounds)))
            .collect::<Vec<_>>();
        for (first_index, first) in bounds.iter().enumerate() {
            for second in bounds.iter().skip(first_index + 1) {
                let overlap = overlap_area(first.1, second.1);
                assert!(
                    overlap <= 8,
                    "room objects overlap by {overlap} pixels: {:?} and {:?}",
                    first.0,
                    second.0
                );
            }
        }

        let mut occupied = std::collections::BTreeSet::new();
        for item in items {
            let start_x = item.tile_x.min(grid.columns);
            let end_x = item
                .tile_x
                .saturating_add(usize::from(item.footprint.width))
                .min(grid.columns);
            let start_y = item.tile_y.min(grid.rows);
            let end_y = item
                .tile_y
                .saturating_add(usize::from(item.footprint.depth))
                .min(grid.rows);
            for tile_y in start_y..end_y {
                for tile_x in start_x..end_x {
                    occupied.insert((tile_x, tile_y));
                }
            }
        }
        let total_tiles = grid.columns * grid.rows;
        let empty_tiles = total_tiles.saturating_sub(occupied.len());
        // Keep at least two fifths of the plate visibly free for the eye to read.
        assert!(
            empty_tiles * 5 >= total_tiles * 2,
            "only {empty_tiles} of {total_tiles} ground tiles remain open"
        );
    }

    #[test]
    fn painter_order_puts_nearer_and_taller_items_last() {
        let mut items = [
            IsoItem {
                tile_x: 2,
                tile_y: 1,
                footprint: IsoFootprint {
                    width: 1,
                    depth: 1,
                    height: 1,
                },
                kind: IsoKind::Desk(0),
            },
            IsoItem {
                tile_x: 0,
                tile_y: 0,
                footprint: IsoFootprint {
                    width: 1,
                    depth: 1,
                    height: 2,
                },
                kind: IsoKind::Worker(0),
            },
            IsoItem {
                tile_x: 1,
                tile_y: 1,
                footprint: IsoFootprint {
                    width: 1,
                    depth: 2,
                    height: 1,
                },
                kind: IsoKind::MeetingTable,
            },
        ];
        painter_order(&mut items);
        assert_eq!(items[0].kind, IsoKind::Worker(0));
        assert_eq!(items[2].kind, IsoKind::Desk(0));
    }

    #[test]
    fn project_labels_are_safe_and_split_without_ellipsis() {
        assert_eq!(project_label("/workspace/they-work"), "THEY-WORK");
        let longish = project_label("/workspace/beta-platform");
        assert_eq!(longish, "BETA-PLATFORM");
        assert_eq!(sign_lines(&longish), vec!["BETA-PLATFORM"]);
        let label = project_label("/workspace/very-long-project-name/with spaces");
        assert!(label.contains('·'));
        assert_eq!(sign_lines(&label).len(), 1);
        let long_label = project_label("/workspace/very-long-project-name");
        let lines = sign_lines(&long_label);
        assert!(lines.iter().all(|line| !line.contains('…')));
        assert_eq!(lines.len(), 2);
        assert_ne!(glyph_5x7('·'), [0; 7]);
    }

    #[test]
    fn isometric_sign_stays_in_its_reserved_band() {
        let mut canvas =
            crate::canvas::Canvas::with_color_depth(160, 88, crate::canvas::ColorDepth::TrueColor);
        let wall_top = 32;
        draw_isometric_sign(&mut canvas, "SUSTAIN", wall_top);
        let sign_colors = [TITLE_COLOR, TITLE_BODY, TITLE_SIGN];
        let pixels = (0..canvas.width())
            .flat_map(|x| (0..canvas.height()).map(move |y| (x, y)))
            .filter(|(x, y)| {
                canvas
                    .pixel(*x, *y)
                    .is_some_and(|color| sign_colors.contains(&color))
            })
            .collect::<Vec<_>>();
        assert!(!pixels.is_empty());
        let min_x = pixels.iter().map(|(x, _)| *x).min().unwrap_or(0);
        let max_x = pixels.iter().map(|(x, _)| *x).max().unwrap_or(0);
        let max_y = pixels.iter().map(|(_, y)| *y).max().unwrap_or(0);
        assert!(max_y < (wall_top - 1) as usize);
        let actual_width = max_x.saturating_sub(min_x).saturating_add(1);
        assert!(
            actual_width * SIGN_MAX_WIDTH_DENOMINATOR <= canvas.width() * SIGN_MAX_WIDTH_NUMERATOR,
            "sign consumes {actual_width} of {} pixels",
            canvas.width()
        );
        assert!(
            min_x >= SIGN_STATUS_RESERVE_CELLS * canvas.pixels_per_cell().0,
            "sign begins at {min_x} inside the reserved status area"
        );
        let x_scale = canvas.pixels_per_cell().0;
        let expected_width = 6 * sign_glyph_pitch(&canvas, x_scale, ROOM_SIGN_EXTRUSION_STEPS)
            + 5 * x_scale
            + ROOM_SIGN_EXTRUSION_STEPS * sign_depth_step(&canvas, x_scale);
        assert!(max_x.saturating_sub(min_x) < expected_width);
    }

    #[test]
    fn long_isometric_sign_steps_down_without_leaving_its_band() {
        let mut canvas =
            crate::canvas::Canvas::with_color_depth(160, 88, crate::canvas::ColorDepth::TrueColor);
        let wall_top = 32;
        draw_isometric_sign(&mut canvas, "VERY-LONG-PROJECT-NAME", wall_top);
        let pixels = (0..canvas.width())
            .flat_map(|x| (0..canvas.height()).map(move |y| (x, y)))
            .filter(|(x, y)| canvas.pixel(*x, *y) == Some(TITLE_COLOR))
            .collect::<Vec<_>>();
        assert!(!pixels.is_empty());
        let min_x = pixels.iter().map(|(x, _)| *x).min().unwrap_or(0);
        let max_x = pixels.iter().map(|(x, _)| *x).max().unwrap_or(0);
        let max_y = pixels.iter().map(|(_, y)| *y).max().unwrap_or(0);
        assert!(max_y < (wall_top - 1) as usize);
        assert!(
            (max_x - min_x + 1) * SIGN_MAX_WIDTH_DENOMINATOR
                <= canvas.width() * SIGN_MAX_WIDTH_NUMERATOR
        );
        assert!(min_x >= SIGN_STATUS_RESERVE_CELLS);
    }

    #[test]
    fn sky_is_deterministic_but_changes_with_time() {
        assert_eq!(sky_color(1_234, 2, 10), sky_color(1_234, 2, 10));
        assert_ne!(
            sky_color(0, 0, 10),
            sky_color((SKY_CYCLE_MS / 4) as Millis, 0, 10)
        );
    }

    #[test]
    fn manager_reaches_attention_pose_after_travel() {
        let origin = (0, 0);
        let target = (20, 10);
        let travel = MANAGER_TRAVEL_MS as i32;
        let x = origin.0 + (target.0 - origin.0) * travel / MANAGER_TRAVEL_MS as i32;
        let y = origin.1 + (target.1 - origin.1) * travel / MANAGER_TRAVEL_MS as i32;
        assert_eq!((x, y), target);
        assert!(
            phase_ms(
                MANAGER_TRAVEL_MS as Millis,
                MANAGER_TRAVEL_MS + MANAGER_HOLD_MS
            ) >= MANAGER_TRAVEL_MS
        );
    }
    #[test]
    fn auto_projection_chooses_room_then_degrades_by_available_space() {
        assert_eq!(
            effective_projection(Projection::Auto, 160, 43),
            Projection::Iso
        );
        assert_eq!(
            effective_projection(Projection::Auto, 96, 25),
            Projection::TopDown
        );
        assert_eq!(
            effective_projection(Projection::Auto, 80, 19),
            Projection::TopDown
        );
        assert_eq!(
            effective_projection(Projection::Auto, 32, 7),
            Projection::List
        );
        assert_eq!(
            effective_projection(Projection::Auto, 110, 20),
            Projection::TopDown
        );
        assert_eq!(
            effective_projection(Projection::Auto, 60, 13),
            Projection::Side
        );
        assert_eq!(
            effective_projection(Projection::Auto, 59, 19),
            Projection::List
        );
        assert_eq!(
            effective_projection(Projection::Side, 80, 50),
            Projection::Side
        );
    }

    #[test]
    fn common_terminal_draws_workers_and_labels_instead_of_a_plain_list() {
        use ratatui::{backend::TestBackend, Terminal};
        let id = OfficeId("/small-office".into());
        let mut office = Office::new(id.clone(), "Small office".into());
        office.workers = (0..5)
            .map(|index| {
                Worker::new(
                    WorkerId(format!("small-{index}")),
                    id.clone(),
                    Agent::Codex,
                    format!("worker {index}"),
                    0,
                )
            })
            .collect();
        for encoding in PixelEncoding::ALL {
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            let mut canvas = Canvas::with_color_depth_and_encoding(
                0,
                0,
                crate::canvas::ColorDepth::TrueColor,
                encoding,
            );
            terminal
                .draw(|frame| {
                    draw(
                        frame,
                        Some(&office),
                        &mut canvas,
                        &SpriteSet::new(),
                        0,
                        0,
                        Projection::Auto,
                        true,
                    );
                })
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(text.contains("top-down"));
            for index in 0..5 {
                assert!(
                    text.contains(&format!("worker {index}")),
                    "missing label at {encoding:?}"
                );
            }
            assert!(
                canvas
                    .pixel_frame()
                    .rgb()
                    .chunks_exact(3)
                    .any(|pixel| pixel == [79, 158, 232]),
                "missing worker clothing at {encoding:?}"
            );
        }
    }
    #[test]
    fn isometric_worker_labels_use_theme_surfaces_over_the_art() {
        use ratatui::{backend::TestBackend, Terminal};
        let id = OfficeId("/label-contrast".into());
        let mut office = Office::new(id.clone(), "Contrast".into());
        for name in ["ALPHA", "BRAVO"] {
            office.workers.push(Worker::new(
                WorkerId(name.into()),
                id.clone(),
                Agent::Codex,
                name.into(),
                0,
            ));
        }
        let mut terminal = Terminal::new(TestBackend::new(120, 36)).unwrap();
        let mut canvas = Canvas::with_color_depth_and_encoding(
            0,
            0,
            crate::canvas::ColorDepth::TrueColor,
            PixelEncoding::HalfBlocks,
        );
        canvas.set_image_cell_size(Some((8, 16)));
        terminal
            .draw(|frame| {
                draw(
                    frame,
                    Some(&office),
                    &mut canvas,
                    &SpriteSet::new(),
                    0,
                    0,
                    Projection::Iso,
                    true,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        for (name, background) in [("ALPHA", PANEL_HIGHLIGHT), ("BRAVO", PANEL)] {
            let row = buffer
                .content
                .chunks(120)
                .find(|row| {
                    row.iter()
                        .map(|cell| cell.symbol())
                        .collect::<String>()
                        .contains(name)
                })
                .expect("worker label should be visible");
            let text = row.iter().map(|cell| cell.symbol()).collect::<String>();
            let start = text.find(name).unwrap();
            assert!(
                row[start..start + name.len()]
                    .iter()
                    .all(|cell| cell.bg == background),
                "{name} must use a semantic theme surface instead of the sampled dark artwork"
            );
        }
    }

    #[test]
    fn project_sign_stays_above_floor_and_within_bounded_area() {
        let mut canvas =
            crate::canvas::Canvas::with_color_depth(80, 40, crate::canvas::ColorDepth::TrueColor);
        draw_flat_project_sign(&mut canvas, "A", 30);
        let sign_colors = [TITLE_COLOR, TITLE_BODY, TITLE_SIGN];
        let mut sign_pixels = 0;
        for x in 0..canvas.width() {
            for y in 0..canvas.height() {
                if let Some(color) = canvas.pixel(x, y) {
                    if sign_colors.contains(&color) {
                        assert!(y < 30, "sign pixel entered floor at ({x}, {y})");
                        sign_pixels += 1;
                    }
                }
            }
        }
        assert!(
            sign_pixels <= canvas.width() * canvas.height() / 3,
            "sign should occupy a modest top band"
        );
    }

    #[test]
    fn light_mode_keeps_floor_outlines_dark() {
        let mut canvas =
            crate::canvas::Canvas::with_color_depth(20, 20, crate::canvas::ColorDepth::TrueColor);
        canvas.set_light_mode(true);
        draw_diamond(&mut canvas, 10, 10, 4, 2, FLOOR_LIGHT, FLOOR_DITHER);
        assert_eq!(canvas.pixel(6, 10), Some(crate::views::LIGHT_INK));
    }

    #[test]
    fn compact_project_sign_keeps_a_flat_face_clear_of_alert_rows() {
        let mut canvas =
            crate::canvas::Canvas::with_color_depth(80, 40, crate::canvas::ColorDepth::TrueColor);
        draw_flat_project_sign(&mut canvas, "A", 30);
        let count_color = |color| {
            (0..canvas.width())
                .flat_map(|x| (0..canvas.height()).map(move |y| (x, y)))
                .filter(|(x, y)| canvas.pixel(*x, *y) == Some(color))
                .count()
        };
        assert!(count_color(TITLE_COLOR) > 0);
        assert_eq!(count_color(TITLE_BODY), 0);
        assert_eq!(count_color(TITLE_SIGN), 0);
        for y in 0..canvas.pixels_per_cell().1 * 2 {
            assert!((0..canvas.width()).all(|x| canvas.pixel(x, y) != Some(TITLE_COLOR)));
        }
    }

    #[test]
    fn flat_project_titles_never_cross_the_native_attention_banner() {
        for (encoding, cell_size) in [
            (PixelEncoding::HalfBlocks, None),
            (PixelEncoding::Quadrants, None),
            (PixelEncoding::Sextants, None),
            (PixelEncoding::HalfBlocks, Some((8, 16))),
        ] {
            for columns in [80, 120] {
                for label in ["00-CHECKOUT", "A-LONG-PROJECT-TITLE"] {
                    let mut canvas = Canvas::with_color_depth_and_encoding(
                        0,
                        0,
                        crate::canvas::ColorDepth::TrueColor,
                        encoding,
                    );
                    canvas.set_cell_pixel_size(cell_size);
                    canvas.resize_for_cells(columns, 18);
                    let (px, py) = canvas.pixels_per_cell();
                    draw_room_project_sign(&mut canvas, label, 0, py * 3);
                    for y in py..py * 2 {
                        for x in 0..px * 36 {
                            assert_ne!(
                                canvas.pixel(x, y),
                                Some(TITLE_COLOR),
                                "title intersects alert at {columns} columns, {encoding:?}, {cell_size:?}: ({x}, {y})"
                            );
                        }
                    }
                    if columns == 120 && cell_size.is_some() && label == "00-CHECKOUT" {
                        assert!(
                            (0..canvas.height()).any(|y| (0..canvas.width())
                                .any(|x| canvas.pixel(x, y) == Some(TITLE_COLOR))),
                            "wide artwork title should remain visible beside the alert"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn worker_plate_names_preserve_distinguishing_suffixes() {
        let labels = (0..5)
            .map(|index| worker_plate_name(&format!("they-work worker {index}"), "they-work"))
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec!["worker 0", "worker 1", "worker 2", "worker 3", "worker 4"]
        );
        assert_eq!(
            labels
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            labels.len()
        );
    }
    #[test]
    fn office_palettes_change_room_materials_but_keep_worker_and_alert_colours() {
        for light in [false, true] {
            let mut floors = std::collections::HashSet::new();
            for palette in 0..4 {
                let mut canvas = Canvas::with_color_depth_and_encoding(
                    4,
                    1,
                    crate::canvas::ColorDepth::TrueColor,
                    PixelEncoding::HalfBlocks,
                );
                canvas.set_light_mode(light);
                for (x, color) in [FLOOR, WALL, Color::Rgb(79, 158, 232), WARNING]
                    .into_iter()
                    .enumerate()
                {
                    canvas.set(x, 0, color);
                }
                let worker = canvas.pixel(2, 0);
                let warning = canvas.pixel(3, 0);
                apply_room_palette(&mut canvas, palette);
                floors.insert(canvas.pixel(0, 0));
                assert_eq!(canvas.pixel(2, 0), worker);
                assert_eq!(canvas.pixel(3, 0), warning);
            }
            assert_eq!(
                floors.len(),
                4,
                "each office palette needs a distinct floor"
            );
        }
    }
    #[test]
    fn resize_plans_have_only_occupied_rows_and_adjacent_nameplates() {
        for encoding in [
            PixelEncoding::HalfBlocks,
            PixelEncoding::Quadrants,
            PixelEncoding::Sextants,
        ] {
            for (width, height) in [(80, 24), (120, 32), (192, 58), (240, 70), (110, 80)] {
                let mut canvas = Canvas::with_color_depth_and_encoding(
                    0,
                    0,
                    crate::canvas::ColorDepth::TrueColor,
                    encoding,
                );
                canvas.resize_for_cells(width, height - 5);
                for count in [1, 3, 20] {
                    let layout = desk_layout(count, width as u16, (height - 5) as u16);
                    for projection in [Projection::TopDown, Projection::Side] {
                        let shown = count.min(if projection == Projection::Side {
                            layout.columns
                        } else {
                            layout.page_size
                        });
                        let plan = FlatRoom::new(&canvas, shown, layout, projection);
                        assert_eq!(plan.stations.len(), shown);
                        assert!(plan.bottom <= canvas.height());
                        let (px, py) = canvas.pixels_per_cell();
                        for station in &plan.stations {
                            assert!(station.center_x >= station.desk_width / 2);
                            assert!(station.center_x + station.desk_width / 2 <= canvas.width());
                            assert!(
                                station.worker_y >= plan.top,
                                "{encoding:?}/{width}x{height}/{count}/{projection:?}: {station:?}"
                            );
                            assert!(
                                station.worker_y + station.worker_height <= station.label_y + py
                            );
                            assert!(station.label_y < plan.bottom);
                            assert!(
                                station.label_y - (station.desk_y + station.desk_height) <= py * 2
                            );
                            assert!(station.worker_width >= px * 3);
                        }
                        for (index, first) in plan.stations.iter().enumerate() {
                            for second in plan.stations.iter().skip(index + 1) {
                                assert!(
                                    first.label_y != second.label_y
                                        || first.center_x.abs_diff(second.center_x)
                                            >= first.label_width * px
                                );
                            }
                        }
                    }
                    if count <= 3 {
                        assert_eq!(layout.rows, 1);
                    }
                }
            }
        }
    }

    #[test]
    fn shared_prefixes_and_duplicate_titles_keep_distinguishable_nameplates() {
        let names = [
            "Daily POD Gmail Reconciliation - deliveries",
            "Daily POD Gmail Reconciliation - invoices",
            "Daily POD Gmail Reconciliation - receipts",
        ];
        let mut workers = names
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                Worker::new(
                    WorkerId(format!("conversation-{index}")),
                    OfficeId("/office".into()),
                    Agent::Codex,
                    name.into(),
                    0,
                )
            })
            .collect::<Vec<_>>();
        for width in [10, 18, 26] {
            let references = workers.iter().collect::<Vec<_>>();
            let labels = (0..workers.len())
                .map(|slot| unique_plate_name(&references, slot, "project", width))
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(labels.len(), workers.len());
            assert!(labels.iter().all(|label| label.chars().count() <= width));
        }
        for worker in &mut workers {
            worker.name = "Identical title".into();
        }
        let references = workers.iter().collect::<Vec<_>>();
        let labels = (0..workers.len())
            .map(|slot| unique_plate_name(&references, slot, "project", 18))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(labels.len(), workers.len());
        assert!(labels.iter().all(|label| label.contains('#')));
    }

    #[test]
    fn room_motion_freezes_sky_figures_and_manager_together() {
        let worker = Worker::new(
            WorkerId("quiet-worker".into()),
            OfficeId("/office".into()),
            Agent::Codex,
            "Quiet worker".into(),
            0,
        );
        let mut office = Office::new(OfficeId("/office".into()), "/office".into());
        office.workers.push(worker);
        let refs = office.workers.iter().collect::<Vec<_>>();
        let looks = worker_looks(&office.workers);
        let sprites = SpriteSet::new();
        sprites.set_animation_time(Some(0));
        let mut canvas = Canvas::with_color_depth_and_encoding(
            0,
            0,
            crate::canvas::ColorDepth::TrueColor,
            PixelEncoding::HalfBlocks,
        );
        canvas.resize_for_cells(120, 38);
        for projection in [Projection::Iso, Projection::TopDown, Projection::Side] {
            let mut frames = Vec::new();
            for now in [20000, 25000, 31000] {
                match projection {
                    Projection::Iso => {
                        draw_room_scene(&mut canvas, "Office", &refs, &looks, &sprites, now, None);
                    }
                    Projection::TopDown => draw_top_down_scene(
                        &mut canvas,
                        &office,
                        &refs,
                        &looks,
                        &sprites,
                        now,
                        desk_layout(1, 120, 38),
                    ),
                    _ => draw_side_scene(&mut canvas, &office, &refs, &looks, &sprites, now),
                }
                frames.push(
                    (0..canvas.height())
                        .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
                        .map(|(x, y)| canvas.pixel(x, y))
                        .collect::<Vec<_>>(),
                );
            }
            assert_eq!(
                frames[0], frames[1],
                "{projection:?} ignores reduced motion"
            );
            assert_eq!(
                frames[1], frames[2],
                "{projection:?} ignores reduced motion"
            );
        }
    }

    #[test]
    fn clouds_never_draw_outside_their_window() {
        let mut canvas = Canvas::with_color_depth(80, 40, crate::canvas::ColorDepth::TrueColor);
        for now in [0, 1000, 3000, 8000, 15000] {
            canvas.fill(WALL);
            draw_sky_area(&mut canvas, 20, 10, 30, 20, now);
            for y in 0..40 {
                for x in 0..80 {
                    if !(20..50).contains(&x) || !(10..30).contains(&y) {
                        assert_eq!(canvas.pixel(x, y), Some(WALL));
                    }
                }
            }
        }
    }

    #[test]
    fn blocked_floor_labels_avoid_the_manager_at_the_rendered_time() {
        let mut workers = (0..5)
            .map(|slot| {
                Worker::new(
                    WorkerId(format!("thread-{slot}")),
                    OfficeId("/office".into()),
                    Agent::Codex,
                    format!("Worker {slot}"),
                    0,
                )
            })
            .collect::<Vec<_>>();
        workers[0].activity = theywork_core::Activity::Waiting {
            detail: "Review".into(),
        };
        let refs = workers.iter().collect::<Vec<_>>();
        let looks = worker_looks(&workers);
        let sprites = SpriteSet::new();
        for encoding in [
            PixelEncoding::HalfBlocks,
            PixelEncoding::Quadrants,
            PixelEncoding::Sextants,
        ] {
            let mut canvas = Canvas::with_color_depth_and_encoding(
                0,
                0,
                crate::canvas::ColorDepth::TrueColor,
                encoding,
            );
            canvas.resize_for_cells(160, 44);
            for now in [0, 1200, 2400, 4000] {
                let grid =
                    draw_room_scene(&mut canvas, "Office", &refs, &looks, &sprites, now, None);
                let body = Rect::new(0, 0, 160, 44);
                for slot in 0..workers.len() {
                    let rect = worker_plate_rect(body, grid, slot, workers.len(), Some(0), now);
                    let obstacles = plate_obstacles(body, grid, slot, workers.len(), Some(0), now);
                    assert!(
                        obstacles
                            .iter()
                            .all(|obstacle| !rects_overlap(rect, *obstacle)),
                        "{encoding:?} at {now} plate {slot} covers a foreground figure"
                    );
                }
            }
        }
    }

    #[test]
    fn isometric_floor_is_a_continuous_surface_at_odd_pixel_sizes() {
        for encoding in [
            PixelEncoding::HalfBlocks,
            PixelEncoding::Quadrants,
            PixelEncoding::Sextants,
        ] {
            for count in [1, 3, 10] {
                let mut canvas = Canvas::with_color_depth_and_encoding(
                    0,
                    0,
                    crate::canvas::ColorDepth::TrueColor,
                    encoding,
                );
                canvas.resize_for_cells(191, 53);
                let grid = make_room_grid(&canvas, count);
                draw_floor_plate(&mut canvas, grid);
                for y in 0..canvas.height() {
                    let occupied = (0..canvas.width())
                        .filter(|x| canvas.pixel(*x, y).is_some())
                        .collect::<Vec<_>>();
                    if let (Some(first), Some(last)) = (occupied.first(), occupied.last()) {
                        assert!(
                            (*first..=*last).all(|x| canvas.pixel(x, y).is_some()),
                            "{encoding:?}/{count} floor crack in row {y}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn midnight_floor_seams_survive_the_256_colour_terminal_palette() {
        let mut canvas = Canvas::with_color_depth_and_encoding(
            0,
            0,
            crate::canvas::ColorDepth::Palette256,
            PixelEncoding::HalfBlocks,
        );
        canvas.resize_for_cells(80, 24);
        draw_flat_floor(&mut canvas, 0, 48);
        apply_room_palette(&mut canvas, 1);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                canvas.render(frame.buffer_mut(), area);
            })
            .expect("frame");
        let floor = terminal.backend().buffer()[(1, 2)].fg;
        let seam = terminal.backend().buffer()[(1, 5)].bg;
        assert_ne!(
            floor, seam,
            "floor and seam collapse to the same indexed colour"
        );
    }

    #[test]
    fn compact_isometric_floor_pages_before_workers_overlap() {
        let mut office = Office::new(OfficeId("/office".into()), "/office".into());
        office.workers = (0..20)
            .map(|slot| {
                Worker::new(
                    WorkerId(format!("thread-{slot}")),
                    office.id.clone(),
                    Agent::Codex,
                    format!("Desk {slot}"),
                    0,
                )
            })
            .collect();
        let mut canvas = Canvas::with_color_depth_and_encoding(
            0,
            0,
            crate::canvas::ColorDepth::TrueColor,
            PixelEncoding::HalfBlocks,
        );
        let sprites = SpriteSet::new();
        for (width, height, capacity) in [(80, 24, 3), (120, 32, 5), (192, 58, 10)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
            let mut actual = OfficeLayout::default();
            terminal
                .draw(|frame| {
                    actual = draw(
                        frame,
                        Some(&office),
                        &mut canvas,
                        &sprites,
                        0,
                        19,
                        Projection::Iso,
                        true,
                    );
                })
                .expect("last page");
            assert_eq!(actual.page_size, capacity);
            assert_eq!(actual.pages, 20usize.div_ceil(capacity));
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(
                text.contains("Desk 19"),
                "{width}x{height} lost last selected worker"
            );
        }
    }
    #[test]
    fn isometric_navigation_reaches_last_worker_and_keeps_identity_through_resize() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut world = theywork_core::World::new();
        for index in 0..20 {
            world.apply(theywork_core::Event {
                at: 0,
                office: OfficeId("/office".into()),
                office_path: "/office".into(),
                worker: WorkerId(format!("thread-{index:02}")),
                agent: Agent::Codex,
                kind: theywork_core::EventKind::Seen {
                    name: format!("Desk {index:02}"),
                    git_branch: None,
                },
            });
        }
        let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
        for (width, height, columns) in [(80, 24, 3), (120, 32, 5), (192, 58, 5)] {
            let mut ui = crate::Ui::new();
            ui.set_image_cell_size(Some((8, 16)));
            ui.restore_preferences(&crate::RendererPreferences {
                projection: "isometric".into(),
                ..Default::default()
            });
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            let capacity = if width >= 192 { 10 } else { columns };
            for _ in 0..capacity {
                ui.handle_key(press(KeyCode::Right));
            }
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(ui.selected_worker(), capacity);
            ui.handle_key(press(KeyCode::End));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(ui.selected_worker(), 19);
            for _ in 0..capacity {
                ui.handle_key(press(KeyCode::Left));
            }
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(ui.selected_worker(), 19 - capacity);
            ui.handle_key(press(KeyCode::Home));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(ui.selected_worker(), 0);
            ui.handle_key(press(KeyCode::Char('j')));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(ui.selected_worker(), columns);
            ui.handle_key(press(KeyCode::Char('k')));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(ui.selected_worker(), 0);
            for index in 1..20 {
                ui.handle_key(press(KeyCode::Right));
                terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
                assert_eq!(ui.selected_worker(), index);
            }
            for (new_width, new_height) in [(192, 58), (80, 24), (120, 32)] {
                terminal.backend_mut().resize(new_width, new_height);
                terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
                assert_eq!(
                    ui.selected_worker(),
                    19,
                    "resize changed the selected conversation"
                );
                let text = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                assert!(text.contains("Desk 19"));
            }
            ui.handle_key(press(KeyCode::Enter));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(ui.view(), crate::View::Desk);
            assert_eq!(ui.selected_worker(), 19);
            ui.handle_key(press(KeyCode::Esc));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(ui.view(), crate::View::Office);
            ui.handle_key(press(KeyCode::Char('k')));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(
                ui.selected_worker(),
                14,
                "latest resized layout must determine vertical navigation"
            );
            ui.handle_key(press(KeyCode::Char('l')));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(ui.selected_worker(), 15);
        }
    }
}
