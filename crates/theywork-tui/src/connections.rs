//! Local connection choices. No credentials or transcript content are stored.

use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use serde::{Deserialize, Serialize};
use theywork_collect::Config;

use crate::{Args, TerminalModeGuard, FRAME};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Connections {
    pub claude: bool,
    pub codex: bool,
    pub claude_home: PathBuf,
    pub codex_home: PathBuf,
}

impl Connections {
    pub fn from_args(args: &Args) -> Result<Self> {
        if let Some(value) = &args.connection_choice {
            return Ok(value.clone());
        }
        let discovered = theywork_collect::discovery_paths();
        let mut value = Self {
            claude: true,
            codex: true,
            claude_home: discovered
                .iter()
                .find(|(agent, _)| *agent == theywork_core::Agent::Claude)
                .map(|(_, path)| path.clone())
                .unwrap_or_else(|| default_home(".claude")),
            codex_home: discovered
                .iter()
                .find(|(agent, _)| *agent == theywork_core::Agent::Codex)
                .map(|(_, path)| path.clone())
                .unwrap_or_else(|| default_home(".codex")),
        };
        value.claude = value.claude_home.is_dir();
        value.codex = value.codex_home.is_dir();
        let saved = args.config_dir.as_deref().map(load).transpose();
        match saved {
            Ok(Some(Some(saved))) => value = saved,
            Err(_) if args.setup => {}
            Err(error) => return Err(error),
            _ => {}
        }
        for (name, agent) in [
            ("THEYWORK_CLAUDE_HOME", theywork_core::Agent::Claude),
            ("THEYWORK_CODEX_HOME", theywork_core::Agent::Codex),
        ] {
            if std::env::var_os(name).is_some_and(|value| !value.is_empty()) {
                if let Some((_, path)) =
                    discovered.iter().find(|(candidate, _)| *candidate == agent)
                {
                    if agent == theywork_core::Agent::Claude {
                        value.claude_home = path.clone();
                    } else {
                        value.codex_home = path.clone();
                    }
                }
            }
        }
        if let Some(sources) = args.sources.as_deref() {
            value.claude = matches!(sources, "all" | "claude");
            value.codex = matches!(sources, "all" | "codex");
        }
        if let Some(path) = &args.claude_home {
            value.claude_home = crate::resolve_filesystem_path(path)?;
        }
        if let Some(path) = &args.codex_home {
            value.codex_home = crate::resolve_filesystem_path(path)?;
        }
        value.claude_home = crate::resolve_filesystem_path(&value.claude_home)?;
        value.codex_home = crate::resolve_filesystem_path(&value.codex_home)?;
        Ok(value)
    }

    pub fn config(&self) -> Config {
        Config {
            claude_home: self.claude.then(|| self.claude_home.clone()),
            codex_home: self.codex.then(|| self.codex_home.clone()),
            active_within: theywork_collect::DEFAULT_ACTIVE_WITHIN,
            only_paths: Vec::new(),
        }
    }

    pub fn apply(&self, args: &mut Args) {
        args.connection_choice = Some(self.clone());
        args.sources = Some(
            match (self.claude, self.codex) {
                (true, true) => "all",
                (true, false) => "claude",
                (false, true) => "codex",
                (false, false) => "none",
            }
            .into(),
        );
        args.claude_home = Some(self.claude_home.clone());
        args.codex_home = Some(self.codex_home.clone());
    }
}

fn default_home(name: &str) -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(name)
}

pub(crate) fn load(directory: &Path) -> Result<Option<Connections>> {
    let path = directory.join("connections.json");
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| {
                format!(
                    "could not read {}; run --setup to choose sources again",
                    path.display()
                )
            })
            .map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| {
            format!(
                "could not read {}; run --setup to choose sources again",
                path.display()
            )
        }),
    }
}

pub(crate) fn save(directory: &Path, connections: &Connections) -> Result<()> {
    std::fs::create_dir_all(directory)?;
    let path = directory.join("connections.json");
    // Only user-selected provider switches and paths; no copied agent records.
    std::fs::write(&path, serde_json::to_vec_pretty(connections)?)
        .with_context(|| format!("could not save {}", path.display()))
}

