//! The presentation screens rendered by this crate.

pub mod cameras;
pub mod desk;
mod guard_scene;
pub mod help;
pub mod office;
pub mod phone;
pub mod settings;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use theywork_core::{Millis, Office, Worker, WorkerStatus};

use crate::canvas::Canvas;
use crate::sprite::{Sprite, SpriteSet, WorkerLook, WORKER_HEAD_HEIGHT};

pub(crate) const BACKGROUND: Color = Color::Rgb(13, 11, 20);
pub(crate) const WALL: Color = Color::Rgb(58, 51, 88);
pub(crate) const FLOOR: Color = Color::Rgb(220, 201, 164);
pub(crate) const PANEL: Color = Color::Rgb(23, 20, 37);
pub(crate) const PANEL_HIGHLIGHT: Color = Color::Rgb(42, 36, 64);
pub(crate) const ATTENTION_PANEL: Color = Color::Rgb(46, 36, 16);
pub(crate) const INK: Color = Color::Rgb(232, 226, 214);
pub(crate) const MUTED: Color = Color::Rgb(138, 130, 153);
pub(crate) const ACCENT: Color = Color::Rgb(88, 214, 232);
pub(crate) const HOT: Color = Color::Rgb(232, 52, 44);
pub(crate) const WARNING: Color = Color::Rgb(240, 180, 41);
pub(crate) const GOOD: Color = Color::Rgb(86, 194, 106);
pub(crate) const SCANLINE: Color = Color::Rgb(42, 36, 64);

pub(crate) const LIGHT_BACKGROUND: Color = Color::Rgb(244, 239, 228);
pub(crate) const LIGHT_PANEL: Color = Color::Rgb(230, 223, 208);
pub(crate) const LIGHT_LINE: Color = Color::Rgb(203, 192, 170);
pub(crate) const LIGHT_INK: Color = Color::Rgb(58, 53, 44);
pub(crate) const LIGHT_WALL: Color = Color::Rgb(207, 198, 224);
pub(crate) const LIGHT_WALL_DARK: Color = Color::Rgb(189, 178, 212);
pub(crate) const LIGHT_FLOOR: Color = Color::Rgb(230, 217, 184);
pub(crate) const LIGHT_WOOD: Color = Color::Rgb(162, 112, 63);
pub(crate) const LIGHT_WOOD_DARK: Color = Color::Rgb(131, 87, 41);
pub(crate) const LIGHT_RUNNING: Color = Color::Rgb(47, 140, 66);
pub(crate) const LIGHT_BLOCKED: Color = Color::Rgb(201, 138, 0);
pub(crate) const LIGHT_FAILED: Color = Color::Rgb(192, 38, 31);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiTheme {
    Dark,
    Light,
}
pub(crate) fn light_color(color: Color) -> Color {
    if color == BACKGROUND {
        LIGHT_BACKGROUND
    } else if color == PANEL {
        LIGHT_PANEL
    } else if color == ATTENTION_PANEL {
        Color::Rgb(237, 219, 176)
    } else if color == PANEL_HIGHLIGHT || color == SCANLINE {
        LIGHT_LINE
    } else if color == INK {
        LIGHT_INK
    } else if color == WALL {
        LIGHT_WALL
    } else if color == Color::Rgb(43, 37, 66) {
        LIGHT_WALL_DARK
    } else if matches!(
        color,
        Color::Rgb(220, 201, 164) | Color::Rgb(192, 170, 130) | Color::Rgb(156, 135, 99)
    ) {
        LIGHT_FLOOR
    } else if color == Color::Rgb(138, 90, 56) {
        LIGHT_WOOD
    } else if matches!(color, Color::Rgb(107, 68, 41) | Color::Rgb(84, 51, 31)) {
        LIGHT_WOOD_DARK
    } else if color == HOT {
        LIGHT_FAILED
    } else if color == WARNING {
        LIGHT_BLOCKED
    } else if color == GOOD {
        LIGHT_RUNNING
    } else {
        color
    }
}
pub(crate) fn remap_buffer_theme(buffer: &mut Buffer, theme: UiTheme) {
    if theme != UiTheme::Light {
        return;
    }
    for cell in &mut buffer.content {
        cell.set_fg(light_color(cell.fg));
        cell.set_bg(light_color(cell.bg));
    }
}

