//! The pixel-art layer: a half-block canvas, sprites, and the presentation views.
//!
//! Owner: renderer dev. This crate reads `theywork_core::World` and draws it.
//! It never performs I/O of its own and never looks at agent files.

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use theywork_core::{Millis, OfficeId, World};

pub mod canvas;
pub mod sprite;
pub mod views;

#[cfg(test)]
mod golden;
use canvas::Canvas;
pub use canvas::PixelFrame;
pub use canvas::{ColorDepth, PixelEncoding};
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
    /// Choose which local conversation folders the host may read.
    Sources,
}

/// Renderer-owned explanation of the terminal presentation policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererDiagnostics {
    pub color_depth: ColorDepth,
    pub color_reason: String,
    pub encoding: PixelEncoding,
    pub encoding_reason: String,
}

/// Portable presentation choices. Automatic terminal capabilities are not
/// persisted, so the same preferences remain usable on another terminal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RendererPreferences {
    pub projection: String,
    pub light: bool,
    pub motion: bool,
    pub name_plates: bool,
    pub color_depth: Option<String>,
    pub encoding: Option<String>,
    pub wardrobe: BTreeMap<String, usize>,
    pub office_palettes: BTreeMap<String, usize>,
}

impl Default for RendererPreferences {
    fn default() -> Self {
        Self {
            projection: "auto".into(),
            light: false,
            motion: true,
            name_plates: true,
            color_depth: None,
            encoding: None,
            wardrobe: BTreeMap::new(),
            office_palettes: BTreeMap::new(),
        }
    }
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
    camera_page_size: usize,
    office_columns: usize,
    known_office_count: usize,
    known_worker_count: usize,
    now: Millis,
    canvas: Canvas,
    sprites: SpriteSet,
    phone_open: bool,
    phone_channel: views::phone::PhoneChannel,
    phone_transition_at: Millis,
    phone_selected: usize,
    phone_workers: Vec<Option<theywork_core::WorkerId>>,
    phone_pending_worker: Option<theywork_core::WorkerId>,
    desk_scroll: usize,
    help_open: bool,
    help_scroll: usize,
    guard_all: bool,
    settings_open: bool,
    settings_cursor: usize,
    projection: views::office::Projection,
    theme: views::UiTheme,
    color_depth: ColorDepth,
    color_locked: bool,
    encoding: PixelEncoding,
    encoding_locked: bool,
    saved_color: Option<ColorDepth>,
    saved_encoding: Option<PixelEncoding>,
    color_reason: String,
    encoding_reason: String,
    motion: bool,
    name_plates: bool,
    attention_workers: Vec<theywork_core::WorkerId>,
    selected_worker_id: Option<theywork_core::WorkerId>,
    wardrobe: BTreeMap<String, usize>,
    office_palettes: BTreeMap<String, usize>,
    selected_office_palette: usize,
}

