//! Local connection choices. No credentials or transcript content are stored.

use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use serde::{Deserialize, Serialize};
use theywork_collect::Config;

use crate::{Args, TerminalModeGuard, FRAME};

const INK: Color = Color::Rgb(235, 229, 245);
const MUTED: Color = Color::Rgb(166, 159, 186);
const ACCENT: Color = Color::Rgb(105, 221, 214);
const BG: Color = Color::Rgb(18, 16, 29);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Connections {
    pub claude: bool,
    pub codex: bool,
    pub claude_home: PathBuf,
    pub codex_home: PathBuf,
}

impl Connections {
    pub fn from_args(args: &Args) -> Result<Self> {
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
        Err(error) => Err(error).with_context(|| format!("could not read {}", path.display())),
    }
}

pub(crate) fn save(directory: &Path, connections: &Connections) -> Result<()> {
    std::fs::create_dir_all(directory)?;
    let path = directory.join("connections.json");
    // Only user-selected provider switches and paths; no copied agent records.
    std::fs::write(&path, serde_json::to_vec_pretty(connections)?)
        .with_context(|| format!("could not save {}", path.display()))
}

pub(crate) enum Action {
    Connect(Connections),
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
    if args.setup
        || (interactive
            && !args.once
            && !args.doctor
            && !args.headless
            && args.sources.is_none()
            && !remembered)
    {
        let mut connections = Connections::from_args(args)?;
        if !remembered && args.sources.is_none() {
            connections.claude = connections.claude_home.is_dir();
            connections.codex = connections.codex_home.is_dir();
        }
        let mut guard = TerminalModeGuard::enter_alternate()?;
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        let action = show(&mut terminal, connections, args.config_dir.as_deref());
        drop(terminal);
        guard.restore()?;
        match action? {
            Action::Connect(value) => {
                if let Some(directory) = &args.config_dir {
                    save(directory, &value)?;
                }
                value.apply(args);
                args.setup = false;
            }
            Action::Demo => args.demo = true,
            Action::Cancel => return Ok(false),
        }
    }
    Ok(true)
}

