//! The pixel-art layer: a half-block canvas, sprites, and the presentation views.
//!
//! Owner: renderer dev. This crate reads `theywork_core::World` and draws it.
//! It never performs I/O of its own and never looks at agent files.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use theywork_core::{Millis, OfficeId, World};

pub mod canvas;
pub mod sprite;
pub mod views;

#[cfg(test)]
mod golden;
use canvas::{Canvas, ColorDepth, PixelEncoding};
use sprite::{look_for_worker, SpriteSet};

/// The current screen in the presentation hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Cameras,
    Office,
    Desk,
}

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
    view: View,
    selected_office: usize,
    selected_office_id: Option<OfficeId>,
    selected_worker: usize,
    camera_columns: usize,
    office_columns: usize,
    known_office_count: usize,
    known_worker_count: usize,
    now: Millis,
    canvas: Canvas,
    sprites: SpriteSet,
    phone_open: bool,
    phone_channel: views::phone::PhoneChannel,
    phone_transition_at: Millis,
    help_open: bool,
    guard_all: bool,
    settings_open: bool,
    settings_cursor: usize,
    projection: views::office::Projection,
    theme: views::UiTheme,
    color_depth: ColorDepth,
    color_locked: bool,
    encoding: PixelEncoding,
    encoding_locked: bool,
    motion: bool,
    name_plates: bool,
}

impl Ui {
    pub fn new() -> Self {
        let color_depth = ColorDepth::from_environment();
        let color_locked = ColorDepth::environment_override();
        let encoding = PixelEncoding::from_environment();
        let encoding_locked = PixelEncoding::environment_override();
        Self {
            view: View::Office,
            selected_office: 0,
            selected_office_id: None,
            selected_worker: 0,
            camera_columns: 1,
            office_columns: 1,
            known_office_count: 0,
            known_worker_count: 0,
            now: 0,
            canvas: Canvas::with_color_depth_and_encoding(0, 0, color_depth, encoding),
            sprites: SpriteSet::new(),
            phone_open: false,
            phone_channel: views::phone::PhoneChannel::Standup,
            phone_transition_at: 0,
            help_open: false,
            guard_all: false,
            settings_open: false,
            settings_cursor: 0,
            projection: views::office::Projection::Auto,
            theme: views::UiTheme::Dark,
            color_depth,
            color_locked,
            encoding,
            encoding_locked,
            motion: true,
            name_plates: true,
        }
    }

    /// Advance animations. Called once per frame, before `draw`.
    pub fn tick(&mut self, now: Millis) {
        self.now = now;
    }

    /// Current presentation screen.
    pub fn view(&self) -> View {
        self.view
    }

    /// Selected office index in the current top-level view's stable order.
    pub fn selected_office(&self) -> usize {
        self.selected_office
    }

    /// Selected worker index in the current office.
    pub fn selected_worker(&self) -> usize {
        self.selected_worker
    }

    /// The active pixel packing used for the next frame.
    pub fn encoding(&self) -> PixelEncoding {
        self.encoding
    }

    /// Whether the phone overlay is currently visible.
    pub fn phone_open(&self) -> bool {
        self.phone_open
    }

    /// Whether the key reference overlay is currently visible.
    pub fn help_open(&self) -> bool {
        self.help_open
    }

    /// The channel selected in the phone overlay.
    pub fn phone_channel(&self) -> views::phone::PhoneChannel {
        self.phone_channel
    }

    /// Handle one key press.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<UiCommand> {
        use crossterm::event::KeyCode;

        if self.help_open {
            if matches!(
                key.code,
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q')
            ) {
                self.help_open = false;
            }
            return None;
        }
        if self.settings_open {
            return self.handle_settings_key(key.code);
        }
        if self.phone_open && self.handle_phone_key(key.code) {
            return None;
        }

