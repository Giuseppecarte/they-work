//! The pixel-art layer: a half-block canvas, sprites, and the three views.
//!
//! Owner: renderer dev. This crate reads `theywork_core::World` and draws it.
//! It never performs I/O of its own and never looks at agent files.

use std::collections::{BTreeMap, VecDeque};

use crossterm::event::KeyEvent;
use ratatui::Frame;
use theywork_core::{Millis, Worker, WorkerId, World};

pub mod canvas;
pub mod sprite;
pub mod views;

use canvas::Canvas;
use sprite::SpriteSet;

const ACTIVITY_HISTORY_CAP: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActivityRecord {
    pub(crate) at: Millis,
    pub(crate) label: String,
    pub(crate) detail: Option<String>,
    pub(crate) branch: Option<String>,
}

impl ActivityRecord {
    fn matches_worker(&self, worker: &Worker) -> bool {
        self.label == worker.activity.label() && self.detail.as_deref() == worker.activity.detail()
    }

    pub(crate) fn display(&self) -> String {
        match self.detail.as_deref() {
            Some(detail) if !detail.is_empty() => format!("{} • {}", self.label, detail),
            _ => self.label.clone(),
        }
    }
}
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
    selected_worker: usize,
    camera_columns: usize,
    office_columns: usize,
    known_office_count: usize,
    known_worker_count: usize,
    now: Millis,
    canvas: Canvas,
    sprites: SpriteSet,
    activity_history: BTreeMap<WorkerId, VecDeque<ActivityRecord>>,
    phone_open: bool,
    phone_channel: views::phone::PhoneChannel,
    phone_transition_at: Millis,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            view: View::Cameras,
            selected_office: 0,
            selected_worker: 0,
            camera_columns: 1,
            office_columns: 1,
            known_office_count: 0,
            known_worker_count: 0,
            now: 0,
            canvas: Canvas::new(0, 0),
            sprites: SpriteSet::new(),
            activity_history: BTreeMap::new(),
            phone_open: false,
            phone_channel: views::phone::PhoneChannel::Standup,
            phone_transition_at: 0,
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

    /// Selected office index in the camera attention-first order.
    pub fn selected_office(&self) -> usize {
        self.selected_office
    }

    /// Selected worker index in the current office.
    pub fn selected_worker(&self) -> usize {
        self.selected_worker
    }

    /// Whether the phone overlay is currently visible.
    pub fn phone_open(&self) -> bool {
        self.phone_open
    }

    /// The channel selected in the phone overlay.
    pub fn phone_channel(&self) -> views::phone::PhoneChannel {
        self.phone_channel
    }

    /// Handle one key press.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<UiCommand> {
        use crossterm::event::KeyCode;

        if self.phone_open && self.handle_phone_key(key.code) {
            return None;
        }

        match key.code {
            KeyCode::Char('q') => Some(UiCommand::Quit),
            KeyCode::Char('p') => {
                self.phone_open = !self.phone_open;
                self.phone_transition_at = self.now;
                None
            }
            KeyCode::Enter => {
                match self.view {
                    View::Cameras if self.known_office_count > 0 => self.view = View::Office,
                    View::Office if self.known_worker_count > 0 => self.view = View::Desk,
                    _ => {}
                }
                None
            }
            KeyCode::Esc | KeyCode::Backspace => {
                self.view = match self.view {
                    View::Cameras => View::Cameras,
                    View::Office => View::Cameras,
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

    /// Draw the current view.
    pub fn draw(&mut self, f: &mut Frame, world: &World) {
        self.remember_activities(world);
        self.known_office_count = world.office_count();
        if self.known_office_count == 0 {
            self.selected_office = 0;
        } else {
            self.selected_office = self.selected_office.min(self.known_office_count - 1);
        }

        let offices = views::cameras::ordered_offices(world, self.now);
        let office = offices.get(self.selected_office).copied();
        self.known_worker_count = office.map_or(0, |value| value.workers.len());
        if self.known_worker_count == 0 {
            self.selected_worker = 0;
        } else {
            self.selected_worker = self.selected_worker.min(self.known_worker_count - 1);
        }

        match self.view {
            View::Cameras => {
                let layout = views::cameras::draw(
                    f,
                    world,
                    &mut self.canvas,
                    &self.sprites,
                    self.now,
                    self.selected_office,
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
                );
                self.office_columns = layout.columns.max(1);
            }
            View::Desk => {
                let worker = office.and_then(|value| value.workers.get(self.selected_worker));
                let history = worker
                    .and_then(|value| self.activity_history.get(&value.id))
                    .map(|items| {
                        items
                            .iter()
                            .map(ActivityRecord::display)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                views::desk::draw(
                    f,
                    office,
                    worker,
                    &history,
                    &mut self.canvas,
                    &self.sprites,
                    self.now,
                );
            }
        }
        if self.phone_open {
            views::phone::draw(
                f,
                views::phone::PhoneDrawContext {
                    world,
                    channel: self.phone_channel,
                    history: &self.activity_history,
                    now: self.now,
                    transition_at: self.phone_transition_at,
                    canvas: &mut self.canvas,
                    sprites: &self.sprites,
                },
            );
        }
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
                self.selected_office = move_grid_index(
                    self.selected_office,
                    self.known_office_count,
                    self.camera_columns.max(1),
                    code,
                );
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

    fn remember_activities(&mut self, world: &World) {
        for office in world.offices() {
            for worker in &office.workers {
                let record = ActivityRecord {
                    at: self.now,
                    label: worker.activity.label().to_string(),
                    detail: worker.activity.detail().map(str::to_string),
                    branch: worker.git_branch.clone(),
                };
                let entry = self.activity_history.entry(worker.id.clone()).or_default();
                let changed = entry.back().is_none_or(|last| !last.matches_worker(worker));
                if changed {
                    entry.push_back(record);
                    while entry.len() > ACTIVITY_HISTORY_CAP {
                        entry.pop_front();
                    }
                }
            }
        }
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
        assert_eq!(ui.view(), View::Cameras);
        ui.handle_key(enter);
        assert_eq!(ui.view(), View::Office);
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("office frame");
        ui.handle_key(enter);
        assert_eq!(ui.view(), View::Desk);
        ui.handle_key(escape);
        assert_eq!(ui.view(), View::Office);
        ui.handle_key(escape);
        assert_eq!(ui.view(), View::Cameras);
        assert_eq!(
            ui.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(UiCommand::Quit)
        );
    }
}
#[cfg(test)]
mod phone_tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use theywork_core::{Activity, Agent, Event, EventKind, OfficeId, WorkerId};

    use super::*;

    fn history_event(worker: &WorkerId, at: Millis, kind: EventKind) -> Event {
        Event {
            at,
            office: OfficeId("/history".into()),
            office_path: "/history".into(),
            worker: worker.clone(),
            agent: Agent::Codex,
            kind,
        }
    }

    fn demo_world(now: Millis) -> World {
        let mut world = World::new();
        for event in theywork_core::demo::events(now) {
            world.apply(event);
        }
        world
    }

    #[test]
    fn phone_history_is_bounded_and_tracks_activity_changes() {
        let worker = WorkerId("/history#dev".into());
        let mut world = World::new();
        let mut ui = Ui::new();
        for index in 0..(ACTIVITY_HISTORY_CAP + 3) {
            let at = index as Millis;
            world.apply(history_event(
                &worker,
                at,
                EventKind::Acted(Activity::Typing {
                    detail: format!("command-{index}"),
                }),
            ));
            ui.tick(at);
            ui.remember_activities(&world);
        }

        let history = ui
            .activity_history
            .get(&worker)
            .expect("worker history should be captured");
        assert_eq!(history.len(), ACTIVITY_HISTORY_CAP);
        assert_eq!(
            history.front().and_then(|item| item.detail.as_deref()),
            Some("command-3")
        );
        assert_eq!(
            history.back().and_then(|item| item.detail.as_deref()),
            Some("command-10")
        );
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
        assert_eq!(ui.view(), View::Cameras);

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
    fn eleven_worker_office_pagination_is_reachable_and_labeled() {
        let world = world_with_office_counts(&[11], false);
        let mut ui = Ui::new();
        ui.tick(BLOCKED_AFTER_MS + 1);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("camera frame");
        ui.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("first office page");
        assert_eq!(views::office::desk_layout(11, 100, 26).pages, 4);

        for _ in 0..3 {
            ui.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            terminal
                .draw(|frame| ui.draw(frame, &world))
                .expect("next office page");
        }
        assert_eq!(ui.selected_worker(), 9);
        assert!(
            buffer_text(&terminal).contains("page 4/4"),
            "page indicator should remain visible on the final page"
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
}
