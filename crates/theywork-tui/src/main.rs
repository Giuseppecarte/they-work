//! they-work: a read-only terminal office for local agent activity.
//!
//! This binary owns command-line policy and the polling loop. Collectors own
//! the data boundary; the renderer owns presentation state.

mod connections;

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use crossterm::cursor::{MoveTo, Show};
use crossterm::event::{self, Event as TermEvent, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::{CrosstermBackend, TestBackend};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;
use theywork_collect::{Config, DiscoveryKind, StoreReport};
use theywork_core::{Agent, Event, Millis, Source, Worker, WorkerStatus, World};
use theywork_render::{ColorDepth, RendererDiagnostics, Ui, UiCommand, View};
use theywork_terminal_image::{
    detect_terminal_with_timeout, Capabilities, CellRect, GraphicsProtocol, ImageSurface,
    RgbaImage, TerminalGeometry, DEFAULT_PROBE_TIMEOUT,
};

/// Redraw interval. Fast enough for smooth sprite animation, slow enough that
/// a full building costs almost nothing.
const FRAME: Duration = Duration::from_millis(100);
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const CLI_DETAIL_LIMIT: usize = 240;
const UNKNOWN_WAITING_DETAIL: &str = "waiting, no pending command identified";
const DOCTOR_PROBE_TIMEOUT: Duration = Duration::from_millis(150);

#[cfg(unix)]
static TERMINATION_SIGNAL: AtomicI32 = AtomicI32::new(0);

const HELP: &str = "\
they-work — a read-only terminal office for local agent activity

USAGE:
  they-work [OPTIONS]

OPTIONS:
  --project <path>         Open one project office
  --all                    Start at the guard office
  --demo                   Show the imaginary company; reads nothing
  --once                   Print one plain-text standup and exit
  --headless               Run the polling loop without a terminal
  --exit-after <duration>  Stop headless mode after e.g. 30s, 5m, or 1h
  --doctor                 Print discovered stores and exit
  --view <iso|top|side>    Choose the starting camera
  --light                  Start with the light appearance
  --dark                   Start with the dark appearance
  --color <auto|true|256|none>
                           Choose terminal color handling
  --setup                  Choose local sources and their folders
  --sources <all|codex|claude|none>
                           Choose which conversations may be read
  --codex-home <path>      Use this Codex data folder
  --claude-home <path>     Use this Claude Code data folder
  --config-dir <path>      Remember sources, appearance and selected floor
  -h, --help               Show this help
";

const READ_PARAGRAPH: &str = "Claude Code data comes from regular .jsonl session files below ~/.claude/projects/; Codex data comes from state_5.sqlite and thread_history_1.sqlite under ~/.codex/ or ~/.codex/sqlite/. Discovery also checks THEYWORK_*_HOME overrides, /data mounts, USERPROFILE, and Windows profiles visible under /mnt/*/Users/*; main databases are opened read-only, SQLite may update an existing -shm coordination sidecar on a writable native store, missing sidecars are never created, symlinks and non-JSONL files are skipped, and project source files are never read.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartView {
    Iso,
    Top,
    Side,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorMode {
    Auto,
    True,
    Palette256,
    None,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Args {
    help: bool,
    demo: bool,
    all: bool,
    once: bool,
    headless: bool,
    exit_after: Option<Duration>,
    doctor: bool,
    project: Option<PathBuf>,
    view: Option<StartView>,
    light: bool,
    dark: bool,
    color: Option<ColorMode>,
    config_dir: Option<PathBuf>,
    setup: bool,
    sources: Option<String>,
    codex_home: Option<PathBuf>,
    claude_home: Option<PathBuf>,
}

fn now_ms() -> Millis {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as Millis)
        .unwrap_or(0)
}

fn parse_args<I>(arguments: I) -> std::result::Result<Args, String>
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = Args::default();
    let mut arguments = arguments.into_iter();

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => parsed.help = true,
            "--demo" => parsed.demo = true,
            "--all" => parsed.all = true,
            "--once" => parsed.once = true,
            "--headless" => parsed.headless = true,
            "--exit-after" => {
                parsed.exit_after = Some(parse_duration(&next_value(
                    &mut arguments,
                    "--exit-after",
                )?)?);
            }
            "--doctor" => parsed.doctor = true,
            "--setup" => parsed.setup = true,
            "--sources" => parsed.sources = Some(next_value(&mut arguments, "--sources")?),
            "--codex-home" => {
                parsed.codex_home = Some(PathBuf::from(next_value(&mut arguments, "--codex-home")?))
            }
            "--claude-home" => {
                parsed.claude_home =
                    Some(PathBuf::from(next_value(&mut arguments, "--claude-home")?))
            }
            value if value.starts_with("--sources=") => {
                parsed.sources = Some(value["--sources=".len()..].into())
            }
            value if value.starts_with("--codex-home=") => {
                parsed.codex_home = Some(PathBuf::from(nonempty_option(
                    "--codex-home",
                    &value["--codex-home=".len()..],
                )?))
            }
            value if value.starts_with("--claude-home=") => {
                parsed.claude_home = Some(PathBuf::from(nonempty_option(
                    "--claude-home",
                    &value["--claude-home=".len()..],
                )?))
            }
            "--light" => parsed.light = true,
            "--dark" => parsed.dark = true,
            "--project" => {
                parsed.project = Some(PathBuf::from(next_value(&mut arguments, "--project")?));
            }
            "--view" => {
                parsed.view = Some(parse_view(&next_value(&mut arguments, "--view")?)?);
            }
            "--color" => {
                parsed.color = Some(parse_color(&next_value(&mut arguments, "--color")?)?);
            }
            "--config-dir" => {
                parsed.config_dir =
                    Some(PathBuf::from(next_value(&mut arguments, "--config-dir")?));
            }
            value if value.starts_with("--project=") => {
                parsed.project = Some(PathBuf::from(nonempty_option(
                    "--project",
                    &value["--project=".len()..],
                )?));
            }
            value if value.starts_with("--view=") => {
                parsed.view = Some(parse_view(&value["--view=".len()..])?);
            }
            value if value.starts_with("--color=") => {
                parsed.color = Some(parse_color(&value["--color=".len()..])?);
            }
            value if value.starts_with("--config-dir=") => {
                parsed.config_dir = Some(PathBuf::from(nonempty_option(
                    "--config-dir",
                    &value["--config-dir=".len()..],
                )?));
            }
            value if value.starts_with("--exit-after=") => {
                parsed.exit_after = Some(parse_duration(&nonempty_option(
                    "--exit-after",
                    &value["--exit-after=".len()..],
                )?)?);
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown option: {value}"));
            }
            value => return Err(format!("unexpected argument: {value}")),
        }
    }

    if parsed
        .sources
        .as_deref()
        .is_some_and(|value| !matches!(value, "all" | "codex" | "claude" | "none"))
    {
        return Err("--sources must be all, codex, claude, or none".into());
    }
    if parsed.setup
        && (parsed.once
            || parsed.doctor
            || parsed.headless
            || parsed.exit_after.is_some()
            || parsed.demo)
    {
        return Err(
            "--setup is interactive; use it without --once, --doctor, --headless or --demo".into(),
        );
    }
    if parsed.exit_after.is_some() {
        parsed.headless = true;
    }
    if parsed.light && parsed.dark {
        return Err("choose only one of --light and --dark".to_string());
    }
    if parsed.project.is_some() && parsed.all {
        return Err("--project and --all cannot be used together".to_string());
    }
    if parsed.once && parsed.doctor {
        return Err("--once and --doctor cannot be used together".to_string());
    }
    if parsed.once && (parsed.headless || parsed.exit_after.is_some()) {
        return Err("--once and --headless cannot be used together".to_string());
    }
    if parsed.doctor && (parsed.headless || parsed.exit_after.is_some()) {
        return Err("--doctor and --headless cannot be used together".to_string());
    }
    if parsed.headless && parsed.exit_after.is_none() {
        return Err("--headless needs --exit-after".to_string());
    }
    if parsed.demo
        && (parsed.project.is_some()
            || parsed.all
            || parsed.doctor
            || parsed.config_dir.is_some()
            || parsed.sources.is_some()
            || parsed.codex_home.is_some()
            || parsed.claude_home.is_some())
    {
        return Err("--demo cannot be combined with project discovery options".to_string());
    }

    Ok(parsed)
}