/// Resolve a settings location without creating it or reading agent data.
pub(crate) fn default_config_dir() -> Option<PathBuf> {
    config_location(
        cfg!(windows),
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        std::env::var_os("APPDATA").map(PathBuf::from),
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from),
    )
}

fn config_location(
    windows: bool,
    xdg: Option<PathBuf>,
    appdata: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    let base = if windows { appdata } else { xdg };
    base.filter(|path| path.is_absolute())
        .or_else(|| {
            home.filter(|path| path.is_absolute())
                .map(|path| path.join(".config"))
        })
        .map(|path| path.join("they-work"))
}

pub(crate) fn print_choose_sources() {
    println!("Connect your team before reading local conversations.");
    println!("Run they-work in a terminal to choose Claude Code, Codex, or an empty tower.");
    println!("For scripts: they-work --sources codex --once (or claude, all, none).");
    println!("Custom folders: --codex-home <folder> / --claude-home <folder>.");
    println!("Try they-work --demo to explore without reading conversations.");
    println!("No conversations were read and no settings were saved.");
}

pub(crate) enum Action {
    Connect { value: Connections, remember: bool },
    Demo,
    Cancel,
}

pub(crate) fn prepare(args: &mut Args) -> Result<bool> {
    if args.demo {
        return Ok(true);
    }
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    if args.setup && !interactive {
        return Err(anyhow!("--setup needs an interactive terminal; use --sources codex|claude|all|none and --codex-home / --claude-home for scripts"));
    }
    let remembered = match args.config_dir.as_deref().map(load).transpose() {
        Ok(value) => value.flatten().is_some(),
        Err(_) if args.setup => false,
        Err(error) => return Err(error),
    };
    let needs_choice = args.sources.is_none() && !remembered;
    if args.setup || (interactive && !args.once && !args.doctor && !args.headless && needs_choice) {
        let mut value = Connections::from_args(args)?;
        if needs_choice {
            value.claude = value.claude_home.is_dir();
            value.codex = value.codex_home.is_dir();
        }
        let capabilities = theywork_terminal_image::detect_terminal().unwrap_or_default();
        let mut guard = TerminalModeGuard::enter_alternate()?;
        let mut terminal = Terminal::new(CrosstermBackend::new(
            crate::frame_output::FrameOutput::new(io::stdout(), capabilities.synchronized_output),
        ))?;
        let action = show(
            &mut terminal,
            value,
            args.config_dir.as_deref(),
            args.remember.unwrap_or(true),
            !args.no_save,
            args.mouse.unwrap_or(true),
            !args.dark
                && (args.light
                    || args
                        .config_dir
                        .as_deref()
                        .and_then(|dir| std::fs::read(dir.join("appearance.json")).ok())
                        .and_then(|bytes| {
                            serde_json::from_slice::<theywork_render::RendererPreferences>(&bytes)
                                .ok()
                        })
                        .is_some_and(|preferences| preferences.light)),
        );
        drop(terminal);
        guard.restore()?;
        match action? {
            Action::Connect { value, remember } => {
                value.apply(args);
                args.remember = Some(remember);
                args.setup = false;
            }
            Action::Demo => args.demo = true,
            Action::Cancel => return Ok(false),
        }
    } else if needs_choice {
        args.consent_needed = true;
    }
    Ok(true)
}

#[derive(Debug)]
struct PathEditor {
    chars: Vec<char>,
    cursor: usize,
}

impl PathEditor {
    fn new(text: &str) -> Self {
        let chars: Vec<_> = text.chars().collect();
        let cursor = chars.len();
        Self { chars, cursor }
    }

    fn text(&self) -> String {
        self.chars.iter().collect()
    }

