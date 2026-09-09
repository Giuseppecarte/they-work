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
        self.customize.open = false;
        self.more_open = false;
        self.focus = None;
    }
    pub(crate) fn activate(&mut self, action: Action) -> Option<UiCommand> {
        let enter_floor = matches!(&action, Action::EnterFloor(_));
        match action {
            Action::More => {
                self.more_open = !self.more_open;
                self.more_cursor = 0;
                None
            }
            Action::Advanced => {
                self.close_panels();
                self.settings_open = true;
                self.advanced_settings = true;
                self.settings_cursor = 0;
                None
            }
            Action::WorkTab(tab) => {
                self.work_panel.select_tab(tab);
                None
            }
            Action::WorkLatest => {
                self.work_panel.latest();
                None
            }
            Action::WorkRecord(id) => {
                self.work_panel.focus_record(id);
                None
            }
            Action::WorkResults => {
                self.work_panel.results();
                None
            }
            Action::WorkExpand => {
                self.work_panel.expanded = !self.work_panel.expanded;
                None
            }
            Action::WorkActions => {
                self.work_panel.toggle_actions();
                None
            }
            Action::ApplyAppearance => {
                if self.customize.open && self.customize.valid() {
                    self.customize.store(
                        &mut self.character_profiles,
                        &mut self.office_designs,
                        &mut self.wardrobe,
                    );
                    self.customize.open = false;
                }
                None
            }
            Action::CancelAppearance => {
                self.customize.open = false;
                None
            }
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
                self.customize.handle_key(
                    crossterm::event::KeyEvent::new(
                        KeyCode::Enter,
                        crossterm::event::KeyModifiers::NONE,
                    ),
                    &mut self.character_profiles,
                    &mut self.office_designs,
                    &mut self.wardrobe,
                );
                None
            }
            Action::Setting(index) => {
                self.settings_cursor = index.min(if self.advanced_settings { 3 } else { 5 });
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
                self.work_panel.select_worker(&id);
                self.work_panel.select_tab(crate::work_brief::WorkTab::Now);
                self.phone_pending_worker = Some(id);
                self.desk_scroll = 0;
                None
            }
            Action::InspectRecord(id, record) => {
                self.close_panels();
                self.work_panel.select_worker(&id);
                self.work_panel.focus_record(record);
                self.phone_pending_worker = Some(id);
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
                self.work_panel.select_worker(&id);
                self.work_panel.select_tab(crate::work_brief::WorkTab::Team);
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
    pub(crate) fn more_actions(&self) -> Vec<Action> {
        let mut actions = crate::interaction::navigation_actions();
        actions.extend([
            Action::Key(KeyCode::Char('s')),
            Action::Key(KeyCode::Char('?')),
        ]);
        actions
    }
    pub(crate) fn draw_toolbar(&mut self, frame: &mut Frame) {
        let full = frame.area();
        if full.height == 0 {
            return;
        }
        let area = Rect::new(full.x, full.y, full.width, 1);
        views::paint_opaque(frame, area, Style::default().bg(views::PANEL));
        let actions = crate::interaction::navigation_actions();
        let total: usize = actions
            .iter()
            .map(|a| a.label().len() + 5)
            .sum::<usize>()
            .saturating_sub(1);
        let overflow = total > area.width as usize;
        let mut x = area.x;
        for action in actions {
            let width = action.label().len() as u16 + 4;
            let reserve = if overflow { 9 } else { 0 };
            if x + width + reserve > area.right() {
                break;
            }
            if let Some(hit) = crate::components::button(
                frame,
                Rect::new(x, area.y, area.right() - x, 1),
                action.label(),
                action.clone(),
                crate::components::ButtonKind::Secondary,
            ) {
                x = hit.area.right() + 1;
                self.frame_hits.push(hit);
            }
        }
        if overflow {
            if let Some(hit) = crate::components::button(
                frame,
                Rect::new(x, area.y, area.right().saturating_sub(x), 1),
                "More",
                Action::More,
                crate::components::ButtonKind::Primary,
            ) {
                self.frame_hits.push(hit);
            }
        }
    }
    pub(crate) fn draw_context(
        &mut self,
        frame: &mut Frame,
        office: Option<&theywork_core::Office>,
    ) {
        let full = frame.area();
        if full.height < 2 {
            return;
        }
        let area = Rect::new(full.x, full.y + 1, full.width, 1);
        let issue = self.observation.errors.first();
        let text = if let Some(issue) = issue {
            format!(
                "! Source issue · c Connections · {}",
                views::safe_display(issue)
            )
        } else if self.view == View::Cameras {
            format!(
                "Software tower · {} {}",
                self.known_office_count,
                if self.known_office_count == 1 {
                    "project"
                } else {
                    "projects"
                }
            )
        } else {
            format!(
                "{} / {}",
                office.map_or("No project", |o| o.name.as_str()),
                if self.view == View::Desk {
                    "Work brief"
                } else if matches!(
                    self.projection,
                    views::office::Projection::Auto | views::office::Projection::Side
                ) {
                    "Office"
                } else {
                    "Compatibility view · limited controls"
                }
            )
        };
        let style = Style::default()
            .fg(if issue.is_some() {
                views::WARNING
            } else {
                views::MUTED
            })
            .bg(views::PANEL);
        views::paint_opaque(frame, area, style);
        Paragraph::new(views::safe_display(&text))
            .style(style)
            .render(area, frame.buffer_mut());
    }
    pub(crate) fn draw_more(&mut self, frame: &mut Frame) {
        let full = frame.area();
        if full.height < 4 {
            return;
        }
        let actions = self.more_actions();
        let area = Rect::new(
            full.x,
            full.y + 2,
            full.width.min(48),
            full.height.saturating_sub(3).min(actions.len() as u16 + 1),
        );
        self.frame_hits
            .retain(|hit| hit.area.y >= full.bottom() - 1);
        views::paint_opaque(
            frame,
            area,
            Style::default().fg(views::INK).bg(views::PANEL),
        );
        crate::components::heading(
            frame,
            Rect::new(area.x + 1, area.y, area.width.saturating_sub(2), 1),
            "Navigation · Esc back",
        );
        let visible = area.height.saturating_sub(1).max(1) as usize;
        let first = self.more_cursor.saturating_sub(visible - 1);
        for (i, action) in actions.into_iter().enumerate().skip(first).take(visible) {
            let row = Rect::new(
                area.x + 1,
                area.y + 1 + (i - first) as u16,
                area.width.saturating_sub(2),
                1,
            );
            let style = Style::default()
                .fg(if i == self.more_cursor {
                    views::ACCENT
                } else {
                    views::INK
                })
                .bg(if i == self.more_cursor {
                    views::PANEL_HIGHLIGHT
                } else {
                    views::PANEL
                });
            let text = format!(
                "{} {:<14} {}",
                if i == self.more_cursor { ">" } else { " " },
                action.label(),
                action.shortcut()
            );
            Paragraph::new(text)
                .style(style)
                .render(row, frame.buffer_mut());
            self.frame_hits.push(HitRegion::new(row, action));
        }
    }
    pub(crate) fn draw_footer(&mut self, frame: &mut Frame) {
        if self.controls.open {
            return;
        }
        let full = frame.area();
        if full.height < 3 {
            return;
        }
        let area = Rect::new(full.x, full.bottom() - 1, full.width, 1);
        views::paint_opaque(frame, area, Style::default().bg(views::PANEL));
        let modal = self.customize.open
            || self.settings_open
            || self.help_open
            || self.finder.open
            || self.workboard.open
            || self.phone_open
            || self.more_open;
        let mut actions = vec![if self.more_open {
            Action::More
        } else if (self.settings_open && self.advanced_settings)
            || (self.workboard.open && self.workboard.showing_coverage())
        {
            Action::Key(KeyCode::Esc)
        } else {
            Action::Close
        }];
        if !modal {
            actions.push(Action::Key(KeyCode::Char('s')));
            if let Some(id) = self.selected_office_id.clone() {
                actions.push(Action::Decorate(id));
            }
        }
        let mut x = area.x;
        for action in actions {
            let label = if matches!(action, Action::Key(KeyCode::Esc) | Action::More) {
                "Back"
            } else {
                action.label()
            };
            if let Some(hit) = crate::components::button(
                frame,
                Rect::new(x, area.y, area.right().saturating_sub(x), 1),
                label,
                action,
                crate::components::ButtonKind::Quiet,
            ) {
                x = hit.area.right() + 1;
                self.frame_hits.push(hit);
            }
        }
        let hint = if self.more_open {
            "↑↓ choose · Enter opens".into()
        } else if self.customize.open {
            "Tab · Apply / Cancel".into()
        } else if self.settings_open {
            "↑↓ select · ←→ edit".into()
        } else if self.help_open {
            "↑↓ scroll · Esc back".into()
        } else if self.finder.open {
            if area.width < 60 {
                "Type · ↑↓ · Enter opens"
            } else {
                "Type to search · ↑↓ results · Enter open"
            }
            .into()
        } else if self.workboard.open && self.workboard.showing_coverage() {
            "↑↓ / PgUp/PgDn read · h back".into()
        } else if self.workboard.open || self.phone_open {
            "↑↓ · Enter opens".into()
        } else if self.view == View::Desk {
            if area.width < 60 {
                "↑↓ read · e expand"
            } else {
                "Tab controls · ↑↓ read · e expand"
            }
            .into()
        } else if !self.navigation_hint.is_empty() {
            self.navigation_hint.clone()
        } else {
            "Tab controls · Enter open · ? help".into()
        };
        Paragraph::new(hint)
            .style(Style::default().fg(views::MUTED))
            .render(
                Rect::new(x, area.y, area.right().saturating_sub(x), 1),
                frame.buffer_mut(),
            );
    }
    pub(crate) fn draw_focus(&self, frame: &mut Frame) {
        if let Some(hit) = self.focus.and_then(|index| self.focus_targets.get(index)) {
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
