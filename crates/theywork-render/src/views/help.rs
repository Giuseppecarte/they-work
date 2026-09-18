//! Compact key reference shared by every top-level view.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;

use super::{has_area, inset, paint_opaque, INK, MUTED, PANEL};

pub(crate) fn draw(frame: &mut Frame, scroll: &mut usize) {
    let full = frame.area();
    let area = Rect::new(
        full.x,
        full.y.saturating_add(2),
        full.width,
        full.height.saturating_sub(3),
    );
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
        .border_style(Style::default().fg(super::ACCENT))
        .style(block_style);
    block.render(popup, frame.buffer_mut());

    let inner = inset(popup, 1);
    if !has_area(inner) {
        return;
    }
    let mut entries = vec![
        "Each floor is a project; each worker is a real task.".to_string(),
        "Select a worker, read its work brief, then choose an available action.".into(),
    ];
    entries.extend(
        crate::interaction::navigation_actions()
            .into_iter()
            .map(|action| format!("{}  {}", action.shortcut(), action.label())),
    );
    entries.extend([
        "Tab / Shift+Tab highlights a whole control; Enter activates it.",
        "Arrows select or scroll. The footer shows what Enter opens and where Esc returns.",
        "Esc: work brief to office, office to tower. Mouse uses the same actions.",
        "Scene: PgUp/PgDn changes floors. 1–9 jumps to a floor; ! finds attention.",
        "Work brief: Now, Activity, Team and Details. e / F7 expands the panel.",
        "Activity: read retained events; Back to latest resumes following updates.",
        "Thinking reports an observed state, not a progress estimate.",
        "Question / approval means human input; team/process waits do not.",
        "Source unavailable / last known means the observation is not current.",
        "m opens an instruction draft for the selected task. F7 expands with the draft intact.",
        "Send and approval choices are explicit. Opening a worker never approves.",
        "Claude messages and decisions use its available official conversation.",
        "b Attention; g the selected worker’s Team. Seen/reviewed marks are local.",
        "d Office design; a Character. Preview changes, Apply to save, Esc to cancel.",
        "s Settings. Advanced contains compatibility cameras and text graphics.",
        "Older cameras retain limited artwork and controls; Tower and Office are the main views.",
        "p Phone: recorded updates. 1–4 or ←→ selects Now, Attention, Edits or Messages.",
        "Mouse is on by default; Settings or --mouse=off returns terminal text selection.",
        "Observation needs no they-work account. Provider controls require their own connection.",
        "q quits outside text entry; ? opens this help.",
    ].into_iter().map(str::to_string));
    let content_height = inner.height;
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
}