    fn insert(&mut self, text: &str) {
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            self.chars.insert(self.cursor, ch);
            self.cursor += 1;
        }
    }

    fn key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.chars.clear();
                self.cursor = 0;
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.chars.len()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.chars.len(),
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.chars.remove(self.cursor);
            }
            KeyCode::Delete if self.cursor < self.chars.len() => {
                self.chars.remove(self.cursor);
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && !ch.is_control() =>
            {
                self.insert(&ch.to_string())
            }
            _ => {}
        }
    }

    /// Keep the insertion point on screen, including at the end of long UTF-8 paths.
    fn viewport(&self, width: usize) -> (String, u16) {
        if width < 2 {
            return (String::new(), 0);
        }
        let mut start = self.cursor;
        let mut used = 0;
        while start > 0 {
            let next = char_width(self.chars[start - 1]);
            if used + next > width.saturating_sub(2) {
                break;
            }
            used += next;
            start -= 1;
        }
        let mut text = if start > 0 {
            "‹".to_string()
        } else {
            String::new()
        };
        let cursor = used + usize::from(start > 0);
        let mut cells = usize::from(start > 0);
        for ch in &self.chars[start..] {
            let next = char_width(*ch);
            if cells + next > width {
                break;
            }
            text.push(*ch);
            cells += next;
        }
        (text, cursor as u16)
    }
}

fn char_width(ch: char) -> usize {
    Line::from(ch.to_string()).width()
}

fn path_label(path: &Path, width: usize) -> String {
    let text = crate::single_line(&path.to_string_lossy());
    if Line::from(text.as_str()).width() <= width {
        return text;
    }
    let tail: Vec<_> = text
        .chars()
        .rev()
        .scan(0, |used, ch| {
            *used += char_width(ch);
            (*used <= width.saturating_sub(1)).then_some(ch)
        })
        .collect();
    format!("…{}", tail.into_iter().rev().collect::<String>())
}

pub(crate) fn folder_status(path: &Path) -> &'static str {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => "Folder found",
        Ok(_) => "This is a file; choose a folder",
        Err(error) if error.kind() == io::ErrorKind::NotFound => "Folder not found",
        Err(_) => "Cannot access this folder",
    }
}

fn invalid_source(value: &Connections) -> Option<usize> {
    [
        (value.claude, &value.claude_home),
        (value.codex, &value.codex_home),
    ]
    .iter()
    .position(|(enabled, path)| *enabled && !path.is_dir())
}

