//! Targets are copied with the frame that was actually presented. A mouse
//! coordinate is never resolved against a newer, still-unseen animation frame.
use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use theywork_core::{OfficeId, WorkerId};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Tower,
    Attention,
    Deliveries,
    Find,
    NewTask,
    Connections,
    Sources,
    SelectFloor(OfficeId),
    EnterFloor(OfficeId),
    Inspect(WorkerId),
    SelectTeam(WorkerId),
    SelectDesks(OfficeId, WorkerId),
    Team(WorkerId),
    Review(WorkerId),
    Controls(WorkerId),
    Character(WorkerId),
    Decorate(OfficeId),
    Control(crate::views::control::Command),
    /// Navigation-only keys for a modal; never use this for approval choices.
    Key(KeyCode),
    ToggleMouse,
    Setting(usize),
    CustomizeField(usize),
    Mark(String),
    NotebookSelect(String),
    ControlProject(String),
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HitRegion {
    pub area: Rect,
    pub action: Action,
}
impl HitRegion {
    pub fn new(area: Rect, action: Action) -> Self {
        Self { area, action }
    }
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.area.x && x < self.area.right() && y >= self.area.y && y < self.area.bottom()
    }
}