fn next_value<I>(arguments: &mut I, flag: &str) -> std::result::Result<String, String>
where
    I: Iterator<Item = String>,
{
    let value = arguments
        .next()
        .ok_or_else(|| format!("{flag} needs a value"))?;
    nonempty_option(flag, &value)
}

fn nonempty_option(flag: &str, value: &str) -> std::result::Result<String, String> {
    if value.is_empty() || value.starts_with("--") {
        Err(format!("{flag} needs a value"))
    } else {
        Ok(value.to_string())
    }
}

fn parse_view(value: &str) -> std::result::Result<StartView, String> {
    match value {
        "iso" => Ok(StartView::Iso),
        "top" => Ok(StartView::Top),
        "side" => Ok(StartView::Side),
        _ => Err(format!(
            "invalid --view value {value:?}; use iso, top, or side"
        )),
    }
}

fn parse_color(value: &str) -> std::result::Result<ColorMode, String> {
    match value {
        "auto" => Ok(ColorMode::Auto),
        "true" => Ok(ColorMode::True),
        "256" => Ok(ColorMode::Palette256),
        "none" => Ok(ColorMode::None),
        _ => Err(format!(
            "invalid --color value {value:?}; use auto, true, 256, or none"
        )),
    }
}

fn parse_duration(value: &str) -> std::result::Result<Duration, String> {
    let split = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    let (amount, suffix) = value.split_at(split);
    let amount = amount.parse::<u64>().map_err(|_| {
        format!(
            "invalid --exit-after value {value:?}; use a positive duration such as 30s, 5m, or 1h"
        )
    })?;
    let multiplier = match suffix {
        "ms" => 1,
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        _ => {
            return Err(format!(
                "invalid --exit-after value {value:?}; use a positive duration such as 30s, 5m, or 1h"
            ));
        }
    };
    let millis = amount
        .checked_mul(multiplier)
        .ok_or_else(|| format!("invalid --exit-after value {value:?}; duration is too large"))?;
    if millis == 0 {
        return Err(format!(
            "invalid --exit-after value {value:?}; duration must be positive"
        ));
    }
    Ok(Duration::from_millis(millis))
}

struct Scan {
    config: Config,
    sources: Vec<Box<dyn Source>>,
    world: World,
    errors: Vec<String>,
}

struct Runtime {
    config: Config,
    sources: Vec<Box<dyn Source>>,
    world: World,
    errors: Vec<String>,
    now: Millis,
    demo: bool,
    start_guard: bool,
    initial_project: Option<String>,
    config_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    let mut args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("error: {error}");
            eprintln!();
            eprintln!("{HELP}");
            std::process::exit(2);
        }
    };

    if args.help {
        print!("{HELP}");
        return Ok(());
    }

    if !args.once && !args.headless {
        install_termination_handlers()?;
    }
    args.config_dir = args
        .config_dir
        .as_deref()
        .map(resolve_filesystem_path)
        .transpose()?;
    if !connections::prepare(&mut args)? {
        return Ok(());
    }

    if args.doctor {
        apply_color_mode(args.color);
        let status = doctor(&args)?;
        if status != 0 {
            std::process::exit(status);
        }
        return Ok(());
    }

    let rss_before = if args.headless {
        resident_bytes()
    } else {
        None
    };
    let mut runtime = build_runtime(&args)?;
    if !args.once && !args.headless {
        install_termination_handlers()?;
    }
    if let (Some(config_dir), Some(project)) = (&runtime.config_dir, args.project.as_ref()) {
        write_selection(config_dir, &normalize_cli_path(project)?)?;
    }

    if !args.once && !args.headless && (!io::stdin().is_terminal() || !io::stdout().is_terminal()) {
        if should_show_first_run(&args, &runtime) {
            let reports = theywork_collect::inspect_selected(&runtime.config, runtime.now);
            render_first_run(&reports, &runtime.world, runtime.now, 0, false)?;
        } else if print_once(&runtime) {
            std::process::exit(1);
        }
        return Ok(());
    }

    if args.once {
        if print_once(&runtime) {
            std::process::exit(1);
        }
        return Ok(());
    }

    if args.headless {
        return run_headless(
            &mut runtime,
            args.exit_after
                .expect("headless mode requires a parsed exit duration"),
            rss_before,
        );
    }

    apply_color_mode(args.color);
    let capabilities = detect_terminal_with_timeout(DEFAULT_PROBE_TIMEOUT).unwrap_or_default();
    let mut terminal_guard = TerminalModeGuard::enter_alternate()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let result = run(&mut terminal, &mut runtime, &args, capabilities);
    drop(terminal);
    terminal_guard.restore()?;
    result
}

struct TerminalModeGuard {
    raw: bool,
    alternate: bool,
}

impl TerminalModeGuard {
    fn enter_raw() -> Result<Self> {
        enable_raw_mode()?;
        Ok(Self {
            raw: true,
            alternate: false,
        })
    }

    fn enter_alternate() -> Result<Self> {
        let mut guard = Self::enter_raw()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        guard.alternate = true;
        Ok(guard)
    }

    fn restore(&mut self) -> Result<()> {
        let raw_result = if self.raw {
            disable_raw_mode().map_err(anyhow::Error::from)
        } else {
            Ok(())
        };
        self.raw = false;
        let screen_result = if self.alternate {
            execute!(io::stdout(), LeaveAlternateScreen, Show).map_err(anyhow::Error::from)
        } else {
            Ok(())
        };
        self.alternate = false;
        raw_result.and(screen_result)
    }
}

impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[cfg(unix)]
extern "C" fn record_termination_signal(signal: libc::c_int) {
    TERMINATION_SIGNAL.store(signal, Ordering::Relaxed);
}

#[cfg(unix)]
fn install_termination_handlers() -> Result<()> {
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        // SAFETY: sigaction is initialized completely before registration;
        // the handler only performs a lock-free atomic store.
        let result = unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = record_termination_signal as usize;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(signal, &action, std::ptr::null_mut())
        };
        if result != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn install_termination_handlers() -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn requested_termination_signal() -> Option<i32> {
    let signal = TERMINATION_SIGNAL.load(Ordering::Relaxed);
    (signal != 0).then_some(signal)
}

#[cfg(not(unix))]
fn requested_termination_signal() -> Option<i32> {
    None
}

fn termination_error() -> Option<anyhow::Error> {
    requested_termination_signal().map(|signal| anyhow!("terminated by signal {signal}"))
}

fn is_ctrl_c(input: KeyEvent) -> bool {
    input.code == KeyCode::Char('c') && input.modifiers.contains(KeyModifiers::CONTROL)
}

fn doctor(args: &Args) -> Result<i32> {
    let config = connections::Connections::from_args(args)?.config();
    let reports = theywork_collect::inspect_selected(&config, now_ms());

    println!("they-work doctor");
    print_terminal_report();
    for report in &reports {
        print_store_report(report);
    }
    println!("read={READ_PARAGRAPH}");
    println!("discovery_overrides={}", discovery_overrides());

    let found_home = reports.iter().any(|report| report.home_found);
    let broken_home = reports
        .iter()
        .any(|report| report.home_found && !report.readable);
    if reports.is_empty() {
        println!("sources=none action=run_they-work_--setup");
    }
    println!("next=they-work --setup (choose sources) | they-work --once (inspect workers)");
    Ok(i32::from(!found_home || broken_home))
}

