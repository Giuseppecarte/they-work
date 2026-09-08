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
        "/ or Ctrl+K finds a project, task, provider or state.",
        "Enter visits a floor, then a desk. Esc / Backspace returns.",
        "Arrows / hjkl select · Tab / Shift-Tab focuses controls.",
        "0 tower · 1–9 floor · ! next worker needing attention.",
        "WORKING: active turn. IDLE: ready for a new task.",
        "WAITING ON YOU: a recorded request needs your response.",
        "NEEDS HELP: an open turn went quiet; inspect its history.",
        "FAILED: the last recorded activity reported an error.",
        "m controls the selected task; n creates a new task.",
        "Codex managed tasks: F5 send, F2 stop, F4 live requests.",
        "F6 reconnects a saved managed task without starting a turn.",
        "Claude: F3 opens its verified official background console.",
        "b notebook: attention, deliveries, changes and team tree.",
        "g team: recorded delegation, session membership or fork.",
        "Notebook r marks seen; it never resolves or approves work.",
        "C connection controls and official provider login.",
        "p phone · 1–4: Now / Attention / Edits / Messages.",
        "Phone: ←→ channel · ↑↓ message · Enter inspect worker.",
        "Scene: PgUp/PgDn floors · Home/End first/last item.",
        "Desk: ↑↓ history · PgUp/PgDn page · ←→ other worker.",
        "Inspector: Home top · End bottom of recorded context.",
        "d office design · a character in inspector · s settings.",
        "Desk: w character · W original. Traits are fictional.",
        "c / C Connections: sources, capabilities and official login.",
        "Click a person to inspect; approving needs its own action.",
        "Mouse is on by default; Settings or --mouse=off disables it.",
        "Observation is read-only and needs no they-work account.",
        "Keep Remember enabled to retain managed Codex tasks.",
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
