//! Search the live project directory without tying navigation to row numbers.

use crate::design::{profile_for, CharacterProfile};
use crate::interaction::{Action, HitRegion};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use std::collections::BTreeMap;
use theywork_core::{Activity, Millis, OfficeId, WorkerId, WorkerStatus, World};

use super::{
    inset, paint_opaque, safe_display, short_path, status_color, ACCENT, INK, MUTED, PANEL,
    PANEL_HIGHLIGHT,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Project(OfficeId),
    Worker(OfficeId, WorkerId),
}

struct Entry {
    target: Target,
    title: String,
    context: String,
    path: String,
    badge: &'static str,
    status: Option<WorkerStatus>,
    searchable: String,
}

#[derive(Default)]
pub(crate) struct Finder {
    pub open: bool,
    query: String,
    cursor: usize,
    entries: Vec<Entry>,
    matches: Vec<usize>,
    selected: usize,
    page_size: usize,
    pub unavailable: bool,
    hits: Vec<HitRegion>,
}

impl Finder {
    pub fn paste(&mut self, text: &str) {
        let text = text
            .chars()
            .map(|character| {
                if character.is_whitespace() {
                    ' '
                } else {
                    character
                }
            })
            .filter(|character| !character.is_control())
            .take(256_usize.saturating_sub(self.query.chars().count()))
            .collect::<String>();
        self.query.insert_str(self.cursor, &text);
        self.cursor += text.len();
        self.selected = 0;
        self.unavailable = false;
        self.filter(None);
    }

    pub fn show(&mut self) {
        self.open = true;
        self.query.clear();
        self.cursor = 0;
        self.selected = 0;
        self.unavailable = false;
        self.filter(None);
    }

    #[cfg(test)]
    pub fn refresh(&mut self, world: &World, now: Millis) {
        self.refresh_entries(world, now, None);
    }

    pub fn refresh_with_profiles(
        &mut self,
        world: &World,
        now: Millis,
        profiles: &BTreeMap<String, CharacterProfile>,
    ) {
        self.refresh_entries(world, now, Some(profiles));
    }

    pub fn hit_regions(&self) -> Vec<HitRegion> {
        self.hits.clone()
    }

    fn refresh_entries(
        &mut self,
        world: &World,
        now: Millis,
        profiles: Option<&BTreeMap<String, CharacterProfile>>,
    ) {
        let selected = self.selected_target();
        self.entries.clear();
        for (floor, office) in world.offices().enumerate() {
            let attention = office
                .workers
                .iter()
                .filter(|w| w.status_at(now).needs_attention())
                .count();
            let context = format!(
                "Floor {:02} · {} workers · {} need attention",
                floor + 1,
                office.workers.len(),
                attention
            );
            let title = safe_display(&office.name);
            let path = safe_display(&office.path);
            self.entries.push(Entry {
                target: Target::Project(office.id.clone()),
                searchable: format!("{title} {path} project floor").to_lowercase(),
                title,
                context,
                path: path.clone(),
                badge: "PROJECT",
                status: None,
            });
            for worker in &office.workers {
                let status = worker.status_at(now);
                let (_, aliases) = match status {
                    WorkerStatus::Running => ("WORKING", "working running active"),
                    WorkerStatus::Idle => ("IDLE", "idle ready"),
                    WorkerStatus::Blocked
                        if matches!(worker.activity, Activity::Waiting { .. }) =>
                    {
                        ("WAITING", "waiting blocked approval attention")
                    }
                    WorkerStatus::Blocked => ("ATTENTION", "attention blocked quiet"),
                    WorkerStatus::Failed => ("FAILED", "failed error attention"),
                };
                let badge = crate::presentation::state_label(worker, now);
                let aliases = match worker.wait_reason {
                    Some(theywork_core::WaitReason::HumanApproval) => {
                        "approval permission request attention"
                    }
                    Some(theywork_core::WaitReason::HumanInput) => {
                        "question input request attention"
                    }
                    Some(theywork_core::WaitReason::AutomaticReview) => "automatic review working",
                    Some(theywork_core::WaitReason::Child) => "waiting team working",
                    Some(theywork_core::WaitReason::Process) => "waiting process working",
                    _ if matches!(worker.activity, Activity::Waiting { .. }) => {
                        "waiting followup attention"
                    }
                    _ => aliases,
                };
                let title = profiles.map_or_else(
                    || safe_display(&worker.name),
                    |profiles| {
                        format!(
                            "{} · {}",
                            safe_display(&profile_for(&worker.id.0, profiles).name),
                            safe_display(&worker.name)
                        )
                    },
                );
                let context = format!(
                    "{} · {} · floor {:02}",
                    safe_display(&office.name),
                    worker.agent.label(),
                    floor + 1
                );
                let searchable = format!(
                    "{title} {path} {} {aliases} {}",
                    worker.agent.label(),
                    worker.git_branch.as_deref().unwrap_or_default()
                )
                .to_lowercase();
                self.entries.push(Entry {
                    target: Target::Worker(office.id.clone(), worker.id.clone()),
                    title,
                    context,
                    path: path.clone(),
                    badge,
                    status: Some(status),
                    searchable,
                });
            }
        }
        self.filter(selected);
    }