fn print_terminal_report() {
    println!(
        "terminal_env TERM={} COLORTERM={} TERM_PROGRAM={} LANG={} LC_ALL={} LC_CTYPE={}",
        environment_value("TERM"),
        environment_value("COLORTERM"),
        environment_value("TERM_PROGRAM"),
        environment_value("LANG"),
        environment_value("LC_ALL"),
        environment_value("LC_CTYPE"),
    );

    let is_terminal = io::stdin().is_terminal() && io::stdout().is_terminal();
    let (capabilities, probe) = if is_terminal {
        match detect_terminal_with_timeout(DOCTOR_PROBE_TIMEOUT) {
            Ok(capabilities) => (capabilities, "completed".to_string()),
            Err(error) => (
                Capabilities::none(),
                format!("failed:{}", single_line(&error.to_string())),
            ),
        }
    } else {
        (Capabilities::none(), "skipped:not_a_tty".to_string())
    };
    let terminal_cells = capabilities
        .terminal_cells
        .or_else(|| {
            is_terminal
                .then(crossterm::terminal::size)
                .and_then(Result::ok)
        })
        .filter(|(columns, rows)| *columns > 0 && *rows > 0);
    let ui = Ui::new();
    let renderer = ui.diagnostics();
    println!(
        "terminal_color depth={} reason={}",
        color_depth_label(renderer.color_depth),
        cli_quoted_value(&renderer.color_reason),
    );
    println!(
        "terminal_encoding encoding={} reason={}",
        renderer.encoding.label(),
        cli_quoted_value(&renderer.encoding_reason),
    );
    println!(
        "terminal_graphics protocol={} probe={} cells={} cell_pixels={}",
        graphics_protocol_label(capabilities.graphics),
        cli_quoted_value(&probe),
        terminal_cells.map_or_else(
            || "unknown".to_string(),
            |(columns, rows)| { format!("{columns}x{rows}") }
        ),
        capabilities.cell_size.map_or_else(
            || "unknown".to_string(),
            |cell| format!("{}x{}", cell.width, cell.height),
        ),
    );
    match terminal_cells.and_then(|cells| diagnostic_frame(ui, capabilities, cells)) {
        Some(frame) => println!(
            "terminal_frame mode={} covered_cells={}x{} source_pixels={}x{}",
            frame.mode, frame.area.width, frame.area.height, frame.width, frame.height,
        ),
        None => println!(
            "terminal_frame mode={} covered_cells=unknown source_pixels=unknown",
            if capabilities.graphics.can_transmit_pixels() && capabilities.cell_size.is_some() {
                "graphics"
            } else {
                "cells"
            }
        ),
    }
    println!("terminal_action={}", terminal_action(&renderer));
}

struct DiagnosticFrame {
    mode: &'static str,
    area: Rect,
    width: usize,
    height: usize,
}

fn diagnostic_frame(
    mut ui: Ui,
    capabilities: Capabilities,
    cells: (u16, u16),
) -> Option<DiagnosticFrame> {
    if cells.0 == 0 || cells.1 == 0 || cells.0 > 500 || cells.1 > 200 {
        return None;
    }
    let image_cell_size = capabilities
        .graphics
        .can_transmit_pixels()
        .then_some(capabilities.cell_size)
        .flatten();
    ui.set_image_cell_size(image_cell_size.map(|cell| (cell.width, cell.height)));
    let mut world = World::new();
    let now = 192_000;
    for event in theywork_core::demo::events(now) {
        world.apply(event);
    }
    world.tick(now);
    ui.tick(now);
    let mut terminal = Terminal::new(TestBackend::new(cells.0, cells.1)).ok()?;
    terminal.draw(|frame| ui.draw(frame, &world)).ok()?;
    let frame = ui.pixel_frame();
    Some(DiagnosticFrame {
        mode: if image_cell_size.is_some() {
            "graphics"
        } else {
            "cells"
        },
        area: frame.cell_area()?,
        width: frame.width(),
        height: frame.height(),
    })
}

fn environment_value(name: &str) -> String {
    std::env::var_os(name).map_or_else(
        || "unset".to_string(),
        |value| cli_quoted_value(&single_line(&value.to_string_lossy())),
    )
}

fn single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn color_depth_label(depth: ColorDepth) -> &'static str {
    match depth {
        ColorDepth::TrueColor => "truecolor",
        ColorDepth::Palette256 => "palette256",
        ColorDepth::None => "none",
    }
}

fn graphics_protocol_label(protocol: GraphicsProtocol) -> &'static str {
    match protocol {
        GraphicsProtocol::Kitty {
            direct_transmission: true,
        } => "kitty-direct",
        GraphicsProtocol::Kitty {
            direct_transmission: false,
        } => "kitty-no-direct",
        GraphicsProtocol::Iterm2 => "iterm2",
        GraphicsProtocol::Sixel => "sixel",
        GraphicsProtocol::None => "none",
    }
}

fn terminal_action(renderer: &RendererDiagnostics) -> &'static str {
    if renderer.encoding != theywork_render::PixelEncoding::Sextants {
        "set_THEYWORK_ENCODING=sextants"
    } else if renderer.color_depth != ColorDepth::TrueColor {
        "set_THEYWORK_COLOR=true"
    } else {
        "none"
    }
}

fn should_show_first_run(args: &Args, runtime: &Runtime) -> bool {
    runtime.start_guard
        && !args.demo
        && !args.all
        && !args.once
        && !args.doctor
        && !args.headless
        && args.project.is_none()
}

fn render_first_run(
    reports: &[StoreReport],
    world: &World,
    now: Millis,
    selected: usize,
    interactive: bool,
) -> Result<()> {
    let mut stdout = io::stdout();
    if interactive {
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    }

    writeln!(stdout, "THEY WORK — first run")?;
    writeln!(
        stdout,
        "A read-only terminal office for the agents already running here."
    )?;
    writeln!(stdout)?;
    writeln!(stdout, "WHAT WAS FOUND")?;
    for report in reports {
        let display = match report.agent {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
        };
        let home_state = if report.home_found {
            "found"
        } else {
            "missing"
        };
        let store_state = if report.readable {
            format!(
                "readable projects={}{} threads={} active={}",
                project_metric(report),
                unresolved_path_suffix(report),
                report.threads,
                report.active_threads
            )
        } else {
            format!(
                "unavailable{}",
                report
                    .error
                    .as_deref()
                    .map(|error| format!(": {}", plain_value(error)))
                    .unwrap_or_default()
            )
        };
        writeln!(stdout, "  {display}: {home_state}")?;
        writeln!(
            stdout,
            "    {}_home={} path={} discovery={}{}",
            report.agent.label(),
            home_state,
            plain_value(&report.path.to_string_lossy()),
            report.discovery.label(),
            unusual_report_suffix(report)
        )?;
        writeln!(
            stdout,
            "    {}_store={store_state} {}",
            report.agent.label(),
            report_diagnostic(report)
        )?;
    }
    writeln!(stdout)?;
    writeln!(stdout, "WHAT THIS READS")?;
    writeln!(stdout, "read={READ_PARAGRAPH}")?;
    writeln!(stdout, "discovery_overrides={}", discovery_overrides())?;

    if !reports.iter().any(|report| report.home_found) {
        writeln!(stdout)?;
        writeln!(
            stdout,
            "No agent home was found; no empty office will be opened."
        )?;
        writeln!(
            stdout,
            "Set THEYWORK_CLAUDE_HOME or THEYWORK_CODEX_HOME to a path visible to this process."
        )?;
    }

    writeln!(stdout)?;
    writeln!(stdout, "PICK AN OFFICE")?;
    let offices = first_run_offices(world, now);
    if offices.is_empty() {
        writeln!(stdout, "  No active offices found yet.")?;
    } else {
        for (index, office) in offices.iter().enumerate() {
            let marker = if index == selected { ">" } else { " " };
            writeln!(
                stdout,
                "{marker} office={} path={} workers={} status={}",
                quoted_value(&office.name),
                quoted_value(&office.path),
                office.workers.len(),
                office_status(office, now)
            )?;
        }
    }
    writeln!(stdout)?;
    writeln!(
        stdout,
        "↑↓ choose   Enter open office   Tab guard office   q quit"
    )?;
    stdout.flush()?;
    Ok(())
}

fn first_run_offices(world: &World, now: Millis) -> Vec<&theywork_core::Office> {
    let mut offices: Vec<_> = world.offices().collect();
    offices.sort_by(|left, right| {
        office_rank(left, now)
            .cmp(&office_rank(right, now))
            .then_with(|| left.path.cmp(&right.path))
    });
    offices
}

fn office_status(office: &theywork_core::Office, now: Millis) -> String {
    let mut blocked = 0;
    let mut failed = 0;
    let mut running = 0;
    let mut idle = 0;
    for worker in &office.workers {
        match worker.status_at(now) {
            WorkerStatus::Blocked => blocked += 1,
            WorkerStatus::Failed => failed += 1,
            WorkerStatus::Running => running += 1,
            WorkerStatus::Idle => idle += 1,
        }
    }
    format!("blocked={blocked} failed={failed} running={running} idle={idle}")
}

