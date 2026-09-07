//! Compact key reference shared by every top-level view.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;

use super::{has_area, inset, paint_opaque, INK, MUTED, PANEL, PANEL_HIGHLIGHT};

pub(crate) fn draw(frame: &mut Frame, scroll: &mut usize) {
    let area = frame.area();
    if !has_area(area) {
        return;
    }

    let width = area.width.saturating_sub(2).clamp(1, 74);
    let height = area.height.saturating_sub(2).clamp(1, 25);
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
        .title(" ? HELP / YOUR TOWER ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL_HIGHLIGHT))
        .style(block_style);
    block.render(popup, frame.buffer_mut());

    let inner = inset(popup, 1);
    if !has_area(inner) {
        return;
    }
    let entries = [
        "Each floor is a project. Each worker is a conversation.",
        "WORKING: an active turn. IDLE: ready for a new task.",
        "! WAITING: approval requested or an open turn went quiet.",
        "× FAILED: the last recorded activity reported an error.",
        "! opens the next worker needing attention across all floors.",
        "Inspect the desk, then answer or approve in the source app.",
        "This office reads local records; it cannot send or approve.",
        "0 tower · Enter opens a floor, then the selected worker.",
        "Esc / Backspace goes back: desk → floor → tower.",
        "Arrows / hjkl select floors or workers.",
        "Tab / Shift-Tab cycles floors · 1–9 jumps to a floor.",
        "Tower: PgUp/PgDn pages · Home/End first/last floor.",
        "Desk: ↑/↓ scroll history · ←/→ switches workers.",
        "p phone · 1–4 channels · ↑/↓ message · Enter inspect.",
        "v cycles office views · s customizes the office.",
        "o changes this floor’s room palette · O restores auto.",
        "Desk: w changes character · W restores the original.",
        "Character traits are fictional; agent behavior is unchanged.",
        "c connects local sources; no extra account or login.",
        "? help · q quits (or closes the current overlay).",
    ];
    let content_height = inner.height.saturating_sub(1);
    if content_height == 0 {
        return;
    }
    let mut wrapped = Vec::new();
    for entry in entries {
        let mut row = String::new();
        for word in entry.split_whitespace() {
            if !row.is_empty()
                && Line::from(format!("{row} {word}")).width() > usize::from(inner.width)
            {
                wrapped.push(Line::from(std::mem::take(&mut row)));
            }
            if !row.is_empty() {
                row.push(' ');
            }
            for character in word.chars() {
                if Line::from(format!("{row}{character}")).width() > usize::from(inner.width)
                    && !row.is_empty()
                {
                    wrapped.push(Line::from(std::mem::take(&mut row)));
                }
                row.push(character);
            }
        }
        wrapped.push(Line::from(row));
    }
    *scroll = (*scroll).min(wrapped.len().saturating_sub(usize::from(content_height)));
    let lines = Text::from(wrapped);
    Paragraph::new(lines)
        .style(Style::default().fg(MUTED).bg(PANEL))
        .scroll(((*scroll).min(u16::MAX as usize) as u16, 0))
        .render(
            Rect::new(inner.x, inner.y, inner.width, content_height),
            frame.buffer_mut(),
        );
    Paragraph::new("↑↓ scroll · Esc close")
        .style(Style::default().fg(INK).bg(PANEL_HIGHLIGHT))
        .render(
            Rect::new(inner.x, inner.y + content_height, inner.width, 1),
            frame.buffer_mut(),
        );
}
