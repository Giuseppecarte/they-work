//! The pixel-art layer: a half-block canvas, sprites, and the three views.
//!
//! Owner: renderer dev. This crate reads `theywork_core::World` and draws it.
//! It never performs I/O of its own and never looks at agent files.

use crossterm::event::KeyEvent;
use ratatui::Frame;
use theywork_core::{Millis, World};

/// What the UI wants the host program to do after handling input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiCommand {
    /// Leave the building.
    Quit,
}

/// All view state: which screen is showing, what is selected, animation phase.
///
/// The host owns the data (`World`); this owns only presentation.
pub struct Ui {
    _private: (),
}

impl Ui {
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Advance animations. Called once per frame, before `draw`.
    pub fn tick(&mut self, _now: Millis) {}

    /// Handle one key press.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<UiCommand> {
        use crossterm::event::KeyCode;
        matches!(key.code, KeyCode::Char('q') | KeyCode::Esc).then_some(UiCommand::Quit)
    }

    /// Draw the current view.
    pub fn draw(&mut self, f: &mut Frame, world: &World) {
        // TODO(renderer dev): replace with the camera grid / office / desk views.
        let text = format!(
            "they-work\n\n{} offices, {} workers\n\nq to quit",
            world.office_count(),
            world.worker_count()
        );
        f.render_widget(ratatui::widgets::Paragraph::new(text), f.area());
    }
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}