impl Ui {
    pub fn new() -> Self {
        let (color_depth, color_locked, color_reason) = ColorDepth::environment_selection();
        let (encoding, encoding_locked, encoding_reason) = PixelEncoding::environment_selection();
        Self {
            view: View::Office,
            selected_office: 0,
            selected_office_id: None,
            selected_worker: 0,
            camera_columns: 1,
            camera_page_size: 1,
            office_columns: 1,
            known_office_count: 0,
            known_worker_count: 0,
            now: 0,
            canvas: Canvas::with_color_depth_and_encoding(0, 0, color_depth, encoding),
            sprites: SpriteSet::new(),
            phone_open: false,
            phone_channel: views::phone::PhoneChannel::Standup,
            phone_transition_at: 0,
            phone_selected: 0,
            phone_workers: Vec::new(),
            phone_pending_worker: None,
            desk_scroll: 0,
            help_open: false,
            help_scroll: 0,
            guard_all: false,
            settings_open: false,
            settings_cursor: 0,
            projection: views::office::Projection::Auto,
            theme: views::UiTheme::Dark,
            color_depth,
            color_locked,
            encoding,
            encoding_locked,
            saved_color: None,
            saved_encoding: None,
            color_reason,
            encoding_reason,
            motion: true,
            name_plates: true,
            attention_workers: Vec::new(),
            selected_worker_id: None,
            wardrobe: BTreeMap::new(),
            office_palettes: BTreeMap::new(),
            selected_office_palette: 0,
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

    /// Open a saved project after the next world snapshot is drawn.
    pub fn open_office(&mut self, office: &OfficeId) {
        self.selected_office_id = Some(office.clone());
        self.selected_worker = 0;
        self.selected_worker_id = None;
        self.view = View::Office;
        self.guard_all = false;
    }

    /// Show every project as one floor in the software tower.
    pub fn open_tower(&mut self) {
        self.view = View::Cameras;
        self.guard_all = true;
    }

    /// Selected worker index in the current office.
    pub fn selected_worker(&self) -> usize {
        self.selected_worker
    }

    /// The active pixel packing used for the next frame.
    pub fn encoding(&self) -> PixelEncoding {
        self.encoding
    }

    /// Explain the colour and character-density choices without duplicating
    /// terminal policy in the host application.
    pub fn diagnostics(&self) -> RendererDiagnostics {
        RendererDiagnostics {
            color_depth: self.color_depth,
            color_reason: self.color_reason.clone(),
            encoding: self.encoding,
            encoding_reason: self.encoding_reason.clone(),
        }
    }

    /// Snapshot user choices for host-owned persistence.
    pub fn preferences(&self) -> RendererPreferences {
        RendererPreferences {
            projection: self.projection.label().into(),
            light: self.theme == views::UiTheme::Light,
            motion: self.motion,
            name_plates: self.name_plates,
            color_depth: self
                .saved_color
                .map(|color| views::settings::color_depth_label(color).into()),
            encoding: self.saved_encoding.map(|encoding| encoding.label().into()),
            wardrobe: self.wardrobe.clone(),
            office_palettes: self.office_palettes.clone(),
        }
    }

    /// Restore presentation preferences while respecting explicit environment overrides.
    pub fn restore_preferences(&mut self, preferences: &RendererPreferences) {
        let mut projection = views::office::Projection::Auto;
        for _ in 0..5 {
            if projection.label() == preferences.projection {
                self.projection = projection;
                break;
            }
            projection = projection.next();
        }
        self.theme = if preferences.light {
            views::UiTheme::Light
        } else {
            views::UiTheme::Dark
        };
        self.motion = preferences.motion;
        self.name_plates = preferences.name_plates;
        self.wardrobe = preferences
            .wardrobe
            .iter()
            .filter(|(_, preset)| **preset < 6)
            .map(|(worker, preset)| (worker.clone(), *preset))
            .collect();
        self.office_palettes = preferences
            .office_palettes
            .iter()
            .filter(|(_, palette)| **palette < 4)
            .map(|(office, palette)| (office.clone(), *palette))
            .collect();
        self.saved_color = match preferences.color_depth.as_deref() {
            Some("truecolor") => Some(ColorDepth::TrueColor),
            Some("256") => Some(ColorDepth::Palette256),
            Some("none") => Some(ColorDepth::None),
            _ => None,
        };
        self.saved_encoding = PixelEncoding::ALL
            .into_iter()
            .find(|encoding| Some(encoding.label()) == preferences.encoding.as_deref());
        if !self.color_locked {
            if let Some(color) = self.saved_color {
                self.color_depth = color;
                self.color_reason = "saved presentation preference".into();
            }
        }
        if !self.encoding_locked {
            if let Some(encoding) = self.saved_encoding {
                self.encoding = encoding;
                self.encoding_reason = "saved presentation preference".into();
            }
        }
    }

    /// Select the real pixel dimensions of one terminal cell for image output.
    ///
    /// `None` retains the character encoding's native density. A zero dimension
    /// is treated as unavailable terminal geometry and also restores that path.
    pub fn set_image_cell_size(&mut self, cell_pixel_size: Option<(u16, u16)>) {
        let cell_pixel_size = cell_pixel_size
            .filter(|&(width, height)| width > 0 && height > 0)
            .map(|(width, height)| (usize::from(width), usize::from(height)));
        self.canvas.set_cell_pixel_size(cell_pixel_size);
    }

    /// Snapshot the latest canvas pixels for an external terminal-image presenter.
    ///
    /// The returned frame owns its RGBA bytes. Presentation protocol selection
    /// and terminal I/O remain the caller's responsibility.
    pub fn pixel_frame(&self) -> PixelFrame {
        self.canvas.pixel_frame()
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

        if key.kind == crossterm::event::KeyEventKind::Release {
            return None;
        }

        if key.code == KeyCode::Char('c') {
            self.settings_open = false;
            self.help_open = false;
            self.phone_open = false;
            return Some(UiCommand::Sources);
        }
        if self.help_open {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.help_scroll = self.help_scroll.saturating_add(1)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.help_scroll = self.help_scroll.saturating_sub(1)
                }
                KeyCode::PageDown => self.help_scroll = self.help_scroll.saturating_add(5),
                KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(5),
                KeyCode::Home => self.help_scroll = 0,
                KeyCode::End => self.help_scroll = usize::MAX,
                _ => {}
            }
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
                self.help_scroll = 0;
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
            KeyCode::Char('v') => {
                self.projection = self.projection.next();
                None
            }
            KeyCode::Char('o') | KeyCode::Char('O') => {
                if key.code == KeyCode::Char('O') {
                    if let Some(office) = &self.selected_office_id {
                        self.office_palettes.remove(&office.0);
                    }
                } else {
                    self.adjust_office_palette(true);
                }
                None
            }
            KeyCode::Char('w') | KeyCode::Char('W') if self.view == View::Desk => {
                if let Some(worker) = &self.selected_worker_id {
                    if key.code == KeyCode::Char('W') {
                        self.wardrobe.remove(&worker.0);
                    } else {
                        let next = self
                            .wardrobe
                            .get(&worker.0)
                            .map_or(0, |preset| (preset + 1) % 6);
                        self.wardrobe.insert(worker.0.clone(), next);
                    }
                }
                None
            }
            KeyCode::Char('!') => {
                if !self.attention_workers.is_empty() {
                    let next = self
                        .selected_worker_id
                        .as_ref()
                        .and_then(|id| {
                            self.attention_workers
                                .iter()
                                .position(|worker| worker == id)
                        })
                        .map_or(0, |index| (index + 1) % self.attention_workers.len());
                    self.phone_pending_worker = Some(self.attention_workers[next].clone());
                    self.phone_open = false;
                    self.desk_scroll = 0;
                }
                None
            }
            KeyCode::Home | KeyCode::End | KeyCode::PageUp | KeyCode::PageDown => {
                if self.view == View::Cameras {
                    self.selected_office = match key.code {
                        KeyCode::Home => 0,
                        KeyCode::End => self.known_office_count.saturating_sub(1),
                        KeyCode::PageUp => {
                            self.selected_office.saturating_sub(self.camera_page_size)
                        }
                        _ => self
                            .selected_office
                            .saturating_add(self.camera_page_size)
                            .min(self.known_office_count.saturating_sub(1)),
                    };
                    self.selected_office_id = None;
                    self.selected_worker = 0;
                }
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
                    View::Cameras => View::Cameras,
                    View::Office => View::Cameras,
                    View::Desk => View::Office,
                };
                self.guard_all = self.view == View::Cameras;
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
        self.selected_worker_id = None;
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
        self.selected_worker_id = None;

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

    fn adjust_office_palette(&mut self, forward: bool) {
        if let Some(office) = &self.selected_office_id {
            self.selected_office_palette =
                (self.selected_office_palette + if forward { 1 } else { 3 }) % 4;
            self.office_palettes
                .insert(office.0.clone(), self.selected_office_palette);
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
            1 => {
                self.theme = if self.theme == views::UiTheme::Dark {
                    views::UiTheme::Light
                } else {
                    views::UiTheme::Dark
                };
            }
            2 if !self.color_locked => {
                self.color_depth = match (self.color_depth, forward) {
                    (ColorDepth::TrueColor, true) => ColorDepth::Palette256,
                    (ColorDepth::Palette256, true) => ColorDepth::None,
                    (ColorDepth::None, true) => ColorDepth::TrueColor,
                    (ColorDepth::TrueColor, false) => ColorDepth::None,
                    (ColorDepth::None, false) => ColorDepth::Palette256,
                    (ColorDepth::Palette256, false) => ColorDepth::TrueColor,
                };
                self.color_reason = "colour selected in renderer settings".to_string();
                self.saved_color = Some(self.color_depth);
            }
            3 => self.motion = !self.motion,
            4 => self.name_plates = !self.name_plates,
            5 if !self.encoding_locked => {
                self.encoding = self.encoding.next(forward);
                self.encoding_reason = "pixel encoding selected in renderer settings".to_string();
                self.saved_encoding = Some(self.encoding);
            }
            6 => self.adjust_office_palette(forward),
            _ => {}
        }
    }

    /// Draw the current view.
    pub fn draw(&mut self, f: &mut Frame, world: &World) {
        self.sprites.set_wardrobe(&self.wardrobe);
        self.sprites.set_office_palettes(&self.office_palettes);
        self.canvas.set_color_depth(self.color_depth);
        self.canvas.set_encoding(self.encoding);
        self.canvas
            .set_light_mode(self.theme == views::UiTheme::Light);
        self.sprites
            .set_animation_time(Some(if self.motion { self.now } else { 0 }));
        let offices = views::cameras::ordered_offices(world, self.now);
        self.attention_workers = offices
            .iter()
            .flat_map(|office| &office.workers)
            .filter(|worker| views::worker_status(worker, self.now).needs_attention())
            .map(|worker| worker.id.clone())
            .collect();
        self.known_office_count = offices.len();
        self.sync_office_selection(&offices);
        if let Some(id) = self.phone_pending_worker.take() {
            if let Some((office_index, worker_index)) =
                offices.iter().enumerate().find_map(|(index, office)| {
                    office
                        .workers
                        .iter()
                        .position(|worker| worker.id == id)
                        .map(|worker| (index, worker))
                })
            {
                self.selected_office = office_index;
                self.selected_office_id = Some(offices[office_index].id.clone());
                self.selected_worker = worker_index;
                self.selected_worker_id = Some(id);
                self.guard_all = false;
                self.view = View::Desk;
            }
        }
        let office = offices.get(self.selected_office).copied();
        self.selected_office_palette =
            office.map_or(0, |office| self.sprites.office_palette_index(office));
        self.known_worker_count = office.map_or(0, |value| value.workers.len());
        if let Some(index) = office.and_then(|office| {
            self.selected_worker_id
                .as_ref()
                .and_then(|id| office.workers.iter().position(|worker| &worker.id == id))
        }) {
            self.selected_worker = index;
        }
        if self.known_worker_count == 0 {
            self.selected_worker = 0;
        } else {
            self.selected_worker = self.selected_worker.min(self.known_worker_count - 1);
        }
        self.selected_worker_id = office
            .and_then(|office| office.workers.get(self.selected_worker))
            .map(|worker| worker.id.clone());

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
                self.camera_page_size = layout.columns.saturating_mul(layout.rows).max(1);
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
                views::desk::draw(
                    f,
                    office,
                    worker,
                    &mut self.canvas,
                    &self.sprites,
                    self.now,
                    &mut self.desk_scroll,
                );
            }
        }
        if self.phone_open {
            let phone_office = if self.guard_all { None } else { office };
            self.phone_workers =
                views::phone::message_workers(self.phone_channel, world, phone_office, self.now);
            self.phone_selected = self
                .phone_selected
                .min(self.phone_workers.len().saturating_sub(1));
            views::phone::draw(
                f,
                views::phone::PhoneDrawContext {
                    world,
                    office: phone_office,
                    channel: self.phone_channel,
                    selected: self.phone_selected,
                    now: self.now,
                    transition_at: self.phone_transition_at,
                    canvas: &mut self.canvas,
                    sprites: &self.sprites,
                },
            );
        }
        if self.help_open {
            views::help::draw(f, &mut self.help_scroll);
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
                    office,
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
            self.phone_selected = 0;
            self.phone_workers.clear();
            return true;
        }

        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.phone_channel = self.phone_channel.previous();
                self.phone_selected = 0;
                self.phone_workers.clear();
                true
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.phone_channel = self.phone_channel.next();
                self.phone_selected = 0;
                self.phone_workers.clear();
                true
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.phone_selected = self.phone_selected.saturating_sub(1);
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.phone_selected = self
                    .phone_selected
                    .saturating_add(1)
                    .min(self.phone_workers.len().saturating_sub(1));
                true
            }
            KeyCode::Enter => {
                self.phone_pending_worker = self
                    .phone_workers
                    .get(self.phone_selected)
                    .cloned()
                    .flatten();
                if self.phone_pending_worker.is_some() {
                    self.phone_open = false;
                }
                true
            }
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('p') | KeyCode::Char('q') => {
                self.phone_open = false;
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
            View::Desk => match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.desk_scroll = self.desk_scroll.saturating_add(1)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.desk_scroll = self.desk_scroll.saturating_sub(1)
                }
                _ => {
                    self.selected_worker =
                        move_grid_index(self.selected_worker, self.known_worker_count, 1, code);
                    self.desk_scroll = 0;
                    self.selected_worker_id = None;
                }
            },
            View::Office => {
                self.selected_worker = move_grid_index(
                    self.selected_worker,
                    self.known_worker_count,
                    self.office_columns.max(1),
                    code,
                );
                self.selected_worker_id = None;
            }
        }
    }

    fn move_office_selection(&mut self, code: crossterm::event::KeyCode, columns: usize) {
        self.selected_office =
            move_grid_index(self.selected_office, self.known_office_count, columns, code);
        self.selected_office_id = None;
        self.selected_worker_id = None;
        self.selected_worker = 0;
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
        assert_eq!(ui.view(), View::Cameras);
        assert_eq!(
            ui.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(UiCommand::Quit)
        );
    }
    #[test]
    fn saved_preferences_round_trip_without_persisting_automatic_terminal_choices() {
        let mut ui = Ui::new();
        assert_eq!(ui.preferences().encoding, None);
        assert_eq!(ui.preferences().color_depth, None);
        let saved = RendererPreferences {
            projection: "top-down".into(),
            light: true,
            motion: false,
            name_plates: false,
            color_depth: Some("256".into()),
            encoding: Some("half-blocks".into()),
            wardrobe: BTreeMap::new(),
            office_palettes: BTreeMap::new(),
        };
        ui.restore_preferences(&saved);
        assert_eq!(ui.preferences(), saved);
        let mut restored = Ui::new();
        restored.color_locked = true;
        restored.color_depth = ColorDepth::TrueColor;
        restored.restore_preferences(&saved);
        assert_eq!(restored.color_depth, ColorDepth::TrueColor);
        assert_eq!(restored.preferences(), saved);
    }

    #[test]
    fn character_choice_stays_with_the_conversation_and_can_be_reset() {
        let world = demo_world(0);
        let mut ui = Ui::new();
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        ui.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let worker = ui.selected_worker_id.clone().unwrap();
        ui.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert_eq!(ui.preferences().wardrobe.get(&worker.0), Some(&0));
        ui.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        let saved = ui.preferences();
        let mut restored = Ui::new();
        restored.restore_preferences(&saved);
        assert_eq!(restored.preferences().wardrobe.get(&worker.0), Some(&1));
        ui.handle_key(KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT));
        assert!(!ui.preferences().wardrobe.contains_key(&worker.0));
    }

    #[test]
    fn room_palette_changes_only_the_selected_project_and_survives_restore() {
        let world = demo_world(0);
        let mut ui = Ui::new();
        ui.color_depth = ColorDepth::TrueColor;
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        let first = ui.selected_office_id.clone().unwrap();
        let before = ui.pixel_frame();
        ui.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        assert_ne!(before.rgba(), ui.pixel_frame().rgba());
        ui.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        let second = ui.selected_office_id.clone().unwrap();
        assert_ne!(first, second);
        assert!(!ui.preferences().office_palettes.contains_key(&second.0));
        ui.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
        let saved = ui.preferences();
        let mut restored = Ui::new();
        restored.restore_preferences(&saved);
        assert_eq!(
            restored.preferences().office_palettes,
            saved.office_palettes
        );
        ui.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT));
        assert!(ui.preferences().office_palettes.contains_key(&first.0));
        assert!(!ui.preferences().office_palettes.contains_key(&second.0));
    }

    #[test]
    fn source_connection_is_an_explicit_host_command() {
        assert_eq!(
            Ui::new().handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
            Some(UiCommand::Sources)
        );
    }

    #[test]
    fn key_releases_do_not_toggle_overlays_or_send_host_commands() {
        let mut ui = Ui::new();
        let mut key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
        ui.handle_key(key);
        assert!(ui.phone_open());
        key.kind = crossterm::event::KeyEventKind::Release;
        assert_eq!(ui.handle_key(key), None);
        assert!(ui.phone_open());
        key.code = KeyCode::Char('c');
        assert_eq!(ui.handle_key(key), None);
        key.code = KeyCode::Char('q');
        assert_eq!(ui.handle_key(key), None);
    }

    #[test]
    fn settings_motion_toggle_updates_the_saved_preference() {
        let mut ui = Ui::new();
        let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
        ui.handle_key(press(KeyCode::Char('s')));
        assert!(ui.settings_open);
        for _ in 0..3 {
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

    #[test]
    fn phone_scroll_opens_the_selected_thread_and_escape_only_closes_overlay() {
        let world = demo_world(0);
        let mut ui = Ui::new();
        ui.guard_all = true;
        ui.phone_open = true;
        ui.tick(1_000);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        assert_eq!(ui.phone_workers.len(), world.worker_count());
        for _ in 0..world.worker_count() {
            ui.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        let target = ui.phone_workers.last().unwrap().clone().unwrap();
        assert_eq!(ui.phone_selected, world.worker_count() - 1);
        ui.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        assert!(!ui.phone_open);
        assert_eq!(ui.view, View::Desk);
        let offices = views::cameras::ordered_offices(&world, ui.now);
        assert_eq!(
            offices[ui.selected_office].workers[ui.selected_worker].id,
            target
        );
        ui.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        ui.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!ui.phone_open);
        assert_eq!(ui.view, View::Desk);
        ui.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        assert_eq!(
            ui.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            None
        );
        assert!(!ui.phone_open);
    }
}
#[cfg(test)]
mod m3_tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::time::{Duration, Instant};
    use theywork_core::{
        Activity, Agent, Beat, Event, EventKind, OfficeId, WorkerId, BLOCKED_AFTER_MS,
    };

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

    fn capture_audit_frame(terminal: &Terminal<TestBackend>, name: &str) {
        if std::env::var_os("THEYWORK_UPDATE_GOLDEN").is_none() {
            return;
        }
        let buffer = terminal.backend().buffer();
        let width = usize::from(buffer.area.width);
        let text = buffer
            .content
            .chunks(width)
            .map(|row| {
                row.iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/design-audit/evidence");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(format!("{name}.txt")),
            format!("{}\n", text.trim_end()),
        )
        .unwrap();
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
    fn twenty_floors_keep_the_selected_project_and_page_visible() {
        let world = world_with_office_counts(&[1; 20], false);
        for size in [(80, 24), (160, 48), (240, 70)] {
            let mut ui = Ui::new();
            ui.open_tower();
            let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            ui.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            let text = buffer_text(&terminal);
            assert_eq!(ui.selected_office(), 19);
            assert!(
                text.contains("20/20"),
                "last floor must stay visible at {size:?}"
            );
            assert!(
                text.contains("F20"),
                "selected feed must be drawn at {size:?}"
            );
            assert!(text.contains("page "), "tower must explain pagination");
            capture_audit_frame(&terminal, &format!("tower-20-floors-{}x{}", size.0, size.1));
            assert!(ui.camera_page_size <= usize::from(size.0 / 26) * usize::from(size.1 / 10));
            ui.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert!(ui.selected_office() < 19);
            ui.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
            assert_eq!(ui.selected_office(), 0);
        }
    }

    #[test]
    fn attention_shortcut_opens_the_worker_in_a_different_project() {
        let world = world_with_office_counts(&[1, 1], false);
        let mut ui = Ui::new();
        ui.tick(BLOCKED_AFTER_MS + 1);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        ui.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        ui.handle_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE));
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        assert_eq!(ui.view(), View::Desk);
        assert_eq!(ui.selected_office(), 0);
    }

    #[test]
    fn small_settings_scroll_to_every_option_and_help_stays_navigable() {
        let world = world_with_office_counts(&[1], false);
        let mut ui = Ui::new();
        let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let mut terminal = Terminal::new(TestBackend::new(24, 10)).unwrap();
        ui.handle_key(press(KeyCode::Char('s')));
        for _ in 0..6 {
            ui.handle_key(press(KeyCode::Down));
        }
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        assert!(buffer_text(&terminal).contains("room"));
        capture_audit_frame(&terminal, "settings-24x10-last-option");
        ui.handle_key(press(KeyCode::Esc));
        ui.handle_key(press(KeyCode::Char('?')));
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        let start = buffer_text(&terminal);
        capture_audit_frame(&terminal, "help-24x10-first-page");
        for _ in 0..5 {
            ui.handle_key(press(KeyCode::PageDown));
        }
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        assert_ne!(buffer_text(&terminal), start);
        ui.handle_key(press(KeyCode::End));
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        assert!(buffer_text(&terminal).contains("overlay"));
        capture_audit_frame(&terminal, "help-24x10-scrolled");
        assert!(buffer_text(&terminal).contains("scroll"));
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
            buffer_text(&terminal).contains("PHONE"),
            "phone first frame should already show its title"
        );
    }

    #[test]
    fn record_controls_never_reach_terminal_cells() {
        let tainted = "visible\u{1b}[31mred\u{1b}[0m\u{9b}2J\u{1b}]0;owned\u{7}";
        let office = format!("/workspace/{tainted}");
        let worker = WorkerId(format!("{office}#{tainted}"));
        let mut world = World::new();
        world.apply(event(
            &office,
            &worker,
            0,
            Agent::Codex,
            EventKind::Seen {
                name: tainted.to_string(),
                git_branch: Some(tainted.to_string()),
            },
        ));
        world.apply(event(
            &office,
            &worker,
            1,
            Agent::Codex,
            EventKind::Did(Beat {
                at: 1,
                activity: Activity::Talking {
                    detail: tainted.to_string(),
                },
                outcome: None,
            }),
        ));

        for destination in ["office", "cameras", "desk"] {
            let mut ui = Ui::new();
            ui.tick(1);
            match destination {
                "cameras" => {
                    ui.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
                }
                "desk" => {
                    ui.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }
                _ => {}
            }
            let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
            terminal
                .draw(|frame| ui.draw(frame, &world))
                .expect("tainted record should render safely");
            assert!(
                terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .flat_map(|cell| cell.symbol().chars())
                    .all(|character| !character.is_control()),
                "{destination} emitted a terminal control from record text"
            );
        }

        let mut ui = Ui::new();
        ui.tick(1);
        ui.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
        for _ in 0..4 {
            terminal
                .draw(|frame| ui.draw(frame, &world))
                .expect("tainted phone record should render safely");
            assert!(terminal
                .backend()
                .buffer()
                .content
                .iter()
                .flat_map(|cell| cell.symbol().chars())
                .all(|character| !character.is_control()));
            ui.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }
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
            "select",
            "Enter",
            "Backspace",
            "Tab",
            "phone",
            "1–4",
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
    fn ui_pixel_frame_snapshots_the_latest_rendered_floor() {
        let world = world_with_office_counts(&[5], false);
        let mut ui = Ui::new();
        ui.tick(BLOCKED_AFTER_MS + 1);
        let mut terminal = Terminal::new(TestBackend::new(160, 48)).expect("floor terminal");
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("floor frame");

        let frame = ui.pixel_frame();
        assert!(frame.width() > 0 && frame.height() > 0);
        assert_eq!(frame.rgba().len(), frame.width() * frame.height() * 4);
        let area = frame.cell_area().expect("rendered floor cell rectangle");
        assert!(area.width > 0 && area.height > 0);
    }

    #[test]
    fn image_density_floor_has_native_dimensions_palette_and_checksum() {
        use std::collections::HashSet;

        let world = world_with_office_counts(&[5], false);
        let mut ui = Ui::new();
        ui.color_depth = ColorDepth::TrueColor;
        ui.encoding = PixelEncoding::Sextants;
        ui.set_image_cell_size(Some((10, 20)));
        ui.tick(BLOCKED_AFTER_MS + 1);
        let mut terminal = Terminal::new(TestBackend::new(160, 48)).expect("floor terminal");
        terminal
            .draw(|frame| ui.draw(frame, &world))
            .expect("native-density floor frame");

        let frame = ui.pixel_frame();
        let area = frame.cell_area().expect("rendered floor cell rectangle");
        assert_eq!(frame.width(), usize::from(area.width) * 10);
        assert_eq!(frame.height(), usize::from(area.height) * 20);
        let colors = frame
            .rgba()
            .chunks_exact(4)
            .filter(|pixel| pixel[3] != 0)
            .map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect::<HashSet<_>>();
        assert!(
            colors.len() < 256,
            "native-density office uses {} colours",
            colors.len()
        );
        let checksum = frame
            .rgba()
            .iter()
            .fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
        eprintln!(
            "native-density frame={}x{} colors={} checksum={checksum:#018x}",
            frame.width(),
            frame.height(),
            colors.len()
        );
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        let repeated = ui.pixel_frame();
        assert_eq!(
            frame.rgba(),
            repeated.rgba(),
            "identical state must yield deterministic native pixels"
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
        ui.sprites.parse_all_cached_frames();
        let parsed_sprites = ui.sprites.parsed_count();
        assert!(
            parsed_sprites > 0,
            "the floor should parse sprites during warm-up"
        );
        let started = Instant::now();

        // Replay the warmed animation interval: advancing into a frame that was
        // never requested is expected to parse it once and is not a cache miss.
        for frame_index in 0..60 {
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
    fn encoding_frame_cost_stays_within_ten_fps_budget() {
        let world = world_with_office_counts(&[5], false);
        for encoding in PixelEncoding::ALL {
            let mut ui = Ui::new();
            ui.color_depth = ColorDepth::TrueColor;
            ui.encoding = encoding;
            let mut terminal =
                Terminal::new(TestBackend::new(160, 48)).expect("large floor terminal");
            for frame_index in 0..20 {
                ui.tick(BLOCKED_AFTER_MS + frame_index as Millis);
                terminal
                    .draw(|frame| ui.draw(frame, &world))
                    .expect("warm-up floor frame");
            }

            let started = Instant::now();
            for frame_index in 20..120 {
                ui.tick(BLOCKED_AFTER_MS + frame_index as Millis);
                terminal
                    .draw(|frame| ui.draw(frame, &world))
                    .expect("measured floor frame");
            }
            let elapsed = started.elapsed();
            let area = ui
                .canvas
                .pixel_frame()
                .cell_area()
                .expect("rendered floor area");
            let mut buffer = ratatui::buffer::Buffer::empty(area);
            let packing_started = Instant::now();
            for _ in 0..100 {
                ui.canvas.render(&mut buffer, area);
            }
            let packing_elapsed = packing_started.elapsed();
            eprintln!(
                "encoding={} frames=100 total_ms={} per_frame_ms={:.2} cached_packing_ms={:.2}",
                encoding.label(),
                elapsed.as_millis(),
                elapsed.as_secs_f64() * 1_000.0 / 100.0,
                packing_elapsed.as_secs_f64() * 1_000.0 / 100.0
            );
            assert!(
                elapsed < Duration::from_secs(5),
                "{encoding:?} should keep 10-fps floor frames under 5 seconds"
            );
        }
    }

    #[test]
    fn failed_worker_floor_has_explicit_alert() {
        let mut world = world_with_office_counts(&[2], false);
        world.apply(event(
            "/workspace/office-0",
            &WorkerId("/workspace/office-0#worker-0".into()),
            0,
            Agent::Codex,
            EventKind::Acted(Activity::Idle),
        ));
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
