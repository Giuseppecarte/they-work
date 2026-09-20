//! The presentation screens rendered by this crate.

pub mod cameras;
pub mod control;
pub mod customize;
pub mod desk;
pub(crate) mod finder;
mod guard_scene;
pub mod help;
pub mod inspector;
pub mod office;
pub mod phone;
pub mod settings;
pub(crate) mod tower;
pub mod workboard;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use theywork_core::{Millis, Worker, WorkerStatus};

use crate::canvas::Canvas;
#[cfg(test)]
use crate::sprite::WORKER_HEAD_HEIGHT;
#[cfg(test)]
use crate::sprite::{Sprite, SpriteSet, WorkerLook};

pub(crate) const BACKGROUND: Color = Color::Rgb(24, 26, 29);
pub(crate) const WALL: Color = Color::Rgb(64, 68, 72);
pub(crate) const FLOOR: Color = Color::Rgb(189, 174, 150);
pub(crate) const PANEL: Color = Color::Rgb(34, 37, 41);
pub(crate) const PANEL_HIGHLIGHT: Color = Color::Rgb(48, 53, 59);
pub(crate) const ATTENTION_PANEL: Color = Color::Rgb(60, 46, 28);
pub(crate) const INK: Color = Color::Rgb(245, 246, 248);
pub(crate) const MUTED: Color = Color::Rgb(185, 191, 200);
pub(crate) const ACCENT: Color = Color::Rgb(128, 185, 255);
pub(crate) const SECONDARY: Color = Color::Rgb(190, 200, 216);
pub(crate) const HOT: Color = Color::Rgb(255, 145, 145);
pub(crate) const WARNING: Color = Color::Rgb(247, 204, 125);
pub(crate) const GOOD: Color = Color::Rgb(147, 214, 172);
pub(crate) const SCANLINE: Color = PANEL_HIGHLIGHT;

pub(crate) const LIGHT_BACKGROUND: Color = Color::Rgb(247, 247, 245);
pub(crate) const LIGHT_PANEL: Color = Color::Rgb(237, 239, 240);
pub(crate) const LIGHT_LINE: Color = Color::Rgb(215, 222, 231);
pub(crate) const LIGHT_INK: Color = Color::Rgb(28, 31, 36);
pub(crate) const LIGHT_WALL: Color = Color::Rgb(243, 243, 239);
pub(crate) const LIGHT_WALL_DARK: Color = Color::Rgb(213, 217, 216);
pub(crate) const LIGHT_FLOOR: Color = Color::Rgb(226, 214, 193);
pub(crate) const LIGHT_WOOD: Color = Color::Rgb(190, 164, 128);
pub(crate) const LIGHT_WOOD_DARK: Color = Color::Rgb(142, 119, 91);
pub(crate) const LIGHT_RUNNING: Color = Color::Rgb(26, 99, 58);
pub(crate) const LIGHT_BLOCKED: Color = Color::Rgb(112, 72, 9);
pub(crate) const LIGHT_FAILED: Color = Color::Rgb(165, 35, 46);
pub(crate) const LIGHT_ACCENT: Color = Color::Rgb(0, 85, 180);
pub(crate) const LIGHT_SECONDARY: Color = Color::Rgb(55, 75, 101);
pub(crate) const LIGHT_MUTED: Color = Color::Rgb(79, 84, 92);

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
    } else if color == ACCENT {
        LIGHT_ACCENT
    } else if color == SECONDARY {
        LIGHT_SECONDARY
    } else if color == MUTED {
        LIGHT_MUTED
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

/// Filled selection stays distinct from request and error colors.
pub(crate) fn selection_style() -> Style {
    Style::default()
        .fg(BACKGROUND)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
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
        area.y.saturating_add(2),
        area.width,
        area.height.saturating_sub(2),
    )
}
pub(crate) fn has_area(area: Rect) -> bool {
    area.width > 0 && area.height > 0
}

