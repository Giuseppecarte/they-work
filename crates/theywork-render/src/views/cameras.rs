//! The software tower: a stable floor directory beside the selected project.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use theywork_core::{Millis, Office, WorkerStatus, World};

use super::{
    below_tab_bar, draw_footer, draw_header, draw_tiny, has_area, inset, paint_opaque, short_path,
    status_color, worker_status, ACCENT, BACKGROUND, HOT, INK, MUTED, PANEL, PANEL_HIGHLIGHT,
    WARNING,
};
use crate::canvas::Canvas;
use crate::interaction::{Action, HitRegion};
use crate::sprite::SpriteSet;

/// Empty observations have different causes and different next steps. Keep this
/// visible in the workspace rather than making people infer it from diagnostics.
pub(crate) fn draw_empty(
    frame: &mut Frame,
    area: Rect,
    observation: &crate::observation::ObservationSummary,
) -> Vec<HitRegion> {
    let area = area.intersection(frame.area());
    if area.width == 0 || area.height == 0 {
        return Vec::new();
    }
    paint_opaque(frame, area, Style::default().fg(INK).bg(BACKGROUND));
    let (title, detail, primary, action) = if observation.enabled_sources == 0 {
        ("YOUR TOWER STARTS HERE", "No sources connected. Choose where to find your conversations, or create a task using an available provider.", "Connect sources", Action::Sources)
    } else if observation.scanning {
        ("LOOKING FOR YOUR TEAM", "Reading the selected local sources. Your projects will appear as floors when conversations are found.", "Connections", Action::Connections)
    } else if !observation.errors.is_empty() {
        ("SOURCE NEEDS ATTENTION", "A local source could not be read. Check its folder and connection; an empty tower does not mean there is no work.", "Check sources", Action::Sources)
    } else if observation.filtered {
        ("NO CONVERSATIONS IN THIS FILTER", "No conversations match the selected project filter. Start without --project to show all selected sources.", "Check sources", Action::Sources)
    } else {
        ("YOUR TEAM HAS NOT ARRIVED YET", "The selected sources contain no conversations yet. Create a task here, or start one in your original agent app.", "New task", Action::NewTask)
    };
    let inner = if area.width > 4 { inset(area, 1) } else { area };
    if !has_area(inner) {
        return Vec::new();
    }
    Paragraph::new(short_path(title, inner.width.into()))
        .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .render(
            Rect::new(inner.x, inner.y, inner.width, 1),
            frame.buffer_mut(),
        );
    let mut action_y = inner.bottom().saturating_sub(1);
    if inner.height > 3 {
        let mut body = detail.to_string();
        if let Some(error) = observation.errors.first() {
            body.push_str("\n\n");
            body.push_str(&super::safe_display(error));
            if observation.errors.len() > 1 {
                body.push_str(&format!(
                    "\n{} additional source errors.",
                    observation.errors.len() - 1
                ));
            }
        }
        let rows = wrapped_rows(&body, inner.width).min(inner.height - 3);
        action_y = (inner.y + 3 + rows).min(action_y);
        Paragraph::new(body)
            .style(Style::default().fg(INK))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .render(
                Rect::new(inner.x, inner.y + 2, inner.width, rows),
                frame.buffer_mut(),
            );
    }
    if action_y <= inner.y {
        return Vec::new();
    }
    let label = format!("[{primary}]");
    let button = Rect::new(inner.x, action_y, inner.width.min(label.len() as u16), 1);
    Paragraph::new(short_path(&label, button.width.into()))
        .style(Style::default().fg(ACCENT).bg(PANEL_HIGHLIGHT))
        .render(button, frame.buffer_mut());
    vec![HitRegion::new(button, action)]
}

fn wrapped_rows(text: &str, width: u16) -> u16 {
    let width = usize::from(width).max(1);
    text.split('\n')
        .map(|line| {
            let mut rows = 1usize;
            let mut used = 0usize;
            for word in line.split_whitespace() {
                let length = Line::from(word).width();
                if used > 0 && used + 1 + length > width {
                    rows += 1;
                    used = 0;
                }
                if used > 0 {
                    used += 1;
                }
                used += length;
                if used > width {
                    rows += (used - 1) / width;
                    used = (used - 1) % width + 1;
                }
            }
            rows
        })
        .sum::<usize>()
        .min(u16::MAX as usize) as u16
}

/// The directory is one column; rows are its visible project capacity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GridLayout {
    pub columns: usize,
    pub rows: usize,
}