pub(crate) fn below_tab_bar(area: Rect) -> Rect {
    Rect::new(
        area.x,
        area.y.saturating_add(1),
        area.width,
        area.height.saturating_sub(1),
    )
}
pub(crate) fn has_area(area: Rect) -> bool {
    area.width > 0 && area.height > 0
}

/// Clear an area to spaces before painting a widget over a previous view.
/// Ratatui styles update a cell's colours but do not remove a previously
/// rendered half-block, so overlays need an explicit opaque backing.
pub(crate) fn paint_opaque(frame: &mut Frame, area: Rect, style: Style) {
    if !has_area(area) {
        return;
    }
    let buffer = frame.buffer_mut();
    buffer.set_style(area, style);
    for row in 0..area.height {
        for column in 0..area.width {
            if let Some(cell) = buffer.cell_mut((area.x + column, area.y + row)) {
                cell.set_symbol(" ");
            }
        }
    }
}
pub(crate) fn office_dot_color(office: &Office, now: Millis) -> Color {
    if office
        .workers
        .iter()
        .any(|worker| worker_status(worker, now) == WorkerStatus::Blocked)
    {
        return WARNING;
    }
    if office
        .workers
        .iter()
        .any(|worker| worker_status(worker, now) == WorkerStatus::Failed)
    {
        return HOT;
    }
    if office
        .workers
        .iter()
        .any(|worker| worker_status(worker, now) == theywork_core::WorkerStatus::Running)
    {
        GOOD
    } else {
        MUTED
    }
}

pub(crate) fn draw_tab_bar(
    frame: &mut Frame,
    offices: &[&Office],
    selected: usize,
    all_selected: bool,
    now: Millis,
) {
    let area = frame.area();
    if area.width == 0 || area.height == 0 {
        return;
    }
    let selected = selected.min(offices.len().saturating_sub(1));
    let tower_style = Style::default()
        .fg(if all_selected { INK } else { MUTED })
        .bg(if all_selected { PANEL_HIGHLIGHT } else { PANEL });
    let title = if area.width >= 38 { " 0 TOWER " } else { "0 " };
    let counter = if offices.is_empty() {
        "no floors".to_string()
    } else {
        format!("{}/{}", selected + 1, offices.len())
    };
    let tail = if area.width >= 62 {
        format!(" {counter} · c sources · ? help ")
    } else if area.width >= 24 {
        format!(" {counter} · ? help ")
    } else {
        format!(" {counter}")
    };
    let available =
        usize::from(area.width).saturating_sub(title.len() + Line::from(tail.as_str()).width());
    let mut spans = vec![Span::styled(title, tower_style)];
    if !offices.is_empty() && available > 4 {
        let visible = (available / 24).max(1).min(offices.len());
        let first = (selected / visible * visible).min(offices.len().saturating_sub(visible));
        let slot_width = available / visible;
        for (index, office) in offices.iter().enumerate().skip(first).take(visible) {
            let style = Style::default()
                .fg(if index == selected { INK } else { MUTED })
                .bg(if index == selected && !all_selected {
                    PANEL_HIGHLIGHT
                } else {
                    PANEL
                });
            let number = format!(
                "{}{} ",
                if index == selected { ">" } else { " " },
                index + 1
            );
            let marker = match office_dot_color(office, now) {
                WARNING => "!",
                HOT => "×",
                GOOD => "●",
                _ => "·",
            };
            let name = short_path(&office.name, slot_width.saturating_sub(number.len() + 2));
            let used = number.len() + Line::from(name.as_str()).width() + 1;
            spans.push(Span::styled(number, style));
            spans.push(Span::styled(name, style));
            spans.push(Span::styled(
                marker,
                style.fg(office_dot_color(office, now)),
            ));
            spans.push(Span::styled(
                " ".repeat(slot_width.saturating_sub(used)),
                style,
            ));
        }
    }
    spans.push(Span::styled(tail, Style::default().fg(MUTED).bg(PANEL)));
    Paragraph::new(Line::from(spans))
        .style(Style::default().bg(PANEL))
        .render(Rect::new(area.x, area.y, area.width, 1), frame.buffer_mut());
}