/// Preserve explicit lines while removing terminal control sequences from records.
pub(crate) fn safe_multiline(text: &str) -> String {
    text.split('\n')
        .map(safe_display)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn wrap_text(text: &str, width: u16) -> Vec<String> {
    text.split('\n')
        .flat_map(|line| desk::wrapped_lines(line, usize::from(width).max(1)))
        .collect()
}

/// Clear an area to spaces before painting a widget over a previous view.
/// Ratatui styles update a cell's colours but do not remove a previously
/// rendered half-block, so overlays need an explicit opaque backing.
pub(crate) fn paint_opaque(frame: &mut Frame, area: Rect, style: Style) {
    if !has_area(area) {
        return;
    }
    let buffer = frame.buffer_mut();
    for row in 0..area.height {
        for column in 0..area.width {
            if let Some(cell) = buffer.cell_mut((area.x + column, area.y + row)) {
                cell.reset();
                cell.set_symbol(" ").set_style(style).set_skip(false);
            }
        }
    }
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

#[derive(Clone, Copy)]
#[cfg(test)]
pub(crate) struct PixelRect {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

#[cfg(test)]
pub(crate) fn render_worker_with_look(
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    worker: &Worker,
    look: &WorkerLook,
    now: i64,
    placement: PixelRect,
) {
    let horizontal_scale = sprite_pixel_width(canvas);
    let sprite = if placement.width / horizontal_scale >= 24 && placement.height >= 34 {
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

#[cfg(test)]
pub(crate) fn render_worker_head_with_look(
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    worker: &Worker,
    look: &WorkerLook,
    now: i64,
    placement: PixelRect,
) {
    let width = placement.width / sprite_pixel_width(canvas);
    let (sprite, source) = sprites.worker_head_fitting(worker, *look, now, width, placement.height);
    render_sprite_region(canvas, &sprite, source, placement);
}

fn sprite_pixel_width(canvas: &Canvas) -> usize {
    if canvas.encoding() == crate::canvas::PixelEncoding::Quadrants && !canvas.has_image_density() {
        2
    } else {
        1
    }
}

#[cfg(test)]
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
    let pixel_width = sprite_pixel_width(canvas);
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
    #[test]
    fn light_text_and_statuses_remain_legible_on_selected_and_attention_panels() {
        use super::*;
        let luminance = |color: Color| {
            let Color::Rgb(r, g, b) = color else {
                panic!("expected RGB token")
            };
            [r, g, b]
                .into_iter()
                .zip([0.2126, 0.7152, 0.0722])
                .map(|(channel, weight)| {
                    let value = f64::from(channel) / 255.0;
                    weight
                        * if value <= 0.04045 {
                            value / 12.92
                        } else {
                            ((value + 0.055) / 1.055).powf(2.4)
                        }
                })
                .sum::<f64>()
        };
        for text in [INK, MUTED, ACCENT, GOOD, WARNING, HOT] {
            for panel in [BACKGROUND, PANEL, PANEL_HIGHLIGHT, ATTENTION_PANEL] {
                let ratio =
                    (luminance(light_color(panel)) + 0.05) / (luminance(light_color(text)) + 0.05);
                assert!(
                    ratio >= 4.5,
                    "{text:?} on {panel:?} has contrast {ratio:.2}"
                );
            }
        }
    }
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
                    cell.set_style(Style::default().add_modifier(Modifier::BOLD));
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
                assert!(!cell.modifier.contains(Modifier::BOLD));
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
    fn full_portrait_keeps_its_aspect_ratio_in_quadrant_cells() {
        use crate::canvas::{ColorDepth, PixelEncoding};
        use theywork_core::{Agent, OfficeId, WorkerId};

        let worker = Worker::new(
            WorkerId("portrait-aspect".into()),
            OfficeId("office".into()),
            Agent::Codex,
            "Portrait".into(),
            0,
        );
        let look = crate::sprite::worker_look(&worker);
        let sprites = SpriteSet::new();
        let mut half = Canvas::with_color_depth(24, 34, ColorDepth::TrueColor);
        let mut quadrants = Canvas::with_color_depth_and_encoding(
            48,
            34,
            ColorDepth::TrueColor,
            PixelEncoding::Quadrants,
        );
        for (canvas, width) in [(&mut half, 24), (&mut quadrants, 48)] {
            render_worker_with_look(
                canvas,
                &sprites,
                &worker,
                &look,
                0,
                PixelRect {
                    x: 0,
                    y: 0,
                    width,
                    height: 34,
                },
            );
        }
        // Both buffers cover the same physical area in typical 1:2 terminal
        // cells. Full portraits must obey the same aspect rule as miniatures.
        for y in 0..34 {
            for x in 0..24 {
                assert_eq!(half.pixel(x, y), quadrants.pixel(x * 2, y));
                assert_eq!(half.pixel(x, y), quadrants.pixel(x * 2 + 1, y));
            }
        }
    }

    #[test]
    fn phone_avatar_is_the_head_crop_of_the_office_sprite() {
        use crate::canvas::{ColorDepth, PixelEncoding};
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
        for encoding in [
            PixelEncoding::HalfBlocks,
            PixelEncoding::Quadrants,
            PixelEncoding::Sextants,
        ] {
            let width = if encoding == PixelEncoding::Quadrants {
                48
            } else {
                24
            };
            let mut full =
                Canvas::with_color_depth_and_encoding(width, 34, ColorDepth::TrueColor, encoding);
            render_worker_with_look(
                &mut full,
                &sprites,
                &worker,
                &look,
                0,
                PixelRect {
                    x: 0,
                    y: 0,
                    width,
                    height: 34,
                },
            );
            let mut head = Canvas::with_color_depth_and_encoding(
                width,
                WORKER_HEAD_HEIGHT,
                ColorDepth::TrueColor,
                encoding,
            );
            render_worker_head_with_look(
                &mut head,
                &sprites,
                &worker,
                &look,
                0,
                PixelRect {
                    x: 0,
                    y: 0,
                    width,
                    height: WORKER_HEAD_HEIGHT,
                },
            );
            for y in 0..WORKER_HEAD_HEIGHT {
                for x in 0..width {
                    assert_eq!(
                        head.pixel(x, y),
                        full.pixel(x, y),
                        "crop mismatch at ({x}, {y}) in {encoding:?}"
                    );
                }
            }
        }
    }
}
