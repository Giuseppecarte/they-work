//! Live gallery for reviewing the isometric floor and camera views.

use std::io;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::event::{self, Event as TerminalEvent};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use theywork_core::{
    Activity, Agent, Event, EventKind, Millis, OfficeId, WorkerId, World, BLOCKED_AFTER_MS,
};
use theywork_render::{Ui, UiCommand};

fn now_ms() -> Millis {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as Millis)
        .unwrap_or(0)
}

fn main() -> io::Result<()> {
    let reality_mode = std::env::args().any(|arg| arg == "--real");
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run(&mut terminal, reality_mode);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    reality_mode: bool,
) -> io::Result<()> {
    let mut world = if reality_mode {
        reality_world(now_ms())
    } else {
        World::new()
    };
    let mut ui = Ui::new();
    loop {
        let now = now_ms();
        if !reality_mode {
            for event in theywork_core::demo::events(now) {
                world.apply(event);
            }
            world.tick(now);
        }
        ui.tick(now);
        terminal.draw(|frame| ui.draw(frame, &world))?;

        if event::poll(Duration::from_millis(100))? {
            if let TerminalEvent::Key(key) = event::read()? {
                if ui.handle_key(key) == Some(UiCommand::Quit) {
                    return Ok(());
                }
            }
        }
    }
}
fn apply_reality_event(
    world: &mut World,
    at: Millis,
    office: &str,
    worker: &WorkerId,
    agent: Agent,
    kind: EventKind,
) {
    world.apply(Event {
        at,
        office: OfficeId(office.to_string()),
        office_path: office.to_string(),
        worker: worker.clone(),
        agent,
        kind,
    });
}

fn reality_world(base: Millis) -> World {
    const OFFICES: [(&str, usize); 6] = [
        ("/reality/eleven-desks", 11),
        ("/reality/eight-desks", 8),
        ("/reality/six-desks", 6),
        ("/reality/four-desks", 4),
        ("/reality/one-desk-a", 1),
        ("/reality/one-desk-b", 1),
    ];
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
    for (office_index, (office, worker_count)) in OFFICES.into_iter().enumerate() {
        for worker_index in 0..worker_count {
            let worker = WorkerId(format!("{office}#worker-{worker_index}"));
            let agent = if worker_index % 2 == 0 {
                Agent::Codex
            } else {
                Agent::Claude
            };
            let name = match (office_index, worker_index) {
                (0, 0) => format!("{}界🛠️", "x".repeat(200)),
                (0, 1) => "审批观察者 🧭".to_string(),
                (1, 0) => "Build failure".to_string(),
                _ => format!("Worker {office_index}-{worker_index} 🧩"),
            };
            let activity = if office_index == 0 && worker_index == 0 {
                Activity::Waiting {
                    detail: "cargo publish --dry-run".into(),
                }
            } else if office_index == 1 && worker_index == 0 {
                Activity::Error {
                    detail: "integration test failed".into(),
                }
            } else {
                match (office_index + worker_index) % 4 {
                    0 => Activity::Editing {
                        detail: format!("src/feature-{office_index}-{worker_index}.rs"),
                    },
                    1 => Activity::Talking {
                        detail: "I have a deterministic update.".into(),
                    },
                    2 => Activity::Reading {
                        detail: format!("src/module-{worker_index}.rs"),
                    },
                    _ => Activity::Thinking,
                }
            };
            let observed_at = if matches!(activity, Activity::Waiting { .. }) {
                base.saturating_sub(BLOCKED_AFTER_MS + 1)
            } else {
                base
            };
            apply_reality_event(
                &mut world,
                observed_at,
                office,
                &worker,
                agent,
                EventKind::Seen {
                    name,
                    git_branch: Some(format!("codex/reality-office-{office_index}")),
                },
            );
            let tokens = if office_index == 0 {
                TOKEN_LADDER.get(worker_index).copied().unwrap_or_default()
            } else {
                (worker_index as u64 + 1) * 750_000 / (office_index as u64 + 1)
            };
            apply_reality_event(
                &mut world,
                observed_at,
                office,
                &worker,
                agent,
                EventKind::Tokens(tokens),
            );
            let in_flight = activity.is_busy();
            apply_reality_event(
                &mut world,
                observed_at,
                office,
                &worker,
                agent,
                EventKind::Turn { in_flight },
            );
            apply_reality_event(
                &mut world,
                observed_at,
                office,
                &worker,
                agent,
                EventKind::Acted(activity),
            );
        }
    }
    world
}