pub(crate) fn inset(area: Rect, amount: u16) -> Rect {
    let horizontal = amount.saturating_mul(2).min(area.width);
    let vertical = amount.saturating_mul(2).min(area.height);
    Rect::new(
        area.x.saturating_add(amount.min(area.width)),
        area.y.saturating_add(amount.min(area.height)),
        area.width.saturating_sub(horizontal),
        area.height.saturating_sub(vertical),
    )
}

pub(crate) fn vertical_bands(
    area: Rect,
    header_height: u16,
    footer_height: u16,
) -> (Rect, Rect, Rect) {
    let header_height = header_height.min(area.height);
    let footer_height = footer_height.min(area.height.saturating_sub(header_height));
    let body_height = area.height.saturating_sub(header_height + footer_height);
    let header = Rect::new(area.x, area.y, area.width, header_height);
    let body = Rect::new(
        area.x,
        area.y.saturating_add(header_height),
        area.width,
        body_height,
    );
    let footer = Rect::new(
        area.x,
        area.y
            .saturating_add(header_height)
            .saturating_add(body_height),
        area.width,
        footer_height,
    );
    (header, body, footer)
}

pub(crate) fn draw_tiny(frame: &mut Frame, message: &str) {
    let area = frame.area();
    if has_area(area) {
        paint_opaque(frame, area, Style::default().fg(INK).bg(BACKGROUND));
        Paragraph::new(message)
            .style(Style::default().fg(INK).bg(BACKGROUND))
            .render(area, frame.buffer_mut());
    }
}

pub(crate) fn draw_header(frame: &mut Frame, area: Rect, title: &str, subtitle: &str) {
    if !has_area(area) {
        return;
    }
    let line = Line::from(vec![
        Span::styled(
            format!("  {title}  "),
            Style::default().fg(INK).add_modifier(Modifier::BOLD),
        ),
        Span::styled(subtitle.to_string(), Style::default().fg(MUTED)),
    ]);
    Paragraph::new(line)
        .style(Style::default().bg(BACKGROUND))
        .render(area, frame.buffer_mut());
}

pub(crate) fn draw_footer(frame: &mut Frame, area: Rect, text: &str) {
    if !has_area(area) {
        return;
    }
    let text = if area.width < 28 {
        "? help · q quit"
    } else if area.width < 50 {
        "Enter open · Esc back · ? help"
    } else if area.width < 76 {
        "arrows move · Enter open · Esc back · ? help"
    } else {
        text
    };
    Paragraph::new(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(text.to_string(), Style::default().fg(MUTED)),
    ]))
    .style(Style::default().bg(BACKGROUND))
    .render(area, frame.buffer_mut());
}

pub(crate) fn draw_panel(frame: &mut Frame, area: Rect, title: &str, selected: bool) -> Rect {
    if has_area(area) {
        let style = if selected {
            Style::default().fg(INK).bg(PANEL_HIGHLIGHT)
        } else {
            Style::default().fg(MUTED).bg(PANEL)
        };
        paint_opaque(frame, area, style);
        Block::default()
            .title(format!(" {title} "))
            .borders(Borders::ALL)
            .border_style(style)
            .style(style)
            .render(area, frame.buffer_mut());
    }
    inset(area, 1)
}

