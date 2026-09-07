//! Local connection choices. No credentials or transcript content are stored.

use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use serde::{Deserialize, Serialize};
use theywork_collect::Config;

use crate::{Args, TerminalModeGuard, FRAME};

const INK: Color = Color::Indexed(255);
const MUTED: Color = Color::Indexed(145);
const ACCENT: Color = Color::Indexed(116);
const BG: Color = Color::Indexed(234);

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
        let mut guard = TerminalModeGuard::enter_alternate()?;
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        let action = show(
            &mut terminal,
            value,
            args.config_dir.as_deref(),
            args.remember.unwrap_or(true),
            !args.no_save,
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

pub(crate) fn show<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut value: Connections,
    config_dir: Option<&Path>,
    remember: bool,
    allow_remember: bool,
) -> Result<Action> {
    let allow_remember = allow_remember && config_dir.is_some();
    let mut remember = remember && allow_remember;
    let mut selected = 0;
    let mut editing: Option<PathEditor> = None;
    let mut error = String::new();
    loop {
        if let Some(error) = crate::termination_error() {
            return Err(error);
        }
        terminal.draw(|frame| {
            draw(
                frame,
                &value,
                selected,
                editing.as_ref(),
                &error,
                remember,
                allow_remember,
            );
            if std::env::var_os("NO_COLOR").is_some()
                || std::env::var("THEYWORK_COLOR")
                    .is_ok_and(|value| matches!(value.as_str(), "none" | "mono" | "monochrome"))
            {
                for cell in &mut frame.buffer_mut().content {
                    cell.set_fg(Color::Reset);
                    cell.set_bg(Color::Reset);
                }
            }
        })?;
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
        let Event::Key(key) = input else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        if crate::is_ctrl_c(key) {
            return Ok(Action::Cancel);
        }
        let size = terminal.size()?;
        if size.width < 40 || size.height < 16 {
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
                                if selected == 0 {
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
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(Action::Cancel),
            KeyCode::Char('d') => return Ok(Action::Demo),
            KeyCode::Down | KeyCode::Tab | KeyCode::Char('j') => {
                selected = (selected + 1) % 3;
                error.clear();
            }
            KeyCode::Up | KeyCode::BackTab | KeyCode::Char('k') => {
                selected = (selected + 2) % 3;
                error.clear();
            }
            KeyCode::Char('m') if allow_remember => {
                remember = !remember;
                error.clear();
            }
            KeyCode::Char(' ') => {
                match selected {
                    0 => value.claude = !value.claude,
                    1 => value.codex = !value.codex,
                    _ if allow_remember => remember = !remember,
                    _ => {}
                }
                error.clear();
            }
            KeyCode::Char('e') if selected < 2 => {
                editing = Some(PathEditor::new(
                    &if selected == 0 {
                        &value.claude_home
                    } else {
                        &value.codex_home
                    }
                    .to_string_lossy(),
                ));
                error.clear();
            }
            KeyCode::Enter => {
                if let Some(index) = invalid_source(&value) {
                    selected = index;
                    error = format!(
                        "{}: e to fix folder, Space to turn off.",
                        if index == 0 { "Claude Code" } else { "Codex" }
                    );
                } else {
                    if let Some(directory) = config_dir.filter(|_| remember) {
                        if save(directory, &value).is_err() {
                            selected = 2;
                            error = "Cannot save settings here.\nSpace: temporary, then Enter to connect.".into();
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

fn draw(
    frame: &mut Frame,
    value: &Connections,
    selected: usize,
    editing: Option<&PathEditor>,
    error: &str,
    remember: bool,
    allow_remember: bool,
) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(BG).fg(INK)),
        area,
    );
    if area.width < 40 || area.height < 16 {
        frame.render_widget(Paragraph::new("CONNECT YOUR TEAM\nEnlarge to 40 x 16 to choose sources.\nq: back / quit   d: demo").wrap(Wrap { trim: false }), area);
        return;
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
    let chunks = Layout::vertical([
        Constraint::Length(if compact { 1 } else { 3 }),
        Constraint::Length(if compact { 1 } else { 3 }),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(2),
        Constraint::Min(2),
        Constraint::Length(2),
    ])
    .split(body);
    let heading = if compact {
        "CONNECT YOUR TEAM"
    } else {
        "THEY WORK  /  YOUR SOFTWARE TOWER\nConnect your team\nOne project per floor. One conversation per worker."
    };
    frame.render_widget(
        Paragraph::new(heading).style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        chunks[0],
    );
    let intro = if compact {
        "Local messages & activity; no sign-in."
    } else {
        "Choose whose local conversations appear here. No sign-in or API key needed.\nReads titles, messages and tool activity from app data folders.\nEverything stays on this computer. Approve requests in the original app."
    };
    frame.render_widget(
        Paragraph::new(intro)
            .style(Style::default().fg(MUTED))
            .wrap(Wrap { trim: false }),
        chunks[1],
    );
    for (index, (title, enabled, path)) in [
        ("CLAUDE CODE", value.claude, &value.claude_home),
        ("CODEX", value.codex, &value.codex_home),
    ]
    .into_iter()
    .enumerate()
    {
        let active = selected == index;
        let rect = chunks[index + 2];
        let (path_text, cursor) = if let Some(editor) = editing.filter(|_| active) {
            editor.viewport(rect.width.saturating_sub(2) as usize)
        } else {
            (path_label(path, rect.width.saturating_sub(2) as usize), 0)
        };
        let status = if !enabled {
            "Off · no conversations will be read"
        } else if path.is_dir() {
            "Ready · reads after you press Enter"
        } else {
            folder_status(path)
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(path_text),
                Line::from(Span::styled(status, Style::default().fg(MUTED))),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        " {} {} {} ",
                        if active { ">" } else { " " },
                        if enabled { "[x]" } else { "[ ]" },
                        title
                    ))
                    .border_style(Style::default().fg(if active { ACCENT } else { MUTED })),
            ),
            rect,
        );
        if active && editing.is_some() {
            frame.set_cursor_position((rect.x + 1 + cursor, rect.y + 1));
        }
    }
    let save_note = if !allow_remember {
        "Saving is disabled for this session."
    } else if remember {
        "Save sources, appearance and floor."
    } else {
        "This run only. Saved choices stay as-is."
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "{} [{}] Remember on this computer",
                if selected == 2 { ">" } else { " " },
                if remember { "x" } else { " " }
            )),
            Line::from(Span::styled(save_note, Style::default().fg(MUTED))),
        ])
        .style(Style::default().fg(if selected == 2 { ACCENT } else { INK })),
        chunks[4],
    );
    let notice = if !error.is_empty() {
        error
    } else if editing.is_some() {
        "App data folder (.codex / .claude)."
    } else if !value.claude && !value.codex {
        "Enter opens an empty tower. Connect later with c, or explore the demo with d."
    } else {
        "Use app data folders (.codex / .claude).\nChoose a source; e edits its folder."
    };
    frame.render_widget(
        Paragraph::new(notice)
            .style(Style::default().fg(if error.is_empty() {
                MUTED
            } else {
                Color::LightRed
            }))
            .wrap(Wrap { trim: false }),
        chunks[5],
    );
    let footer = if editing.is_some() {
        "←→ move  Home/End  Ctrl+U clear\nEnter apply folder   Esc cancel edit"
    } else if !value.claude && !value.codex {
        "↑↓ choose  Space on/off  e edit folder\nEnter empty tower   d demo   Esc back"
    } else {
        "↑↓ choose  Space on/off  e edit folder\nEnter connect   d demo   Esc back"
    };
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(ACCENT)),
        chunks[6],
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
                    "Enter connect",
                    "CLAUDE CODE",
                    "CODEX",
                    "Remember on this computer",
                    "e edit folder",
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