        if key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT) {
            self.cycle_office(false);
            return None;
        }
        match key.code {
            KeyCode::Char('q') => Some(UiCommand::Quit),
            KeyCode::Char('?') => {
                self.help_open = true;
                None
            }
            KeyCode::Char('p') => {
                self.phone_open = !self.phone_open;
                self.phone_transition_at = self.now;
                None
            }
            KeyCode::Char('s') => {
                self.settings_open = true;
                self.settings_cursor = 0;
                self.phone_open = false;
                None
            }
            KeyCode::Char('0') => {
                self.guard_all = true;
                self.view = View::Cameras;
                None
            }
            KeyCode::Char(digit @ '1'..='9') => {
                let index = usize::from(digit as u8 - b'1');
                self.jump_office(index);
                None
            }
            KeyCode::Char('c') => {
                self.projection = self.projection.next();
                None
            }
            KeyCode::Tab => {
                self.cycle_office(true);
                None
            }
            KeyCode::BackTab => {
                self.cycle_office(false);
                None
            }
            KeyCode::Enter => {
                match self.view {
                    View::Cameras if self.known_office_count > 0 => {
                        self.view = View::Office;
                        self.guard_all = false;
                    }
                    View::Office if self.known_worker_count > 0 => self.view = View::Desk,
                    _ => {}
                }
                None
            }
            KeyCode::Esc | KeyCode::Backspace => {
                self.view = match self.view {
                    View::Cameras | View::Office => self.view,
                    View::Desk => View::Office,
                };
                None
            }
            KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Char('h')
            | KeyCode::Char('j')
            | KeyCode::Char('k')
            | KeyCode::Char('l') => {
                self.move_selection(key.code);
                None
            }
            _ => None,
        }
    }

    fn cycle_office(&mut self, forward: bool) {
        if self.known_office_count == 0 {
            self.guard_all = true;
            self.view = View::Cameras;
            return;
        }
        let count = self.known_office_count;
        self.selected_office = if forward {
            self.selected_office.saturating_add(1) % count
        } else {
            (self.selected_office + count - 1) % count
        };
        self.selected_office_id = None;
        self.selected_worker = 0;
        self.guard_all = false;
        self.view = View::Office;
    }

    fn jump_office(&mut self, index: usize) {
        if index >= self.known_office_count {
            return;
        }
        self.selected_office = index;
        self.selected_office_id = None;
        self.selected_worker = 0;

        self.guard_all = false;
        self.view = View::Office;
    }
    fn handle_settings_key(&mut self, code: KeyCode) -> Option<UiCommand> {
        match code {
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('s') | KeyCode::Char('q') => {
                self.settings_open = false;
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_cursor = (self.settings_cursor + 6) % 7;
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.settings_cursor = (self.settings_cursor + 1) % 7;
                None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.adjust_setting(false);
                None
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
                self.adjust_setting(true);
                None
            }
            _ => None,
        }
    }

    fn adjust_setting(&mut self, forward: bool) {
        match self.settings_cursor {
            0 => {
                self.projection = if forward {
                    self.projection.next()
                } else {
                    self.projection.previous()
                };
            }
            1 | 2 => {
                self.theme = if self.theme == views::UiTheme::Dark {
                    views::UiTheme::Light
                } else {
                    views::UiTheme::Dark
                };
            }
            3 if !self.color_locked => {
                self.color_depth = match (self.color_depth, forward) {
                    (ColorDepth::TrueColor, true) => ColorDepth::Palette256,
                    (ColorDepth::Palette256, true) => ColorDepth::None,
                    (ColorDepth::None, true) => ColorDepth::TrueColor,
                    (ColorDepth::TrueColor, false) => ColorDepth::None,
                    (ColorDepth::None, false) => ColorDepth::Palette256,
                    (ColorDepth::Palette256, false) => ColorDepth::TrueColor,
                };
            }
            4 => self.motion = !self.motion,
            5 => self.name_plates = !self.name_plates,
            6 if !self.encoding_locked => self.encoding = self.encoding.next(forward),
            _ => {}
        }
    }

    /// Draw the current view.
    pub fn draw(&mut self, f: &mut Frame, world: &World) {
        self.canvas.set_color_depth(self.color_depth);
        self.canvas.set_encoding(self.encoding);
        self.canvas
            .set_light_mode(self.theme == views::UiTheme::Light);
        self.sprites
            .set_animation_time(Some(if self.motion { self.now } else { 0 }));
        let offices = views::cameras::ordered_offices(world, self.now);
        self.known_office_count = offices.len();
        self.sync_office_selection(&offices);
        let office = offices.get(self.selected_office).copied();
        self.known_worker_count = office.map_or(0, |value| value.workers.len());
        if self.known_worker_count == 0 {
            self.selected_worker = 0;
        } else {
            self.selected_worker = self.selected_worker.min(self.known_worker_count - 1);
        }

        views::draw_tab_bar(f, &offices, self.selected_office, self.guard_all, self.now);
        match self.view {
            View::Cameras => {
                let layout = views::cameras::draw(
                    f,
                    world,
                    &mut self.canvas,
                    &self.sprites,
                    self.now,
                    self.selected_office,
                    self.guard_all,
                );
                self.camera_columns = layout.columns.max(1);
            }
            View::Office => {
                let layout = views::office::draw(
                    f,
                    office,
                    &mut self.canvas,
                    &self.sprites,
                    self.now,
                    self.selected_worker,
                    self.projection,
                    self.name_plates,
                );
                self.office_columns = layout.columns.max(1);
            }
            View::Desk => {
                let worker = office.and_then(|value| value.workers.get(self.selected_worker));
                views::desk::draw(f, office, worker, &mut self.canvas, &self.sprites, self.now);
            }
        }
        if self.phone_open {
            views::phone::draw(
                f,
                views::phone::PhoneDrawContext {
                    world,
                    channel: self.phone_channel,
                    now: self.now,
                    transition_at: self.phone_transition_at,
                    canvas: &mut self.canvas,
                    sprites: &self.sprites,
                },
            );
        }
        if self.help_open {
            views::help::draw(f);
        }
        if self.settings_open {
            let settings_worker = office.and_then(|office| {
                office
                    .workers
                    .get(self.selected_worker)
                    .map(|worker| (worker, look_for_worker(&office.workers, worker)))
            });
            views::settings::draw(
                f,
                views::settings::SettingsDrawContext {
                    projection: self.projection,
                    theme: self.theme,
                    color_depth: self.color_depth,
                    color_locked: self.color_locked,
                    encoding: self.encoding,
                    encoding_locked: self.encoding_locked,
                    motion: self.motion,
                    name_plates: self.name_plates,
                    cursor: self.settings_cursor,
                    worker: settings_worker,
                    now: self.now,
                    canvas: &mut self.canvas,
                    sprites: &self.sprites,
                },
            );
        }
        views::draw_tab_bar(f, &offices, self.selected_office, self.guard_all, self.now);
        views::remap_buffer_theme(f.buffer_mut(), self.theme);
        if self.canvas.color_depth() == ColorDepth::None {
            Canvas::strip_colors(f.buffer_mut());
        }
    }

    fn sync_office_selection(&mut self, offices: &[&theywork_core::Office]) {
        if offices.is_empty() {
            self.selected_office = 0;
            self.selected_office_id = None;
            return;
        }
        let selected = self
            .selected_office_id
            .as_ref()
            .and_then(|id| offices.iter().position(|office| &office.id == id))
            .unwrap_or_else(|| self.selected_office.min(offices.len() - 1));
        self.selected_office = selected;
        self.selected_office_id = Some(offices[selected].id.clone());
    }

    fn handle_phone_key(&mut self, code: crossterm::event::KeyCode) -> bool {
        use crossterm::event::KeyCode;

        let direct = match code {
            KeyCode::Char('1') => Some(views::phone::PhoneChannel::Standup),
            KeyCode::Char('2') => Some(views::phone::PhoneChannel::Blocked),
            KeyCode::Char('3') => Some(views::phone::PhoneChannel::Shipping),
            KeyCode::Char('4') => Some(views::phone::PhoneChannel::Watercooler),
            _ => None,
        };
        if let Some(channel) = direct {
            self.phone_channel = channel;
            return true;
        }

        match code {
            KeyCode::Left | KeyCode::Up | KeyCode::Char('h') | KeyCode::Char('k') => {
                self.phone_channel = self.phone_channel.previous();
                true
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('l') => {
                self.phone_channel = self.phone_channel.next();
                true
            }
            _ => false,
        }
    }

    fn move_selection(&mut self, code: crossterm::event::KeyCode) {
        match self.view {
            View::Cameras => {
                self.move_office_selection(code, self.camera_columns.max(1));
            }
            View::Office | View::Desk => {
                self.selected_worker = move_grid_index(
                    self.selected_worker,
                    self.known_worker_count,
                    self.office_columns.max(1),
                    code,
                );
            }
        }
    }

    fn move_office_selection(&mut self, code: crossterm::event::KeyCode, columns: usize) {
        self.selected_office =
            move_grid_index(self.selected_office, self.known_office_count, columns, code);
        self.selected_office_id = None;
    }
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

fn move_grid_index(
    index: usize,
    count: usize,
    columns: usize,
    code: crossterm::event::KeyCode,
) -> usize {
    use crossterm::event::KeyCode;
    if count == 0 {
        return 0;
    }
    let last = count - 1;
    match code {
        KeyCode::Left | KeyCode::Char('h') => index.saturating_sub(1),
        KeyCode::Right | KeyCode::Char('l') => index.saturating_add(1).min(last),
        KeyCode::Up | KeyCode::Char('k') => index.saturating_sub(columns),
        KeyCode::Down | KeyCode::Char('j') => index.saturating_add(columns).min(last),
        _ => index.min(last),
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use theywork_core::demo;

    use super::*;

    fn demo_world(now: Millis) -> World {
        let mut world = World::new();
        for event in demo::events(now) {
            world.apply(event);
        }
        world
    }

    #[test]
    fn draw_handles_demo_world_at_normal_and_tiny_sizes() {
        let world = demo_world(12_000);
        for (width, height) in [(80, 24), (24, 10), (4, 4), (1, 1)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            let mut ui = Ui::new();
            ui.tick(12_000);
            terminal
                .draw(|frame| ui.draw(frame, &world))
                .expect("normal or tiny frame should render");
        }
    }

    #[test]
    fn navigation_descends_and_ascends_through_the_view_hierarchy() {
        let world = demo_world(0);
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut ui = Ui::new();
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("camera frame");

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let first_office = ui.selected_office;
        assert_eq!(ui.view(), View::Office);
        ui.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(ui.view(), View::Office);
        assert_ne!(ui.selected_office, first_office);
        ui.handle_key(enter);
        assert_eq!(ui.view(), View::Desk);
        ui.handle_key(escape);
        assert_eq!(ui.view(), View::Office);
        ui.handle_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE));
        assert_eq!(ui.view(), View::Cameras);
        assert!(ui.guard_all);
        ui.handle_key(enter);
        assert_eq!(ui.view(), View::Office);
        assert!(!ui.guard_all);
        ui.handle_key(escape);
        assert_eq!(ui.view(), View::Office);
        assert_eq!(
            ui.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(UiCommand::Quit)
        );
    }
    #[test]
    fn settings_motion_toggle_is_session_only() {
        let mut ui = Ui::new();
        let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
        ui.handle_key(press(KeyCode::Char('s')));
        assert!(ui.settings_open);
        for _ in 0..4 {
            ui.handle_key(press(KeyCode::Down));
        }
        assert!(ui.motion);
        ui.handle_key(press(KeyCode::Enter));
        assert!(!ui.motion);
        ui.handle_key(press(KeyCode::Char('s')));
        assert!(!ui.settings_open);
    }
}
#[cfg(test)]
mod phone_tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;

    fn demo_world(now: Millis) -> World {
        let mut world = World::new();
        for event in theywork_core::demo::events(now) {
            world.apply(event);
        }
        world
    }

    #[test]
    fn phone_overlay_navigates_channels_and_renders_at_tiny_sizes() {
        let world = demo_world(0);
        let mut ui = Ui::new();
        ui.tick(1_000);
        let press = |code| KeyEvent::new(code, KeyModifiers::NONE);

        assert!(!ui.phone_open());
        ui.handle_key(press(KeyCode::Char('p')));
        assert!(ui.phone_open());
        ui.handle_key(press(KeyCode::Char('3')));
        assert_eq!(ui.phone_channel(), views::phone::PhoneChannel::Shipping);
        ui.handle_key(press(KeyCode::Right));
        assert_eq!(ui.phone_channel(), views::phone::PhoneChannel::Watercooler);
        ui.handle_key(press(KeyCode::Left));
        assert_eq!(ui.phone_channel(), views::phone::PhoneChannel::Shipping);
        assert_eq!(ui.view(), View::Office);

        for (width, height) in [(80, 24), (24, 10), (4, 4), (1, 1)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| ui.draw(frame, &world))
                .expect("phone overlay should render");
        }

        ui.handle_key(press(KeyCode::Char('p')));
        assert!(!ui.phone_open());
    }
}
#[cfg(test)]
mod m3_tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::time::{Duration, Instant};
    use theywork_core::{Activity, Agent, Event, EventKind, OfficeId, WorkerId, BLOCKED_AFTER_MS};

    use super::*;

    fn event(office: &str, worker: &WorkerId, at: Millis, agent: Agent, kind: EventKind) -> Event {
        Event {
            at,
            office: OfficeId(office.to_string()),
            office_path: office.to_string(),
            worker: worker.clone(),
            agent,
            kind,
        }
    }

    fn world_with_office_counts(counts: &[usize], long_names: bool) -> World {
        const TOKEN_LADDER: [u64; 11] = [
            136_934_015,
            4_900_000,
            900_000,
            120_000,
            35_000,
            10_000,
            4_900,
            1_000,
            100,
            10,
            0,
        ];
        let mut world = World::new();
        for (office_index, worker_count) in counts.iter().copied().enumerate() {
            let office = if long_names && office_index == 0 {
                format!("/workspace/{}", "o".repeat(200))
            } else {
                format!("/workspace/office-{office_index}")
            };
            for worker_index in 0..worker_count {
                let worker = WorkerId(format!("{office}#worker-{worker_index}"));
                let name = if long_names && office_index == 0 && worker_index == 0 {
                    format!("{}界🛠️", "x".repeat(200))
                } else {
                    format!("Worker {office_index}-{worker_index}")
                };
                let agent = if worker_index % 2 == 0 {
                    Agent::Codex
                } else {
                    Agent::Claude
                };
                let activity = if office_index == 0 && worker_index == 0 {
                    Activity::Waiting {
                        detail: "approve release".into(),
                    }
                } else if worker_index % 3 == 0 {
                    Activity::Editing {
                        detail: format!("src/module-{worker_index}.rs"),
                    }
                } else {
                    Activity::Idle
                };
                world.apply(event(
                    &office,
                    &worker,
                    0,
                    agent,
                    EventKind::Seen {
                        name,
                        git_branch: Some(format!("codex/office-{office_index}")),
                    },
                ));
                let tokens = if office_index == 0 {
                    TOKEN_LADDER[worker_index.min(TOKEN_LADDER.len() - 1)]
                } else {
                    worker_index as u64 * 1_000
                };
                world.apply(event(&office, &worker, 0, agent, EventKind::Tokens(tokens)));
                world.apply(event(
                    &office,
                    &worker,
                    0,
                    agent,
                    EventKind::Turn {
                        in_flight: activity.is_busy(),
                    },
                ));
                world.apply(event(
                    &office,
                    &worker,
                    0,
                    agent,
                    EventKind::Acted(activity),
                ));
            }
        }
        world
    }

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn six_uneven_offices_and_single_worker_floor_render() {
        let world = world_with_office_counts(&[11, 8, 6, 4, 1, 1], false);
        assert_eq!(world.office_count(), 6);
        assert_eq!(world.worker_count(), 31);
        assert_eq!(
            views::cameras::grid_layout(6, 80, 20),
            views::cameras::GridLayout {
                columns: 3,
                rows: 2,
            }
        );

        let mut ui = Ui::new();
        ui.tick(BLOCKED_AFTER_MS + 1);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
        ui.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("crowded camera wall should render");
        assert!(
            buffer_text(&terminal).contains('!'),
            "blocked Waiting worker should be visible in the camera wall"
        );

        for _ in 0..5 {
            ui.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("last one-worker office should render");
        assert_eq!(ui.selected_office(), 5);
        ui.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("one-worker office floor should render");
        assert_eq!(ui.selected_worker(), 0);
    }

    #[test]
    fn eleven_worker_floor_pagination_is_reachable_and_labeled() {
        let world = world_with_office_counts(&[11], false);
        let mut ui = Ui::new();
        ui.tick(BLOCKED_AFTER_MS + 1);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("first floor page");
        assert_eq!(views::office::desk_layout(11, 100, 26).pages, 2);

        for _ in 0..2 {
            ui.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            terminal
                .draw(|frame| ui.draw(frame, &world))
                .expect("next floor page");
        }
        assert_eq!(ui.selected_worker(), 10);
        let text = buffer_text(&terminal);
        assert!(
            text.contains("page 2/2"),
            "page indicator should remain visible on the final page"
        );
        assert!(
            text.contains("+1 overflow"),
            "overflow indicator should identify workers beyond the visible desks"
        );
    }

    #[test]
    fn pathological_names_render_in_every_view_and_at_tiny_sizes() {
        let world = world_with_office_counts(&[1], true);
        let worker = &world.offices().next().expect("office").workers[0];
        assert!(worker.name.chars().count() >= 200);

        let mut ui = Ui::new();
        ui.tick(BLOCKED_AFTER_MS + 1);
        for (width, height) in [(100, 30), (24, 10), (16, 8), (4, 4), (1, 1)] {
            let mut terminal =
                Terminal::new(TestBackend::new(width, height)).expect("test terminal");
            terminal
                .draw(|frame| ui.draw(frame, &world))
                .expect("pathological names should never panic");
        }

        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
        ui.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("office name plate should render");
        ui.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("desk name plate should render");
        ui.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("phone name plate should render");
        assert!(
            buffer_text(&terminal).contains("MESSAGES"),
            "phone first frame should already show its title"
        );
    }
    #[test]
    fn isometric_floor_renders_project_scene_and_manager_alert() {
        let world = world_with_office_counts(&[11, 8, 6, 4, 1, 1], false);
        let mut ui = Ui::new();
        ui.tick(BLOCKED_AFTER_MS + 1);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("isometric floor frame");
        assert_eq!(ui.view(), View::Office);
        let initial = buffer_text(&terminal);
        for label in ["FLOOR", "office-0", "BLOCKED", "MANAGER ON FLOOR"] {
            assert!(initial.contains(label), "floor should show {label}");
        }

        ui.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        ui.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("overflow floor frame");
        let overflow = buffer_text(&terminal);
        assert_eq!(ui.selected_worker(), 10);
        assert!(overflow.contains("page 2/2"));
        assert!(overflow.contains("+1 overflow"));
    }

    #[test]
    fn help_overlay_lists_bindings_and_closes_cleanly() {
        let world = world_with_office_counts(&[1], false);
        let mut ui = Ui::new();
        let press = |code| KeyEvent::new(code, KeyModifiers::NONE);

        assert_eq!(ui.handle_key(press(KeyCode::Char('?'))), None);
        assert!(ui.help_open());
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("help frame");
        let text = buffer_text(&terminal);
        for binding in [
            "HELP",
            "move",
            "Enter",
            "Backspace",
            "Tab",
            "phone",
            "1-4",
            "q",
        ] {
            assert!(text.contains(binding), "help should list {binding}");
        }

        for (width, height) in [(24, 10), (4, 4)] {
            let mut terminal =
                Terminal::new(TestBackend::new(width, height)).expect("small terminal");
            terminal
                .draw(|frame| ui.draw(frame, &world))
                .expect("help should fit small terminals");
        }

        assert_eq!(ui.handle_key(press(KeyCode::Char('?'))), None);
        assert!(!ui.help_open());
        assert_eq!(ui.handle_key(press(KeyCode::Char('?'))), None);
        assert!(ui.help_open());
        assert_eq!(ui.handle_key(press(KeyCode::Esc)), None);
        assert!(!ui.help_open());
        assert_eq!(ui.handle_key(press(KeyCode::Char('?'))), None);
        assert_eq!(ui.handle_key(press(KeyCode::Char('q'))), None);
        assert!(!ui.help_open());
        assert_eq!(
            ui.handle_key(press(KeyCode::Char('q'))),
            Some(UiCommand::Quit)
        );
    }

    #[test]
    fn repeated_floor_frames_reuse_canvas_storage() {
        let world = world_with_office_counts(&[11, 8, 6, 4, 1, 1], false);
        let mut ui = Ui::new();
        ui.tick(BLOCKED_AFTER_MS + 1);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("first floor frame");
        let capacity = ui.canvas.pixel_capacity();
        let started = Instant::now();

        for frame_index in 0..120 {
            ui.tick(BLOCKED_AFTER_MS + 2 + frame_index as Millis);
            terminal
                .draw(|frame| ui.draw(frame, &world))
                .expect("repeated floor frame");
        }

        assert_eq!(
            ui.canvas.pixel_capacity(),
            capacity,
            "floor frames should reuse the canvas allocation"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "repeated floor rendering should remain bounded"
        );
    }

    #[test]
    fn large_floor_at_ten_fps_reuses_canvas_and_parsed_sprites() {
        let world = world_with_office_counts(&[24, 22, 20, 18, 16, 14, 12, 10, 8, 6, 4, 2], false);
        let mut ui = Ui::new();
        ui.selected_office = world.office_count().saturating_sub(1);
        let mut terminal =
            Terminal::new(TestBackend::new(120, 40)).expect("large-world test terminal");

        for frame_index in 0..60 {
            ui.tick(BLOCKED_AFTER_MS + frame_index as Millis * 100);
            terminal
                .draw(|frame| ui.draw(frame, &world))
                .expect("warm-up floor frame");
        }

        let capacity = ui.canvas.pixel_capacity();
        let parsed_sprites = ui.sprites.parsed_count();
        assert!(
            parsed_sprites > 0,
            "the floor should parse sprites during warm-up"
        );
        let started = Instant::now();

        for frame_index in 60..160 {
            ui.tick(BLOCKED_AFTER_MS + frame_index as Millis * 100);
            terminal
                .draw(|frame| ui.draw(frame, &world))
                .expect("steady-state floor frame");
        }

        assert_eq!(
            ui.canvas.pixel_capacity(),
            capacity,
            "large floor frames should reuse the canvas allocation"
        );
        assert_eq!(
            ui.sprites.parsed_count(),
            parsed_sprites,
            "steady-state floor frames should not parse new sprite frames"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "120 floor frames at 10 fps should remain bounded"
        );
    }

    #[test]
    fn failed_worker_floor_has_explicit_alert() {
        let mut world = world_with_office_counts(&[2], false);
        let failed = WorkerId("/workspace/office-0#worker-1".into());
        world.apply(event(
            "/workspace/office-0",
            &failed,
            0,
            Agent::Claude,
            EventKind::Acted(Activity::Error {
                detail: "integration test failed".into(),
            }),
        ));
        let mut ui = Ui::new();
        ui.tick(1);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("failed worker floor frame");
        let text = buffer_text(&terminal);
        assert!(text.contains("FAILED"));
        assert!(text.contains("CHECK DESK"));
    }
}