pub(crate) fn show<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut value: Connections,
    config_dir: Option<&Path>,
) -> Result<Action> {
    let mut selected = 0;
    let mut editing: Option<String> = None;
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
                editing.as_deref(),
                &error,
                config_dir.is_some(),
            )
        })?;
        if !event::poll(FRAME)? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        if crate::is_ctrl_c(key) {
            return Ok(Action::Cancel);
        }
        if let Some(input) = &mut editing {
            match key.code {
                KeyCode::Esc => editing = None,
                KeyCode::Enter => {
                    if !input.trim().is_empty() {
                        let path = crate::resolve_filesystem_path(Path::new(input))?;
                        if selected == 0 {
                            value.claude_home = path;
                        } else {
                            value.codex_home = path;
                        }
                        editing = None;
                    }
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) if !c.is_control() => input.push(c),
                _ => {}
            }
            continue;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(Action::Cancel),
            KeyCode::Char('d') => return Ok(Action::Demo),
            KeyCode::Up | KeyCode::Down | KeyCode::Tab | KeyCode::Char('j' | 'k') => {
                selected = 1 - selected
            }
            KeyCode::Char(' ') => {
                if selected == 0 {
                    value.claude = !value.claude;
                } else {
                    value.codex = !value.codex;
                }
                error.clear();
            }
            KeyCode::Char('e') => {
                editing = Some(
                    if selected == 0 {
                        &value.claude_home
                    } else {
                        &value.codex_home
                    }
                    .to_string_lossy()
                    .into_owned(),
                )
            }
            KeyCode::Enter => {
                let missing = [
                    (value.claude, &value.claude_home),
                    (value.codex, &value.codex_home),
                ]
                .into_iter()
                .find(|(enabled, path)| *enabled && !path.is_dir());
                if missing.is_some() {
                    error =
                        "Selected folder is missing. Press e to edit, or Space to disconnect it."
                            .into();
                } else {
                    return Ok(Action::Connect(value));
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn draw(
    frame: &mut Frame,
    value: &Connections,
    selected: usize,
    editing: Option<&str>,
    error: &str,
    remembered: bool,
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
    let body = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + area.height.saturating_sub(28) / 2,
        width,
        area.height.min(28),
    );
    let compact = area.height < 24;
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(if compact { 2 } else { 4 }),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(body);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                " THEY WORK  /  YOUR SOFTWARE TOWER",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )),
            Line::from(" Connect your team"),
            Line::from(Span::styled(
                " One project per floor. One conversation per worker.",
                Style::default().fg(MUTED),
            )),
        ]),
        chunks[0],
    );
    let explanation = if compact {
        "Local data only. No account or API key needed."
    } else {
        "Choose which local conversations can appear in your tower.\nNo account or API key needed. Prompts, tool activity and titles stay on this machine.\nApprove requests in the original agent app; this office observes them."
    };
    frame.render_widget(
        Paragraph::new(explanation)
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
        let label = format!(
            " {} {} {} ",
            if active { ">" } else { " " },
            if enabled { "[x]" } else { "[ ]" },
            title
        );
        let status = if !enabled {
            "Disconnected"
        } else if path.is_dir() {
            "Folder found · contents read only after connecting"
        } else {
            "Folder missing · e to edit or Space to disconnect"
        };
        let path_text = if active { editing.unwrap_or("") } else { "" };
        let path_text = if path_text.is_empty() && editing.is_none() {
            path.to_string_lossy().into_owned()
        } else if active {
            path_text.to_string()
        } else {
            path.to_string_lossy().into_owned()
        };
        let lines = vec![
            Line::from(crate::single_line(&path_text)),
            Line::from(Span::styled(status, Style::default().fg(MUTED))),
        ];
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(label)
                    .border_style(Style::default().fg(if active { ACCENT } else { MUTED })),
            ),
            chunks[index + 2],
        );
    }
    let notice = if !error.is_empty() {
        error
    } else if editing.is_some() {
        "Editing folder · Backspace to erase · Enter apply · Esc cancel"
    } else if remembered {
        "Choices will be remembered in your selected config directory."
    } else {
        "Choices apply to this run. Use --config-dir <folder> to remember them."
    };
    frame.render_widget(
        Paragraph::new(notice)
            .style(Style::default().fg(if error.is_empty() {
                MUTED
            } else {
                Color::LightRed
            }))
            .wrap(Wrap { trim: false }),
        chunks[4],
    );
    frame.render_widget(
        Paragraph::new("↑↓ select  Space on/off  e folder\nEnter connect   d demo   Esc back")
            .style(Style::default().fg(ACCENT)),
        chunks[5],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn disabled_sources_have_no_collector_paths() {
        let value = Connections {
            claude: false,
            codex: true,
            claude_home: "/secret/claude".into(),
            codex_home: "/chosen/codex".into(),
        };
        let config = value.config();
        assert!(config.claude_home.is_none());
        assert_eq!(config.codex_home, Some(PathBuf::from("/chosen/codex")));
    }

    #[test]
    fn setup_actions_remain_visible_in_supported_sizes() {
        let value = Connections {
            claude: true,
            codex: false,
            claude_home: "/a/very/long/folder/with/a/project".into(),
            codex_home: "/codex".into(),
        };
        for (w, h) in [(40, 16), (80, 24), (120, 40), (10, 3)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal
                .draw(|f| draw(f, &value, 0, None, "", false))
                .unwrap();
            if w >= 40 {
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|c| c.symbol())
                    .collect();
                assert!(text.contains("Enter connect"), "{w}x{h}: {text}");
                assert!(text.contains("CLAUDE CODE"));
                assert!(text.contains("CODEX"));
            }
        }
    }
}
