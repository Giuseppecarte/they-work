//! One project's dense isometric office floor.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Frame;
use theywork_core::{Millis, Office, Worker, WorkerStatus};

use crate::canvas::Canvas;
use crate::sprite::{worker_looks, Sprite, SpriteSet, WorkerLook};
use crate::views::render_worker_with_look;

use super::{
    draw_footer, draw_header, draw_tiny, has_area, short_path, status_color, worker_status, ACCENT,
    FLOOR, INK, MUTED, PANEL, PANEL_HIGHLIGHT, WALL, WARNING,
};

const MAX_DESKS: usize = 10;
const MIN_WIDTH: u16 = 16;
const MIN_HEIGHT: u16 = 7;
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
const RUG: Color = Color::Rgb(42, 36, 64);
const RUG_LIGHT: Color = Color::Rgb(58, 51, 88);
const MACHINE: Color = Color::Rgb(42, 36, 64);
const MACHINE_LIGHT: Color = Color::Rgb(138, 130, 153);
const TITLE_SIGN: Color = Color::Rgb(92, 15, 12);
const TITLE_BODY: Color = Color::Rgb(142, 26, 21);
const TITLE_COLOR: Color = Color::Rgb(232, 52, 44);
const NIGHT_SKY_TOP: Color = Color::Rgb(13, 11, 20);
const NIGHT_SKY_BOTTOM: Color = Color::Rgb(58, 51, 88);
const DAY_SKY_TOP: Color = Color::Rgb(88, 214, 232);
const DAY_SKY_BOTTOM: Color = Color::Rgb(90, 169, 201);

fn outline_color(canvas: &Canvas) -> Color {
    if canvas.is_light_mode() {
        super::LIGHT_INK
    } else {
        OUTLINE
    }
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
}

impl Projection {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Auto => Self::Iso,
            Self::Iso => Self::TopDown,
            Self::TopDown => Self::Side,
            Self::Side => Self::Auto,
        }
    }

    pub(crate) fn previous(self) -> Self {
        match self {
            Self::Auto => Self::Side,
            Self::Iso => Self::Auto,
            Self::TopDown => Self::Iso,
            Self::Side => Self::TopDown,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Iso => "isometric",
            Self::TopDown => "top-down",
            Self::Side => "side",
        }
    }
}

pub(crate) fn effective_projection(projection: Projection, width: u16, height: u16) -> Projection {
    match projection {
        Projection::Auto if height >= width / 2 => Projection::TopDown,
        Projection::Auto if width >= 110 && height <= 22 => Projection::Side,

        Projection::Auto => Projection::Iso,
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
    let rows = if height >= 14 { 2 } else { 1 };
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
    Rug,
    Cable,
    Plant,
    Cooler,
    Coffee,
    Printer,
    MeetingTable,
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
        IsoKind::Rug => 0,
        IsoKind::Cable => 1,
        IsoKind::Plant | IsoKind::Cooler | IsoKind::Coffee | IsoKind::Printer => 2,
        IsoKind::MeetingTable => 3,
        IsoKind::Worker(_) => 4,
        IsoKind::Manager => 5,
        IsoKind::Desk(_) => 6,
    }
}

fn painter_order(items: &mut [IsoItem]) {
    items.sort_by_key(|item| {
        (
            item.tile_x
                .saturating_add(item.tile_y)
                .saturating_add(usize::from(item.footprint.depth)),
            item.tile_y,
            kind_rank(item.kind),
        )
    });
}

#[derive(Debug, Clone, Copy)]
struct IsoGrid {
    columns: usize,
    rows: usize,
    tile_width: i32,
    tile_height: i32,
    origin_x: i32,
    origin_y: i32,
}

impl IsoGrid {
    fn center(self, tile_x: usize, tile_y: usize) -> (i32, i32) {
        (
            self.origin_x + (tile_x as i32 - tile_y as i32) * self.tile_width / 2,
            self.origin_y + (tile_x as i32 + tile_y as i32) * self.tile_height / 2,
        )
    }