pub(crate) fn fill_office_background(canvas: &mut Canvas, sprites: &SpriteSet) -> usize {
    canvas.fill(WALL);
    let wall_height = canvas.height().saturating_mul(2) / 3;
    let floor_start = wall_height.min(canvas.height());
    for y in floor_start..canvas.height() {
        for x in 0..canvas.width() {
            canvas.set(x, y, FLOOR);
        }
    }
    let wall_width = canvas.scale_width(sprites.wall_tile.width().max(1));
    let wall_height_tile = canvas.scale_half_height(sprites.wall_tile.height().max(1));
    for y in (0..floor_start).step_by(wall_height_tile) {
        for x in (0..canvas.width()).step_by(wall_width) {
            canvas.blit_scaled(&sprites.wall_tile, x, y, wall_width, wall_height_tile);
        }
    }

    let floor_width = canvas.scale_width(sprites.floor_tile.width().max(1));
    let floor_height = canvas.scale_half_height(sprites.floor_tile.height().max(1));
    for y in (floor_start..canvas.height()).step_by(floor_height) {
        for x in (0..canvas.width()).step_by(floor_width) {
            canvas.blit_scaled(&sprites.floor_tile, x, y, floor_width, floor_height);
        }
    }
    floor_start
}

#[derive(Clone, Copy)]
pub(crate) struct PixelRect {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

pub(crate) fn render_worker_with_look(
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    worker: &Worker,
    look: &WorkerLook,
    now: i64,
    placement: PixelRect,
) {
    let horizontal_scale = compact_pixel_width(canvas);
    let sprite = if placement.width >= 24 && placement.height >= 34 {
        sprites.worker_frame(worker, *look, now)
    } else {
        sprites.worker_frame_fitting(
            worker,
            *look,
            now,
            placement.width / horizontal_scale,
            placement.height,
        )
    };
    render_sprite_region(
        canvas,
        &sprite,
        (0, 0, sprite.width(), sprite.height()),
        placement,
    );
}

pub(crate) fn render_worker_head_with_look(
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    worker: &Worker,
    look: &WorkerLook,
    now: i64,
    placement: PixelRect,
) {
    let width = if placement.width >= 24 && placement.height >= WORKER_HEAD_HEIGHT {
        placement.width
    } else {
        placement.width / compact_pixel_width(canvas)
    };
    let (sprite, source) = sprites.worker_head_fitting(worker, *look, now, width, placement.height);
    render_sprite_region(canvas, &sprite, source, placement);
}

fn compact_pixel_width(canvas: &Canvas) -> usize {
    if canvas.encoding() == crate::canvas::PixelEncoding::Quadrants && !canvas.has_image_density() {
        2
    } else {
        1
    }
}

fn render_sprite_region(
    canvas: &mut Canvas,
    sprite: &Sprite,
    source: (usize, usize, usize, usize),
    placement: PixelRect,
) {
    let PixelRect {
        x,
        y,
        width,
        height,
    } = placement;
    let (source_x, source_y, source_width, source_height) = source;
    if width == 0 || height == 0 || source_width == 0 || source_height == 0 {
        return;
    }
    let pixel_width = if source_width < 24 {
        compact_pixel_width(canvas)
    } else {
        1
    };
    let integer_scale = (width / source_width / pixel_width)
        .min(height / source_height)
        .max(1);
    let draw_width = source_width
        .saturating_mul(integer_scale)
        .saturating_mul(pixel_width);
    let draw_height = source_height.saturating_mul(integer_scale);
    let draw_x = x.saturating_add(width.saturating_sub(draw_width) / 2);
    let draw_y = y.saturating_add(height.saturating_sub(draw_height) / 2);
    for target_y in 0..draw_height.min(height) {
        let sample_y = source_y + target_y.saturating_mul(source_height) / draw_height;
        for target_x in 0..draw_width.min(width) {
            let sample_x = source_x + target_x.saturating_mul(source_width) / draw_width;
            if let Some(color) = sprite.pixel(sample_x, sample_y) {
                canvas.set(draw_x + target_x, draw_y + target_y, color);
            }
        }
    }
}

pub(crate) fn paint_scanlines(buffer: &mut Buffer, area: Rect, now: i64) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let phase = now.div_euclid(140).rem_euclid(3) as u16;
    for row in 0..area.height {
        if row % 3 != phase {
            continue;
        }
        for column in 0..area.width {
            if let Some(cell) = buffer.cell_mut((area.x + column, area.y + row)) {
                if cell.symbol() == " " {
                    cell.set_bg(SCANLINE);
                }
            }
        }
    }
}