    fn filter(&mut self, keep: Option<Target>) {
        let query = self.query.to_lowercase();
        let terms = query.split_whitespace().collect::<Vec<_>>();
        self.matches = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                if terms.is_empty() {
                    matches!(entry.target, Target::Project(_))
                } else {
                    terms.iter().all(|term| entry.searchable.contains(term))
                }
            })
            .map(|(index, _)| index)
            .collect();
        if !terms.is_empty() {
            self.matches.sort_by_key(|index| {
                let title = self.entries[*index].title.to_lowercase();
                if title == query {
                    0
                } else if title.starts_with(&query) {
                    1
                } else if terms.iter().all(|term| title.contains(term)) {
                    2
                } else {
                    3
                }
            });
        }
        self.selected = keep
            .and_then(|target| {
                self.matches
                    .iter()
                    .position(|index| self.entries[*index].target == target)
            })
            .unwrap_or_else(|| self.selected.min(self.matches.len().saturating_sub(1)));
    }

    fn selected_entry(&self) -> Option<&Entry> {
        self.matches
            .get(self.selected)
            .and_then(|index| self.entries.get(*index))
    }

    fn selected_target(&self) -> Option<Target> {
        self.selected_entry().map(|entry| entry.target.clone())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Target> {
        let mut changed = false;
        match key.code {
            KeyCode::Esc => self.open = false,
            KeyCode::Enter => {
                if self.page_size == 0 {
                    return None;
                }
                let target = self.selected_target();
                if target.is_some() {
                    self.open = false;
                }
                return target;
            }
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => {
                self.selected = (self.selected + 1).min(self.matches.len().saturating_sub(1))
            }
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(self.page_size.max(1)),
            KeyCode::PageDown => {
                self.selected = (self.selected + self.page_size.max(1))
                    .min(self.matches.len().saturating_sub(1))
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.query.len(),
            KeyCode::Left => {
                self.cursor = self.query[..self.cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(index, _)| index)
            }
            KeyCode::Right => {
                self.cursor += self.query[self.cursor..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8)
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let previous = self.query[..self.cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(index, _)| index);
                self.query.drain(previous..self.cursor);
                self.cursor = previous;
                changed = true;
            }
            KeyCode::Delete if self.cursor < self.query.len() => {
                self.query.remove(self.cursor);
                changed = true;
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.query.clear();
                self.cursor = 0;
                changed = true;
            }
            KeyCode::Char(character)
                if !character.is_control()
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && self.query.chars().count() < 256 =>
            {
                self.query.insert(self.cursor, character);
                self.cursor += character.len_utf8();
                changed = true;
            }
            _ => {}
        }
        if changed {
            self.selected = 0;
            self.unavailable = false;
            self.filter(None);
        }
        None
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        self.hits.clear();
        let screen = frame.area();
        let width = screen.width.saturating_sub(4).min(100);
        let height = screen.height.saturating_sub(4).min(25);
        if width < 28 || height < 10 {
            self.page_size = 0;
            paint_opaque(frame, screen, Style::default().fg(INK).bg(PANEL));
            Paragraph::new("Find team\nEnlarge window\nEsc close")
                .render(screen, frame.buffer_mut());
            return;
        }
        let popup = Rect::new(
            screen.x + (screen.width - width) / 2,
            screen.y + (screen.height - height) / 2,
            width,
            height,
        );
        let style = Style::default().fg(INK).bg(PANEL);
        paint_opaque(frame, popup, style);
        Block::default()
            .title(" FIND YOUR TEAM ")
            .borders(Borders::ALL)
            .border_style(style.fg(ACCENT))
            .style(style)
            .render(popup, frame.buffer_mut());
        let inner = inset(popup, 1);
        let query_width = usize::from(inner.width.saturating_sub(3));
        let mut start = 0;
        while Line::from(&self.query[start..self.cursor]).width() >= query_width.max(1) {
            start += self.query[start..].chars().next().map_or(1, char::len_utf8);
        }
        let typed = short_path(&self.query[start..], query_width);
        let input = if self.query.is_empty() {
            "Project, task, provider or state…".to_string()
        } else {
            typed
        };
        Paragraph::new(Line::from(vec![
            Span::styled("/ ", style.fg(ACCENT)),
            Span::styled(
                short_path(&input, query_width),
                if self.query.is_empty() {
                    style.fg(MUTED)
                } else {
                    style
                },
            ),
        ]))
        .render(
            Rect::new(inner.x, inner.y, inner.width, 1),
            frame.buffer_mut(),
        );
        let results_y = inner.y + 2;
        self.page_size = usize::from(inner.height.saturating_sub(5) / 2).max(1);
        let first = self.selected / self.page_size * self.page_size;
        let summary = if self.unavailable {
            "That item left the tower. Choose another result.".to_string()
        } else if self.query.trim().is_empty() {
            format!(
                "{} projects · type to find conversations",
                self.matches.len()
            )
        } else {
            format!(
                "{} {} · {}–{}",
                self.matches.len(),
                if self.matches.len() == 1 {
                    "match"
                } else {
                    "matches"
                },
                if self.matches.is_empty() {
                    0
                } else {
                    first + 1
                },
                (first + self.page_size).min(self.matches.len())
            )
        };
        Paragraph::new(short_path(&summary, inner.width as usize))
            .style(style.fg(MUTED))
            .render(
                Rect::new(inner.x, inner.y + 1, inner.width, 1),
                frame.buffer_mut(),
            );
        if self.matches.is_empty() {
            let message = if self.entries.is_empty() {
                "No connected conversations yet.\nEsc to return, then c to connect sources."
            } else {
                "No matching projects or conversations.\nTry a task name, codex, claude, or attention."
            };
            Paragraph::new(message)
                .style(style.fg(MUTED))
                .wrap(ratatui::widgets::Wrap { trim: false })
                .render(
                    Rect::new(
                        inner.x,
                        results_y,
                        inner.width,
                        inner.height.saturating_sub(5),
                    ),
                    frame.buffer_mut(),
                );
        }
        for (row, matched) in self
            .matches
            .iter()
            .enumerate()
            .skip(first)
            .take(self.page_size)
        {
            let entry = &self.entries[*matched];
            let selected = row == self.selected;
            let row_style = style.bg(if selected { PANEL_HIGHLIGHT } else { PANEL });
            let y = results_y + ((row - first) * 2) as u16;
            self.hits.push(HitRegion::new(
                Rect::new(inner.x, y, inner.width, 2),
                match &entry.target {
                    Target::Project(id) => Action::EnterFloor(id.clone()),
                    Target::Worker(_, id) => Action::Inspect(id.clone()),
                },
            ));
            let badge_width = (entry.badge.len() + 2).min(usize::from(inner.width) / 2);
            let title_width = usize::from(inner.width).saturating_sub(badge_width + 2);
            let line = format!(
                "{} {}",
                if selected { ">" } else { " " },
                short_path(&entry.title, title_width)
            );
            Paragraph::new(line)
                .style(row_style.add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }))
                .render(Rect::new(inner.x, y, inner.width, 1), frame.buffer_mut());
            Paragraph::new(entry.badge)
                .style(row_style.fg(entry.status.map_or(ACCENT, status_color)))
                .render(
                    Rect::new(inner.right() - badge_width as u16, y, badge_width as u16, 1),
                    frame.buffer_mut(),
                );
            Paragraph::new(short_path(
                &format!("  {}", entry.context),
                inner.width as usize,
            ))
            .style(row_style.fg(MUTED))
            .render(
                Rect::new(inner.x, y + 1, inner.width, 1),
                frame.buffer_mut(),
            );
        }
        let path = self
            .selected_entry()
            .map_or("", |entry| entry.path.as_str());
        Paragraph::new(tail(path, inner.width as usize))
            .style(style.fg(MUTED))
            .render(
                Rect::new(inner.x, inner.bottom() - 2, inner.width, 1),
                frame.buffer_mut(),
            );
        Paragraph::new(short_path(
            "↑↓ choose · PgUp/Dn page · Enter open · Esc back · Ctrl+U clear",
            inner.width as usize,
        ))
        .style(style.fg(ACCENT))
        .render(
            Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
            frame.buffer_mut(),
        );
        let cursor_x = inner.x + 2 + Line::from(&self.query[start..self.cursor]).width() as u16;
        frame.set_cursor_position((cursor_x.min(inner.right() - 1), inner.y));
    }
}