pub fn grid_layout(office_count: usize, width: u16, height: u16) -> GridLayout {
    if office_count == 0 || width == 0 || height == 0 {
        return GridLayout::default();
    }
    let capacity = if width >= 110 {
        height.saturating_sub(2) as usize / 3
    } else {
        (height as usize / 3).clamp(2, 7).saturating_sub(1)
    };
    GridLayout {
        columns: 1,
        rows: office_count.min(capacity.max(1)),
    }
}

/// Canonical path order gives each project a stable floor number.
pub(crate) fn ordered_offices(world: &World, _now: Millis) -> Vec<&Office> {
    world.offices().collect()
}

#[derive(Default)]
struct StatusCounts {
    running: usize,
    idle: usize,
    blocked: usize,
    failed: usize,
    unavailable: usize,
    stale: usize,
}
impl StatusCounts {
    fn for_office(office: &Office, now: Millis) -> Self {
        let mut result = Self::default();
        result.add_office(office, now);
        result
    }
    fn add_office(&mut self, office: &Office, now: Millis) {
        for worker in &office.workers {
            match crate::presentation::state_label(worker, now) {
                "Source unavailable" => {
                    self.unavailable += 1;
                    continue;
                }
                "Last known state" => {
                    self.stale += 1;
                    continue;
                }
                _ => {}
            }
            match worker_status(worker, now) {
                WorkerStatus::Running => self.running += 1,
                WorkerStatus::Idle => self.idle += 1,
                WorkerStatus::Blocked => self.blocked += 1,
                WorkerStatus::Failed => self.failed += 1,
            }
        }
    }
    fn color(&self) -> ratatui::style::Color {
        if self.blocked > 0 {
            WARNING
        } else if self.failed > 0 {
            HOT
        } else if self.running > 0 {
            ACCENT
        } else {
            MUTED
        }
    }
    fn marker(&self) -> &'static str {
        if self.unavailable > 0 || self.stale > 0 {
            "~"
        } else if self.blocked > 0 {
            "!"
        } else if self.failed > 0 {
            "×"
        } else if self.running > 0 {
            "▸"
        } else {
            "·"
        }
    }
    fn compact(&self) -> String {
        let mut parts = Vec::new();
        if self.unavailable > 0 {
            parts.push(format!("{} offline", self.unavailable));
        }
        if self.stale > 0 {
            parts.push(format!("{} stale", self.stale));
        }
        if self.blocked > 0 {
            parts.push(format!("!{} attention", self.blocked));
        }
        if self.failed > 0 {
            parts.push(format!("×{} failed", self.failed));
        }
        parts.push(format!("{} working · {} idle", self.running, self.idle));
        parts.join(" · ")
    }
    fn line(&self) -> Line<'static> {
        let mut line = vec![
            Span::styled(
                format!("  {} working", self.running),
                Style::default().fg(ACCENT),
            ),
            Span::styled(format!(" · {} idle", self.idle), Style::default().fg(MUTED)),
            Span::styled(
                format!(" · {} attention", self.blocked),
                Style::default().fg(if self.blocked > 0 { WARNING } else { MUTED }),
            ),
            Span::styled(
                format!(" · {} failed", self.failed),
                Style::default().fg(if self.failed > 0 { HOT } else { MUTED }),
            ),
        ];
        if self.unavailable > 0 || self.stale > 0 {
            line.insert(
                0,
                Span::styled(
                    format!(" {} offline · {} stale ·", self.unavailable, self.stale),
                    Style::default().fg(WARNING),
                ),
            );
        }
        Line::from(line)
    }
}