    fn desk_tile(self, slot: usize) -> (usize, usize) {
        (slot % self.columns, slot / self.columns)
    }
}

fn make_grid(width: usize, height: usize, columns: usize, rows: usize) -> IsoGrid {
    let columns = columns.max(1);
    let rows = rows.max(1);
    let tile_width =
        (width.saturating_mul(2) / columns.saturating_add(rows).max(1)).clamp(8, 18) as i32;
    let tile_height = if height < 24 {
        2
    } else {
        (tile_width / 3).clamp(3, 6)
    };
    let floor_extent = ((columns + rows) as i32 * tile_height + 1) / 2;
    let floor_top = height.saturating_sub(floor_extent.max(1) as usize + 1) as i32;
    let origin_x = width as i32 / 2 - (columns as i32 - rows as i32) * tile_width / 4;
    IsoGrid {
        columns,
        rows,
        tile_width,
        tile_height,
        origin_x,
        origin_y: floor_top + tile_height / 2,
    }
}

fn phase_ms(now: Millis, period: u64) -> u64 {
    if period == 0 {
        return 0;
    }
    now.max(0) as u64 % period
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
    draw_line(canvas, top.0, top.1, left.0, left.1, FLOOR_LIGHT);
    draw_line(canvas, top.0, top.1, right.0, right.1, WINDOW_LIGHT);
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
        set_pixel(
            canvas,
            cloud_x + offset,
            cloud_y,
            blend_color(WALL, INK, daylight),
        );
    }
    for offset in [1, 2, 4, 5, 6] {
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

fn draw_backdrop(canvas: &mut Canvas, floor_top: usize, now: Millis) {
    canvas.fill(WALL);
    let width = canvas.width() as i32;
    let floor_top = floor_top.min(canvas.height()) as i32;
    if width <= 0 || floor_top <= 0 {
        return;
    }

    let window_count = if width >= 54 {
        3
    } else if width >= 28 {
        2
    } else {
        1
    };
    let gap = 2_i32;
    let window_width = ((width - gap * (window_count + 1)) / window_count).max(5);
    let window_y = if floor_top >= 13 {
        7
    } else {
        (floor_top / 3).max(1)
    };
    let window_height = floor_top.saturating_sub(window_y + 2);
    for index in 0..window_count {
        let x = gap + index * (window_width + gap);
        draw_window(
            canvas,
            x,
            window_y,
            window_width.min(width.saturating_sub(x)),
            window_height,
            now + index as Millis * 1_700,
        );
    }

    draw_line(
        canvas,
        0,
        floor_top.saturating_sub(1),
        width - 1,
        floor_top.saturating_sub(1),
        WALL_LIGHT,
    );
    for column in (3..width.max(3)).step_by(11) {
        set_pixel(canvas, column, floor_top.saturating_sub(2), WALL_LIGHT);
    }

    let machine_y = floor_top.saturating_sub(5);
    if width >= 24 && machine_y > window_y {
        let machine_width = (width / 12).clamp(4, 7);
        draw_rect_outline(
            canvas,
            2,
            machine_y,
            machine_width,
            4,
            outline_color(canvas),
        );
        fill_rect(canvas, 3, machine_y + 1, machine_width - 2, 2, MACHINE);
        set_pixel(canvas, 4, machine_y + 1, MACHINE_LIGHT);
        if width >= 42 {
            let printer_x = width - machine_width - 3;
            draw_rect_outline(
                canvas,
                printer_x,
                machine_y,
                machine_width,
                4,
                outline_color(canvas),
            );
            fill_rect(
                canvas,
                printer_x + 1,
                machine_y + 1,
                machine_width - 2,
                2,
                MACHINE,
            );
            set_pixel(canvas, printer_x + 2, machine_y + 2, INK);
        }
    }
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

fn block_text_step(scale: usize) -> usize {
    6usize.saturating_mul(scale.max(1))
}

fn block_text_width(text: &str, scale: usize) -> usize {
    let count = text.chars().count();
    count
        .saturating_mul(block_text_step(scale))
        .saturating_sub(scale.max(1))
}

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

fn draw_project_name(canvas: &mut Canvas, label: &str, floor_top: usize) {
    if canvas.width() == 0 || canvas.height() == 0 {
        return;
    }
    let lines = sign_lines(label);
    let scale = if lines.len() == 1
        && lines[0].chars().count() <= 8
        && block_text_width(&lines[0], 2) <= canvas.width()
    {
        2
    } else {
        1
    };
    let line_pitches = lines
        .iter()
        .map(|line| 7 * scale + block_text_width(line, scale) / 2 + 2)
        .collect::<Vec<_>>();
    let total_height = line_pitches.iter().sum::<usize>().saturating_sub(2);
    let mut line_top = floor_top.saturating_sub(total_height.saturating_add(1));
    for (line_index, line) in lines.iter().enumerate() {
        let width = block_text_width(line, scale);
        if width == 0 {
            continue;
        }
        let x0 = canvas.width().saturating_sub(width) / 2;
        let board_left = x0 as i32 - 7;
        let board_top = line_top.saturating_sub(1) as i32;
        let board_height = 7 * scale + width / 2 + 8;
        fill_rect(
            canvas,
            board_left,
            board_top,
            width as i32 + 14,
            board_height as i32,
            TITLE_SIGN,
        );
        for (character_index, character) in line.chars().enumerate() {
            let glyph = glyph_5x7(character);
            let glyph_x = x0 + character_index * block_text_step(scale);
            for (row_index, bits) in glyph.iter().enumerate() {
                for column_index in 0..5 {
                    if bits & (1 << (4 - column_index)) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let x = glyph_x + column_index * scale + sx;
                            let logical_x = x.saturating_sub(x0);
                            let y = line_top + row_index * scale + sy + logical_x / 2;
                            for depth in (1..=6).rev() {
                                let offset = depth * 2;
                                let color = if depth >= 4 { TITLE_SIGN } else { TITLE_BODY };
                                set_pixel(canvas, x as i32 - offset, y as i32 + offset, color);
                            }
                            set_pixel(canvas, x as i32, y as i32, TITLE_COLOR);
                        }
                    }
                }
            }
        }
        line_top = line_top.saturating_add(line_pitches[line_index]);
    }
}