pub(super) fn tail(text: &str, width: usize) -> String {
    if Line::from(text).width() <= width {
        return text.into();
    }
    let mut start = 0;
    while Line::from(&text[start..]).width() + 1 > width && start < text.len() {
        start += text[start..].chars().next().map_or(1, char::len_utf8);
    }
    format!("…{}", &text[start..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use theywork_core::{Agent, Event, EventKind};

    fn seen(path: &str, id: &str, title: &str) -> Event {
        Event {
            at: 0,
            office: OfficeId(path.into()),
            office_path: path.into(),
            worker: WorkerId(id.into()),
            agent: Agent::Codex,
            kind: EventKind::Seen {
                name: title.into(),
                git_branch: Some("feature/search".into()),
            },
        }
    }
    fn press(finder: &mut Finder, code: KeyCode) -> Option<Target> {
        finder.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }
    fn query(finder: &mut Finder, text: &str) {
        finder.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        for character in text.chars() {
            press(finder, KeyCode::Char(character));
        }
    }
    fn draw(finder: &mut Finder, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| finder.draw(frame)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn search_distinguishes_duplicate_projects_and_prioritizes_exact_task_titles() {
        let mut world = World::new();
        world.apply(seen("/clients/red/api", "red", "Fix login"));
        world.apply(seen("/clients/blue/api", "blue", "Fix login regression"));
        let mut finder = Finder::default();
        finder.show();
        finder.refresh(&world, 0);
        assert_eq!(
            finder.matches.len(),
            2,
            "empty search is a project directory"
        );
        query(&mut finder, "clients/red");
        assert_eq!(
            finder.matches.len(),
            2,
            "project and its worker share the path"
        );
        query(&mut finder, "fix login");
        assert_eq!(
            finder.selected_target(),
            Some(Target::Worker(
                OfficeId("/clients/red/api".into()),
                WorkerId("red".into())
            ))
        );
        query(&mut finder, "codex regression");
        assert_eq!(finder.matches.len(), 1);
        assert_eq!(
            finder.selected_entry().unwrap().title,
            "Fix login regression"
        );
        query(&mut finder, "feature/search");
        assert_eq!(finder.matches.len(), 2);
    }

    #[test]
    fn refresh_preserves_identity_when_live_results_shift() {
        let mut world = World::new();
        for index in 1..=20 {
            world.apply(seen(
                &format!("/project-{index:02}"),
                &format!("w{index}"),
                "Review tests",
            ));
        }
        let mut finder = Finder::default();
        finder.show();
        finder.refresh(&world, 0);
        draw(&mut finder, 80, 24);
        press(&mut finder, KeyCode::PageDown);
        let selected = finder.selected_target();
        world.apply(seen("/aaa", "new", "Review tests"));
        finder.refresh(&world, 0);
        assert_eq!(finder.selected_target(), selected);
        assert_eq!(press(&mut finder, KeyCode::Enter), selected);
        assert!(!finder.open);
    }

    #[test]
    fn unicode_editing_and_narrow_cursor_remain_in_bounds() {
        let mut finder = Finder::default();
        finder.show();
        query(&mut finder, "diseño界");
        press(&mut finder, KeyCode::Left);
        press(&mut finder, KeyCode::Backspace);
        assert_eq!(finder.query, "diseñ界");
        press(&mut finder, KeyCode::Delete);
        assert_eq!(finder.query, "diseñ");
        press(&mut finder, KeyCode::Home);
        press(&mut finder, KeyCode::Char('é'));
        assert_eq!(finder.query, "édiseñ");
        press(&mut finder, KeyCode::End);
        query(&mut finder, &"ñ界".repeat(120));
        for (width, height) in [(80, 24), (120, 36), (110, 80), (32, 14), (4, 4), (1, 1)] {
            let text = draw(&mut finder, width, height);
            if width >= 80 {
                assert!(text.contains("Esc back"));
            }
        }
        assert_eq!(
            press(&mut finder, KeyCode::Enter),
            None,
            "tiny view must not open an invisible selection"
        );
        press(&mut finder, KeyCode::Esc);
        assert!(!finder.open);
    }

    #[test]
    fn paste_is_bounded_text_and_never_a_navigation_key() {
        let mut finder = Finder::default();
        finder.show();
        finder.paste("qcs?\n界\u{1b}");
        assert_eq!(finder.query, "qcs? 界");
        assert!(finder.open);
        press(&mut finder, KeyCode::Home);
        finder.paste("API ");
        assert_eq!(finder.query, "API qcs? 界");
        finder.paste(&"x".repeat(300));
        assert_eq!(finder.query.chars().count(), 256);
    }

    #[test]
    fn quiet_turn_does_not_match_a_request_for_approval() {
        let mut world = World::new();
        let mut event = seen("/project", "quiet", "Inspect tests");
        world.apply(event.clone());
        event.kind = EventKind::Turn { in_flight: true };
        world.apply(event);
        let mut finder = Finder::default();
        finder.show();
        finder.refresh(&world, theywork_core::BLOCKED_AFTER_MS + 1);
        query(&mut finder, "attention");
        assert_eq!(finder.matches.len(), 1);
        query(&mut finder, "approval");
        assert!(finder.matches.is_empty());
    }

    #[test]
    fn aliases_and_real_titles_both_find_the_same_stable_task_target() {
        let mut world = World::new();
        world.apply(seen("/project", "worker", "Repair retry handling"));
        let profiles = BTreeMap::from([(
            "worker".into(),
            CharacterProfile {
                name: "Avery".into(),
                ..Default::default()
            },
        )]);
        let mut finder = Finder::default();
        finder.show();
        finder.refresh_with_profiles(&world, 0, &profiles);
        for text in ["Avery", "retry handling"] {
            query(&mut finder, text);
            assert_eq!(finder.matches.len(), 1);
            assert!(finder.selected_entry().unwrap().title.contains("Avery"));
            assert!(finder
                .selected_entry()
                .unwrap()
                .title
                .contains("Repair retry"));
            draw(&mut finder, 80, 24);
            assert!(finder
                .hit_regions()
                .iter()
                .any(|hit| hit.action == Action::Inspect(WorkerId("worker".into()))));
        }
        draw(&mut finder, 4, 4);
        assert!(finder.hit_regions().is_empty());
    }
}