fn print_store_report(report: &StoreReport) {
    let label = report.agent.label();
    let home_state = if report.home_found {
        "found"
    } else {
        "missing"
    };
    println!(
        "{label}_home={home_state} path={} discovery={}{}",
        plain_value(&report.path.to_string_lossy()),
        report.discovery.label(),
        unusual_report_suffix(report)
    );

    let store_state = if report.readable {
        "readable"
    } else {
        "unavailable"
    };
    print!(
        "{label}_store={store_state} projects={}{} threads={} active={}",
        project_metric(report),
        unresolved_path_suffix(report),
        report.threads,
        report.active_threads
    );
    if let Some(error) = report.error.as_deref() {
        print!(" reason={}", plain_value(error));
    }
    print!(" {}", report_diagnostic(report));
    println!();
}

fn project_metric(report: &StoreReport) -> String {
    match (report.projects, report.unresolved_paths) {
        (0, unresolved) if unresolved > 0 => "unresolved".to_string(),
        (projects, unresolved) if unresolved > 0 => format!("{projects}+unresolved"),
        (projects, _) => projects.to_string(),
    }
}

fn unresolved_path_suffix(report: &StoreReport) -> String {
    if report.unresolved_paths == 0 {
        String::new()
    } else {
        format!(" unresolved_paths={}", report.unresolved_paths)
    }
}

fn report_diagnostic(report: &StoreReport) -> String {
    if !report.home_found {
        return format!(
            "status=not_found looked={} override={} action=set_override{}",
            candidate_paths(report),
            override_name(report.agent),
            unusual_diagnostic(report)
        );
    }
    if !report.readable {
        return format!(
            "status=unreadable action=check_permissions_or_set_{}",
            override_name(report.agent)
        );
    }
    if report_is_empty(report) {
        return format!(
            "status=empty action=installed_never_run_here note=\"installed, never run here\"{}",
            unusual_diagnostic(report)
        );
    }
    format!(
        "status=ready action=read_only{}",
        unusual_diagnostic(report)
    )
}

fn override_name(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "THEYWORK_CLAUDE_HOME",
        Agent::Codex => "THEYWORK_CODEX_HOME",
    }
}

fn candidate_paths(report: &StoreReport) -> String {
    let mut paths = report.candidates.iter().collect::<Vec<_>>();
    if paths.is_empty() {
        paths.push(&report.path);
    }
    paths
        .into_iter()
        .map(|path| plain_value(&path.to_string_lossy()))
        .collect::<Vec<_>>()
        .join("|")
}

fn report_is_empty(report: &StoreReport) -> bool {
    report.readable && report.projects == 0 && report.threads == 0
}

fn unusual_report_suffix(report: &StoreReport) -> String {
    if report.discovery == DiscoveryKind::WslCrossover || unusual_path(&report.path) {
        " source=unusual".to_string()
    } else {
        String::new()
    }
}

fn unusual_diagnostic(report: &StoreReport) -> String {
    if report.discovery == DiscoveryKind::WslCrossover || unusual_path(&report.path) {
        " source=unusual confirm_path=true".to_string()
    } else {
        String::new()
    }
}

fn unusual_path(path: &Path) -> bool {
    let lower = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let components = lower
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    components.len() >= 5
        && components[0] == "mnt"
        && components[1].len() == 1
        && components[2] == "users"
}

fn discovery_overrides() -> String {
    let overrides = ["THEYWORK_CLAUDE_HOME", "THEYWORK_CODEX_HOME"]
        .into_iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| format!("{name}={}", plain_value(&value)))
        })
        .collect::<Vec<_>>()
        .join(" ");
    if overrides.is_empty() {
        "none".to_string()
    } else {
        overrides
    }
}

fn build_runtime(args: &Args) -> Result<Runtime> {
    let now = now_ms();
    if args.demo {
        let mut world = World::new();
        for event in theywork_core::demo::events(now) {
            world.apply(event);
        }
        world.tick(now);
        return Ok(Runtime {
            config: empty_config(),
            sources: Vec::new(),
            world,
            errors: Vec::new(),
            now,
            demo: true,
            start_guard: false,
            initial_project: None,
            config_dir: args.config_dir.clone(),
        });
    }

    let base_config = connections::Connections::from_args(args)?.config();
    let config_dir = args
        .config_dir
        .as_deref()
        .map(resolve_filesystem_path)
        .transpose()?;
    let explicit = args
        .project
        .as_deref()
        .map(normalize_cli_path)
        .transpose()?;
    let remembered = if explicit.is_none() && !args.all {
        config_dir
            .as_deref()
            .map(read_selection)
            .transpose()?
            .flatten()
    } else {
        None
    };
    let current = if explicit.is_none() && !args.all {
        current_project()?
    } else {
        None
    };

    // A selected floor is a view preference. Only --project restricts the
    // input boundary; launching inside a repository must retain the tower.
    let scan = if let Some(project) = explicit.as_deref() {
        scoped_scan(&base_config, project, now)
    } else {
        scan_config(&base_config, now)
    };
    let initial_project = explicit
        .or(remembered)
        .or(current)
        .filter(|path| scan.world.offices().any(|office| office.path == *path));
    let start_guard = args.all || initial_project.is_none();

    Ok(Runtime {
        config: scan.config,
        sources: scan.sources,
        world: scan.world,
        errors: scan.errors,
        now,
        demo: false,
        start_guard,
        initial_project,
        config_dir,
    })
}

fn empty_config() -> Config {
    Config {
        claude_home: None,
        codex_home: None,
        active_within: theywork_collect::DEFAULT_ACTIVE_WITHIN,
        only_paths: Vec::new(),
    }
}

fn scoped_scan(base_config: &Config, project: &str, now: Millis) -> Scan {
    let mut config = base_config.clone();
    config.only_paths = vec![PathBuf::from(project)];
    scan_config(&config, now)
}

fn scan_config(config: &Config, now: Millis) -> Scan {
    let mut sources = theywork_collect::sources(config);
    let mut world = World::new();
    let mut errors = Vec::new();

    for source in &mut sources {
        let source_name = source.name();
        match source.poll(now) {
            Ok(events) => {
                for event in events {
                    world.apply(event);
                }
            }
            Err(error) => errors.push(format!("{source_name}: {error}")),
        }
    }
    world.tick(now);

    Scan {
        config: config.clone(),
        sources,
        world,
        errors,
    }
}

fn resolve_filesystem_path(input: &Path) -> Result<PathBuf> {
    let spelling = input.to_string_lossy();
    if spelling == "~" || spelling.starts_with("~/") || spelling.starts_with("~\\") {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow!("cannot expand ~: HOME or USERPROFILE is not set"))?;
        return Ok(PathBuf::from(home).join(spelling.get(2..).unwrap_or("")));
    }
    if input.is_absolute() || looks_absolute_spelling(&spelling) {
        Ok(PathBuf::from(spelling.replace('\\', "/")))
    } else {
        Ok(std::env::current_dir()?.join(input))
    }
}

fn normalize_cli_path(input: &Path) -> Result<String> {
    let absolute = resolve_filesystem_path(input)?;
    let root = find_git_root(&absolute).unwrap_or(absolute);
    let normalized = theywork_collect::normalize_office_path(&root.to_string_lossy());
    if normalized.is_empty() {
        Err(anyhow!("project path is empty"))
    } else {
        Ok(normalized)
    }
}

fn current_project() -> Result<Option<String>> {
    let current = std::env::current_dir()?;
    Ok(find_git_root(&current)
        .map(|root| theywork_collect::normalize_office_path(&root.to_string_lossy())))
}

fn find_git_root(path: &Path) -> Option<PathBuf> {
    let mut current = if path.is_file() {
        path.parent().map(Path::to_path_buf)?
    } else {
        path.to_path_buf()
    };
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn looks_absolute_spelling(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.starts_with("//")
        || value.starts_with("\\\\")
        || (bytes.len() >= 3 && bytes[1] == b':' && (bytes[2] == b'/' || bytes[2] == b'\\'))
}

fn read_selection(config_dir: &Path) -> Result<Option<String>> {
    let selection_path = config_dir.join("project");
    let contents = match fs::read_to_string(&selection_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(anyhow!(
                "could not read {}: {error}",
                selection_path.display()
            ));
        }
    };
    let value = contents.trim();
    if value.is_empty() {
        return Err(anyhow!(
            "{} is empty; expected one normalized project path",
            selection_path.display()
        ));
    }
    let normalized = theywork_collect::normalize_office_path(value);
    if normalized.is_empty() {
        return Err(anyhow!(
            "{} does not contain a project path",
            selection_path.display()
        ));
    }
    Ok(Some(normalized))
}

