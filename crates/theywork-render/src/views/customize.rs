//! Local character and room editing; these controls never send provider input.
use super::{paint_opaque, safe_display, ACCENT, INK, MUTED, PANEL, PANEL_HIGHLIGHT};
use crate::{
    design::{self, CharacterProfile, OfficeDesign},
    interaction::{Action, HitRegion},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Paragraph, Widget},
    Frame,
};
use std::collections::BTreeMap;
use theywork_core::{OfficeId, WorkerId};

#[derive(Default)]
pub struct Customize {
    pub open: bool,
    worker: Option<WorkerId>,
    office: Option<OfficeId>,
    pub field: usize,
    profile: CharacterProfile,
    design: OfficeDesign,
    costume: usize,
    cursor: usize,
    readable: bool,
}
impl Customize {
    pub fn show_character(
        &mut self,
        id: WorkerId,
        profiles: &BTreeMap<String, CharacterProfile>,
        wardrobe: &BTreeMap<String, usize>,
    ) {
        self.profile = design::profile_for(&id.0, profiles);
        self.cursor = self.profile.name.len();
        self.costume = wardrobe
            .get(&id.0)
            .copied()
            .unwrap_or_else(|| crate::living_office::art::identity(&id.0, None).costume as usize);
        self.worker = Some(id);
        self.office = None;
        self.field = 0;
        self.open = true;
    }
    pub fn show_office(&mut self, id: OfficeId, designs: &BTreeMap<String, OfficeDesign>) {
        self.design = design::office_design_for(&id.0, designs);
        self.office = Some(id);
        self.worker = None;
        self.field = 0;
        self.open = true;
    }
    pub fn paste(&mut self, text: &str) {
        if self.worker.is_some() && self.field == 0 && self.readable {
            let room = 32usize.saturating_sub(self.profile.name.chars().count());
            let text: String = text
                .chars()
                .filter(|ch| !ch.is_control())
                .take(room)
                .collect();
            self.profile.name.insert_str(self.cursor, &text);
            self.cursor += text.len();
        }
    }
    pub fn valid(&self) -> bool {
        self.worker.is_none() || !self.profile.name.trim().is_empty()
    }
    pub fn store(
        &self,
        profiles: &mut BTreeMap<String, CharacterProfile>,
        designs: &mut BTreeMap<String, OfficeDesign>,
        wardrobe: &mut BTreeMap<String, usize>,
    ) {
        if !self.valid() {
            return;
        }
        if let Some(id) = &self.worker {
            if !self.profile.name.trim().is_empty() {
                profiles.insert(id.0.clone(), self.profile.clone());
            }
            wardrobe.insert(id.0.clone(), self.costume);
        }
        if let Some(id) = &self.office {
            designs.insert(id.0.clone(), self.design.clone());
        }
    }
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        profiles: &mut BTreeMap<String, CharacterProfile>,
        designs: &mut BTreeMap<String, OfficeDesign>,
        wardrobe: &mut BTreeMap<String, usize>,
    ) {
        if key.code == KeyCode::Esc {
            self.open = false;
            return;
        }
        if !self.readable {
            return;
        }
        let fields = if self.worker.is_some() { 3 } else { 5 };
        let count = fields + 2;
        if key.code == KeyCode::Enter
            && (self.field == fields || key.modifiers.contains(KeyModifiers::CONTROL))
        {
            if self.valid() {
                self.store(profiles, designs, wardrobe);
                self.open = false;
            }
            return;
        }
        if key.code == KeyCode::Enter && self.field == fields + 1 {
            self.open = false;
            return;
        }
        match key.code {
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.field = (self.field + count - 1) % count
            }
            KeyCode::Tab | KeyCode::Down => self.field = (self.field + 1) % count,
            KeyCode::BackTab | KeyCode::Up => self.field = (self.field + count - 1) % count,
            _ if self.worker.is_some() && self.field == 0 => match key.code {
                KeyCode::Char(ch)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.paste(&ch.to_string())
                }
                KeyCode::Left if self.cursor > 0 => {
                    self.cursor -= self.profile.name[..self.cursor]
                        .chars()
                        .next_back()
                        .map_or(0, char::len_utf8)
                }
                KeyCode::Right if self.cursor < self.profile.name.len() => {
                    self.cursor += self.profile.name[self.cursor..]
                        .chars()
                        .next()
                        .map_or(0, char::len_utf8)
                }
                KeyCode::Home => self.cursor = 0,
                KeyCode::End => self.cursor = self.profile.name.len(),
                KeyCode::Backspace if self.cursor > 0 => {
                    let start = self.cursor
                        - self.profile.name[..self.cursor]
                            .chars()
                            .next_back()
                            .map_or(0, char::len_utf8);
                    self.profile.name.replace_range(start..self.cursor, "");
                    self.cursor = start;
                }
                KeyCode::Delete if self.cursor < self.profile.name.len() => {
                    let end = self.cursor
                        + self.profile.name[self.cursor..]
                            .chars()
                            .next()
                            .map_or(0, char::len_utf8);
                    self.profile.name.replace_range(self.cursor..end, "");
                }
                _ => {}
            },
            KeyCode::Left | KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                let delta = if key.code == KeyCode::Left { 2 } else { 1 };
                if self.worker.is_some() {
                    if self.field == 1 {
                        self.costume = (self.costume + if delta == 2 { 11 } else { 1 }) % 12;
                    }
                    if self.field == 2 {
                        for _ in 0..delta {
                            self.profile.style = self.profile.style.next();
                        }
                    }
                } else {
                    match self.field {
                        0 => {
                            for _ in 0..delta {
                                self.design.preset = self.design.preset.next();
                            }
                        }
                        1 => self.design.entrance = (self.design.entrance + delta) % 3,
                        2 => self.design.desks = (self.design.desks + delta) % 3,
                        3 => self.design.meeting = (self.design.meeting + delta) % 3,
                        4 => self.design.rest = (self.design.rest + delta) % 3,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    /// Apply an uncommitted draft to temporary render maps only.
    pub fn preview(
        &self,
        profiles: &mut BTreeMap<String, CharacterProfile>,
        designs: &mut BTreeMap<String, OfficeDesign>,
        wardrobe: &mut BTreeMap<String, usize>,
    ) {
        if self.open {
            self.store(profiles, designs, wardrobe);
        }
    }
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) -> Vec<HitRegion> {
        paint_opaque(frame, area, Style::default().bg(PANEL).fg(INK));
        let area = Rect {
            height: area.height.min(24),
            ..area
        };
        self.readable = area.width >= 24 && area.height >= 9;
        if !self.readable {
            Paragraph::new("Appearance · enlarge terminal\nEsc return")
                .render(area, frame.buffer_mut());
            return Vec::new();
        }
        let inner = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
        let title = if self.worker.is_some() {
            "CHARACTER"
        } else {
            "OFFICE DESIGN"
        };
        Paragraph::new(format!("{title} · preview"))
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .render(
                Rect::new(inner.x, inner.y, inner.width, 1),
                frame.buffer_mut(),
            );
        let choices = if self.worker.is_some() {
            vec![
                (
                    "Name",
                    if self.field == 0 {
                        format!(
                            "{}▏{}",
                            safe_display(&self.profile.name[..self.cursor]),
                            safe_display(&self.profile.name[self.cursor..])
                        )
                    } else {
                        safe_display(&self.profile.name)
                    },
                ),
                (
                    "Outfit",
                    crate::living_office::art::COSTUMES[self.costume].into(),
                ),
                ("Animation", self.profile.style.label().into()),
            ]
        } else {
            vec![
                ("Preset", self.design.preset.label().into()),
                (
                    "Entrance",
                    ["Emblem", "Plant wall", "Notice board"][self.design.entrance as usize].into(),
                ),
                (
                    "Desks",
                    ["Classic", "Books & tools", "Soft lighting"][self.design.desks as usize]
                        .into(),
                ),
                (
                    "Meeting",
                    ["Roundtable", "Whiteboard", "Project wall"][self.design.meeting as usize]
                        .into(),
                ),
                (
                    "Rest",
                    ["Coffee", "Green corner", "Reading nook"][self.design.rest as usize].into(),
                ),
            ]
        };
        let mut hits = Vec::new();
        let compact = area.width < 36 || area.height < 18;
        let row_height = if compact { 1 } else { 2 };
        let visible = (inner.height.saturating_sub(4) / row_height).max(1) as usize;
        let first = self
            .field
            .min(choices.len().saturating_sub(1))
            .saturating_sub(visible - 1);
        for (index, (label, value)) in choices.iter().enumerate().skip(first).take(visible) {
            let rect = Rect::new(
                inner.x,
                inner.y + 2 + (index - first) as u16 * row_height,
                inner.width,
                row_height,
            );
            let style = Style::default()
                .fg(if index == self.field { INK } else { MUTED })
                .bg(if index == self.field {
                    PANEL_HIGHLIGHT
                } else {
                    PANEL
                });
            paint_opaque(frame, rect, style);
            Paragraph::new(if compact {
                format!(
                    "{} {label}: {value}",
                    if index == self.field { ">" } else { " " }
                )
            } else {
                format!(
                    "{} {label}\n  {value}",
                    if index == self.field { ">" } else { " " }
                )
            })
            .style(style)
            .render(rect, frame.buffer_mut());
            hits.push(HitRegion::new(rect, Action::CustomizeField(index)));
        }
        let y = inner.bottom() - 2;
        let field_count = choices.len();
        for (x, label, action, index) in [
            (inner.x, "Apply", Action::ApplyAppearance, field_count),
            (
                inner.x + 11,
                "Cancel",
                Action::CancelAppearance,
                field_count + 1,
            ),
        ] {
            let kind = if self.field == index {
                crate::components::ButtonKind::Primary
            } else {
                crate::components::ButtonKind::Secondary
            };
            if index != field_count || self.valid() {
                if let Some(hit) = crate::components::button(
                    frame,
                    Rect::new(x, y, inner.right().saturating_sub(x), 1),
                    label,
                    action,
                    kind,
                ) {
                    hits.push(hit);
                }
            }
        }
        let zone = choices
            .get(self.field)
            .map(|(name, _)| *name)
            .unwrap_or("All changes");
        Paragraph::new(if self.valid() {
            format!("Preview: {zone}")
        } else {
            "Name cannot be empty".into()
        })
        .style(Style::default().fg(MUTED))
        .render(
            Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
            frame.buffer_mut(),
        );
        hits
    }
}
