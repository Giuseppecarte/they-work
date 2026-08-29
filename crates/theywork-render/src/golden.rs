//! Deterministic golden-frame support for the renderer.
//!
//! Each cell is written as `symbol|foreground|background`, one row per line.
//! Goldens are intentionally test-only: the renderer remains I/O-free at runtime.
//! To deliberately accept an art change, run:
//!
//! `THEYWORK_UPDATE_GOLDEN=1 cargo test -p theywork-render golden::tests::snapshots_match_checked_in_goldens`
//!
//! Review the resulting files under `tests/goldens/` before committing them.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;
use theywork_core::{
    Activity, Agent, Event, EventKind, Millis, OfficeId, WorkerId, World, BLOCKED_AFTER_MS,
};

use crate::canvas::{Canvas, ColorDepth};
use crate::views::UiTheme;
use crate::{Ui, View};

const SNAPSHOT_NOW: Millis = BLOCKED_AFTER_MS + 12_000;
const NORMAL_SIZE: (u16, u16) = (80, 24);
const SMALL_SIZE: (u16, u16) = (32, 12);

#[derive(Debug, Clone, Copy)]
pub(crate) enum SnapshotView {
    Cameras,
    Guard,
    Office,
    Desk,
    Phone,
    Help,
    Settings,
}

impl SnapshotView {
    fn name(self) -> &'static str {
        match self {
            Self::Cameras => "cameras",
            Self::Guard => "guard",
            Self::Office => "office",
            Self::Desk => "desk",
            Self::Phone => "phone",
            Self::Help => "help",
            Self::Settings => "settings",
        }
    }
}
fn theme_name(theme: UiTheme) -> &'static str {
    match theme {
        UiTheme::Dark => "dark",
        UiTheme::Light => "light",
    }
}

const CASES: [(SnapshotView, &str); 7] = [
    (SnapshotView::Cameras, "cameras"),
    (SnapshotView::Guard, "guard"),
    (SnapshotView::Office, "office"),
    (SnapshotView::Desk, "desk"),
    (SnapshotView::Phone, "phone"),
    (SnapshotView::Help, "help"),
    (SnapshotView::Settings, "settings"),
];

const SIZES: [(&str, (u16, u16)); 2] = [("normal", NORMAL_SIZE), ("small", SMALL_SIZE)];

pub(crate) fn render_snapshot(view: SnapshotView, size: (u16, u16), theme: UiTheme) -> String {
    let world = snapshot_world();
    render_view(&world, view, size, SNAPSHOT_NOW, theme)
}

fn render_view(
    world: &World,
    view: SnapshotView,
    size: (u16, u16),
    now: Millis,
    theme: UiTheme,
) -> String {
    let mut ui = configured_ui(view, now, theme);
    let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).expect("snapshot terminal");
    terminal
        .draw(|frame| ui.draw(frame, world))
        .expect("snapshot frame");
    serialize_buffer(view, size, now, theme, terminal.backend().buffer())
}

fn configured_ui(view: SnapshotView, now: Millis, theme: UiTheme) -> Ui {
    let mut ui = Ui::new();
    // Snapshots use a canonical color depth so the files do not depend on the
    // environment in which the test suite happens to run.
    ui.canvas = Canvas::with_color_depth(0, 0, ColorDepth::TrueColor);
    ui.theme = theme;
    ui.tick(now);
    match view {
        SnapshotView::Cameras => {
            ui.view = View::Cameras;
        }
        SnapshotView::Guard => {
            ui.view = View::Cameras;
            ui.guard_all = true;
        }
        SnapshotView::Office => {
            ui.view = View::Office;
        }
        SnapshotView::Desk => {
            ui.view = View::Desk;
        }
        SnapshotView::Phone => {
            ui.view = View::Cameras;
            ui.phone_open = true;
            ui.phone_channel = crate::views::phone::PhoneChannel::Blocked;
            ui.phone_transition_at = 0;
        }
        SnapshotView::Help => {
            ui.view = View::Office;
            ui.selected_office = 5;
            ui.help_open = true;
        }
        SnapshotView::Settings => {
            ui.view = View::Office;
            ui.settings_open = true;
            ui.settings_cursor = 0;
        }
    }
    ui
}