pub(crate) fn draw_project_sign(canvas: &mut Canvas, name: &str, floor_top: usize) {
    draw_project_name(canvas, &project_label(name), floor_top);
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

fn draw_floor_plate(canvas: &mut Canvas, grid: IsoGrid) {
    for depth in 0..grid.columns.saturating_add(grid.rows) {
        for tile_y in 0..grid.rows {
            if depth < tile_y {
                continue;
            }
            let tile_x = depth - tile_y;
            if tile_x >= grid.columns {
                continue;
            }
            let (center_x, center_y) = grid.center(tile_x, tile_y);
            let base = if (tile_x + tile_y).is_multiple_of(2) {
                FLOOR
            } else {
                FLOOR_DARK
            };
            draw_diamond(
                canvas,
                center_x,
                center_y,
                grid.tile_width / 2,
                grid.tile_height / 2,
                base,
                FLOOR_DITHER,
            );
            if grid.tile_width >= 12 {
                draw_line(
                    canvas,
                    center_x - grid.tile_width / 4,
                    center_y,
                    center_x,
                    center_y + grid.tile_height / 3,
                    FLOOR_DARK,
                );
            }
        }
    }
}

fn draw_machine(
    canvas: &mut Canvas,
    center_x: i32,
    center_y: i32,
    width: i32,
    height: i32,
    accent: Color,
) {
    let x = center_x - width / 2;
    let y = center_y - height;
    fill_rect(canvas, x, y, width, height, MACHINE);
    draw_rect_outline(canvas, x, y, width, height, outline_color(canvas));
    if width > 3 && height > 2 {
        fill_rect(canvas, x + 1, y + 1, width - 2, 1, accent);
        set_pixel(canvas, x + width / 2, y + height - 2, MACHINE_LIGHT);
    }
}

fn draw_table(canvas: &mut Canvas, grid: IsoGrid, center_x: i32, center_y: i32) {
    let half_width = (grid.tile_width / 3).max(2);
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
    draw_diamond(
        canvas,
        center_x,
        center_y,
        (grid.tile_width / 2).saturating_sub(2).max(1),
        (grid.tile_height / 2).saturating_sub(1).max(1),
        RUG,
        RUG_LIGHT,
    );
}

fn draw_item(
    canvas: &mut Canvas,
    grid: IsoGrid,
    item: IsoItem,
    workers: &[&Worker],
    looks: &[WorkerLook],
    sprites: &SpriteSet,
    now: Millis,
) {
    let (base_x, base_y) = grid.center(item.tile_x, item.tile_y);
    let prop_separation = if grid.columns <= 4 {
        grid.tile_width / 4
    } else {
        0
    };
    let (center_x, center_y) = match item.kind {
        IsoKind::Plant | IsoKind::Cooler => (base_x - prop_separation, base_y),
        IsoKind::Coffee | IsoKind::Printer => (base_x + prop_separation, base_y),
        _ => (base_x, base_y),
    };
    match item.kind {
        IsoKind::Rug => draw_rug(canvas, grid, center_x, center_y),
        IsoKind::Cable => {
            let (next_x, next_y) = grid.center(
                item.tile_x
                    .saturating_add(1)
                    .min(grid.columns.saturating_sub(1)),
                item.tile_y,
            );
            draw_line(
                canvas,
                center_x,
                center_y,
                next_x,
                next_y,
                outline_color(canvas),
            );
            if grid.tile_width >= 12 {
                set_pixel(canvas, center_x + 2, center_y - 1, ACCENT);
            }
        }
        IsoKind::Plant => {
            let width = (grid.tile_width / 2).clamp(4, 9) as usize;
            let height = (grid.tile_height + 6).clamp(5, 12) as usize;
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
            let width = (grid.tile_width / 2).clamp(4, 9) as usize;
            let height = (grid.tile_height + 7).clamp(6, 13) as usize;
            blit_scaled_signed(
                canvas,
                &sprites.water_cooler,
                center_x - width as i32 / 2,
                center_y - height as i32,
                width,
                height,
            );
        }
        IsoKind::Coffee => {
            draw_machine(
                canvas,
                center_x,
                center_y,
                (grid.tile_width / 2).clamp(4, 8),
                (grid.tile_height + 3).clamp(4, 8),
                TABLE_LIGHT,
            );
            set_pixel(canvas, center_x, center_y - 2, INK);
        }
        IsoKind::Printer => {
            draw_machine(
                canvas,
                center_x,
                center_y,
                (grid.tile_width / 2).clamp(4, 8),
                (grid.tile_height + 2).clamp(4, 8),
                ACCENT,
            );
            set_pixel(canvas, center_x - 1, center_y - 1, INK);
        }
        IsoKind::MeetingTable => draw_table(canvas, grid, center_x, center_y),
        IsoKind::Worker(index) => {
            let Some(worker) = workers.get(index) else {
                return;
            };
            let Some(look) = looks.get(index) else {
                return;
            };
            let sprite = sprites.worker_frame(worker, *look, now);
            let width = (grid.tile_width * 2 / 3).clamp(5, 12) as usize;
            let height = (grid.tile_height * 2 + 5).clamp(6, 14) as usize;
            blit_scaled_signed(
                canvas,
                &sprite,
                center_x - width as i32 / 2,
                center_y - grid.tile_height / 2 - height as i32 + 2,
                width,
                height,
            );
        }
        IsoKind::Manager => {
            let origin = grid.center(0, grid.rows.saturating_sub(1));
            let target = (center_x, center_y);
            let cycle = MANAGER_TRAVEL_MS + MANAGER_HOLD_MS;
            let phase = phase_ms(now, cycle);
            let travel = phase.min(MANAGER_TRAVEL_MS);
            let manager_x =
                origin.0 + (target.0 - origin.0) * travel as i32 / MANAGER_TRAVEL_MS as i32;
            let manager_y =
                origin.1 + (target.1 - origin.1) * travel as i32 / MANAGER_TRAVEL_MS as i32;
            let attention = phase >= MANAGER_TRAVEL_MS;
            let sprite = sprites.manager_animation(attention).frame_at(now);
            let width = (grid.tile_width * 2 / 3).clamp(5, 11) as usize;
            let height = (grid.tile_height * 2 + 5).clamp(6, 14) as usize;
            blit_scaled_signed(
                canvas,
                sprite,
                manager_x - width as i32 / 2,
                manager_y - grid.tile_height / 2 - height as i32 + 2,
                width,
                height,
            );
            if attention {
                set_pixel(
                    canvas,
                    manager_x + width as i32 / 2 + 1,
                    manager_y - height as i32,
                    WARNING,
                );
            }
        }
        IsoKind::Desk(_) => {
            let width = (grid.tile_width - 3).clamp(5, 16) as usize;
            let height = (grid.tile_height + 2).clamp(3, 8) as usize;
            blit_scaled_signed(
                canvas,
                &sprites.desk,
                center_x - width as i32 / 2,
                center_y + grid.tile_height / 3 - height as i32 / 2,
                width,
                height,
            );
        }
    }
}

fn draw_scene(
    canvas: &mut Canvas,
    office: &Office,
    visible_workers: &[&Worker],
    looks: &[WorkerLook],
    sprites: &SpriteSet,
    now: Millis,
    layout: OfficeLayout,
) -> IsoGrid {
    canvas.clear();
    let grid = make_grid(
        canvas.width(),
        canvas.height(),
        layout.columns,
        layout.rows.saturating_add(1),
    );
    let floor_top = grid.origin_y.saturating_sub(grid.tile_height / 2).max(0) as usize;
    draw_backdrop(canvas, floor_top, now);
    draw_project_name(canvas, &project_label(&office.name), floor_top);
    draw_floor_plate(canvas, grid);

    let mut items = Vec::with_capacity(visible_workers.len().saturating_mul(3) + 8);
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
            kind: IsoKind::Rug,
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

    let prop_row = grid.rows.saturating_sub(1);
    let middle = grid.columns / 2;
    items.extend([
        IsoItem {
            tile_x: 0,
            tile_y: prop_row,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 2,
            },
            kind: IsoKind::Plant,
        },
        IsoItem {
            tile_x: grid.columns.saturating_sub(1),
            tile_y: prop_row,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 2,
            },
            kind: IsoKind::Cooler,
        },
        IsoItem {
            tile_x: middle,
            tile_y: prop_row,
            footprint: IsoFootprint {
                width: 2,
                depth: 1,
                height: 1,
            },
            kind: IsoKind::MeetingTable,
        },
        IsoItem {
            tile_x: middle.saturating_sub(1),
            tile_y: prop_row,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 1,
            },
            kind: IsoKind::Coffee,
        },
        IsoItem {
            tile_x: middle.saturating_add(1).min(grid.columns.saturating_sub(1)),
            tile_y: prop_row,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 1,
            },
            kind: IsoKind::Printer,
        },
    ]);

    if grid.columns > 1 {
        items.push(IsoItem {
            tile_x: 0,
            tile_y: 0,
            footprint: IsoFootprint {
                width: 1,
                depth: 1,
                height: 0,
            },
            kind: IsoKind::Cable,
        });
    }

    if let Some((slot, _)) = visible_workers
        .iter()
        .enumerate()
        .find(|(_, worker)| worker_status(worker, now) == WorkerStatus::Blocked)
    {
        let (tile_x, tile_y) = grid.desk_tile(slot);
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
        draw_item(canvas, grid, item, visible_workers, looks, sprites, now);
    }
    grid
}