pub(crate) fn draw(
    frame: &mut Frame,
    world: &World,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    now: Millis,
    selected: usize,
    _all_selected: bool,
) -> GridLayout {
    let area = below_tab_bar(frame.area());
    if area.width < 16 || area.height < 6 {
        draw_tiny(frame, "they-work · enlarge the terminal to see the tower");
        return GridLayout::default();
    }
    let offices = ordered_offices(world, now);
    let mut counts = StatusCounts::default();
    for office in &offices {
        counts.add_office(office, now);
    }
    let (header, body, footer) = super::vertical_bands(area, 2, 2);
    draw_header(
        frame,
        Rect::new(header.x, header.y, header.width, 1),
        "SOFTWARE TOWER",
        &format!(
            "{} {} · {} {}",
            offices.len(),
            if offices.len() == 1 {
                "floor"
            } else {
                "floors"
            },
            world.worker_count(),
            if world.worker_count() == 1 {
                "worker"
            } else {
                "workers"
            }
        ),
    );
    if header.height > 1 {
        Paragraph::new(counts.line())
            .style(Style::default().bg(BACKGROUND))
            .render(
                Rect::new(header.x, header.y + 1, header.width, 1),
                frame.buffer_mut(),
            );
    }
    draw_footer(
        frame,
        footer,
        "↑↓ floors · Enter visit · ! attention · / find · c sources · ? help",
    );
    if !has_area(body) {
        return GridLayout::default();
    }
    if offices.is_empty() {
        Paragraph::new("YOUR TOWER STARTS HERE\n\nEach project is a floor. Each conversation is a worker.\n\nPress c to choose local sources, then start a conversation in a project.")
            .style(Style::default().fg(INK).bg(BACKGROUND)).wrap(ratatui::widgets::Wrap {trim:false}).render(inset(body,1),frame.buffer_mut());
        return GridLayout::default();
    }
    let selected = selected.min(offices.len() - 1);
    let layout = grid_layout(offices.len(), body.width, body.height);
    let capacity = layout.rows.max(1);
    let first = selected / capacity * capacity;
    let visible_count = capacity.min(offices.len() - first);
    let wide = body.width >= 110;
    let (directory, focus) = if wide {
        let dw = (body.width / 3).clamp(34, 48);
        (
            Rect::new(
                body.x,
                body.y,
                dw,
                body.height.min(visible_count as u16 * 3 + 2),
            ),
            Rect::new(
                body.x + dw + 1,
                body.y,
                (body.width - dw - 1).min(80 + offices[selected].workers.len().min(5) as u16 * 16),
                body.height
                    .min(23 + offices[selected].workers.len().min(6) as u16 * 3),
            ),
        )
    } else {
        let dh = (capacity as u16 + 2).min(body.height.saturating_sub(5));
        (
            Rect::new(body.x, body.y, body.width, dh),
            Rect::new(body.x, body.y + dh, body.width, body.height - dh),
        )
    };
    draw_directory(
        frame, directory, &offices, first, capacity, selected, now, wide,
    );
    draw_focus(
        frame,
        focus,
        canvas,
        sprites,
        offices[selected],
        selected + 1,
        now,
    );
    if footer.height > 1 {
        let text = format!(
            "  Floor {}/{} · page {}/{} · PgUp/PgDn pages · Home first · End last",
            selected + 1,
            offices.len(),
            first / capacity + 1,
            offices.len().div_ceil(capacity)
        );
        Paragraph::new(short_path(&text, footer.width as usize))
            .style(Style::default().fg(MUTED).bg(BACKGROUND))
            .render(
                Rect::new(footer.x, footer.y + 1, footer.width, 1),
                frame.buffer_mut(),
            );
    }
    layout
}