fn serialize_buffer(
    view: SnapshotView,
    size: (u16, u16),
    now: Millis,
    theme: UiTheme,
    buffer: &Buffer,
) -> String {
    let mut output = String::new();
    writeln!(output, "they-work golden v1").expect("string write");
    writeln!(
        output,
        "view={} theme={} size={}x{} now={} depth=truecolor",
        view.name(),
        theme_name(theme),
        size.0,
        size.1,
        now
    )
    .expect("string write");

    let width = size.0 as usize;
    let height = size.1 as usize;
    for row in 0..height {
        write!(output, "{row:03}:").expect("string write");
        for column in 0..width {
            output.push(' ');
            if let Some(cell) = buffer
                .content
                .get(row.saturating_mul(width).saturating_add(column))
            {
                write_cell(&mut output, cell.symbol(), cell.fg, cell.bg);
            } else {
                output.push('?');
            }
        }
        output.push('\n');
    }
    output
}

fn write_cell(output: &mut String, symbol: &str, foreground: Color, background: Color) {
    write_symbol(output, symbol);
    output.push('|');
    write_color(output, foreground);
    output.push('|');
    write_color(output, background);
}

fn write_symbol(output: &mut String, symbol: &str) {
    if symbol.is_empty() {
        output.push('∅');
        return;
    }
    for character in symbol.chars() {
        match character {
            ' ' => output.push('·'),
            '\\' => output.push_str("\\\\"),
            '|' => output.push_str("\\|"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            character if character.is_control() => {
                write!(output, "\\u{{{:x}}}", character as u32).expect("string write");
            }
            character => output.push(character),
        }
    }
}

fn write_color(output: &mut String, color: Color) {
    match color {
        Color::Reset => output.push_str("reset"),
        Color::Black => output.push_str("black"),
        Color::Red => output.push_str("red"),
        Color::Green => output.push_str("green"),
        Color::Yellow => output.push_str("yellow"),
        Color::Blue => output.push_str("blue"),
        Color::Magenta => output.push_str("magenta"),
        Color::Cyan => output.push_str("cyan"),
        Color::Gray => output.push_str("gray"),
        Color::DarkGray => output.push_str("dark-gray"),
        Color::LightRed => output.push_str("light-red"),
        Color::LightGreen => output.push_str("light-green"),
        Color::LightYellow => output.push_str("light-yellow"),
        Color::LightBlue => output.push_str("light-blue"),
        Color::LightMagenta => output.push_str("light-magenta"),
        Color::LightCyan => output.push_str("light-cyan"),
        Color::White => output.push_str("white"),
        Color::Indexed(index) => write!(output, "indexed({index})").expect("string write"),
        Color::Rgb(red, green, blue) => {
            write!(output, "rgb({red},{green},{blue})").expect("string write");
        }
    }
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("goldens")
        .join(name)
}

fn update_requested() -> bool {
    std::env::var("THEYWORK_UPDATE_GOLDEN").ok().as_deref() == Some("1")
}

fn assert_golden(name: &str, actual: &str) {
    let path = golden_path(name);
    if update_requested() {
        fs::create_dir_all(path.parent().expect("golden directory"))
            .expect("create golden directory");
        fs::write(&path, actual).expect("write golden");
        return;
    }

    let expected = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "missing golden {name} at {}: {error}; regenerate deliberately with THEYWORK_UPDATE_GOLDEN=1",
            path.display()
        )
    });
    if expected != actual {
        panic!(
            "golden mismatch for {name}\n{}\nregenerate deliberately with THEYWORK_UPDATE_GOLDEN=1",
            readable_diff(&expected, actual)
        );
    }
}

fn readable_diff(expected: &str, actual: &str) -> String {
    let expected_lines = expected.lines().collect::<Vec<_>>();
    let actual_lines = actual.lines().collect::<Vec<_>>();
    let line_count = expected_lines.len().max(actual_lines.len());
    let mut output = String::new();
    let mut changes = 0usize;

    for line in 0..line_count {
        let expected_line = expected_lines.get(line).copied().unwrap_or("<missing>");
        let actual_line = actual_lines.get(line).copied().unwrap_or("<missing>");
        if expected_line == actual_line {
            continue;
        }
        changes = changes.saturating_add(1);
        if changes <= 80 {
            writeln!(output, "-{line:03}: {expected_line}").expect("string write");
            writeln!(output, "+{line:03}: {actual_line}").expect("string write");
        }
    }
    if changes > 80 {
        writeln!(output, "... {} more changed rows", changes - 80).expect("string write");
    }
    output
}