fn write_selection(config_dir: &Path, project: &str) -> Result<()> {
    fs::create_dir_all(config_dir)?;

    let temporary = config_dir.join(format!(".project.{}.tmp", std::process::id()));
    let selection_path = config_dir.join("project");
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(project.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, &selection_path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| anyhow!("could not persist {}: {error}", selection_path.display()))
}

fn apply_color_mode(mode: Option<ColorMode>) {
    match mode {
        None => {}
        Some(ColorMode::Auto) => {
            std::env::remove_var("THEYWORK_COLOR");
        }
        Some(ColorMode::True) => {
            std::env::set_var("THEYWORK_COLOR", "true");
        }
        Some(ColorMode::Palette256) => {
            std::env::set_var("THEYWORK_COLOR", "256");
        }
        Some(ColorMode::None) => {
            std::env::remove_var("THEYWORK_COLOR");
            std::env::set_var("NO_COLOR", "1");
        }
    }
}

fn configure_ui(
    ui: &mut Ui,
    args: &Args,
    start_guard: bool,
    mut preferences: theywork_render::RendererPreferences,
) {
    if let Some(view) = args.view {
        preferences.projection = match view {
            StartView::Iso => "isometric",
            StartView::Top => "top-down",
            StartView::Side => "side",
        }
        .into();
    }
    if args.light {
        preferences.light = true;
    }
    if args.dark {
        preferences.light = false;
    }
    if args.color == Some(ColorMode::Auto) {
        preferences.color_depth = None;
    }
    ui.restore_preferences(&preferences);
    if start_guard {
        ui.open_tower();
    }
}

struct PollResult {
    events: Vec<Event>,
    errors: Vec<String>,
}

struct Poller {
    results: Receiver<PollResult>,
    stop: Sender<()>,
    thread: Option<thread::JoinHandle<Vec<Box<dyn Source>>>>,
}