fn draw_top_down_scene(
    canvas: &mut Canvas,
    office: &Office,
    visible_workers: &[&Worker],
    looks: &[WorkerLook],
    sprites: &SpriteSet,
    now: Millis,
    layout: OfficeLayout,
) {
    canvas.clear();
    let floor_top = (canvas.height() / 3).max(4).min(canvas.height());
    draw_backdrop(canvas, floor_top, now);
    draw_project_name(canvas, &project_label(&office.name), floor_top);
    fill_rect(
        canvas,
        0,
        floor_top as i32,
        canvas.width() as i32,
        canvas.height().saturating_sub(floor_top) as i32,
        FLOOR_LIGHT,
    );
    draw_line(
        canvas,
        0,
        floor_top as i32,
        canvas.width() as i32 - 1,
        floor_top as i32,
        outline_color(canvas),
    );

    let columns = layout.columns.max(1);
    let rows = layout.rows.max(1);
    let floor_height = canvas.height().saturating_sub(floor_top).max(1);
    let cell_width = (canvas.width() / columns).max(1);
    let cell_height = (floor_height / rows).max(1);
    for row in 0..rows {
        for column in 0..columns {
            let cell_x = column.saturating_mul(cell_width);
            let cell_y = floor_top.saturating_add(row.saturating_mul(cell_height));
            let width = if column + 1 == columns {
                canvas.width().saturating_sub(cell_x)
            } else {
                cell_width
            };
            let height = if row + 1 == rows {
                canvas.height().saturating_sub(cell_y)
            } else {
                cell_height
            };
            let base = if (row + column).is_multiple_of(2) {
                FLOOR_LIGHT
            } else {
                FLOOR_DITHER
            };
            fill_rect(
                canvas,
                cell_x as i32,
                cell_y as i32,
                width as i32,
                height as i32,
                base,
            );
            draw_rect_outline(
                canvas,
                cell_x as i32,
                cell_y as i32,
                width as i32,
                height as i32,
                outline_color(canvas),
            );
        }
    }

    for (slot, worker) in visible_workers.iter().enumerate() {
        let column = slot % columns;
        let row = slot / columns;
        let cell_x = column.saturating_mul(cell_width);
        let cell_y = floor_top.saturating_add(row.saturating_mul(cell_height));
        let Some(look) = looks.get(slot) else {
            continue;
        };
        let worker_width = cell_width.saturating_sub(3).clamp(5, 12);
        let worker_height = cell_height.saturating_sub(5).clamp(6, 18);
        let worker_x = cell_x.saturating_add(cell_width.saturating_sub(worker_width) / 2);
        let worker_y = cell_y.saturating_add(cell_height.saturating_sub(worker_height + 4));
        render_worker_with_look(
            canvas,
            sprites,
            worker,
            look,
            now,
            super::PixelRect {
                x: worker_x,
                y: worker_y,
                width: worker_width,
                height: worker_height,
            },
        );
        let desk_width = cell_width.saturating_sub(2).clamp(5, 16);
        let desk_x = cell_x.saturating_add(cell_width.saturating_sub(desk_width) / 2);
        let desk_y = cell_y.saturating_add(cell_height.saturating_sub(4));
        canvas.blit_scaled(&sprites.desk, desk_x, desk_y, desk_width, 4);
    }

    if canvas.width() >= 8 {
        let prop_height = 6.min(canvas.height().saturating_sub(floor_top));
        canvas.blit_scaled(
            &sprites.plant,
            1,
            floor_top.saturating_add(1),
            6,
            prop_height.max(1),
        );
    }
}