fn fixture_event(
    at: Millis,
    office: &str,
    worker: &WorkerId,
    agent: Agent,
    kind: EventKind,
) -> Event {
    Event {
        at,
        office: OfficeId(office.to_string()),
        office_path: office.to_string(),
        worker: worker.clone(),
        agent,
        kind,
    }
}

fn snapshot_world() -> World {
    const OFFICES: [(&str, usize); 6] = [
        ("/golden/sustain", 5),
        ("/golden/giin-jalisco", 5),
        ("/golden/gamma-research", 4),
        ("/golden/delta-design", 3),
        ("/golden/epsilon-tools", 2),
        ("/golden/zeta-lab", 1),
    ];

    let mut world = World::new();
    for (office_index, (office, worker_count)) in OFFICES.into_iter().enumerate() {
        for worker_index in 0..worker_count {
            let worker = WorkerId(format!("{office}#worker-{worker_index}"));
            let agent = if worker_index.is_multiple_of(2) {
                Agent::Codex
            } else {
                Agent::Claude
            };
            let activity = match (office_index, worker_index) {
                (0, 0) => Activity::Waiting {
                    detail: "approve the release".into(),
                },
                (0, 1) => Activity::Error {
                    detail: "worker needs attention".into(),
                },
                (1, 0) => Activity::Error {
                    detail: "integration test failed".into(),
                },
                (2, 0) => Activity::Typing {
                    detail: "cargo test -p theywork-render".into(),
                },
                (3, 0) => Activity::Reading {
                    detail: "src/views/office.rs".into(),
                },
                (4, 0) => Activity::Talking {
                    detail: "shipping a deterministic update".into(),
                },
                (5, 0) => Activity::Thinking,
                (_, index) if index % 4 == 0 => Activity::Editing {
                    detail: format!("src/module-{index}.rs"),
                },
                (_, index) if index % 4 == 1 => Activity::Searching {
                    detail: format!("status:{index}"),
                },
                _ => Activity::Idle,
            };
            let observed_at = if office_index == 0 && worker_index == 0 {
                SNAPSHOT_NOW - BLOCKED_AFTER_MS - 1
            } else {
                SNAPSHOT_NOW - 900
            };
            let name = format!(
                "{} worker {worker_index}",
                office.rsplit('/').next().unwrap_or(office)
            );
            world.apply(fixture_event(
                observed_at,
                office,
                &worker,
                agent,
                EventKind::Seen {
                    name,
                    git_branch: Some(format!("codex/golden-{office_index}")),
                },
            ));
            world.apply(fixture_event(
                observed_at,
                office,
                &worker,
                agent,
                EventKind::Tokens(
                    (office_index as u64 + 1) * 125_000 + worker_index as u64 * 7_500,
                ),
            ));
            world.apply(fixture_event(
                observed_at,
                office,
                &worker,
                agent,
                EventKind::Turn {
                    in_flight: activity.is_busy(),
                },
            ));
            world.apply(fixture_event(
                observed_at,
                office,
                &worker,
                agent,
                EventKind::Acted(activity),
            ));
        }
    }
    world
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_match_checked_in_goldens() {
        for theme in [UiTheme::Dark, UiTheme::Light] {
            for (view, view_name) in CASES {
                for (size_name, size) in SIZES {
                    let golden_name =
                        format!("{view_name}.{}.{size_name}.golden", theme_name(theme));
                    let actual = render_snapshot(view, size, theme);
                    assert_golden(&golden_name, &actual);
                }
            }
        }
    }

    #[test]
    fn snapshot_diff_reports_changed_rows() {
        let diff = readable_diff("row 0\nrow 1\n", "row 0\nchanged\n");
        assert!(diff.contains("-001: row 1"));
        assert!(diff.contains("+001: changed"));
    }

    #[test]
    fn snapshot_fixture_is_tall_and_contains_attention_states() {
        let world = snapshot_world();
        assert_eq!(world.office_count(), 6);
        assert!(world.offices().any(|office| {
            office
                .workers
                .iter()
                .any(|worker| worker.status_at(SNAPSHOT_NOW).needs_attention())
        }));
        assert_eq!(world.worker_count(), 20);
    }
}