pub(crate) fn show(
    terminal: &mut crate::OfficeTerminal,
    mut value: Connections,
    config_dir: Option<&Path>,
    remember: bool,
    allow_remember: bool,
    mouse_enabled: bool,
    light: bool,
) -> Result<Action> {
    let mut mouse = crate::MouseCaptureGuard { enabled: false };
    mouse.sync(mouse_enabled)?;
    let allow_remember = allow_remember && config_dir.is_some();
    let mut remember = remember && allow_remember;
    let mut selected = 5;
    let mut editing: Option<PathEditor> = None;
    let mut error = String::new();
    loop {
        if let Some(error) = crate::termination_error() {
            return Err(error);
        }
        let mut presented = Vec::new();
        let mut presented_size = Rect::default();
        terminal.backend_mut().writer_mut().begin();
        terminal.draw(|frame| {
            presented_size = frame.area();
            presented = source_hits(frame.area(), editing.is_some());
            if !allow_remember {
                presented.retain(|(_, target, _)| *target != Some(4));
            }
            draw(
                frame,
                &value,
                selected,
                editing.as_ref(),
                &error,
                remember,
                allow_remember,
            );
            let monochrome = std::env::var_os("NO_COLOR").is_some()
                || std::env::var("THEYWORK_COLOR")
                    .is_ok_and(|value| matches!(value.as_str(), "none" | "mono" | "monochrome"));
            theywork_render::components::theme_buffer(frame.buffer_mut(), light, monochrome);
            if !monochrome
                && theywork_render::canvas::Canvas::new(0, 0).color_depth()
                    == theywork_render::ColorDepth::Palette256
            {
                theywork_render::canvas::Canvas::quantize_colors(frame.buffer_mut());
            }
        })?;
        terminal.backend_mut().writer_mut().finish()?;
        if !event::poll(FRAME)? {
            continue;
        }
        let input = event::read()?;
        if let Event::Paste(text) = &input {
            if let Some(editor) = &mut editing {
                if text.trim().contains(['\n', '\r']) {
                    error = "Paste one folder path at a time.".into();
                } else {
                    editor.insert(text.trim());
                    error.clear();
                }
            }
            continue;
        }
        let key = match input {
            Event::Key(key) => key,
            Event::Mouse(event)
                if mouse_enabled
                    && event.kind
                        == crossterm::event::MouseEventKind::Down(
                            crossterm::event::MouseButton::Left,
                        ) =>
            {
                let current = terminal.size()?;
                if current.width != presented_size.width || current.height != presented_size.height
                {
                    continue;
                }
                let Some((_, target, code)) = presented
                    .iter()
                    .rev()
                    .find(|(area, _, _)| area.contains((event.column, event.row).into()))
                else {
                    continue;
                };
                if let Some(index) = target {
                    selected = *index;
                }
                crossterm::event::KeyEvent::new(*code, KeyModifiers::NONE)
            }
            _ => continue,
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        if crate::is_ctrl_c(key) {
            return Ok(Action::Cancel);
        }
        let size = terminal.size()?;
        if size.width < 32 || size.height < 14 {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => return Ok(Action::Cancel),
                KeyCode::Char('d') => return Ok(Action::Demo),
                _ => continue,
            }
        }
        if let Some(editor) = &mut editing {
            match key.code {
                KeyCode::Esc => {
                    editing = None;
                    error.clear();
                }
                KeyCode::Enter => {
                    let text = editor.text();
                    if text.trim().is_empty() {
                        error = "Enter a folder path, or press Esc to cancel.".into();
                    } else {
                        match crate::resolve_filesystem_path(Path::new(text.trim())) {
                            Ok(path) => {
                                if selected < 2 {
                                    value.claude_home = path;
                                } else {
                                    value.codex_home = path;
                                }
                                editing = None;
                                error.clear();
                            }
                            Err(_) => {
                                error = "Could not use that path. Check the folder and try again."
                                    .into()
                            }
                        }
                    }
                }
                _ => {
                    editor.key(key);
                    error.clear();
                }
            }
            continue;
        }
        let code = if key.code == KeyCode::Enter {
            match selected {
                0 | 2 | 4 => KeyCode::Char(' '),
                1 | 3 => KeyCode::Char('e'),
                6 => KeyCode::Char('d'),
                7 => KeyCode::Esc,
                _ => KeyCode::F(5),
            }
        } else {
            key.code
        };
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(Action::Cancel),
            KeyCode::Char('d') => return Ok(Action::Demo),
            KeyCode::Down | KeyCode::Tab | KeyCode::Char('j') => {
                selected = (selected + 1) % 8;
                if selected == 4 && !allow_remember {
                    selected = 5;
                }
                error.clear();
            }
            KeyCode::Up | KeyCode::BackTab | KeyCode::Char('k') => {
                selected = (selected + 7) % 8;
                if selected == 4 && !allow_remember {
                    selected = 3;
                }
                error.clear();
            }
            KeyCode::Char('m') if allow_remember => {
                remember = !remember;
                error.clear();
            }
            KeyCode::Char(' ') => {
                match selected {
                    0 | 1 => value.claude = !value.claude,
                    2 | 3 => value.codex = !value.codex,
                    4 if allow_remember => remember = !remember,
                    _ => {}
                }
                error.clear();
            }
            KeyCode::Char('e') if selected < 4 => {
                editing = Some(PathEditor::new(
                    &if selected < 2 {
                        &value.claude_home
                    } else {
                        &value.codex_home
                    }
                    .to_string_lossy(),
                ));
                error.clear();
            }
            KeyCode::F(5) => {
                if let Some(index) = invalid_source(&value) {
                    selected = index * 2;
                    error = format!(
                        "{}: e to fix folder, Space to turn off.",
                        if index == 0 { "Claude Code" } else { "Codex" }
                    );
                } else {
                    if let Some(directory) = config_dir.filter(|_| remember) {
                        if save(directory, &value).is_err() {
                            selected = 4;
                            error =
                                "Cannot save here. Turn Remember off, then F5 to connect.".into();
                            continue;
                        }
                    }
                    return Ok(Action::Connect { value, remember });
                }
            }
            _ => {}
        }
    }
}