fn draw_side_scene(
    canvas: &mut Canvas,
    office: &Office,
    visible_workers: &[&Worker],
    looks: &[WorkerLook],
    sprites: &SpriteSet,
    now: Millis,
) {
    canvas.clear();
    let floor_top = canvas.height().saturating_mul(2) / 3;
    draw_backdrop(canvas, floor_top, now);
    draw_project_name(canvas, &project_label(&office.name), floor_top);
    fill_rect(
        canvas,
        0,
        floor_top as i32,
        canvas.width() as i32,
        canvas.height().saturating_sub(floor_top) as i32,
        FLOOR_DARK,
    );
    draw_line(
        canvas,
        0,
        floor_top as i32,
        canvas.width() as i32 - 1,
        floor_top as i32,
        FLOOR_LIGHT,
    );
    if canvas.height() > floor_top + 1 {
        draw_line(
            canvas,
            0,
            floor_top as i32 + 1,
            canvas.width() as i32 - 1,
            floor_top as i32 + 1,
            outline_color(canvas),
        );
    }

    let count = visible_workers.len().max(1);
    for (slot, worker) in visible_workers.iter().enumerate() {
        let Some(look) = looks.get(slot) else {
            continue;
        };
        let x = (slot + 1).saturating_mul(canvas.width()) / (count + 1);
        let desk_width = (canvas.width() / count).clamp(7, 18);
        let desk_x = x.saturating_sub(desk_width / 2);
        let desk_y = floor_top.saturating_sub(4);
        canvas.blit_scaled(&sprites.desk, desk_x, desk_y, desk_width, 4);
        let worker_width = desk_width.saturating_sub(2).clamp(5, 12);
        let worker_height = floor_top.saturating_sub(6).clamp(6, 18);
        let worker_x = x.saturating_sub(worker_width / 2);
        let worker_y = floor_top.saturating_sub(worker_height + 5);
        render_worker_with_look(
            canvas,
            sprites,
            worker,
            look,
            now,
            super::PixelRect {
                x: worker_x,
                y: worker_y,
                width: worker_width,
                height: worker_height,
            },
        );
    }
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

fn draw_nameplates(
    frame: &mut Frame,
    body: Rect,
    grid: IsoGrid,
    workers: &[&Worker],
    start: usize,
    selected: usize,
    now: Millis,
) {
    if body.width == 0 || body.height == 0 {
        return;
    }
    let max_label_width = (grid.tile_width.max(6) as u16 / 2).max(6).min(body.width);
    for (slot, worker) in workers.iter().enumerate() {
        let (center_x, center_y) = grid.desk_tile(slot);
        let (center_x, center_y) = grid.center(center_x, center_y);
        let label_width = max_label_width.min(body.width).max(1);
        let mut prefix = " ";
        let status = worker_status(worker, now);
        if start + slot == selected {
            prefix = ">";
        } else if status == WorkerStatus::Blocked {
            prefix = "!";
        } else if status == WorkerStatus::Failed {
            prefix = "×";
        }
        let name = short_path(&worker.name, usize::from(label_width.saturating_sub(2)));
        let label = format!("{prefix} {name}");
        let x = (center_x - i32::from(label_width) / 2)
            .clamp(0, i32::from(body.width.saturating_sub(label_width)));
        let y_px = center_y + grid.tile_height / 2 + 2;
        let y = y_px
            .div_euclid(2)
            .clamp(0, i32::from(body.height.saturating_sub(1)));
        let style = if start + slot == selected {
            Style::default().fg(INK).bg(PANEL_HIGHLIGHT)
        } else {
            Style::default().fg(status_color(status)).bg(PANEL)
        };
        Paragraph::new(Line::from(Span::styled(label, style)))
            .style(style)
            .render(
                Rect::new(
                    body.x.saturating_add(x as u16),
                    body.y.saturating_add(y as u16),
                    label_width,
                    1,
                ),
                frame.buffer_mut(),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_projection_nameplates(
    frame: &mut Frame,
    body: Rect,
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
    let columns = layout.columns.max(1);
    let rows = layout.rows.max(1);
    let cell_width = (usize::from(body.width) / columns).max(1);
    for (slot, worker) in workers.iter().enumerate() {
        let column = slot % columns;
        let row = slot / columns;
        let cell_x = column.saturating_mul(cell_width);
        let width = if column + 1 == columns {
            usize::from(body.width).saturating_sub(cell_x)
        } else {
            cell_width
        }
        .max(1);
        let label_width = width.min(24);
        let status = worker_status(worker, now);
        let prefix = if start + slot == selected {
            ">"
        } else if status == WorkerStatus::Blocked {
            "!"
        } else if status == WorkerStatus::Failed {
            "×"
        } else {
            " "
        };
        let name = short_path(
            &worker.name,
            label_width.saturating_sub(prefix.chars().count() + 1),
        );
        let style = if start + slot == selected {
            Style::default().fg(INK).bg(PANEL_HIGHLIGHT)
        } else {
            Style::default().fg(status_color(status)).bg(PANEL)
        };
        let x = body
            .x
            .saturating_add(cell_x as u16)
            .min(body.x.saturating_add(body.width.saturating_sub(1)));
        let y = match projection {
            Projection::TopDown => body.y.saturating_add(
                ((row + 1).saturating_mul(usize::from(body.height)) / rows).saturating_sub(1)
                    as u16,
            ),
            Projection::Side | Projection::Iso | Projection::Auto => {
                body.y.saturating_add(body.height.saturating_sub(1))
            }
        };
        let available = body.x.saturating_add(body.width).saturating_sub(x);
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
    let layout = desk_layout(office.workers.len(), area.width, body_height);
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
            "←↑↓→ / hjkl desks   Enter open   Tab cameras   c camera   page {}/{}   +{} overflow   p phone   ? help",
            page.saturating_add(1),
            layout.pages.max(1),
            overflow
        )
    } else {
        format!(
            "←↑↓→ / hjkl desks   Enter open   Tab cameras   c camera   page {}/{}   p phone   ? help",
            page.saturating_add(1),
            layout.pages.max(1)
        )
    };
    draw_footer(frame, footer, &footer_text);
    if !has_area(body) {
        return layout;
    }

    canvas.resize(body.width as usize, body.height as usize * 2);
    let iso_grid = match effective {
        Projection::Iso | Projection::Auto => Some(draw_scene(
            canvas,
            office,
            &visible_workers,
            &looks,
            sprites,
            now,
            layout,
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
    };
    canvas.render(frame.buffer_mut(), body);
    if name_plates {
        if let Some(grid) = iso_grid {
            draw_nameplates(frame, body, grid, &visible_workers, start, selected, now);
        } else {
            draw_projection_nameplates(
                frame,
                body,
                &visible_workers,
                start,
                selected,
                now,
                effective,
                layout,
            );
        }
    }
    if (blocked_count > 0 || failed_count > 0) && body.width >= 24 && body.height >= 2 {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(project_label("/workspace/sustain"), "SUSTAIN");
        let longish = project_label("/workspace/giin-jalisco");
        assert_eq!(longish, "GIIN-JALISCO");
        assert_eq!(sign_lines(&longish), vec!["GIIN-JALISCO"]);
        assert!(block_text_width(&longish, 2) > 100);
        assert!(block_text_width(&longish, 1) <= 100);
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
    fn auto_projection_chooses_each_camera_shape() {
        assert_eq!(
            effective_projection(Projection::Auto, 80, 30),
            Projection::Iso
        );
        assert_eq!(
            effective_projection(Projection::Auto, 80, 50),
            Projection::TopDown
        );
        assert_eq!(
            effective_projection(Projection::Auto, 110, 20),
            Projection::Side
        );
        assert_eq!(
            effective_projection(Projection::Side, 80, 50),
            Projection::Side
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
    fn project_sign_has_face_body_and_six_step_base_layers() {
        let mut canvas =
            crate::canvas::Canvas::with_color_depth(80, 40, crate::canvas::ColorDepth::TrueColor);
        draw_project_name(&mut canvas, "A", 30);
        let count_color = |color| {
            (0..canvas.width())
                .flat_map(|x| (0..canvas.height()).map(move |y| (x, y)))
                .filter(|(x, y)| canvas.pixel(*x, *y) == Some(color))
                .count()
        };
        assert!(count_color(TITLE_COLOR) > 0);
        assert!(count_color(TITLE_BODY) > 0);
        assert!(count_color(TITLE_SIGN) > 0);
    }
}