impl Poller {
    fn start(mut sources: Vec<Box<dyn Source>>) -> Self {
        let (result_tx, result_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let thread = thread::spawn(move || {
            loop {
                let now = now_ms();
                let mut events = Vec::new();
                let mut errors = Vec::new();
                for source in &mut sources {
                    match source.poll(now) {
                        Ok(source_events) => events.extend(source_events),
                        Err(error) => {
                            errors.push(format!("{}: {}", error.source_name, error.message))
                        }
                    }
                }
                if result_tx.send(PollResult { events, errors }).is_err() {
                    break;
                }
                match stop_rx.recv_timeout(POLL_INTERVAL) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
            sources
        });
        Self {
            results: result_rx,
            stop: stop_tx,
            thread: Some(thread),
        }
    }

    fn drain(&self) -> impl Iterator<Item = PollResult> + '_ {
        self.results.try_iter()
    }

    fn stop(&mut self) -> Vec<Box<dyn Source>> {
        let _ = self.stop.send(());
        self.thread
            .take()
            .and_then(|thread| thread.join().ok())
            .unwrap_or_default()
    }
}

fn run_headless(runtime: &mut Runtime, duration: Duration, rss_before: Option<u64>) -> Result<()> {
    let started = Instant::now();
    let cpu_started = process_cpu_ticks();
    let deadline = started + duration;
    let (initial_offices, initial_workers, mut previous_workers) = roster_snapshot(&runtime.world);
    let mut seen_workers = previous_workers.clone();
    let mut offices_min = initial_offices;
    let mut offices_max = initial_offices;
    let mut workers_min = initial_workers;
    let mut workers_max = initial_workers;
    let mut roster_changes = 0;
    let mut workers_joined = 0;
    let mut workers_left = 0;
    let mut frames = 0;
    let mut polls = 0;
    let mut events = 0;
    let mut poll_errors = 0;
    let mut errors: HashSet<String> = runtime.errors.iter().cloned().collect();
    let rss_after_initial_scan = resident_bytes();
    let mut poller = Poller::start(std::mem::take(&mut runtime.sources));

    loop {
        let frame_started = Instant::now();
        let now = now_ms();
        runtime.now = now;
        if runtime.demo {
            for event in theywork_core::demo::events(now) {
                runtime.world.apply(event);
            }
        } else {
            for result in poller.drain() {
                polls += 1;
                events += result.events.len();
                for event in result.events {
                    runtime.world.apply(event);
                }
                poll_errors += result.errors.len();
                errors.extend(result.errors);
            }
        }
        runtime.world.tick(now);

        let (office_count, worker_count, current_workers) = roster_snapshot(&runtime.world);
        offices_min = offices_min.min(office_count);
        offices_max = offices_max.max(office_count);
        workers_min = workers_min.min(worker_count);
        workers_max = workers_max.max(worker_count);
        workers_joined += current_workers.difference(&previous_workers).count();
        workers_left += previous_workers.difference(&current_workers).count();
        if current_workers != previous_workers {
            roster_changes += 1;
        }
        seen_workers.extend(current_workers.iter().cloned());
        previous_workers = current_workers;
        frames += 1;

        if Instant::now() >= deadline {
            break;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let sleep_for = FRAME.saturating_sub(frame_started.elapsed()).min(remaining);
        if !sleep_for.is_zero() {
            std::thread::sleep(sleep_for);
        }
    }

    poller.stop();
    let elapsed = started.elapsed();
    let cpu_finished = process_cpu_ticks();
    let (final_offices, final_workers, _) = roster_snapshot(&runtime.world);
    let effective_fps = frames as f64 / elapsed.as_secs_f64().max(f64::EPSILON);
    println!("they-work --headless");
    println!(
        "target_fps=10 effective_fps={effective_fps:.2} frame_ms={} poll_interval_ms={} exit_after_ms={} elapsed_ms={} frames={} polls={} events={} poll_errors={}",
        FRAME.as_millis(),
        POLL_INTERVAL.as_millis(),
        duration.as_millis(),
        elapsed.as_millis(),
        frames,
        polls,
        events,
        poll_errors
    );
    println!(
        "roster initial_offices={} final_offices={} office_min={} office_max={} initial_workers={} final_workers={} worker_min={} worker_max={} changes={} joined={} left={} unique_workers={}",
        initial_offices,
        final_offices,
        offices_min,
        offices_max,
        initial_workers,
        final_workers,
        workers_min,
        workers_max,
        roster_changes,
        workers_joined,
        workers_left,
        seen_workers.len()
    );
    println!("rss_before_bytes={}", optional_metric(rss_before));
    println!(
        "rss_after_initial_scan_bytes={}",
        optional_metric(rss_after_initial_scan)
    );
    println!("rss_after_bytes={}", optional_metric(resident_bytes()));
    match process_cpu_usage(cpu_started, cpu_finished, elapsed) {
        Some((seconds, percent)) => {
            println!("cpu_seconds={seconds:.3} cpu_average_percent={percent:.3}");
        }
        None => println!("cpu_seconds=unavailable cpu_average_percent=unavailable"),
    }
    for error in errors {
        println!("collector_error={}", plain_value(&error));
    }
    Ok(())
}

fn roster_snapshot(world: &World) -> (usize, usize, HashSet<String>) {
    let mut workers = HashSet::new();
    for office in world.offices() {
        for worker in &office.workers {
            workers.insert(format!("{}::{}", office.path, worker.id.0));
        }
    }
    (world.office_count(), workers.len(), workers)
}

#[cfg(target_os = "linux")]
fn resident_bytes() -> Option<u64> {
    let statm = fs::read_to_string("/proc/self/statm").ok()?;
    let pages = statm.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    pages.checked_mul(system_page_size()?)
}

#[cfg(target_os = "linux")]
fn system_page_size() -> Option<u64> {
    // SAFETY: sysconf reads immutable process/system configuration and does
    // not dereference pointers or mutate Rust-owned memory.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    u64::try_from(page_size).ok().filter(|value| *value > 0)
}

#[cfg(target_os = "linux")]
fn process_cpu_ticks() -> Option<(u64, u64)> {
    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    let ticks = parse_process_cpu_ticks(&stat)?;
    // SAFETY: as above, this only queries the kernel's clock tick frequency.
    let ticks_per_second = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    let ticks_per_second = u64::try_from(ticks_per_second)
        .ok()
        .filter(|value| *value > 0)?;
    Some((ticks, ticks_per_second))
}

#[cfg(not(target_os = "linux"))]
fn resident_bytes() -> Option<u64> {
    None
}

#[cfg(not(target_os = "linux"))]
fn process_cpu_ticks() -> Option<(u64, u64)> {
    None
}

#[cfg(any(target_os = "linux", test))]
fn parse_process_cpu_ticks(stat: &str) -> Option<u64> {
    let after_name = stat.rsplit_once(')')?.1;
    let fields = after_name.split_whitespace().collect::<Vec<_>>();
    let user = fields.get(11)?.parse::<u64>().ok()?;
    let system = fields.get(12)?.parse::<u64>().ok()?;
    user.checked_add(system)
}

fn process_cpu_usage(
    started: Option<(u64, u64)>,
    finished: Option<(u64, u64)>,
    elapsed: Duration,
) -> Option<(f64, f64)> {
    let ((start_ticks, start_rate), (end_ticks, end_rate)) = (started?, finished?);
    if start_rate != end_rate || elapsed.is_zero() {
        return None;
    }
    let seconds = end_ticks.checked_sub(start_ticks)? as f64 / start_rate as f64;
    let percent = seconds / elapsed.as_secs_f64() * 100.0;
    Some((seconds, percent))
}

fn optional_metric(value: Option<u64>) -> String {
    value.map_or_else(|| "unavailable".to_string(), |value| value.to_string())
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    runtime: &mut Runtime,
    args: &Args,
    capabilities: Capabilities,
) -> Result<()> {
    let mut ui = Ui::new();
    let preferences = runtime
        .config_dir
        .as_deref()
        .and_then(|directory| fs::read(directory.join("appearance.json")).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    configure_ui(&mut ui, args, runtime.start_guard, preferences);
    if let Some(path) = &runtime.initial_project {
        if let Some(office) = runtime.world.offices().find(|office| office.path == *path) {
            ui.open_office(&office.id);
        }
    }
    let mut active_args = args.clone();
    let terminal_cells = terminal.size()?;
    let image_cell_size = capabilities
        .graphics
        .can_transmit_pixels()
        .then_some(capabilities.cell_size)
        .flatten();
    ui.set_image_cell_size(image_cell_size.map(|size| (size.width, size.height)));
    let mut image_presenter =
        TerminalImagePresenter::new(capabilities, (terminal_cells.width, terminal_cells.height));
    let mut poller = Poller::start(std::mem::take(&mut runtime.sources));
    let result = (|| -> Result<()> {
        loop {
            if let Some(error) = termination_error() {
                return Err(error);
            }
            let now = now_ms();

            if runtime.demo {
                for event in theywork_core::demo::events(now) {
                    runtime.world.apply(event);
                }
            } else {
                for result in poller.drain() {
                    for event in result.events {
                        runtime.world.apply(event);
                    }
                    for error in result.errors {
                        if !runtime.errors.contains(&error) {
                            runtime.errors.push(error);
                        }
                    }
                }
            }

            runtime.world.tick(now);
            ui.tick(now);

            let terminal_cells = terminal.size()?;
            image_presenter.resize(
                terminal.backend_mut(),
                (terminal_cells.width, terminal_cells.height),
            )?;
            let mut pixel_frame = None;
            terminal.draw(|frame| {
                ui.draw(frame, &runtime.world);
                if image_presenter.enabled() {
                    let snapshot = ui.pixel_frame();
                    if let Some(area) = snapshot.cell_area() {
                        skip_image_cells(frame.buffer_mut(), area);
                    }
                    pixel_frame = Some(snapshot);
                }
            })?;
            image_presenter.present(terminal.backend_mut(), pixel_frame)?;

            if event::poll(FRAME)? {
                if let TermEvent::Key(input) = event::read()? {
                    if is_ctrl_c(input) {
                        return Ok(());
                    }
                    let previous_view = ui.view();
                    match ui.handle_key(input) {
                        Some(UiCommand::Quit) => return Ok(()),
                        Some(UiCommand::Sources) => {
                            let paused_sources = poller.stop();
                            // Joining can finish an in-flight poll. Keep its events
                            // before replacing the channel and resuming the cursors.
                            for result in poller.drain() {
                                for event in result.events {
                                    runtime.world.apply(event);
                                }
                                for error in result.errors {
                                    if !runtime.errors.contains(&error) {
                                        runtime.errors.push(error);
                                    }
                                }
                            }
                            image_presenter.present(terminal.backend_mut(), None)?;
                            let value = connections::Connections::from_args(&active_args)?;
                            let action = connections::show(
                                terminal,
                                value,
                                active_args.config_dir.as_deref(),
                            )?;
                            match action {
                                connections::Action::Connect(value) => {
                                    if let Some(directory) = &active_args.config_dir {
                                        connections::save(directory, &value)?;
                                    }
                                    value.apply(&mut active_args);
                                    active_args.demo = false;
                                    active_args.setup = false;
                                    *runtime = build_runtime(&active_args)?;
                                    ui.open_tower();
                                }
                                connections::Action::Demo => {
                                    active_args.demo = true;
                                    *runtime = build_runtime(&active_args)?;
                                    ui.open_tower();
                                }
                                connections::Action::Cancel => runtime.sources = paused_sources,
                            }
                            poller = Poller::start(std::mem::take(&mut runtime.sources));
                            terminal.clear()?;
                        }
                        None => {}
                    }
                    if previous_view == View::Cameras && ui.view() == View::Office {
                        persist_selected_office(runtime, ui.selected_office(), now)?;
                    }
                }
            }
        }
    })();
    poller.stop();
    if let Some(directory) = &runtime.config_dir {
        fs::create_dir_all(directory)?;
        fs::write(
            directory.join("appearance.json"),
            serde_json::to_vec_pretty(&ui.preferences())?,
        )?;
        persist_selected_office(runtime, ui.selected_office(), now_ms())?;
    }
    result
}

struct TerminalImagePresenter {
    surface: Option<ImageSurface>,
    next_frame: Instant,
    last_area: Option<Rect>,
}

impl TerminalImagePresenter {
    fn new(capabilities: Capabilities, cells: (u16, u16)) -> Self {
        let protocol = capabilities.graphics;
        let surface =
            (protocol.can_transmit_pixels() && capabilities.cell_size.is_some()).then(|| {
                ImageSurface::new(
                    protocol,
                    TerminalGeometry::new(cells.0, cells.1, capabilities.cell_size),
                )
            });
        Self {
            surface,
            next_frame: Instant::now(),
            last_area: None,
        }
    }

    fn enabled(&self) -> bool {
        self.surface.is_some()
    }

    fn resize<W: Write>(&mut self, output: &mut W, cells: (u16, u16)) -> Result<()> {
        let Some(surface) = self.surface.as_mut() else {
            return Ok(());
        };
        let current = surface.geometry();
        let geometry = TerminalGeometry::new(cells.0, cells.1, current.cell_size);
        if current != geometry {
            surface.resize(output, geometry)?;
            self.next_frame = Instant::now();
            self.last_area = None;
        }
        Ok(())
    }

    fn present<W: Write>(
        &mut self,
        output: &mut W,
        pixel_frame: Option<theywork_render::PixelFrame>,
    ) -> Result<()> {
        let Some(surface) = self.surface.as_mut() else {
            return Ok(());
        };
        let Some(pixel_frame) = pixel_frame else {
            surface.clear(output)?;
            self.last_area = None;
            output.flush()?;
            return Ok(());
        };
        let Some(area) = pixel_frame.cell_area() else {
            surface.clear(output)?;
            self.last_area = None;
            output.flush()?;
            return Ok(());
        };
        let rectangle = CellRect::new(area.x, area.y, area.width, area.height);
        let frame_size = (
            u32::try_from(pixel_frame.width())?,
            u32::try_from(pixel_frame.height())?,
        );
        if surface.geometry().pixel_size(rectangle) != Some(frame_size) {
            surface.clear(output)?;
            self.last_area = None;
            output.flush()?;
            return Ok(());
        }
        let started = Instant::now();
        if self.last_area == Some(area) && started < self.next_frame {
            return Ok(());
        }
        let image = RgbaImage::new(frame_size.0, frame_size.1, pixel_frame.rgba().to_vec())?;
        let report = surface.draw(output, &image, rectangle)?;
        output.flush()?;
        self.last_area = Some(area);
        if surface.protocol() == theywork_terminal_image::GraphicsProtocol::Sixel {
            self.next_frame =
                started + sixel_frame_interval(report.written_bytes, started.elapsed());
        }
        Ok(())
    }
}

fn sixel_frame_interval(bytes: usize, write_time: Duration) -> Duration {
    // Conservative pacing, not a claim about a particular terminal's parser.
    // Drop intermediate animation frames; never build a transmission backlog.
    Duration::from_millis(200)
        .max(Duration::from_secs_f64(bytes as f64 / 8_000_000.0))
        .max(write_time.saturating_mul(2))
}

fn skip_image_cells(buffer: &mut Buffer, area: Rect) {
    let right = area.x.saturating_add(area.width);
    let bottom = area.y.saturating_add(area.height);
    for y in area.y..bottom {
        for x in area.x..right {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_skip(true);
            }
        }
    }
}

fn persist_selected_office(runtime: &Runtime, selected: usize, now: Millis) -> Result<()> {
    if runtime.demo {
        return Ok(());
    }
    let (Some(config_dir), Some(project)) = (
        runtime.config_dir.as_deref(),
        selected_office_path(&runtime.world, selected, now),
    ) else {
        return Ok(());
    };
    write_selection(config_dir, &project)
}

fn selected_office_path(world: &World, selected: usize, now: Millis) -> Option<String> {
    let _ = now;
    world
        .offices()
        .nth(selected)
        .map(|office| office.path.clone())
}

fn print_once(runtime: &Runtime) -> bool {
    let mut errors = runtime.errors.clone();
    if !runtime.demo {
        for report in theywork_collect::inspect_selected(&runtime.config, runtime.now) {
            match report.error {
                Some(error) if report.home_found => {
                    let entry = format!("{}: {error}", report.agent.label());
                    if !errors.contains(&entry) {
                        errors.push(entry);
                    }
                }
                _ => {}
            }
        }
    }

    println!("they-work --once");
    println!("timestamp_ms={}", runtime.now);
    println!(
        "projects={} workers={}",
        runtime.world.office_count(),
        runtime.world.worker_count()
    );

    let mut offices: Vec<_> = runtime.world.offices().collect();
    offices.sort_by(|left, right| {
        office_rank(left, runtime.now)
            .cmp(&office_rank(right, runtime.now))
            .then_with(|| left.path.cmp(&right.path))
    });

    for office in offices {
        println!(
            "office={} workers={}",
            plain_value(&office.path),
            office.workers.len()
        );
        let mut workers: Vec<&Worker> = office.workers.iter().collect();
        workers.sort_by(|left, right| {
            status_rank(left.status_at(runtime.now))
                .cmp(&status_rank(right.status_at(runtime.now)))
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.id.cmp(&right.id))
        });
        for worker in workers {
            print_worker(worker, runtime.now);
        }
    }

    if runtime.world.worker_count() == 0 {
        println!(
            "next=Start a conversation in a selected source, or run they-work --setup / --doctor."
        );
    } else {
        println!("next=Open they-work to inspect a desk. Respond to approval requests in the original agent app.");
    }
    let has_errors = !errors.is_empty();
    for error in errors {
        println!("collector_error={}", plain_value(&error));
    }
    has_errors
}

fn print_worker(worker: &Worker, now: Millis) {
    let status = worker.status_at(now);
    print!(
        "  worker name={} agent={} status={}",
        quoted_value(&worker.name),
        worker.agent.label(),
        status.label(),
    );
    if status == WorkerStatus::Blocked {
        let waiting_on = worker_waiting_detail(worker).unwrap_or(UNKNOWN_WAITING_DETAIL);
        print!(" waiting_on={}", cli_quoted_value(waiting_on));
    } else if let Some(detail) = worker_detail(worker) {
        print!(" detail={}", cli_quoted_value(detail));
    }
    if status != WorkerStatus::Blocked {
        print!(" activity={}", worker.activity.label());
    }
    print!(
        " idle_age={} tokens={}",
        format_age(now.saturating_sub(worker.last_seen)),
        worker.tokens_used
    );
    println!();
}

fn worker_detail(worker: &Worker) -> Option<&str> {
    worker
        .activity
        .detail()
        .filter(|detail| !detail.is_empty())
        .or_else(|| {
            worker
                .history
                .iter()
                .rev()
                .find_map(|beat| beat.activity.detail().filter(|detail| !detail.is_empty()))
        })
}

fn worker_waiting_detail(worker: &Worker) -> Option<&str> {
    waiting_activity_detail(&worker.activity).filter(|detail| !detail.is_empty())
}

fn waiting_activity_detail(activity: &theywork_core::Activity) -> Option<&str> {
    match activity {
        theywork_core::Activity::Typing { detail }
        | theywork_core::Activity::Reading { detail }
        | theywork_core::Activity::Editing { detail }
        | theywork_core::Activity::Searching { detail }
        | theywork_core::Activity::Waiting { detail } => Some(detail),
        theywork_core::Activity::Thinking
        | theywork_core::Activity::Talking { .. }
        | theywork_core::Activity::Idle
        | theywork_core::Activity::Error { .. } => None,
    }
}

fn office_rank(office: &theywork_core::Office, now: Millis) -> u8 {
    if office
        .workers
        .iter()
        .any(|worker| worker.status_at(now) == WorkerStatus::Blocked)
    {
        0
    } else if office
        .workers
        .iter()
        .any(|worker| worker.status_at(now) == WorkerStatus::Failed)
    {
        1
    } else {
        2
    }
}

fn status_rank(status: WorkerStatus) -> u8 {
    match status {
        WorkerStatus::Blocked => 0,
        WorkerStatus::Failed => 1,
        WorkerStatus::Running => 2,
        WorkerStatus::Idle => 3,
    }
}

fn format_age(age: Millis) -> String {
    let seconds = age.max(0) as u64 / 1_000;
    let days = seconds / 86_400;
    let hours = seconds % 86_400 / 3_600;
    let minutes = seconds % 3_600 / 60;
    let seconds = seconds % 60;
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{seconds}s"));
    }
    parts.join(" ")
}