fn source_layout(area: Rect) -> Option<Vec<Rect>> {
    if area.width < 32 || area.height < 14 {
        return None;
    }
    let width = area.width.min(88);
    let height = area.height.min(26);
    let body = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let compact = height < 22 || width < 64;
    Some(
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(if compact { 1 } else { 3 }),
            Constraint::Length(if compact { 3 } else { 4 }),
            Constraint::Length(if compact { 3 } else { 4 }),
            Constraint::Length(if compact { 1 } else { 2 }),
            Constraint::Min(2),
            Constraint::Length(3),
        ])
        .split(body)
        .to_vec(),
    )
}

fn source_buttons(area: Rect, editing: bool) -> Vec<(Rect, &'static str, usize, KeyCode)> {
    let labels = if editing {
        vec![("Apply", 5, KeyCode::Enter), ("Cancel", 7, KeyCode::Esc)]
    } else {
        vec![
            ("Connect", 5, KeyCode::F(5)),
            ("Demo", 6, KeyCode::Char('d')),
            ("Back", 7, KeyCode::Esc),
        ]
    };
    let mut x = area.x;
    labels
        .into_iter()
        .map(|(label, focus, code)| {
            let rect = Rect::new(x, area.y + 1, label.len() as u16 + 4, 1);
            x = rect.right() + 1;
            (rect, label, focus, code)
        })
        .filter(|(rect, _, _, _)| rect.right() <= area.right())
        .collect()
}

fn source_hits(area: Rect, editing: bool) -> Vec<(Rect, Option<usize>, KeyCode)> {
    let Some(chunks) = source_layout(area) else {
        return vec![];
    };
    let mut hits = Vec::new();
    if !editing {
        for index in 0..2 {
            let card = chunks[index + 2];
            hits.push((
                Rect::new(card.x, card.y, card.width, 1),
                Some(index * 2),
                KeyCode::Char(' '),
            ));
            hits.push((
                Rect::new(card.x, card.y + 1, card.width, 1),
                Some(index * 2 + 1),
                KeyCode::Char('e'),
            ));
        }
        hits.push((
            Rect::new(chunks[4].x, chunks[4].y, chunks[4].width, 1),
            Some(4),
            KeyCode::Char('m'),
        ));
    }
    hits.extend(
        source_buttons(chunks[6], editing)
            .into_iter()
            .map(|(rect, _, focus, code)| (rect, (!editing).then_some(focus), code)),
    );
    hits
}

