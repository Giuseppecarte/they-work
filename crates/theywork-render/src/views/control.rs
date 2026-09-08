//! Pure presentation for explicit commands. The host executes every action.

use std::collections::{BTreeMap, BTreeSet};

use crate::interaction::{Action, HitRegion};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::Frame;
use theywork_core::{Agent, WorkerId};

use super::{paint_opaque, safe_display, ACCENT, BACKGROUND, INK, MUTED, PANEL, WARNING};

#[derive(Clone, Debug, Default)]
pub struct TaskAccess {
    pub send: bool,
    pub interrupt: bool,
    pub native: bool,
    pub reconnect: bool,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct Choice {
    pub label: String,
    pub response: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct Request {
    pub id: String,
    pub worker: WorkerId,
    pub origin: String,
    pub title: String,
    pub detail: String,
    pub choices: Vec<Choice>,
    pub questions: Vec<Question>,
}

#[derive(Clone, Debug)]
pub struct Question {
    pub id: String,
    pub prompt: String,
    pub options: Vec<String>,
    pub secret: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ControlStatus {
    pub can_start_codex: bool,
    pub can_start_claude: bool,
    pub tasks: BTreeMap<String, TaskAccess>,
    pub requests: Vec<Request>,
    pub codex: String,
    pub claude: String,
    pub notice: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Sources,
    Start {
        provider: Agent,
        project: String,
        prompt: String,
    },
    Send {
        worker: WorkerId,
        prompt: String,
    },
    OpenNative {
        worker: WorkerId,
    },
    Interrupt {
        worker: WorkerId,
    },
    Reconnect {
        worker: WorkerId,
    },
    Reply {
        request: String,
        response: serde_json::Value,
    },
    Login {
        provider: Agent,
    },
}

#[derive(Default)]
pub struct Input {
    pub text: String,
    cursor: usize,
}

impl Input {
    pub fn set(&mut self, text: String) {
        self.cursor = text.len();
        self.text = text;
    }
    pub fn paste(&mut self, text: &str) {
        let clean: String = text
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .take(16000usize.saturating_sub(self.text.chars().count()))
            .collect();
        self.text.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
    }
    pub fn key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.paste(&c.to_string())
            }
            KeyCode::Enter => self.paste("\n"),
            KeyCode::Left => {
                self.cursor = self.text[..self.cursor]
                    .char_indices()
                    .last()
                    .map_or(0, |(n, _)| n)
            }
            KeyCode::Right => {
                self.cursor += self.text[self.cursor..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8)
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            KeyCode::Backspace if self.cursor > 0 => {
                let before = self.text[..self.cursor]
                    .char_indices()
                    .last()
                    .map_or(0, |(n, _)| n);
                self.text.replace_range(before..self.cursor, "");
                self.cursor = before;
            }
            KeyCode::Delete if self.cursor < self.text.len() => {
                let end = self.cursor
                    + self.text[self.cursor..]
                        .chars()
                        .next()
                        .map_or(0, char::len_utf8);
                self.text.replace_range(self.cursor..end, "");
            }
            _ => {}
        }
    }
    fn display(&self) -> String {
        format!(
            "{}▏{}",
            super::safe_multiline(&self.text[..self.cursor]),
            super::safe_multiline(&self.text[self.cursor..])
        )
    }
}

pub struct ControlPanel {
    pub open: bool,
    pub status: ControlStatus,
    pub target: Option<WorkerId>,
    pub target_label: String,
    pub provider: Agent,
    project: Input,
    message: Input,
    drafts: BTreeMap<String, String>,
    pending_draft: Option<(String, String)>,
    field: usize,
    request_index: usize,
    expired_selection: bool,
    question_index: usize,
    answers: BTreeMap<String, Input>,
    requests_open: bool,
    connections_open: bool,
    sending: bool,
    usable_geometry: bool,
    scroll: u16,
    hits: Vec<HitRegion>,
    buttons: Vec<(String, Action)>,
    button_focus: Option<usize>,
    projects: Vec<(String, String)>,
    projects_open: bool,
    project_index: usize,
    presented: bool,
}

impl Default for ControlPanel {
    fn default() -> Self {
        Self {
            open: false,
            status: ControlStatus::default(),
            target: None,
            target_label: String::new(),
            provider: Agent::Codex,
            project: Input::default(),
            message: Input::default(),
            drafts: BTreeMap::new(),
            pending_draft: None,
            field: 2,
            request_index: 0,
            expired_selection: false,
            question_index: 0,
            answers: BTreeMap::new(),
            requests_open: false,
            connections_open: false,
            sending: false,
            usable_geometry: false,
            scroll: 0,
            hits: Vec::new(),
            buttons: Vec::new(),
            button_focus: None,
            projects: Vec::new(),
            projects_open: false,
            project_index: 0,
            presented: false,
        }
    }
}

impl ControlPanel {
    pub fn hit_regions(&self) -> Vec<HitRegion> {
        self.hits.clone()
    }
    pub fn set_projects(&mut self, mut projects: Vec<(String, String)>) {
        projects.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut paths = BTreeSet::new();
        projects.retain(|(_, path)| paths.insert(path.clone()));
        self.projects = projects;
        self.project_index = self
            .project_index
            .min(self.projects.len().saturating_sub(1));
    }
    pub fn select_project(&mut self, path: String) {
        if self.target.is_none()
            && self
                .projects
                .iter()
                .any(|(_, candidate)| candidate == &path)
        {
            self.remember_draft();
            self.project.set(path);
            self.restore_draft();
            self.projects_open = false;
            self.field = 2;
            self.button_focus = None;
        }
    }
    pub fn show_requests(&mut self, worker: Option<WorkerId>) {
        self.remember_draft();
        self.target = worker;
        self.open = true;
        self.requests_open = true;
        self.connections_open = false;
        self.projects_open = false;
        self.request_index = 0;
        self.expired_selection = false;
        self.scroll = 0;
        self.question_index = 0;
        self.button_focus = None;
        self.buttons.clear();
        self.hits.clear();
    }
    /// Revalidate a command captured from a presented frame against current
    /// capabilities and request identity before forwarding it to the host.
    pub fn activate(&mut self, command: Command) -> Option<Command> {
        if self.sending {
            return None;
        }
        match &command {
            Command::Reply { request, response } => {
                if !self.usable_geometry || !self.requests_open || self.expired_selection {
                    return None;
                }
                let current = self
                    .status
                    .requests
                    .iter()
                    .filter(|request| self.target.as_ref().is_none_or(|id| id == &request.worker))
                    .nth(self.request_index)?;
                if &current.id != request {
                    return None;
                }
                let answers = self.question_response(current);
                if !current
                    .choices
                    .iter()
                    .any(|choice| &choice.response == response)
                    && answers.as_ref() != Some(response)
                {
                    return None;
                }
                self.sending = true;
            }
            Command::Interrupt { worker }
            | Command::OpenNative { worker }
            | Command::Reconnect { worker } => {
                let access = self.status.tasks.get(&worker.0)?;
                let allowed = match &command {
                    Command::Interrupt { .. } => access.interrupt,
                    Command::OpenNative { .. } => access.native,
                    _ => access.reconnect,
                };
                if !allowed {
                    return None;
                }
                self.sending = true;
            }
            Command::Send { worker, prompt } => {
                if self.target.as_ref() != Some(worker)
                    || &self.message.text != prompt
                    || !self.usable_geometry
                {
                    return None;
                }
                return self.handle_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE));
            }
            Command::Start {
                provider,
                project,
                prompt,
            } => {
                if self.target.is_some()
                    || self.provider != *provider
                    || self.project.text.trim() != project
                    || &self.message.text != prompt
                    || !self.usable_geometry
                {
                    return None;
                }
                return self.handle_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE));
            }
            Command::Login { .. } => {
                if !self.connections_open || !self.usable_geometry {
                    return None;
                }
                self.sending = true;
            }
            Command::Sources => {
                if !self.connections_open || !self.usable_geometry {
                    return None;
                }
                self.open = false;
            }
        }
        Some(command)
    }
    fn question_response(&self, request: &Request) -> Option<serde_json::Value> {
        if request.questions.is_empty() {
            return None;
        }
        let mut answers = serde_json::Map::new();
        for question in &request.questions {
            let answer = self
                .answers
                .get(&format!("{}:{}", request.id, question.id))?
                .text
                .trim();
            if answer.is_empty() {
                return None;
            }
            answers.insert(question.id.clone(), serde_json::json!({"answers":[answer]}));
        }
        Some(serde_json::json!({"answers":answers}))
    }
    fn activate_button(&mut self, action: Action) -> Option<Command> {
        match action {
            Action::Control(command) => self.activate(command),
            Action::Key(key) => self.handle_key(KeyEvent::new(key, KeyModifiers::NONE)),
            Action::Close => {
                self.open = false;
                None
            }
            _ => None,
        }
    }
    fn build_buttons(&mut self) {
        self.buttons.clear();
        if self.sending {
            return;
        }
        if self.connections_open {
            self.buttons.extend([
                ("Local sources".into(), Action::Control(Command::Sources)),
                (
                    "Codex login".into(),
                    Action::Control(Command::Login {
                        provider: Agent::Codex,
                    }),
                ),
                (
                    "Claude login".into(),
                    Action::Control(Command::Login {
                        provider: Agent::Claude,
                    }),
                ),
            ]);
        } else if self.requests_open && !self.expired_selection {
            if let Some(request) = self
                .status
                .requests
                .iter()
                .filter(|r| self.target.as_ref().is_none_or(|id| id == &r.worker))
                .nth(self.request_index)
            {
                self.buttons
                    .extend(request.choices.iter().enumerate().map(|(index, choice)| {
                        (
                            format!("{} {}", index + 1, choice.label),
                            Action::Control(Command::Reply {
                                request: request.id.clone(),
                                response: choice.response.clone(),
                            }),
                        )
                    }));
                if let Some(response) = self.question_response(request) {
                    self.buttons.push((
                        "Submit answers".into(),
                        Action::Control(Command::Reply {
                            request: request.id.clone(),
                            response,
                        }),
                    ));
                }
            }
        } else if let Some(worker) = &self.target {
            if let Some(access) = self.status.tasks.get(&worker.0) {
                if access.send && !self.message.text.trim().is_empty() {
                    self.buttons.push((
                        "Send instruction".into(),
                        Action::Control(Command::Send {
                            worker: worker.clone(),
                            prompt: self.message.text.clone(),
                        }),
                    ));
                }
                if access.native {
                    self.buttons.push((
                        "Open conversation".into(),
                        Action::Control(Command::OpenNative {
                            worker: worker.clone(),
                        }),
                    ));
                }
                if access.interrupt {
                    self.buttons.push((
                        "Stop task".into(),
                        Action::Control(Command::Interrupt {
                            worker: worker.clone(),
                        }),
                    ));
                }
                if access.reconnect {
                    self.buttons.push((
                        "Reconnect".into(),
                        Action::Control(Command::Reconnect {
                            worker: worker.clone(),
                        }),
                    ));
                }
            }
            if self
                .status
                .requests
                .iter()
                .any(|request| request.worker == *worker)
            {
                self.buttons
                    .push(("Review requests".into(), Action::Key(KeyCode::F(4))));
            }
        } else {
            let available = if self.provider == Agent::Codex {
                self.status.can_start_codex
            } else {
                self.status.can_start_claude
            };
            if available
                && !self.project.text.trim().is_empty()
                && !self.message.text.trim().is_empty()
            {
                self.buttons.push((
                    "Start task".into(),
                    Action::Control(Command::Start {
                        provider: self.provider,
                        project: self.project.text.trim().into(),
                        prompt: self.message.text.clone(),
                    }),
                ));
            }
            if !self.projects.is_empty() {
                self.buttons
                    .push(("Choose project".into(), Action::Key(KeyCode::F(7))));
            }
        }
        self.buttons.push(("Back".into(), Action::Close));
        self.button_focus = self
            .button_focus
            .filter(|index| *index < self.buttons.len());
    }
    fn draw_buttons(&mut self, frame: &mut Frame, area: Rect) {
        let mut x = area.x;
        let mut y = area.y;
        let mut visible = 0;
        for (index, (label, action)) in self.buttons.iter().enumerate() {
            let text = format!("[ {} ]", safe_display(label));
            let width = (ratatui::text::Line::from(text.as_str()).width() as u16).min(area.width);
            if x + width > area.right() {
                x = area.x;
                y += 1;
            }
            if y >= area.bottom() {
                break;
            }
            let button = Rect::new(x, y, width, 1);
            Paragraph::new(text)
                .style(
                    Style::default()
                        .fg(if self.button_focus == Some(index) {
                            BACKGROUND
                        } else {
                            INK
                        })
                        .bg(if self.button_focus == Some(index) {
                            ACCENT
                        } else {
                            super::PANEL_HIGHLIGHT
                        }),
                )
                .render(button, frame.buffer_mut());
            self.hits.push(HitRegion::new(button, action.clone()));
            visible += 1;
            x += width + 1;
        }
        self.buttons.truncate(visible);
        self.button_focus = self.button_focus.filter(|index| *index < visible);
    }
    pub fn busy(&self) -> bool {
        self.sending
    }
    pub fn set_status(&mut self, mut status: ControlStatus) {
        let selected = self
            .status
            .requests
            .iter()
            .filter(|r| self.target.as_ref().is_none_or(|id| *id == r.worker))
            .nth(self.request_index)
            .map(|r| r.id.clone());
        status.notice = self.status.notice.clone();
        self.status = status;
        if let Some(id) = selected {
            if let Some(index) = self
                .status
                .requests
                .iter()
                .filter(|r| {
                    self.target
                        .as_ref()
                        .is_none_or(|target| *target == r.worker)
                })
                .position(|r| r.id == id)
            {
                self.request_index = index;
            } else {
                self.expired_selection = true;
                self.status.notice="The selected request expired. Use ↑/↓ to inspect another request before answering.".into();
            }
        }
        self.answers.retain(|key, _| {
            self.status
                .requests
                .iter()
                .any(|r| key.starts_with(&format!("{}:", r.id)))
        });
    }
    fn draft_key(&self) -> String {
        self.target
            .as_ref()
            .map(|id| id.0.clone())
            .unwrap_or_else(|| {
                format!(
                    "new:{}:{}:{}",
                    self.provider.label(),
                    self.project.text.trim().len(),
                    self.project.text.trim()
                )
            })
    }
    fn remember_draft(&mut self) {
        let key = self.draft_key();
        if self.message.text.is_empty() {
            self.drafts.remove(&key);
        } else {
            self.drafts.insert(key, self.message.text.clone());
        }
    }
    fn restore_draft(&mut self) {
        self.message.set(
            self.drafts
                .get(&self.draft_key())
                .cloned()
                .unwrap_or_default(),
        );
    }

    pub fn show_task(&mut self, id: WorkerId, label: String, provider: Agent) {
        self.remember_draft();
        self.target = Some(id.clone());
        self.target_label = label;
        self.provider = provider;
        self.message
            .set(self.drafts.get(&id.0).cloned().unwrap_or_default());
        self.open = true;
        self.field = 2;
        self.requests_open = false;
        self.expired_selection = false;
        self.connections_open = false;
        self.scroll = 0;
        self.projects_open = false;
        self.button_focus = None;
        self.buttons.clear();
        self.hits.clear();
    }
    pub fn show_new(&mut self, project: String) {
        self.remember_draft();
        self.target = None;
        self.target_label.clear();
        self.project.set(project);
        self.restore_draft();
        self.open = true;
        self.field = if self.project.text.is_empty() { 1 } else { 2 };
        self.requests_open = false;
        self.expired_selection = false;
        self.connections_open = false;
        self.scroll = 0;
        self.projects_open = false;
        self.button_focus = None;
        self.buttons.clear();
        self.hits.clear();
    }
    pub fn show_connections(&mut self) {
        self.remember_draft();
        self.open = true;
        self.connections_open = true;
        self.requests_open = false;
        self.projects_open = false;
        self.button_focus = None;
        self.buttons.clear();
        self.hits.clear();
    }
    pub fn complete(&mut self, notice: String, accepted: bool) {
        self.sending = false;
        self.status.notice = notice;
        if let Some((key, text)) = self.pending_draft.take().filter(|_| accepted) {
            if self.drafts.get(&key).is_some_and(|draft| *draft == text) {
                self.drafts.remove(&key);
            }
            if self.draft_key() == key && self.message.text == text {
                self.message.set(String::new());
                self.remember_draft();
            }
        }
    }
    pub fn paste(&mut self, text: &str) {
        if self.button_focus.is_some() || self.projects_open {
            return;
        }
        if self.requests_open && !self.sending {
            if let Some(request) = self
                .status
                .requests
                .iter()
                .filter(|r| self.target.as_ref().is_none_or(|id| *id == r.worker))
                .nth(self.request_index)
            {
                if let Some(question) = request.questions.get(self.question_index) {
                    self.answers
                        .entry(format!("{}:{}", request.id, question.id))
                        .or_default()
                        .paste(text);
                }
            }
            return;
        }
        if !self.sending && !self.requests_open && !self.connections_open {
            if self.field == 1 {
                self.remember_draft();
                self.project.paste(&text.replace(['\n', '\r', '\t'], " "));
                self.restore_draft();
            } else if self.field == 2 {
                self.message.paste(text);
            }
        }
    }
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Command> {
        if key.code == KeyCode::Esc {
            self.remember_draft();
            self.open = false;
            return None;
        }
        if !self.usable_geometry {
            return None;
        }
        if self.sending {
            return None;
        }
        if self.projects_open {
            match key.code {
                KeyCode::Up => self.project_index = self.project_index.saturating_sub(1),
                KeyCode::Down => {
                    self.project_index =
                        (self.project_index + 1).min(self.projects.len().saturating_sub(1))
                }
                KeyCode::Enter => {
                    if let Some((_, path)) = self.projects.get(self.project_index) {
                        self.select_project(path.clone());
                    }
                }
                KeyCode::F(7) => self.projects_open = false,
                _ => {}
            }
            return None;
        }
        if key.code == KeyCode::F(7)
            && self.target.is_none()
            && !self.connections_open
            && !self.requests_open
        {
            self.projects_open = true;
            self.button_focus = None;
            return None;
        }
        if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
            if let Some((_, action)) = self
                .button_focus
                .and_then(|index| self.buttons.get(index))
                .cloned()
            {
                return self.activate_button(action);
            }
        }
        if self.button_focus.is_some()
            && matches!(
                key.code,
                KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
            )
        {
            return None;
        }
        if key.code == KeyCode::BackTab
            && self.target.is_none()
            && !self.requests_open
            && !self.connections_open
            && self.button_focus.is_none()
        {
            self.field = (self.field + 2) % 3;
            return None;
        }
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
            && !self.buttons.is_empty()
            && (self.button_focus.is_some()
                || self.connections_open
                || (self.requests_open
                    && self
                        .status
                        .requests
                        .iter()
                        .filter(|r| self.target.as_ref().is_none_or(|id| id == &r.worker))
                        .nth(self.request_index)
                        .is_some_and(|r| r.questions.is_empty()))
                || (!self.requests_open
                    && !self.connections_open
                    && (self.target.is_some() || self.field == 2)))
        {
            let next = if key.code == KeyCode::BackTab {
                self.button_focus
                    .unwrap_or(0)
                    .checked_sub(1)
                    .unwrap_or(self.buttons.len() - 1)
            } else {
                self.button_focus.map_or(0, |index| index + 1)
            };
            self.button_focus = if next < self.buttons.len() {
                Some(next)
            } else {
                self.field = if self.target.is_none() { 0 } else { 2 };
                self.question_index = 0;
                None
            };
            return None;
        }
        if self.connections_open {
            if key.code == KeyCode::PageDown {
                self.scroll = self.scroll.saturating_add(5);
                return None;
            }
            if key.code == KeyCode::PageUp {
                self.scroll = self.scroll.saturating_sub(5);
                return None;
            }
            let command = match key.code {
                KeyCode::Char('1') => Some(Command::Login {
                    provider: Agent::Codex,
                }),
                KeyCode::Char('2') => Some(Command::Login {
                    provider: Agent::Claude,
                }),
                KeyCode::Char('3') => Some(Command::Sources),
                _ => None,
            };
            return command.and_then(|command| self.activate(command));
        }
        if key.code == KeyCode::F(4) {
            self.requests_open = !self.requests_open;
            self.request_index = 0;
            self.scroll = 0;
            return None;
        }
        if self.requests_open {
            if self.expired_selection {
                if matches!(key.code, KeyCode::Up | KeyCode::Down) {
                    self.expired_selection = false;
                    self.question_index = 0;
                }
                return None;
            }
            let requests: Vec<_> = self
                .status
                .requests
                .iter()
                .filter(|r| self.target.as_ref().is_none_or(|id| *id == r.worker))
                .collect();
            if let Some(request) = requests
                .get(self.request_index)
                .filter(|r| !r.questions.is_empty())
            {
                self.question_index = self.question_index.min(request.questions.len() - 1);
                if self.sending {
                    return None;
                }
                if key.code == KeyCode::Tab {
                    if self.question_index + 1 == request.questions.len()
                        && !self.buttons.is_empty()
                    {
                        self.button_focus = Some(0);
                        return None;
                    }
                    self.question_index = (self.question_index + 1) % request.questions.len();
                    return None;
                }
                if key.code == KeyCode::F(5) {
                    let mut answers = serde_json::Map::new();
                    for question in &request.questions {
                        let answer = self
                            .answers
                            .get(&format!("{}:{}", request.id, question.id))
                            .map(|input| input.text.trim())
                            .unwrap_or("");
                        if answer.is_empty() {
                            self.status.notice = "Answer each question before submitting.".into();
                            return None;
                        }
                        answers
                            .insert(question.id.clone(), serde_json::json!({"answers":[answer]}));
                    }
                    self.sending = true;
                    return Some(Command::Reply {
                        request: request.id.clone(),
                        response: serde_json::json!({"answers":answers}),
                    });
                }
                if !matches!(
                    key.code,
                    KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown
                ) {
                    let question = &request.questions[self.question_index];
                    let input = self
                        .answers
                        .entry(format!("{}:{}", request.id, question.id))
                        .or_default();
                    if let KeyCode::F(n @ 6..=12) = key.code {
                        if let Some(option) = question.options.get((n - 6) as usize) {
                            input.set(option.clone());
                        }
                    } else {
                        input.key(key);
                    }
                    return None;
                }
            }
            match key.code {
                KeyCode::Down => {
                    self.request_index =
                        (self.request_index + 1).min(requests.len().saturating_sub(1))
                }
                KeyCode::Up => self.request_index = self.request_index.saturating_sub(1),
                KeyCode::PageDown => self.scroll = self.scroll.saturating_add(5),
                KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(5),
                KeyCode::Char(n @ '1'..='9') => {
                    if let Some(request) = requests.get(self.request_index) {
                        if let Some(choice) = request.choices.get((n as u8 - b'1') as usize) {
                            if self.presented && !self.hits.iter().any(|hit|matches!(&hit.action,Action::Control(Command::Reply{request:id,response}) if id==&request.id && response==&choice.response)) {return None;}
                            if self.sending {
                                return None;
                            }
                            self.sending = true;
                            return Some(Command::Reply {
                                request: request.id.clone(),
                                response: choice.response.clone(),
                            });
                        }
                    }
                }
                _ => {}
            }
            return None;
        }
        let access = self
            .target
            .as_ref()
            .and_then(|id| self.status.tasks.get(&id.0));
        if key.code == KeyCode::F(6) && access.is_some_and(|a| a.reconnect) {
            self.sending = true;
            return self
                .target
                .clone()
                .map(|worker| Command::Reconnect { worker });
        }
        if key.code == KeyCode::F(2) {
            self.sending = access.is_some_and(|a| a.interrupt);
            return self
                .target
                .clone()
                .filter(|_| access.is_some_and(|a| a.interrupt))
                .map(|worker| Command::Interrupt { worker });
        }
        if key.code == KeyCode::F(3)
            || (key.code == KeyCode::Enter && access.is_some_and(|a| a.native))
        {
            self.sending = access.is_some_and(|a| a.native);
            return self
                .target
                .clone()
                .filter(|_| access.is_some_and(|a| a.native))
                .map(|worker| Command::OpenNative { worker });
        }
        if self.sending {
            return None;
        }
        if key.code == KeyCode::Tab && self.target.is_none() {
            self.field = (self.field + 1) % 3;
            return None;
        }
        if self.field == 0
            && matches!(
                key.code,
                KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
            )
        {
            self.remember_draft();
            self.provider = if self.provider == Agent::Codex {
                Agent::Claude
            } else {
                Agent::Codex
            };
            self.restore_draft();
            return None;
        }
        if key.code == KeyCode::F(5)
            || (key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            if self.message.text.trim().is_empty() {
                self.status.notice = "Write an instruction first.".into();
                return None;
            }
            let action = if let Some(id) = &self.target {
                if !access.is_some_and(|a| a.send) {
                    self.status.notice =
                        "This task is observed. Open its original client to continue.".into();
                    return None;
                }
                Command::Send {
                    worker: id.clone(),
                    prompt: self.message.text.clone(),
                }
            } else {
                if !(if self.provider == Agent::Codex {
                    self.status.can_start_codex
                } else {
                    self.status.can_start_claude
                }) {
                    self.status.notice = if self.provider == Agent::Codex {
                        format!("Codex creation unavailable: {}", self.status.codex)
                    } else {
                        format!("Claude creation unavailable: {}", self.status.claude)
                    };
                    return None;
                }
                if self.project.text.trim().is_empty() {
                    self.status.notice = "Choose a project folder first.".into();
                    return None;
                }
                Command::Start {
                    provider: self.provider,
                    project: self.project.text.trim().into(),
                    prompt: self.message.text.clone(),
                }
            };
            self.sending = true;
            self.status.notice = format!(
                "Sending to {} · waiting for confirmation",
                if self.target.is_some() {
                    self.target_label.clone()
                } else {
                    format!("{} / {}", self.provider.label(), self.project.text)
                }
            );
            self.pending_draft = Some((self.draft_key(), self.message.text.clone()));
            self.remember_draft();
            return Some(action);
        }
        if self.field == 1 && self.target.is_none() {
            if key.code != KeyCode::Enter {
                self.remember_draft();
                self.project.key(key);
                self.restore_draft();
            }
        } else if self.field == 2 {
            self.message.key(key);
        }
        None
    }
    pub fn draw(&mut self, frame: &mut Frame) {
        let area = super::below_tab_bar(frame.area());
        self.draw_in(frame, area);
    }
    pub fn draw_in(&mut self, frame: &mut Frame, area: Rect) {
        self.presented = true;
        self.hits.clear();
        self.build_buttons();
        let minimum_height = if self.requests_open || self.connections_open || self.projects_open {
            12
        } else {
            16
        };
        self.usable_geometry = area.width >= 28 && area.height >= minimum_height;
        paint_opaque(frame, area, Style::default().fg(INK).bg(BACKGROUND));
        if !self.usable_geometry {
            Paragraph::new("Task controls · enlarge terminal\nEsc return")
                .render(area, frame.buffer_mut());
            return;
        }
        let inner = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
        if self.projects_open {
            Paragraph::new("CHOOSE A PROJECT")
                .style(Style::default().fg(ACCENT))
                .render(
                    Rect::new(inner.x, inner.y, inner.width, 1),
                    frame.buffer_mut(),
                );
            if let Some((_, path)) = self.projects.get(self.project_index) {
                let path = super::finder::tail(&safe_display(path), inner.width as usize);
                Paragraph::new(path)
                    .style(Style::default().fg(MUTED))
                    .render(
                        Rect::new(inner.x, inner.y + 1, inner.width, 1),
                        frame.buffer_mut(),
                    );
            }
            let capacity = usize::from(inner.height.saturating_sub(4)).max(1);
            let first = self.project_index / capacity * capacity;
            for (index, (name, path)) in self.projects.iter().enumerate().skip(first).take(capacity)
            {
                let row = Rect::new(
                    inner.x,
                    inner.y + 3 + (index - first) as u16,
                    inner.width,
                    1,
                );
                Paragraph::new(format!(
                    "{} {}",
                    if index == self.project_index {
                        ">"
                    } else {
                        " "
                    },
                    safe_display(name)
                ))
                .style(Style::default().fg(INK).bg(if index == self.project_index {
                    super::PANEL_HIGHLIGHT
                } else {
                    BACKGROUND
                }))
                .render(row, frame.buffer_mut());
                self.hits
                    .push(HitRegion::new(row, Action::ControlProject(path.clone())));
            }
            Paragraph::new("↑↓ choose · Enter select · F7 back")
                .style(Style::default().fg(MUTED))
                .render(
                    Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
                    frame.buffer_mut(),
                );
            return;
        }
        if self.connections_open {
            let text=format!("CONNECTIONS\n\nChoose which local conversations appear in your office with Local sources.\n\nCodex: {}\nClaude: {}\n\nLogin opens the provider's official flow. Existing conversations remain tied to their owning runtime.\nClaude instructions and approvals use its official conversation.\n\n{}",safe_display(&self.status.codex),safe_display(&self.status.claude),safe_display(&self.status.notice));
            let body = Rect::new(
                inner.x,
                inner.y,
                inner.width,
                inner.height.saturating_sub(4),
            );
            let lines = super::wrap_text(&text, body.width);
            self.scroll = self.scroll.min(
                lines
                    .len()
                    .saturating_sub(body.height as usize)
                    .min(u16::MAX as usize) as u16,
            );
            Paragraph::new(lines.join("\n"))
                .scroll((self.scroll, 0))
                .render(body, frame.buffer_mut());
            self.draw_buttons(
                frame,
                Rect::new(inner.x, inner.bottom() - 4, inner.width, 3),
            );
            Paragraph::new("Tab focus · Enter choose · Esc back")
                .style(Style::default().fg(MUTED))
                .render(
                    Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
                    frame.buffer_mut(),
                );
            return;
        }
        let heading = if self.requests_open {
            "REVIEW REQUEST"
        } else if self.target.is_none() {
            "NEW TASK"
        } else {
            "TASK CONTROLS"
        };
        Paragraph::new(heading)
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .render(
                Rect::new(inner.x, inner.y, inner.width, 1),
                frame.buffer_mut(),
            );
        if self.requests_open {
            let requests: Vec<_> = self
                .status
                .requests
                .iter()
                .filter(|r| self.target.as_ref().is_none_or(|id| *id == r.worker))
                .collect();
            self.request_index = self.request_index.min(requests.len().saturating_sub(1));
            let text = if self.expired_selection {
                "The selected request is no longer pending.\n↑/↓ explicitly selects another request.".into()
            } else if let Some(request) = requests.get(self.request_index) {
                let questions = request
                    .questions
                    .iter()
                    .enumerate()
                    .map(|(index, question)| {
                        let value = self.answers.get(&format!("{}:{}", request.id, question.id));
                        let answer = if question.secret {
                            value
                                .map(|input| "•".repeat(input.text.chars().count()))
                                .unwrap_or_default()
                        } else {
                            value.map(Input::display).unwrap_or_else(|| "▏".into())
                        };
                        format!(
                            "{} {}\n{}\nAnswer: {}",
                            if index == self.question_index {
                                ">"
                            } else {
                                " "
                            },
                            super::safe_multiline(&question.prompt),
                            question
                                .options
                                .iter()
                                .enumerate()
                                .map(|(i, s)| format!("F{} {}", i + 6, safe_display(s)))
                                .collect::<Vec<_>>()
                                .join(" · "),
                            answer
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                format!(
                    "PENDING REQUEST {}/{} · {}\n{}\n\n{}\n\n{}\n{}",
                    self.request_index + 1,
                    requests.len(),
                    safe_display(&request.origin),
                    safe_display(&request.title),
                    super::safe_multiline(&request.detail),
                    request
                        .choices
                        .iter()
                        .enumerate()
                        .map(|(n, c)| format!("{}  {}", n + 1, c.label))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    questions
                )
            } else {
                "No unresolved requests from this connection.\nA historical request cannot be approved here.".into()
            };
            let body = Rect::new(
                inner.x,
                inner.y + 2,
                inner.width,
                inner.height.saturating_sub(7),
            );
            let lines = super::wrap_text(&text, body.width);
            self.scroll = self.scroll.min(
                lines
                    .len()
                    .saturating_sub(body.height as usize)
                    .min(u16::MAX as usize) as u16,
            );
            let paragraph = Paragraph::new(lines.join("\n"));
            paragraph
                .scroll((self.scroll, 0))
                .render(body, frame.buffer_mut());
        } else {
            let access = self
                .target
                .as_ref()
                .and_then(|id| self.status.tasks.get(&id.0));
            let identity = if self.target.is_some() {
                format!(
                    "{} · {}\n{}",
                    self.provider.label(),
                    safe_display(&self.target_label),
                    access.map_or("Observation only", |a| a.description.as_str())
                )
            } else {
                format!(
                    "{} Provider: {}  ←/→ change\n{} Project: {}",
                    if self.field == 0 { ">" } else { " " },
                    self.provider.label(),
                    if self.field == 1 { ">" } else { " " },
                    self.project.display()
                )
            };
            Paragraph::new(identity)
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(MUTED))
                .render(
                    Rect::new(inner.x, inner.y + 2, inner.width, 3),
                    frame.buffer_mut(),
                );
            let body = Rect::new(
                inner.x,
                inner.y + 6,
                inner.width,
                inner.height.saturating_sub(11),
            );
            paint_opaque(frame, body, Style::default().bg(PANEL));
            let message = if access.is_some_and(|a| a.native) {
                "Press Enter to talk or approve in the official console.\nReturn here when you detach or exit.".into()
            } else {
                self.message.display()
            };
            let paragraph = Paragraph::new(super::wrap_text(&message, body.width).join("\n"))
                .style(Style::default().fg(INK).bg(PANEL));
            let scroll = super::wrap_text(&self.message.text[..self.message.cursor], body.width)
                .len()
                .saturating_sub(body.height as usize)
                .min(u16::MAX as usize) as u16;
            paragraph
                .scroll((scroll, 0))
                .render(body, frame.buffer_mut());
        }
        Paragraph::new(safe_display(&self.status.notice))
            .style(Style::default().fg(WARNING))
            .render(
                Rect::new(inner.x, inner.bottom() - 5, inner.width, 1),
                frame.buffer_mut(),
            );
        self.draw_buttons(
            frame,
            Rect::new(inner.x, inner.bottom() - 4, inner.width, 3),
        );
        let help = if self.requests_open {
            "Tab focus · Enter choose · PgUp/Dn read · Esc back"
        } else {
            "Tab fields/buttons · Enter choose · Esc back"
        };
        Paragraph::new(help)
            .style(Style::default().fg(MUTED))
            .render(
                Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
                frame.buffer_mut(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ready_panel() -> ControlPanel {
        ControlPanel {
            usable_geometry: true,
            ..Default::default()
        }
    }
    #[test]
    fn utf8_editing_and_paste_preserve_boundaries() {
        let mut input = Input::default();
        input.paste("hé🦀");
        input.key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        input.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(input.text, "h🦀");
        input.paste("中");
        assert_eq!(input.text, "h中🦀");
    }
    #[test]
    fn drafts_keep_their_target_and_typing_cannot_trigger_commands() {
        let mut panel = ready_panel();
        panel.show_task(WorkerId("a".into()), "A".into(), Agent::Codex);
        panel.paste("fix a");
        panel.show_task(WorkerId("b".into()), "B".into(), Agent::Codex);
        panel.paste("fix b");
        panel.show_task(WorkerId("a".into()), "A".into(), Agent::Codex);
        assert_eq!(panel.message.text, "fix a");
        assert!(panel
            .handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))
            .is_none());
        assert!(panel.open);
    }

    fn approval(id: &str) -> Request {
        Request {
            id: id.into(),
            worker: WorkerId("a".into()),
            origin: "Project A / task A".into(),
            title: "Approval".into(),
            detail: "A concrete command".into(),
            choices: vec![Choice {
                label: "Allow once".into(),
                response: serde_json::json!({"decision":"accept"}),
            }],
            questions: vec![],
        }
    }

    #[test]
    fn a_replacement_request_requires_deliberate_selection() {
        let mut panel = ready_panel();
        panel.show_task(WorkerId("a".into()), "A".into(), Agent::Codex);
        panel.set_status(ControlStatus {
            requests: vec![approval("old")],
            ..Default::default()
        });
        panel.handle_key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE));
        panel.set_status(ControlStatus {
            requests: vec![approval("new")],
            ..Default::default()
        });
        assert!(panel
            .handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE))
            .is_none());
        panel.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(
            panel.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE)),
            Some(Command::Reply {
                request: "new".into(),
                response: serde_json::json!({"decision":"accept"})
            })
        );
        assert!(
            panel
                .handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE))
                .is_none(),
            "an unacknowledged reply is not sent twice"
        );
    }

    #[test]
    fn structured_answers_preserve_request_and_question_ids() {
        let mut request = approval("q-1");
        request.choices.clear();
        request.questions = vec![
            Question {
                id: "language".into(),
                prompt: "Language?".into(),
                options: vec!["Rust".into()],
                secret: false,
            },
            Question {
                id: "name".into(),
                prompt: "Name?".into(),
                options: vec![],
                secret: false,
            },
        ];
        let mut panel = ready_panel();
        panel.show_task(WorkerId("a".into()), "A".into(), Agent::Codex);
        panel.set_status(ControlStatus {
            requests: vec![request],
            ..Default::default()
        });
        panel.handle_key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE));
        panel.handle_key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));
        assert!(
            panel
                .handle_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE))
                .is_none(),
            "all questions must be answered"
        );
        panel.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        panel.paste("Café");
        assert_eq!(
            panel.handle_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)),
            Some(Command::Reply {
                request: "q-1".into(),
                response: serde_json::json!({"answers":{"language":{"answers":["Rust"]},"name":{"answers":["Café"]}}})
            })
        );
    }

    #[test]
    fn acknowledgement_cannot_clear_another_tasks_draft() {
        let mut panel = ready_panel();
        panel.show_task(WorkerId("b".into()), "B".into(), Agent::Codex);
        panel.paste("Keep B");
        panel.show_task(WorkerId("a".into()), "A".into(), Agent::Codex);
        panel.paste("Send A");
        panel.status.tasks.insert(
            "a".into(),
            TaskAccess {
                send: true,
                ..Default::default()
            },
        );
        assert!(
            matches!(panel.handle_key(KeyEvent::new(KeyCode::F(5),KeyModifiers::NONE)),Some(Command::Send{worker,..}) if worker.0=="a")
        );
        panel.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        panel.show_task(WorkerId("b".into()), "B".into(), Agent::Codex);
        panel.complete("Confirmed".into(), true);
        assert_eq!(panel.message.text, "Keep B");
        panel.show_task(WorkerId("a".into()), "A".into(), Agent::Codex);
        assert!(panel.message.text.is_empty());
    }

    #[test]
    fn new_task_drafts_stay_with_their_provider_and_project() {
        let mut panel = ready_panel();
        panel.show_new("/project-a".into());
        panel.paste("Codex A");
        panel.show_new("/project-b".into());
        assert!(panel.message.text.is_empty());
        panel.paste("Codex B");
        // Tab from instruction returns to provider, where Right selects Claude.
        panel.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        panel.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(panel.provider, Agent::Claude);
        assert!(panel.message.text.is_empty());
        panel.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        panel.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        panel.paste("Claude B");
        panel.show_new("/project-a".into());
        assert!(panel.message.text.is_empty());
        panel.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        panel.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(panel.message.text, "Codex A");
        panel.show_new("/project-b".into());
        assert_eq!(panel.message.text, "Codex B");
    }

    fn draw_panel(panel: &mut ControlPanel, width: u16, height: u16) -> String {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| panel.draw_in(frame, Rect::new(0, 0, width, height)))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn request_buttons_capture_exact_identity_and_reject_replacement_or_changed_choices() {
        let mut panel = ready_panel();
        panel.show_requests(Some(WorkerId("a".into())));
        panel.set_status(ControlStatus {
            requests: vec![approval("old")],
            ..Default::default()
        });
        let text = draw_panel(&mut panel, 40, 24);
        assert!(text.contains("REVIEW REQUEST"));
        assert!(!text.contains("NEW TASK"));
        let command = panel
            .hit_regions()
            .iter()
            .find_map(|hit| match &hit.action {
                Action::Control(command @ Command::Reply { .. }) => Some(command.clone()),
                _ => None,
            })
            .unwrap();
        assert!(!panel
            .hit_regions()
            .iter()
            .any(|hit| matches!(hit.action, Action::Key(KeyCode::Char('1'..='9')))));
        panel.set_status(ControlStatus {
            requests: vec![approval("new")],
            ..Default::default()
        });
        assert!(panel.activate(command.clone()).is_none());
        panel.show_requests(Some(WorkerId("a".into())));
        let mut changed = approval("old");
        changed.choices[0].response = serde_json::json!({"decision":"decline"});
        panel.set_status(ControlStatus {
            requests: vec![changed],
            ..Default::default()
        });
        assert!(panel.activate(command).is_none());
    }

    #[test]
    fn tab_focus_activates_the_visible_request_button_and_prevents_double_reply() {
        let mut panel = ready_panel();
        panel.show_requests(Some(WorkerId("a".into())));
        panel.set_status(ControlStatus {
            requests: vec![approval("current")],
            ..Default::default()
        });
        draw_panel(&mut panel, 40, 24);
        panel.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(
            matches!(panel.handle_key(KeyEvent::new(KeyCode::Enter,KeyModifiers::NONE)),Some(Command::Reply{request,..}) if request=="current")
        );
        assert!(panel
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .is_none());
    }

    #[test]
    fn operational_hits_follow_current_capabilities_and_never_promote_observed_tasks() {
        let mut panel = ready_panel();
        panel.show_task(WorkerId("a".into()), "Observed".into(), Agent::Codex);
        panel.status.tasks.insert(
            "a".into(),
            TaskAccess {
                native: true,
                description: "Open the original conversation".into(),
                ..Default::default()
            },
        );
        draw_panel(&mut panel, 40, 24);
        assert!(panel
            .hit_regions()
            .iter()
            .any(|hit| matches!(hit.action, Action::Control(Command::OpenNative { .. }))));
        assert!(!panel.hit_regions().iter().any(|hit| matches!(
            hit.action,
            Action::Control(Command::Interrupt { .. } | Command::Send { .. })
        )));
        assert!(panel
            .activate(Command::Interrupt {
                worker: WorkerId("a".into())
            })
            .is_none());
        panel.status.tasks.clear();
        assert!(panel
            .activate(Command::OpenNative {
                worker: WorkerId("a".into())
            })
            .is_none());
    }

    #[test]
    fn project_picker_keeps_the_full_known_path_and_connections_offer_sources() {
        let mut panel = ready_panel();
        panel.set_projects(vec![(
            "Team project".into(),
            "/work/project with spaces".into(),
        )]);
        panel.show_new(String::new());
        panel.handle_key(KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE));
        let text = draw_panel(&mut panel, 40, 24);
        assert!(text.contains("/work/project with spaces"));
        assert!(panel
            .hit_regions()
            .iter()
            .any(|hit| hit.action == Action::ControlProject("/work/project with spaces".into())));
        panel.select_project("/work/project with spaces".into());
        assert_eq!(panel.project.text, "/work/project with spaces");
        panel.select_project("/not-known".into());
        assert_eq!(panel.project.text, "/work/project with spaces");
        panel.show_connections();
        let text = draw_panel(&mut panel, 40, 24);
        assert!(text.contains("Local sources"));
        assert!(panel
            .hit_regions()
            .iter()
            .any(|hit| hit.action == Action::Control(Command::Sources)));
        assert_eq!(panel.activate(Command::Sources), Some(Command::Sources));
        assert!(!panel.busy());
    }

    #[test]
    fn project_picker_disambiguates_equal_names_and_removes_non_adjacent_duplicate_paths() {
        let mut panel = ready_panel();
        panel.set_projects(vec![
            ("A".into(), "/team-a/project".into()),
            ("Project".into(), "/team-b/project".into()),
            ("Project".into(), "/team-c/project".into()),
            ("Z".into(), "/team-a/project".into()),
        ]);
        assert_eq!(panel.projects.len(), 3);
        panel.show_new(String::new());
        panel.handle_key(KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE));
        panel.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(draw_panel(&mut panel, 40, 24).contains("/team-b/project"));
        panel.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(draw_panel(&mut panel, 40, 24).contains("/team-c/project"));
        panel.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(panel.project.text, "/team-c/project");
    }

    #[test]
    fn tab_can_return_from_buttons_to_fields_without_editing_a_hidden_draft() {
        let mut panel = ready_panel();
        panel.status.can_start_codex = true;
        panel.show_new("/project".into());
        panel.paste("Keep this instruction");
        draw_panel(&mut panel, 40, 24);
        panel.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(panel.button_focus, Some(0));
        panel.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        panel.paste("ignored paste");
        assert_eq!(panel.message.text, "Keep this instruction");
        for _ in 0..panel.buttons.len() {
            panel.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        }
        assert_eq!(panel.button_focus, None);
        assert_eq!(panel.field, 0);
        panel.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(panel.provider, Agent::Claude);
        assert!(panel.message.text.is_empty());
    }
}
