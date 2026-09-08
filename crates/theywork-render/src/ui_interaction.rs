use super::*;
use crate::interaction::{Action, HitRegion};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Paragraph, Widget},
};

impl Ui {
    pub fn mouse_enabled(&self) -> bool {
        self.mouse_enabled
    }
    pub fn set_mouse_enabled(&mut self, enabled: bool) {
        self.mouse_enabled = enabled;
        self.invalidate_pointer();
    }
    pub fn invalidate_pointer(&mut self) {
        self.presented_hits.clear();
        self.presented_size = None;
    }
    /// Called only after the host successfully writes the current scene.
    pub fn frame_presented(&mut self) {
        self.presented_hits = self.frame_hits.clone();
        self.presented_size = Some(self.drawn_size);
    }
    pub fn actions_changed_since_presented(&self) -> bool {
        !self
            .frame_hits
            .iter()
            .map(|hit| &hit.action)
            .eq(self.presented_hits.iter().map(|hit| &hit.action))
    }
    pub fn hit_regions(&self) -> &[HitRegion] {
        &self.frame_hits
    }
    pub fn handle_mouse(&mut self, event: MouseEvent) -> Option<UiCommand> {
        if !self.mouse_enabled || self.presented_size != Some(self.drawn_size) {
            return None;
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let action = self
                    .presented_hits
                    .iter()
                    .rev()
                    .find(|hit| hit.contains(event.column, event.row))
                    .map(|hit| hit.action.clone());
                self.focus = None;
                action.and_then(|action| self.activate(action))
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let down = event.kind == MouseEventKind::ScrollDown;
                self.handle_key(KeyEvent::new(
                    if down {
                        KeyCode::PageDown
                    } else {
                        KeyCode::PageUp
                    },
                    KeyModifiers::NONE,
                ))
            }
            _ => None,
        }
    }
    pub(crate) fn control_command(command: views::control::Command) -> UiCommand {
        if matches!(command, views::control::Command::Sources) {
            UiCommand::Sources
        } else {
            UiCommand::Control(command)
        }
    }
    fn close_panels(&mut self) {
        if self.controls.open {
            self.controls
                .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        }
        self.workboard.open = false;
        self.finder.open = false;
        self.phone_open = false;
        self.settings_open = false;
        self.help_open = false;
        if self.customize.open {
            self.customize.store(
                &mut self.character_profiles,
                &mut self.office_designs,
                &mut self.wardrobe,
            );
            self.customize.open = false;
        }
        self.focus = None;
    }
    pub(crate) fn activate(&mut self, action: Action) -> Option<UiCommand> {
        let enter_floor = matches!(&action, Action::EnterFloor(_));
        match action {
            Action::Control(command) => self.controls.activate(command).map(Self::control_command),
            Action::Sources => {
                if self.controls.busy() {
                    return None;
                }
                self.close_panels();
                Some(UiCommand::Sources)
            }
            Action::Key(code) => self.handle_key(KeyEvent::new(code, KeyModifiers::NONE)),
            Action::ControlProject(path) => {
                self.controls.select_project(path);
                None
            }
            Action::NotebookSelect(key) => {
                self.workboard.select_key(&key);
                None
            }
            Action::Mark(key) => {
                // The action retains the record identity, including across new entries.
                if !self.workboard.rows.iter().any(|row| row.key == key) {
                    return None;
                }
                let markers = if self.workboard.channel == views::workboard::Channel::Deliveries {
                    &mut self.review_memory.reviewed
                } else {
                    &mut self.review_memory.acknowledged
                };
                if markers.remove(&key).is_none() {
                    markers.insert(key, self.now);
                }
                self.review_memory.prune();
                None
            }
            Action::CustomizeField(field) => {
                self.customize.field = field;
                None
            }
            Action::Setting(index) => {
                self.settings_cursor = index.min(7);
                self.handle_settings_key(KeyCode::Right)
            }
            Action::ToggleMouse => {
                self.set_mouse_enabled(!self.mouse_enabled);
                None
            }
            Action::Close => {
                let modal = self.controls.open
                    || self.customize.open
                    || self.workboard.open
                    || self.finder.open
                    || self.settings_open
                    || self.help_open
                    || self.phone_open;
                self.close_panels();
                if !modal {
                    self.view = match self.view {
                        View::Desk => View::Office,
                        _ => View::Cameras,
                    };
                    self.guard_all = self.view == View::Cameras;
                }
                None
            }
            Action::Tower => {
                self.close_panels();
                self.open_tower();
                None
            }
            Action::Attention | Action::Deliveries => {
                self.close_panels();
                self.workboard.scope_all = true;
                self.workboard.show(if action == Action::Attention {
                    views::workboard::Channel::Attention
                } else {
                    views::workboard::Channel::Deliveries
                });
                None
            }
            Action::Find => {
                self.close_panels();
                self.finder.show();
                None
            }
            Action::NewTask => {
                self.close_panels();
                self.pending_control = Some(true);
                None
            }
            Action::Connections => {
                self.close_panels();
                self.controls.show_connections();
                None
            }
            Action::SelectFloor(id) | Action::EnterFloor(id) => {
                let enter = enter_floor;
                self.close_panels();
                self.selected_office_id = Some(id);
                self.selected_worker_id = None;
                self.selected_worker = 0;
                self.selected_team = None;
                self.view = if enter { View::Office } else { View::Cameras };
                self.guard_all = !enter;
                None
            }
            Action::Inspect(id) => {
                self.close_panels();
                self.phone_pending_worker = Some(id);
                self.desk_scroll = 0;
                None
            }
            Action::Controls(id) => {
                self.close_panels();
                self.phone_pending_worker = Some(id);
                self.pending_control = Some(false);
                None
            }
            Action::Review(id) => {
                self.close_panels();
                if self.controls.status.requests.iter().any(|r| r.worker == id) {
                    self.controls.show_requests(Some(id));
                    None
                } else if self
                    .controls
                    .status
                    .tasks
                    .get(&id.0)
                    .is_some_and(|access| access.native)
                {
                    self.controls
                        .activate(views::control::Command::OpenNative { worker: id })
                        .map(Self::control_command)
                } else {
                    self.phone_pending_worker = Some(id);
                    self.desk_scroll = 0;
                    None
                }
            }
            Action::Team(id) => {
                self.close_panels();
                self.phone_pending_worker = Some(id.clone());
                self.workboard.scope_all = false;
                self.workboard.show(views::workboard::Channel::Team);
                self.workboard.focus_worker(id);
                None
            }
            Action::SelectTeam(id) => {
                self.selected_team = Some(id.clone());
                self.selected_worker_id = Some(id);
                self.selected_worker = 0;
                None
            }
            Action::SelectDesks(office, worker) => {
                self.selected_office_id = Some(office);
                self.selected_team = None;
                self.selected_worker_id = Some(worker);
                None
            }
            Action::Character(id) => {
                self.close_panels();
                self.customize
                    .show_character(id, &self.character_profiles, &self.wardrobe);
                None
            }
            Action::Decorate(id) => {
                self.close_panels();
                self.customize.show_office(id, &self.office_designs);
                None
            }
        }
    }
    pub(crate) fn draw_toolbar(&mut self, frame: &mut Frame) {
        let area = frame.area();
        if area.height < 3 {
            return;
        }
        let actions = [
            ("Tower", Action::Tower),
            ("Attention", Action::Attention),
            ("Deliveries", Action::Deliveries),
            ("Find", Action::Find),
            ("New task", Action::NewTask),
            ("Connections", Action::Connections),
        ];
        let compact = area.width < 64;
        let short = ["Tower", "Help!", "Results", "Find", "New", "Connect"];
        let mut x = area.x;
        for (index, (name, action)) in actions.into_iter().enumerate() {
            let label = format!("[{}]", if compact { short[index] } else { name });
            let width = (label.len() as u16).min(area.right().saturating_sub(x));
            if width < label.len() as u16 {
                break;
            }
            let rect = Rect::new(x, area.y + 1, width, 1);
            views::paint_opaque(frame, rect, Style::default().bg(views::PANEL));
            Paragraph::new(label)
                .style(Style::default().fg(views::ACCENT).bg(views::PANEL))
                .render(rect, frame.buffer_mut());
            self.frame_hits.push(HitRegion::new(rect, action));
            x += width + 1;
        }
    }
    pub(crate) fn draw_footer(&mut self, frame: &mut Frame) {
        if self.controls.open {
            return;
        }
        let area = frame.area();
        if area.height < 5 {
            return;
        }
        let row = area.bottom() - 2;
        let footer = Rect::new(area.x, row, area.width, 2);
        views::paint_opaque(frame, footer, Style::default().bg(views::PANEL));
        let mut actions = vec![
            ("Back", Action::Close),
            ("Settings", Action::Key(KeyCode::Char('s'))),
        ];
        if !self.controls.open && !self.customize.open && !self.workboard.open && !self.finder.open
        {
            if let Some(office) = self.selected_office_id.clone() {
                actions.push(("Design", Action::Decorate(office)));
            }
            if self.view == View::Desk {
                if let Some(worker) = self.selected_worker_id.clone() {
                    actions.push(("Character", Action::Character(worker)));
                }
            }
        }
        if self.controls.open
            || self.customize.open
            || self.workboard.open
            || self.finder.open
            || self.settings_open
            || self.help_open
        {
            actions.truncate(1);
        }
        let mut x = area.x;
        for (label, action) in actions {
            let text = format!("[{}]", label);
            let width = text.len() as u16;
            if x + width > area.right() {
                break;
            }
            let rect = Rect::new(x, row, width, 1);
            Paragraph::new(text)
                .style(Style::default().fg(views::ACCENT).bg(views::PANEL))
                .render(rect, frame.buffer_mut());
            self.frame_hits.push(HitRegion::new(rect, action));
            x += width + 1;
        }
        let hint = if self.controls.open {
            "Tab fields · Enter action · Esc back"
        } else if self.customize.open {
            "Tab fields · arrows edit · Esc save & return"
        } else if self.view == View::Cameras {
            "Tab controls · arrows select · Enter office · PgUp/PgDn floors · q quit"
        } else {
            "Tab controls · arrows select · Enter inspect · Esc back · q quit"
        };
        let pagination = if self.view == View::Office
            && self.known_worker_count > self.office_page_size
        {
            format!(
                "page {}/{} · ",
                self.selected_worker / self.office_page_size.max(1) + 1,
                self.known_worker_count
                    .div_ceil(self.office_page_size.max(1))
            )
        } else if self.view == View::Cameras && self.known_office_count > self.camera_page_size {
            format!(
                "page {}/{} · ",
                self.selected_office / self.camera_page_size.max(1) + 1,
                self.known_office_count
                    .div_ceil(self.camera_page_size.max(1))
            )
        } else {
            String::new()
        };
        Paragraph::new(format!("{pagination}{hint}"))
            .style(Style::default().fg(views::MUTED).bg(views::PANEL))
            .render(
                Rect::new(area.x, row + 1, area.width, 1),
                frame.buffer_mut(),
            );
    }
    pub(crate) fn draw_focus(&self, frame: &mut Frame) {
        if let Some(hit) = self.focus.and_then(|index| self.frame_hits.get(index)) {
            if hit.area.width == 0 || hit.area.height == 0 {
                return;
            }
            let rect = Rect::new(hit.area.x, hit.area.y, 1, 1);
            views::paint_opaque(frame, rect, Style::default().bg(views::PANEL_HIGHLIGHT));
            Paragraph::new(">")
                .style(
                    Style::default()
                        .fg(views::ACCENT)
                        .add_modifier(Modifier::BOLD),
                )
                .render(rect, frame.buffer_mut());
        }
    }
}