#[allow(clippy::too_many_arguments)]
fn draw_directory(
    frame: &mut Frame,
    area: Rect,
    offices: &[&Office],
    first: usize,
    capacity: usize,
    selected: usize,
    now: Millis,
    wide: bool,
) {
    Block::default()
        .title(" FLOORS / PROJECTS ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MUTED))
        .style(Style::default().bg(PANEL))
        .render(area, frame.buffer_mut());
    let inner = inset(area, 1);
    if !has_area(inner) {
        return;
    }
    for (index, office) in offices.iter().enumerate().skip(first).take(capacity) {
        let y = inner.y + ((index - first) * if wide { 3 } else { 1 }) as u16;
        if y >= inner.bottom() {
            break;
        }
        let counts = StatusCounts::for_office(office, now);
        let active = index == selected;
        let style = Style::default()
            .fg(if active { INK } else { MUTED })
            .bg(if active { PANEL_HIGHLIGHT } else { PANEL });
        let row = Rect::new(
            inner.x,
            y,
            inner.width,
            if wide { 2.min(inner.bottom() - y) } else { 1 },
        );
        paint_opaque(frame, row, style);
        let number = format!("{} F{:02}", if active { ">" } else { " " }, index + 1);
        let (name_width, summary) = if wide || inner.width < 30 {
            (inner.width.saturating_sub(9) as usize, None)
        } else {
            let summary = counts.compact();
            let max_summary = (inner.width / 2).max(18) as usize;
            (
                inner.width.saturating_sub(10 + max_summary as u16) as usize,
                Some(short_path(&summary, max_summary)),
            )
        };
        let mut spans = vec![
            Span::styled(number, style.fg(if active { ACCENT } else { MUTED })),
            Span::styled(format!(" {} ", counts.marker()), style.fg(counts.color())),
            Span::styled(
                format!(
                    "{:<width$}",
                    short_path(&office.name, name_width),
                    width = name_width
                ),
                style,
            ),
        ];
        if let Some(summary) = summary {
            spans.push(Span::styled(
                format!(" {summary}"),
                style.fg(counts.color()),
            ));
        }
        Paragraph::new(Line::from(spans))
            .style(style)
            .render(Rect::new(inner.x, y, inner.width, 1), frame.buffer_mut());
        if wide && row.height > 1 {
            Paragraph::new(short_path(
                &format!(
                    "     {} {} · {}",
                    office.workers.len(),
                    if office.workers.len() == 1 {
                        "worker"
                    } else {
                        "workers"
                    },
                    counts.compact()
                ),
                inner.width as usize,
            ))
            .style(style.fg(counts.color()))
            .render(
                Rect::new(inner.x, y + 1, inner.width, 1),
                frame.buffer_mut(),
            );
            if y + 2 < inner.bottom() {
                Paragraph::new("─".repeat(inner.width as usize))
                    .style(Style::default().fg(super::WALL).bg(PANEL))
                    .render(
                        Rect::new(inner.x, y + 2, inner.width, 1),
                        frame.buffer_mut(),
                    );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_focus(
    frame: &mut Frame,
    area: Rect,
    canvas: &mut Canvas,
    sprites: &SpriteSet,
    office: &Office,
    floor: usize,
    now: Millis,
) {
    if !has_area(area) {
        return;
    }
    let counts = StatusCounts::for_office(office, now);
    let inner = super::draw_panel(
        frame,
        area,
        &format!(
            " F{floor} · {} ",
            short_path(&office.name, area.width.saturating_sub(13) as usize)
        ),
        true,
    );
    if inner.height < 3 {
        return;
    }
    let stats = counts.compact();
    Paragraph::new(stats)
        .style(Style::default().fg(counts.color()).bg(PANEL))
        .render(
            Rect::new(inner.x, inner.y, inner.width, 1),
            frame.buffer_mut(),
        );
    let path_rows = usize::from(inner.height >= 9) as u16;
    if path_rows > 0 {
        Paragraph::new(path_tail(
            &office.path,
            inner.width.saturating_sub(1) as usize,
        ))
        .style(Style::default().fg(MUTED).bg(PANEL))
        .render(
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
            frame.buffer_mut(),
        );
    }
    let available = inner.height.saturating_sub(1 + path_rows);
    let scene_height = if available >= 20 {
        available
            .saturating_sub(1 + office.workers.len().min(6) as u16 * 3)
            .clamp(12, 18)
    } else {
        available.saturating_sub(2).max(1)
    };
    let scene = Rect::new(inner.x, inner.y + 1 + path_rows, inner.width, scene_height);
    canvas.resize_for_cells(scene.width as usize, scene.height as usize);
    let markers = super::guard_scene::draw(canvas, office, sprites, now);
    canvas.render(frame.buffer_mut(), scene);
    let roster_y = scene.bottom();
    let remaining = inner.bottom().saturating_sub(roster_y);
    if remaining == 0 {
        return;
    }
    let workers = super::guard_scene::ranked_workers(office, now);
    let stride = if remaining as usize > workers.len().min(6) * 3 {
        3
    } else {
        1
    };
    let listed = workers
        .len()
        .min(remaining.saturating_sub(1) as usize / stride);
    let extra = if markers.len() < workers.len() {
        format!(" · {} characters shown", markers.len())
    } else {
        String::new()
    };
    let title = if listed < workers.len() {
        format!(
            " TEAM · {listed} of {} listed (+{} more) · Enter visit",
            workers.len(),
            workers.len() - listed
        )
    } else {
        format!(
            " TEAM · {} {}{} · Enter visit",
            workers.len(),
            if workers.len() == 1 {
                "conversation"
            } else {
                "conversations"
            },
            extra
        )
    };
    Paragraph::new(title)
        .style(
            Style::default()
                .fg(INK)
                .bg(PANEL)
                .add_modifier(Modifier::BOLD),
        )
        .render(
            Rect::new(inner.x, roster_y, inner.width, 1),
            frame.buffer_mut(),
        );
    for (index, worker) in workers.iter().take(listed).enumerate() {
        let status = worker_status(worker, now);
        let label = match status {
            WorkerStatus::Running => "WORKING",
            WorkerStatus::Idle => "IDLE",
            WorkerStatus::Blocked => "NEEDS HELP",
            WorkerStatus::Failed => "FAILED",
        };
        let width = inner.width.saturating_sub(14) as usize;
        let y = roster_y + 1 + (index * stride) as u16;
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {:<11}", label),
                Style::default().fg(status_color(status)),
            ),
            Span::styled(short_path(&worker.name, width), Style::default().fg(INK)),
        ]))
        .style(Style::default().bg(PANEL))
        .render(Rect::new(inner.x, y, inner.width, 1), frame.buffer_mut());
        if stride > 1 {
            let summary = super::desk::inspection_summary(worker, now);
            let prefix = match status {
                WorkerStatus::Running => "Latest: ",
                WorkerStatus::Idle => "Last update: ",
                _ => "Reason: ",
            };
            Paragraph::new(format!(
                "             {}",
                short_path(&format!("{prefix}{}", summary.detail), width)
            ))
            .style(Style::default().fg(MUTED).bg(PANEL))
            .render(
                Rect::new(inner.x, y + 1, inner.width, 1),
                frame.buffer_mut(),
            );
        }
    }
}

fn path_tail(path: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let safe = super::safe_display(path);
    if Line::from(safe.as_str()).width() <= width {
        return safe;
    }
    let mut tail = String::new();
    for character in safe.chars().rev() {
        let candidate = format!("{character}{tail}");
        if Line::from(candidate.as_str()).width() + 1 > width {
            break;
        }
        tail = candidate;
    }
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_states_explain_the_cause_and_keep_a_visible_next_action() {
        use crate::observation::ObservationSummary;
        use ratatui::{backend::TestBackend, Terminal};
        let cases = [
            (
                ObservationSummary {
                    enabled_sources: 0,
                    ..Default::default()
                },
                "YOUR TOWER",
                Action::Sources,
            ),
            (
                ObservationSummary {
                    scanning: true,
                    ..Default::default()
                },
                "LOOKING",
                Action::Connections,
            ),
            (
                ObservationSummary {
                    errors: vec!["Folder unavailable".into()],
                    ..Default::default()
                },
                "SOURCE NEEDS",
                Action::Sources,
            ),
            (
                ObservationSummary {
                    filtered: true,
                    ..Default::default()
                },
                "NO CONVERSATIONS",
                Action::Sources,
            ),
            (ObservationSummary::default(), "YOUR TEAM", Action::NewTask),
        ];
        for (summary, expected, action) in cases {
            for (width, height) in [(32, 14), (80, 24), (192, 58)] {
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let area = Rect::new(1, 2, width - 2, height - 3);
                let mut hits = Vec::new();
                terminal
                    .draw(|frame| hits = draw_empty(frame, area, &summary))
                    .unwrap();
                assert_eq!(hits.len(), 1);
                assert_eq!(hits[0].action, action);
                assert_eq!(hits[0].area.intersection(area), hits[0].area);
                let text = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                assert!(text.contains(expected), "{width}x{height}: {text}");
                assert!(text.contains('['));
            }
        }
    }
    use theywork_core::{Activity, Agent, Event, EventKind, OfficeId, WorkerId, BLOCKED_AFTER_MS};

    fn event(office: &str, worker: &str, at: Millis, kind: EventKind) -> Event {
        Event {
            at,
            office: OfficeId(office.to_string()),
            office_path: office.to_string(),
            worker: WorkerId(worker.to_string()),
            agent: Agent::Codex,
            kind,
        }
    }

    fn status_world() -> World {
        let mut world = World::new();
        world.apply(event(
            "/a-failed",
            "failed",
            0,
            EventKind::Acted(Activity::Error {
                detail: "compiler stopped".into(),
            }),
        ));
        world.apply(event(
            "/b-plain",
            "plain",
            0,
            EventKind::Seen {
                name: "Plain worker".into(),
                git_branch: None,
            },
        ));
        world.apply(event(
            "/y-blocked",
            "blocked",
            0,
            EventKind::Turn { in_flight: true },
        ));
        world.apply(event(
            "/y-blocked",
            "blocked",
            0,
            EventKind::Acted(Activity::Waiting {
                detail: "approve deploy".into(),
            }),
        ));
        world.apply(event(
            "/z-blocked-later",
            "blocked-later",
            0,
            EventKind::Turn { in_flight: true },
        ));
        world.apply(event(
            "/z-blocked-later",
            "blocked-later",
            0,
            EventKind::Acted(Activity::Typing {
                detail: "cargo test".into(),
            }),
        ));
        world
    }
    #[test]
    fn floor_numbers_do_not_change_when_a_worker_needs_attention() {
        let world = status_world();
        let offices = ordered_offices(&world, BLOCKED_AFTER_MS + 1);
        let names = offices
            .iter()
            .map(|office| office.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            ["a-failed", "b-plain", "y-blocked", "z-blocked-later"]
        );
        assert_eq!(
            worker_status(&offices[2].workers[0], BLOCKED_AFTER_MS + 1),
            WorkerStatus::Blocked
        );
        assert_eq!(
            worker_status(&offices[0].workers[0], BLOCKED_AFTER_MS + 1),
            WorkerStatus::Failed
        );
    }

    #[test]
    fn tower_directory_stacks_projects_in_one_column() {
        assert_eq!(
            grid_layout(6, 160, 44),
            GridLayout {
                columns: 1,
                rows: 6,
            }
        );
        assert_eq!(
            grid_layout(4, 160, 44),
            GridLayout {
                columns: 1,
                rows: 4,
            }
        );
    }
    #[test]
    fn tower_exposes_attention_and_last_floor_without_changing_identity() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut world = status_world();
        for index in 0..16 {
            world.apply(event(
                &format!("/project-{index:02}/same-name"),
                &format!("worker-{index}"),
                0,
                EventKind::Seen {
                    name: format!("Task {index}"),
                    git_branch: None,
                },
            ));
        }
        let sprites = SpriteSet::new();
        for (width, height) in [(80, 24), (120, 32), (192, 58)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut canvas = Canvas::new(0, 0);
            let mut layout = GridLayout::default();
            terminal
                .draw(|frame| {
                    layout = draw(
                        frame,
                        &world,
                        &mut canvas,
                        &sprites,
                        BLOCKED_AFTER_MS + 1,
                        19,
                        false,
                    );
                })
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert_eq!(layout.columns, 1);
            assert!(text.contains("20/20"));
            assert!(text.contains("F20"));
            assert!(text.contains("NEEDS HELP"));
            assert!(!text.contains("REC"));
            assert!(!text.contains("t+"));
            assert!(text.contains("/ find"));
            if width >= 110 {
                assert!(text.contains("No recent activity. No approval was identified."));
            }
        }
    }

    #[test]
    fn directory_attention_survives_a_narrow_row() {
        use ratatui::{backend::TestBackend, Terminal};
        let world = status_world();
        let offices = ordered_offices(&world, 0);
        let mut terminal = Terminal::new(TestBackend::new(80, 8)).unwrap();
        terminal
            .draw(|frame| {
                draw_directory(frame, Rect::new(0, 0, 80, 8), &offices, 0, 4, 2, 0, false)
            })
            .unwrap();
        let rows = (0..8)
            .map(|y| {
                (0..80)
                    .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert!(rows
            .iter()
            .any(|row| row.contains("y-blocked") && row.contains("!1 attention")));
        assert!(rows
            .iter()
            .any(|row| row.contains("a-failed") && row.contains("×1 failed")));
    }

    #[test]
    fn compact_team_preview_reports_the_unlisted_conversations() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut world = World::new();
        for index in 0..3 {
            world.apply(event(
                "/project",
                &index.to_string(),
                0,
                EventKind::Seen {
                    name: format!("Task {index}"),
                    git_branch: None,
                },
            ));
        }
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| {
                draw(
                    frame,
                    &world,
                    &mut Canvas::new(0, 0),
                    &SpriteSet::new(),
                    0,
                    0,
                    false,
                );
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("1 of 3 listed (+2 more)"));
    }

    #[test]
    fn project_paths_preserve_the_distinguishing_end_at_small_widths() {
        assert_eq!(path_tail("/work/client-a/app", 12), "…lient-a/app");
        assert_ne!(
            path_tail("/work/client-a/app", 12),
            path_tail("/work/client-b/app", 12)
        );
        for width in 0..20 {
            assert!(Line::from(path_tail("/路径/客户-a/app", width)).width() <= width);
        }
    }
}