pub(crate) fn grid_rect(area: Rect, index: usize, columns: usize, rows: usize) -> Rect {
    if columns == 0 || rows == 0 {
        return Rect::new(area.x, area.y, 0, 0);
    }
    let column = index % columns;
    let row = index / columns;
    if row >= rows {
        return Rect::new(area.x, area.y, 0, 0);
    }
    let x0 = area.x as u32 + (area.width as u32 * column as u32 / columns as u32);
    let x1 = area.x as u32 + (area.width as u32 * (column + 1) as u32 / columns as u32);
    let y0 = area.y as u32 + (area.height as u32 * row as u32 / rows as u32);
    let y1 = area.y as u32 + (area.height as u32 * (row + 1) as u32 / rows as u32);
    Rect::new(
        x0.min(u16::MAX as u32) as u16,
        y0.min(u16::MAX as u32) as u16,
        x1.saturating_sub(x0).min(u16::MAX as u32) as u16,
        y1.saturating_sub(y0).min(u16::MAX as u32) as u16,
    )
}

pub(crate) fn worker_status(worker: &Worker, now: Millis) -> WorkerStatus {
    worker.status_at(now)
}

pub(crate) fn status_color(status: WorkerStatus) -> Color {
    match status {
        WorkerStatus::Running => GOOD,
        WorkerStatus::Idle => MUTED,
        WorkerStatus::Blocked => WARNING,
        WorkerStatus::Failed => HOT,
    }
}

pub(crate) fn status_style(status: WorkerStatus) -> Style {
    Style::default().fg(status_color(status))
}

pub(crate) fn status_marker(status: WorkerStatus) -> Option<&'static str> {
    match status {
        WorkerStatus::Blocked => Some("!"),
        WorkerStatus::Failed => Some("×"),
        WorkerStatus::Running | WorkerStatus::Idle => None,
    }
}

pub(crate) fn elapsed_ms(now: Millis, then: Millis) -> Millis {
    now.saturating_sub(then).max(0)
}

pub(crate) fn duration_label(milliseconds: Millis) -> String {
    let seconds = elapsed_ms(milliseconds, 0).div_euclid(1_000);
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    }
}

pub(crate) fn human_tokens(tokens: u64) -> String {
    let (unit, suffix): (u64, &str) = if tokens >= 1_000_000_000_000 {
        (1_000_000_000_000, "T")
    } else if tokens >= 1_000_000_000 {
        (1_000_000_000, "B")
    } else if tokens >= 1_000_000 {
        (1_000_000, "M")
    } else if tokens >= 1_000 {
        (1_000, "K")
    } else {
        (1, "")
    };
    if unit == 1 {
        return tokens.to_string();
    }
    let whole = tokens / unit;
    let tenths = ((tokens % unit) as u128 * 10 / unit as u128) as u64;
    if tenths == 0 {
        format!("{whole}{suffix}")
    } else {
        format!("{whole}.{tenths}{suffix}")
    }
}

pub(crate) fn timestamp(now: i64) -> String {
    format!("t+{:06}s", now.max(0).div_euclid(1_000) % 1_000_000)
}