fn plain_value(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character == '\n' || character == '\r' || character == '\t' {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn quoted_value(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn cli_quoted_value(value: &str) -> String {
    let detail = cli_detail(value);
    let mut output = String::with_capacity(detail.len().min(CLI_DETAIL_LIMIT) + 2);
    output.push('"');
    let mut used = 0;
    for character in detail.chars() {
        match character {
            '\\' => {
                if used + 2 > CLI_DETAIL_LIMIT {
                    break;
                }
                output.push_str("\\\\");
                used += 2;
            }
            '"' => {
                if used + 2 > CLI_DETAIL_LIMIT {
                    break;
                }
                output.push_str("\\\"");
                used += 2;
            }
            character => {
                if used >= CLI_DETAIL_LIMIT {
                    break;
                }
                output.push(character);
                used += 1;
            }
        }
    }
    output.push('"');
    output
}

fn cli_detail(value: &str) -> String {
    let mut output = String::new();
    for word in value.split_whitespace() {
        let separator = usize::from(!output.is_empty());
        let available = CLI_DETAIL_LIMIT.saturating_sub(output.chars().count() + separator);
        if available == 0 {
            break;
        }
        if !output.is_empty() {
            output.push(' ');
        }
        output.extend(word.chars().take(available));
        if output.chars().count() >= CLI_DETAIL_LIMIT {
            break;
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use theywork_terminal_image::{CellSize, GraphicsProtocol};

    fn parse(arguments: &[&str]) -> std::result::Result<Args, String> {
        parse_args(arguments.iter().map(|argument| (*argument).to_string()))
    }

    #[test]
    fn parses_the_real_command_surface() {
        let args = parse(&[
            "--project",
            ".",
            "--view",
            "side",
            "--light",
            "--color",
            "256",
            "--config-dir",
            "/tmp/settings",
        ])
        .unwrap();
        assert_eq!(args.view, Some(StartView::Side));
        assert!(args.light);
        assert_eq!(args.color, Some(ColorMode::Palette256));
        assert_eq!(args.config_dir, Some(PathBuf::from("/tmp/settings")));
    }

    #[test]
    fn accepts_demo_once_and_help() {
        assert!(parse(&["--demo", "--once"]).unwrap().demo);
        assert!(parse(&["--help"]).unwrap().help);
    }

    #[test]
    fn explicit_camera_and_light_override_saved_preferences_absolutely() {
        for (arguments, projection, light) in [
            (vec!["--view", "side", "--light"], "side", true),
            (vec!["--view", "iso", "--dark"], "isometric", false),
            (vec!["--view", "top", "--light"], "top-down", true),
        ] {
            let args = parse(&arguments).unwrap();
            let saved = theywork_render::RendererPreferences {
                projection: "list".into(),
                light: true,
                ..Default::default()
            };
            let mut ui = Ui::new();
            configure_ui(&mut ui, &args, false, saved);
            assert_eq!(ui.preferences().projection, projection);
            assert_eq!(ui.preferences().light, light);
        }
    }

    #[test]
    fn pausing_collectors_preserves_the_last_batch_and_does_not_replay_it() {
        struct Once(bool);
        impl Source for Once {
            fn name(&self) -> &'static str {
                "fixture"
            }
            fn poll(
                &mut self,
                now: Millis,
            ) -> std::result::Result<Vec<Event>, theywork_core::SourceError> {
                if self.0 {
                    return Ok(Vec::new());
                }
                self.0 = true;
                Ok(vec![Event {
                    at: now,
                    office: theywork_core::OfficeId("/fixture".into()),
                    office_path: "/fixture".into(),
                    worker: theywork_core::WorkerId("one".into()),
                    agent: Agent::Codex,
                    kind: theywork_core::EventKind::Seen {
                        name: "One worker".into(),
                        git_branch: None,
                    },
                }])
            }
        }
        let mut poller = Poller::start(vec![Box::new(Once(false))]);
        let sources = poller.stop();
        let batches: Vec<_> = poller.drain().collect();
        assert_eq!(batches.iter().map(|b| b.events.len()).sum::<usize>(), 1);
        let mut resumed = Poller::start(sources);
        resumed.stop();
        assert_eq!(resumed.drain().map(|b| b.events.len()).sum::<usize>(), 0);
    }

    #[test]
    fn parses_bounded_headless_mode() {
        let args = parse(&["--headless", "--exit-after", "250ms"]).unwrap();
        assert!(args.headless);
        assert_eq!(args.exit_after, Some(Duration::from_millis(250)));
        assert!(parse(&["--headless"]).is_err());
        assert!(parse(&["--exit-after", "0s"]).is_err());
        assert!(parse(&["--exit-after", "ten minutes"]).is_err());
    }

    #[test]
    fn rejects_invalid_values_and_conflicts() {
        assert!(parse(&["--view", "front"]).is_err());
        assert!(parse(&["--color", "16"]).is_err());
        assert!(parse(&["--project"]).is_err());
        assert!(parse(&["--light", "--dark"]).is_err());
        assert!(parse(&["--project", "/repo", "--all"]).is_err());
    }

    #[test]
    fn formats_plain_text_ages_and_quotes() {
        assert_eq!(format_age(0), "0s");
        assert_eq!(format_age(3_723_000), "1h 2m 3s");
        assert_eq!(quoted_value("a\n\"b"), "\"a\\n\\\"b\"");
    }

    #[test]
    fn cli_details_are_one_bounded_line() {
        let detail = cli_quoted_value(&format!("first\nsecond\t{}", "x ".repeat(300)));
        assert!(!detail.contains("\\n"));
        assert!(!detail.contains("\\t"));
        assert!(detail.chars().count() <= CLI_DETAIL_LIMIT + 2);
    }

    #[test]
    fn parses_linux_process_cpu_ticks_and_computes_average() {
        let stat = "123 (a worker name) S 1 2 3 4 5 6 7 8 9 10 25 15 0 0";
        assert_eq!(parse_process_cpu_ticks(stat), Some(40));
        assert_eq!(
            process_cpu_usage(Some((100, 100)), Some((125, 100)), Duration::from_secs(5)),
            Some((0.25, 5.0))
        );
    }

    #[test]
    fn blocked_output_does_not_relabel_old_history_as_pending() {
        let mut worker = Worker::new(
            theywork_core::WorkerId("worker".into()),
            theywork_core::OfficeId("[non-project]".into()),
            Agent::Codex,
            "worker".into(),
            0,
        );
        worker.turn_in_flight = true;
        worker.remember(theywork_core::Beat {
            at: 1,
            activity: theywork_core::Activity::Typing {
                detail: "an earlier command".into(),
            },
            outcome: Some(theywork_core::Outcome::Exited(0)),
        });
        assert_eq!(worker_waiting_detail(&worker), None);
    }

    #[test]
    fn image_presenter_encodes_the_renderer_snapshot_when_supported() {
        let mut world = World::new();
        for event in theywork_core::demo::events(0) {
            world.apply(event);
        }
        let mut ui = Ui::new();
        ui.set_image_cell_size(Some((8, 16)));
        ui.tick(0);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
        let mut snapshot = None;
        terminal
            .draw(|frame| {
                ui.draw(frame, &world);
                snapshot = Some(ui.pixel_frame());
            })
            .expect("render demo frame");
        let snapshot = snapshot.expect("renderer snapshot");
        let area = snapshot.cell_area().expect("pixel frame area");
        assert_eq!(snapshot.width(), usize::from(area.width) * 8);
        assert_eq!(snapshot.height(), usize::from(area.height) * 16);

        let capabilities = Capabilities {
            graphics: GraphicsProtocol::Sixel,
            cell_size: Some(CellSize::new(8, 16)),
            terminal_cells: Some((80, 24)),
        };
        let mut presenter = TerminalImagePresenter::new(capabilities, (80, 24));
        let mut output = Vec::new();
        presenter
            .present(&mut output, Some(snapshot))
            .expect("encode image frame");
        assert!(output.windows(3).any(|window| window == b"\x1bPq"));
    }

    #[test]
    fn image_presenter_is_a_noop_without_graphics_support() {
        let mut presenter = TerminalImagePresenter::new(Capabilities::none(), (80, 24));
        let mut output = Vec::new();
        presenter
            .present(&mut output, None)
            .expect("disabled presenter");
        assert!(output.is_empty());
    }

    #[test]
    fn image_presenter_falls_back_to_cells_without_pixel_geometry() {
        let capabilities = Capabilities {
            graphics: GraphicsProtocol::Sixel,
            cell_size: None,
            terminal_cells: Some((80, 24)),
        };
        let presenter = TerminalImagePresenter::new(capabilities, (80, 24));
        assert!(!presenter.enabled());
    }

    #[test]
    fn doctor_frame_uses_the_real_renderer_destination() {
        let capabilities = Capabilities {
            graphics: GraphicsProtocol::Kitty {
                direct_transmission: true,
            },
            cell_size: Some(CellSize::new(10, 20)),
            terminal_cells: Some((160, 48)),
        };
        let frame = diagnostic_frame(Ui::new(), capabilities, (160, 48))
            .expect("diagnostic renderer frame");
        assert_eq!(frame.mode, "graphics");
        assert_eq!((frame.area.width, frame.area.height), (160, 43));
        assert_eq!((frame.width, frame.height), (1_600, 860));
    }

    #[test]
    fn sixel_pacing_limits_rate_bytes_and_slow_writes() {
        assert_eq!(
            sixel_frame_interval(10_000, Duration::from_millis(10)),
            Duration::from_millis(200)
        );
        assert_eq!(
            sixel_frame_interval(8_000_000, Duration::ZERO),
            Duration::from_secs(1)
        );
        assert_eq!(
            sixel_frame_interval(1, Duration::from_secs(2)),
            Duration::from_secs(4)
        );
    }
}
