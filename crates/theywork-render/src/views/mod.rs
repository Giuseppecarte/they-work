//! The three screens presented by the renderer.

pub mod cameras;
pub mod desk;
pub mod office;
pub mod phone;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use theywork_core::{Activity, Millis, Worker, WorkerStatus};

use crate::canvas::Canvas;
use crate::sprite::SpriteSet;

pub(crate) const BACKGROUND: Color = Color::Rgb(24, 22, 39);
pub(crate) const WALL: Color = Color::Rgb(49, 40, 65);
pub(crate) const FLOOR: Color = Color::Rgb(86, 58, 65);
pub(crate) const PANEL: Color = Color::Rgb(35, 32, 53);
pub(crate) const PANEL_HIGHLIGHT: Color = Color::Rgb(67, 57, 91);
pub(crate) const INK: Color = Color::Rgb(248, 226, 182);
pub(crate) const MUTED: Color = Color::Rgb(164, 157, 190);
pub(crate) const ACCENT: Color = Color::Rgb(111, 202, 255);
pub(crate) const HOT: Color = Color::Rgb(255, 117, 122);
pub(crate) const WARNING: Color = Color::Rgb(255, 205, 113);
pub(crate) const GOOD: Color = Color::Rgb(126, 219, 151);
pub(crate) const SCANLINE: Color = Color::Rgb(42, 37, 59);

pub(crate) fn has_area(area: Rect) -> bool {
    area.width > 0 && area.height > 0
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
    let wall_width = sprites.wall_tile.width().max(1);
    let wall_height_tile = sprites.wall_tile.height().max(1);
    for y in (0..floor_start).step_by(wall_height_tile) {
        for x in (0..canvas.width()).step_by(wall_width) {
            canvas.blit(&sprites.wall_tile, x, y);
        }
    }

    let floor_width = sprites.floor_tile.width().max(1);
    let floor_height = sprites.floor_tile.height().max(1);
    for y in (floor_start..canvas.height()).step_by(floor_height) {
        for x in (0..canvas.width()).step_by(floor_width) {
            canvas.blit(&sprites.floor_tile, x, y);
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

pub(crate) fn render_worker(
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    worker: &Worker,
    now: i64,
    placement: PixelRect,
) {
    let PixelRect {
        x,
        y,
        width,
        height,
    } = placement;
    let sprite = sprites
        .worker_animation(worker.agent, &worker.activity)
        .frame_at(now);
    if width == 0 || height == 0 {
        return;
    }
    if width == sprite.width() && height == sprite.height() {
        canvas.blit(sprite, x, y);
    } else {
        canvas.blit_scaled(sprite, x, y, width, height);
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

pub(crate) fn activity_style(activity: &Activity) -> Style {
    let color = match activity {
        Activity::Error { .. } => HOT,
        Activity::Waiting { .. } => Color::Rgb(255, 205, 113),
        Activity::Idle => MUTED,
        Activity::Thinking => Color::Rgb(208, 167, 255),
        _ => GOOD,
    };
    Style::default().fg(color)
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

pub(crate) fn token_bar(tokens: u64, max_tokens: u64, width: usize) -> String {
    if width < 4 {
        return String::new();
    }
    let slots = width.saturating_sub(2).max(1);
    let filled = if tokens == 0 || max_tokens == 0 {
        0
    } else if tokens >= max_tokens {
        slots
    } else {
        let ratio = ((tokens as f64) + 1.0).ln() / ((max_tokens as f64) + 1.0).ln();
        (ratio * slots as f64).ceil().clamp(1.0, slots as f64) as usize
    };
    format!(
        "[{}{}]",
        "#".repeat(filled),
        ".".repeat(slots.saturating_sub(filled))
    )
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

pub(crate) fn short_path(path: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= max_chars {
        return path.to_string();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    let tail: String = chars.iter().rev().take(max_chars - 1).copied().collect();
    format!("…{}", tail.chars().rev().collect::<String>())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_labels_use_human_units() {
        assert_eq!(human_tokens(136_934_015), "136.9M");
        assert_eq!(human_tokens(4_900_000), "4.9M");
        assert_eq!(human_tokens(0), "0");
    }

    #[test]
    fn token_bar_uses_a_logarithmic_scale_against_the_office_maximum() {
        let maximum = 136_934_015;
        assert_eq!(token_bar(0, maximum, 10), "[........]");
        assert_eq!(token_bar(maximum, maximum, 10), "[########]");
        assert!(
            token_bar(1_000, maximum, 10).contains('#'),
            "small nonzero token counts should remain visible"
        );
    }

    #[test]
    fn short_path_never_splits_utf8_and_respects_tiny_widths() {
        let long_ascii = "x".repeat(200);
        let elided = short_path(&long_ascii, 16);
        assert_eq!(elided.chars().count(), 16);
        assert!(elided.starts_with("…"));

        let unicode = "界🛠️é".repeat(80);
        for width in 0..=16 {
            assert!(
                short_path(&unicode, width).chars().count() <= width,
                "elided text exceeded width {width}"
            );
        }
        assert_eq!(short_path(&unicode, 1), "…");
        assert_eq!(short_path(&unicode, 2).chars().count(), 2);
    }
}