pub(crate) fn safe_display(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for character in text.chars() {
        let code = character as u32;
        if character.is_control()
            || (0x1f000..=0x1faff).contains(&code)
            || (0x2600..=0x27bf).contains(&code)
            || code == 0xfe0f
        {
            output.push('·');
        } else {
            output.push(character);
        }
    }
    output
}
pub(crate) fn short_path(path: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let safe = safe_display(path);
    if Line::from(safe.as_str()).width() <= max_chars {
        return safe;
    }
    let mut head = String::new();
    let mut width = 0;
    for character in safe.chars() {
        let cell_width = Line::from(character.to_string()).width();
        if width + cell_width > max_chars - 1 {
            break;
        }
        head.push(character);
        width += cell_width;
    }
    format!("{head}…")
}
#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::Terminal;

    use super::*;
    #[test]
    fn opaque_paint_replaces_previous_symbols_and_background() {
        let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("test terminal");
        let area = Rect::new(2, 2, 5, 3);
        terminal
            .draw(|frame| {
                for cell in &mut frame.buffer_mut().content {
                    cell.set_symbol("X");
                    cell.set_bg(Color::Blue);
                }
                paint_opaque(frame, area, Style::default().bg(PANEL));
            })
            .expect("opaque paint should render");
        let buffer = terminal.backend().buffer();
        for row in area.y..area.y + area.height {
            for column in area.x..area.x + area.width {
                let cell = &buffer.content[usize::from(row) * 12 + usize::from(column)];
                assert_eq!(cell.symbol(), " ");
                assert_eq!(cell.bg, PANEL);
            }
        }
    }

    #[test]
    fn token_labels_use_human_units() {
        assert_eq!(human_tokens(136_934_015), "136.9M");
        assert_eq!(human_tokens(4_900_000), "4.9M");
        assert_eq!(human_tokens(0), "0");
    }

    #[test]
    fn short_path_never_splits_utf8_and_respects_tiny_widths() {
        let long_ascii = "x".repeat(200);
        let elided = short_path(&long_ascii, 16);
        assert_eq!(elided.chars().count(), 16);
        assert!(elided.ends_with("…"));

        assert_eq!(short_path("abcdefghijk", 6), "abcde…");
        let unicode = "界🛠️é".repeat(80);
        for width in 0..=16 {
            assert!(
                Line::from(short_path(&unicode, width)).width() <= width,
                "elided text exceeded width {width}"
            );
        }
        assert_eq!(short_path(&unicode, 1), "…");
        assert_eq!(short_path(&unicode, 2), "…");
    }
    #[test]
    fn safe_display_replaces_controls_and_emoji() {
        let safe = safe_display("worker 😀 🛠️\n");
        assert!(safe.contains('·'));
        assert!(safe.chars().all(|character| {
            let code = character as u32;
            !character.is_control()
                && !(0x1f000..=0x1faff).contains(&code)
                && !(0x2600..=0x27bf).contains(&code)
                && code != 0xfe0f
        }));
    }

    #[test]
    fn phone_avatar_is_the_head_crop_of_the_office_sprite() {
        use theywork_core::{Agent, OfficeId, WorkerId};

        let worker = Worker::new(
            WorkerId("crop-worker".into()),
            OfficeId("crop-office".into()),
            Agent::Codex,
            "Crop test".into(),
            0,
        );
        let look = crate::sprite::worker_look(&worker);
        let sprites = SpriteSet::new();
        let mut full = Canvas::new(24, 34);
        render_worker_with_look(
            &mut full,
            &sprites,
            &worker,
            &look,
            0,
            PixelRect {
                x: 0,
                y: 0,
                width: 24,
                height: 34,
            },
        );
        let mut head = Canvas::new(24, WORKER_HEAD_HEIGHT);
        render_worker_head_with_look(
            &mut head,
            &sprites,
            &worker,
            &look,
            0,
            PixelRect {
                x: 0,
                y: 0,
                width: 24,
                height: WORKER_HEAD_HEIGHT,
            },
        );
        for y in 0..WORKER_HEAD_HEIGHT {
            for x in 0..24 {
                assert_eq!(
                    head.pixel(x, y),
                    full.pixel(x, y),
                    "crop mismatch at ({x}, {y})"
                );
            }
        }
    }
}