fn draw(
    frame: &mut Frame,
    value: &Connections,
    selected: usize,
    editing: Option<&PathEditor>,
    error: &str,
    remember: bool,
    allow_remember: bool,
) {
    let colors = theywork_render::components::palette();
    let base = Style::default().bg(colors.background).fg(colors.ink);
    let area = frame.area();
    frame.render_widget(Block::default().style(base), area);
    let Some(chunks) = source_layout(area) else {
        frame.render_widget(
            Paragraph::new(
                "Connections / Sources\nUse 32 × 14 to choose folders.\nEsc: back   d: demo",
            )
            .wrap(Wrap { trim: false }),
            area,
        );
        return;
    };
    let compact = chunks[2].height == 3;
    frame.render_widget(
        Paragraph::new("Connections / Sources")
            .style(Style::default().fg(colors.ink).add_modifier(Modifier::BOLD)),
        chunks[0],
    );
    let intro = if compact {
        "Local observation · no account"
    } else {
        "Choose the conversations visible in your tower.\nReads titles, messages and activity from local app data folders.\nNo they-work account. Provider controls require a verified connection."
    };
    frame.render_widget(
        Paragraph::new(intro)
            .style(Style::default().fg(colors.muted))
            .wrap(Wrap { trim: false }),
        chunks[1],
    );
    for (index, (title, enabled, path)) in [
        ("Claude Code", value.claude, &value.claude_home),
        ("Codex", value.codex, &value.codex_home),
    ]
    .into_iter()
    .enumerate()
    {
        let card = chunks[index + 2];
        let active = selected / 2 == index && selected < 4;
        let name = Rect::new(card.x, card.y, card.width, 1);
        frame.render_widget(
            Paragraph::new(format!(
                "{} [{}] {title}",
                if selected == index * 2 { ">" } else { " " },
                if enabled { "x" } else { " " }
            ))
            .style(Style::default().fg(if selected == index * 2 {
                colors.accent
            } else {
                colors.ink
            })),
            name,
        );
        let (text, cursor) = if let Some(editor) = editing.filter(|_| active) {
            editor.viewport(card.width.saturating_sub(3) as usize)
        } else {
            (path_label(path, card.width.saturating_sub(3) as usize), 0)
        };
        frame.render_widget(
            Paragraph::new(format!(
                "{} {text}",
                if selected == index * 2 + 1 { ">" } else { "·" }
            ))
            .style(Style::default().fg(if active {
                colors.accent
            } else {
                colors.muted
            })),
            Rect::new(card.x, card.y + 1, card.width, 1),
        );
        if active && editing.is_some() {
            frame.set_cursor_position((card.x + 2 + cursor, card.y + 1));
        }
        let status = if active && editing.is_some() {
            "Editing folder"
        } else if !enabled {
            "Off · no conversations read"
        } else if path.is_dir() {
            "Ready · reads after Connect"
        } else {
            folder_status(path)
        };
        frame.render_widget(
            Paragraph::new(format!("  {status}")).style(Style::default().fg(
                if enabled && !path.is_dir() {
                    colors.warning
                } else {
                    colors.muted
                },
            )),
            Rect::new(card.x, card.y + 2, card.width, 1),
        );
    }
    let remember_text = if !allow_remember {
        "Temporary session · saving off".into()
    } else {
        format!(
            "{} [{}] Remember on this computer",
            if selected == 4 { ">" } else { " " },
            if remember { "x" } else { " " }
        )
    };
    frame.render_widget(
        Paragraph::new(remember_text).style(Style::default().fg(if selected == 4 {
            colors.accent
        } else {
            colors.ink
        })),
        Rect::new(chunks[4].x, chunks[4].y, chunks[4].width, 1),
    );
    if chunks[4].height > 1 {
        frame.render_widget(
            Paragraph::new(if remember {
                "Save sources and local appearance."
            } else {
                "Saved choices stay unchanged."
            })
            .style(Style::default().fg(colors.muted)),
            Rect::new(chunks[4].x, chunks[4].y + 1, chunks[4].width, 1),
        );
    }
    let notice = if !error.is_empty() {
        error
    } else if editing.is_some() {
        "App data folder: .codex / .claude"
    } else if !value.claude && !value.codex {
        "Connect opens an empty tower.\nYou can add sources later."
    } else {
        "Space: toggle · e: edit folder"
    };
    frame.render_widget(
        Paragraph::new(notice)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(if error.is_empty() {
                colors.muted
            } else {
                colors.warning
            })),
        chunks[5],
    );
    let footer = chunks[6];
    frame.render_widget(
        Paragraph::new(if editing.is_some() {
            "←→ move · Ctrl+U clear"
        } else {
            "Tab/↑↓ choose · Enter activate"
        })
        .style(Style::default().fg(colors.muted)),
        Rect::new(footer.x, footer.y, footer.width, 1),
    );
    for (rect, label, focus, _) in source_buttons(footer, editing.is_some()) {
        frame.render_widget(
            Paragraph::new(format!("[ {label} ]")).style(
                Style::default()
                    .fg(if selected == focus {
                        colors.accent
                    } else {
                        colors.ink
                    })
                    .add_modifier(if selected == focus {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            rect,
        );
    }
    frame.render_widget(
        Paragraph::new(if editing.is_some() {
            "Enter applies · Esc cancels"
        } else {
            "F5 connect · d demo · Esc back"
        })
        .style(Style::default().fg(colors.muted)),
        Rect::new(footer.x, footer.bottom() - 1, footer.width, 1),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;
    use ratatui::backend::TestBackend;

    fn example() -> Connections {
        Connections {
            claude: false,
            codex: true,
            claude_home: "/secret/claude".into(),
            codex_home: "/chosen/codex".into(),
        }
    }

    #[test]
    fn source_click_targets_only_visible_explicit_controls() {
        for size in [
            (32, 14),
            (80, 24),
            (120, 36),
            (192, 58),
            (110, 80),
            (240, 70),
        ] {
            let area = Rect::new(0, 0, size.0, size.1);
            let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
            terminal
                .draw(|f| draw(f, &example(), 0, None, "", true, true))
                .unwrap();
            let hits = source_hits(area, false);
            assert_eq!(hits.len(), 8);
            for (rect, _, _) in &hits {
                assert_eq!(*rect, rect.intersection(area));
            }
            let connect = hits
                .iter()
                .find(|(_, _, code)| *code == KeyCode::F(5))
                .unwrap()
                .0;
            let row = (connect.x..connect.right())
                .map(|x| terminal.backend().buffer()[(x, connect.y)].symbol())
                .collect::<String>();
            assert!(row.contains("Connect"));
            assert!(!source_hits(area, true)
                .iter()
                .any(|(_, target, _)| target.is_some()));
        }
        assert!(source_hits(Rect::new(0, 0, 31, 13), false).is_empty());
    }

    #[test]
    fn disabled_sources_have_no_collector_paths() {
        let config = example().config();
        assert!(config.claude_home.is_none());
        assert_eq!(config.codex_home, Some(PathBuf::from("/chosen/codex")));
        assert_eq!(invalid_source(&example()), Some(1));
    }

    #[test]
    fn editor_preserves_unicode_and_edits_at_cursor() {
        let mut input = PathEditor::new("/東京/foler");
        input.key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        input.key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        input.insert("d");
        assert_eq!(input.text(), "/東京/folder");
        input.key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        input.key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
        input.insert("C:/");
        assert_eq!(input.text(), "C:/東京/folder");
        input.key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        input.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(input.text(), "C:/東京/folde");
        input.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(input.text(), "");
    }

    #[test]
    fn long_path_editor_keeps_cursor_visible() {
        let mut editor = PathEditor::new("/a/very/long/東京/folder/.codex");
        let (text, cursor) = editor.viewport(18);
        assert!(text.ends_with("/.codex"));
        assert!(text.starts_with('‹'));
        assert!(cursor < 18);
        assert!(Line::from(text).width() <= 18);
        editor.cursor = 0;
        let (text, cursor) = editor.viewport(18);
        assert!(text.starts_with("/a/very"));
        assert_eq!(cursor, 0);
    }

    #[test]
    fn config_location_respects_platform_and_ignores_relative_environment() {
        let home = std::env::current_dir().unwrap();
        let xdg = home.join("xdg");
        let appdata = home.join("appdata");
        assert_eq!(
            config_location(
                false,
                Some(xdg.clone()),
                Some(appdata.clone()),
                Some(home.clone())
            ),
            Some(xdg.join("they-work"))
        );
        assert_eq!(
            config_location(true, None, Some(appdata.clone()), Some(home.clone())),
            Some(appdata.join("they-work"))
        );
        assert_eq!(
            config_location(false, Some("relative".into()), None, Some(home.clone())),
            Some(home.join(".config/they-work"))
        );
        assert_eq!(config_location(false, None, None, None), None);
    }

    #[test]
    fn setup_actions_and_memory_choice_remain_visible() {
        for (w, h) in [(40, 16), (80, 24), (120, 40), (10, 3)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal
                .draw(|f| draw(f, &example(), 0, None, "", true, true))
                .unwrap();
            if w >= 40 {
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|c| c.symbol())
                    .collect();
                for expected in [
                    "F5 connect",
                    "Claude Code",
                    "Codex",
                    "Remember on this computer",
                    "e: edit folder",
                ] {
                    assert!(
                        text.contains(expected),
                        "{w}x{h} missing {expected}: {text}"
                    );
                }
            }
        }
    }
}
