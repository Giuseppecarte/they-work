//! they-work: watch your AI coding agents as employees in a pixel-art office.
//!
//! This binary is only wiring: it polls the collectors, folds events into the
//! world, and hands the world to the renderer. All the interesting code lives
//! in the other three crates.

use std::io;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use crossterm::event::{self, Event as TermEvent};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use theywork_core::{Millis, Source, World};
use theywork_render::{Ui, UiCommand};

/// Redraw interval. Fast enough for smooth sprite animation, slow enough that
/// a full building costs almost nothing.
const FRAME: Duration = Duration::from_millis(100);

fn now_ms() -> Millis {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as Millis)
        .unwrap_or(0)
}

struct Args {
    demo: bool,
}

fn parse_args() -> Args {
    let mut demo = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--demo" => demo = true,
            "--help" | "-h" => {
                println!(
                    "they-work — a virtual office for your AI coding agents\n\n\
                     USAGE:\n    they-work [--demo]\n\n\
                     OPTIONS:\n    \
                     --demo    Show an imaginary company. Reads nothing.\n    \
                     -h, --help\n\n\
                     they-work only ever reads agent transcripts. It never writes\n\
                     to them and never uses the network."
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }
    Args { demo }
}

fn main() -> Result<()> {
    let args = parse_args();

    let mut sources: Vec<Box<dyn Source>> = if args.demo {
        Vec::new()
    } else {
        theywork_collect::sources(&theywork_collect::Config::discover())
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let result = run(&mut terminal, &mut sources, args.demo);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    sources: &mut [Box<dyn Source>],
    demo: bool,
) -> Result<()> {
    let mut world = World::new();
    let mut ui = Ui::new();

    loop {
        let now = now_ms();

        if demo {
            for ev in theywork_core::demo::events(now) {
                world.apply(ev);
            }
        } else {
            for source in sources.iter_mut() {
                // A failing collector must never take the building down; the
                // other agent may still be perfectly healthy.
                if let Ok(events) = source.poll(now) {
                    for ev in events {
                        world.apply(ev);
                    }
                }
            }
        }
        world.tick(now);
        ui.tick(now);

        terminal.draw(|f| ui.draw(f, &world))?;

        if event::poll(FRAME)? {
            if let TermEvent::Key(key) = event::read()? {
                if ui.handle_key(key) == Some(UiCommand::Quit) {
                    return Ok(());
                }
            }
        }
    }
}
