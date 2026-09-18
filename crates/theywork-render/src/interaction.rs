//! Targets are copied with the frame that was actually presented. A mouse
//! coordinate is never resolved against a newer, still-unseen animation frame.
use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use theywork_core::{OfficeId, WorkerId};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Tower,
    More,
    Advanced,
    WorkTab(crate::work_brief::WorkTab),
    WorkLatest,
    WorkRecord(String),
    WorkResults,
    WorkExpand,
    WorkActions,
    ApplyAppearance,
    CancelAppearance,
    Attention,
    Deliveries,
    Find,
    NewTask,
    Connections,
    Sources,
    SelectFloor(OfficeId),
    EnterFloor(OfficeId),
    Inspect(WorkerId),
    InspectRecord(WorkerId, String),
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

/// One keyboard stop per semantic action. Keep its first position in reading
/// order, but prefer its last physical target (usually the native nameplate)
/// for the visible focus marker. Mouse hit regions remain unchanged.
pub fn focus_targets(hits: &[HitRegion]) -> Vec<HitRegion> {
    let mut targets: Vec<HitRegion> = Vec::new();
    for hit in hits
        .iter()
        .filter(|hit| hit.area.width > 0 && hit.area.height > 0)
    {
        if let Some(existing) = targets.iter_mut().find(|item| item.action == hit.action) {
            existing.area = hit.area;
        } else {
            targets.push(hit.clone());
        }
    }
    targets
}

impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Tower => "Tower",
            Self::Attention => "Attention",
            Self::Deliveries => "Deliveries",
            Self::Find => "Search",
            Self::NewTask => "New task",
            Self::Connections => "Connections",
            Self::More => "More",
            Self::Advanced => "Advanced",
            Self::Key(KeyCode::Char('s')) => "Settings",
            Self::Key(KeyCode::Char('?')) => "Help",
            Self::Close => "Back",
            Self::Decorate(_) => "Design",
            Self::Character(_) => "Character",
            Self::Inspect(_) | Self::InspectRecord(_, _) => "Work brief",
            Self::EnterFloor(_) => "Office",
            Self::SelectFloor(_) => "Select floor",
            Self::WorkTab(tab) => tab.label(),
            Self::WorkLatest => "Back to latest",
            Self::WorkResults => "Results",
            Self::WorkExpand => "Expand / collapse",
            Self::WorkActions | Self::Controls(_) => "Task actions",
            Self::Review(_) => "Review request",
            Self::Team(_) | Self::SelectTeam(_) => "Team",
            Self::SelectDesks(_, _) => "Desks",
            Self::Key(KeyCode::Char('q')) => "Quit",
            _ => "Open",
        }
    }
    pub fn shortcut(&self) -> &'static str {
        match self {
            Self::Tower => "0",
            Self::Attention => "b",
            Self::Find => "/",
            Self::NewTask => "n",
            Self::Connections => "c",
            Self::Key(KeyCode::Char('s')) => "s",
            Self::Key(KeyCode::Char('?')) => "?",
            Self::Close => "Esc",
            Self::Decorate(_) => "d",
            Self::Character(_) => "a",
            _ => "",
        }
    }
}
pub fn navigation_actions() -> Vec<Action> {
    vec![
        Action::Tower,
        Action::Attention,
        Action::Deliveries,
        Action::Find,
        Action::NewTask,
        Action::Connections,
    ]
}
