//! Compact key reference shared by every top-level view.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};
use ratatui::Frame;

use super::{has_area, inset, paint_opaque, INK, MUTED, PANEL, PANEL_HIGHLIGHT};

pub(crate) fn draw(frame: &mut Frame) {
    let area = frame.area();
    if !has_area(area) {
        return;
    }

    let width = area.width.saturating_sub(2).clamp(1, 64);
    let height = area.height.saturating_sub(2).clamp(1, 16);
    let popup = Rect::new(
        area.x.saturating_add(area.width.saturating_sub(width) / 2),
        area.y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    );
    let block_style = Style::default().fg(INK).bg(PANEL);
    paint_opaque(frame, popup, block_style);
    let block = Block::default()
        .title(" ? HELP ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL_HIGHLIGHT))
        .style(block_style);
    block.render(popup, frame.buffer_mut());

    let inner = inset(popup, 1);
    if !has_area(inner) {
        return;
    }
    let lines = Text::from(vec![
        Line::from("move    arrows / hjkl (floor, camera, worker)"),
        Line::from("tabs    1-9 jump; Tab / Shift-Tab cycle; 0 guard"),
        Line::from("open    Enter"),
        Line::from("back    Esc / Backspace"),
        Line::from("camera  c cycles isometric / top-down / side"),
        Line::from("phone   p; 1-4; arrows / hjkl"),
        Line::from("settings s; session only"),
        Line::from("help    ?; Esc/q close; q quits outside"),
    ]);
    Paragraph::new(lines)
        .style(Style::default().fg(MUTED).bg(PANEL))
        .wrap(Wrap { trim: false })
        .render(inner, frame.buffer_mut());
}
