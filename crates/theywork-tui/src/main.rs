//! they-work: a read-only terminal office for local agent activity.
//!
//! This binary owns command-line policy and the polling loop. Collectors own
//! the data boundary; the renderer owns presentation state.

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event as TermEvent, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use theywork_collect::{Config, StoreReport};
use theywork_core::{Agent, Millis, Source, Worker, WorkerStatus, World};
use theywork_render::{Ui, UiCommand, View};

/// Redraw interval. Fast enough for smooth sprite animation, slow enough that
/// a full building costs almost nothing.
const FRAME: Duration = Duration::from_millis(100);
const POLL_INTERVAL: Duration = Duration::from_secs(1);

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
  --config-dir <path>      Opt in to remembering the selected office
  -h, --help               Show this help
";

const READ_PARAGRAPH: &str = "Claude Code data comes from regular .jsonl session files below ~/.claude/projects/; symlinks and non-JSONL files are skipped. Codex data comes from ~/.codex/sqlite/state_5.sqlite and ~/.codex/sqlite/thread_history_1.sqlite, opened read-only. The collectors inspect filesystem metadata and .git directory markers to group activity under a project root; they do not read project source files.";

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
        && (parsed.project.is_some() || parsed.all || parsed.doctor || parsed.config_dir.is_some())
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
    config_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FirstRunAction {
    Open(String),
    Guard,
    Stop,
    Quit,
}

fn main() -> Result<()> {
    let args = match parse_args(std::env::args().skip(1)) {
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

    if args.doctor {
        let status = doctor();
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
    if let (Some(config_dir), Some(project)) = (&runtime.config_dir, args.project.as_ref()) {
        write_selection(config_dir, &normalize_cli_path(project)?)?;
    }

    if should_show_first_run(&args, &runtime) {
        match first_run_screen(&runtime)? {
            FirstRunAction::Open(project) => {
                if let Some(config_dir) = runtime.config_dir.as_deref() {
                    write_selection(config_dir, &project)?;
                }
                select_project(&mut runtime, &project);
            }
            FirstRunAction::Guard => runtime.start_guard = true,
            FirstRunAction::Stop | FirstRunAction::Quit => return Ok(()),
        }
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
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let result = run(&mut terminal, &mut runtime, &args);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn doctor() -> i32 {
    let config = Config::discover();
    let reports = theywork_collect::inspect(&config, now_ms());

    println!("they-work doctor");
    for report in &reports {
        print_store_report(report);
    }
    println!("read={READ_PARAGRAPH}");
    println!("discovery_overrides={}", discovery_overrides());

    let found_home = reports.iter().any(|report| report.home_found);
    let broken_home = reports
        .iter()
        .any(|report| report.home_found && report.error.is_some());
    i32::from(!found_home || broken_home)
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

fn first_run_screen(runtime: &Runtime) -> Result<FirstRunAction> {
    let reports = theywork_collect::inspect(&runtime.config, runtime.now);
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    if !interactive {
        render_first_run(&reports, &runtime.world, runtime.now, 0, false)?;
        return Ok(FirstRunAction::Stop);
    }

    let has_home = reports.iter().any(|report| report.home_found);
    enable_raw_mode()?;
    let result = (|| -> Result<FirstRunAction> {
        let mut selected = 0;
        loop {
            render_first_run(&reports, &runtime.world, runtime.now, selected, true)?;
            if !event::poll(FRAME)? {
                continue;
            }
            let input = event::read()?;
            let TermEvent::Key(input) = input else {
                continue;
            };
            let offices = first_run_offices(&runtime.world, runtime.now);
            match input.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !offices.is_empty() {
                        selected = selected.saturating_add(1).min(offices.len() - 1);
                    }
                }
                KeyCode::Home => selected = 0,
                KeyCode::End if !offices.is_empty() => selected = offices.len() - 1,
                KeyCode::Enter if !offices.is_empty() => {
                    return Ok(FirstRunAction::Open(offices[selected].path.clone()));
                }
                KeyCode::Tab if has_home => return Ok(FirstRunAction::Guard),
                KeyCode::Tab => return Ok(FirstRunAction::Stop),
                KeyCode::Char('q') | KeyCode::Esc => return Ok(FirstRunAction::Quit),
                _ => {}
            }
        }
    })();
    let restored = disable_raw_mode();
    match (result, restored) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error.into()),
        (Ok(action), Ok(())) => Ok(action),
    }
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
                "projects={} threads={} active={}",
                report.projects, report.threads, report.active_threads
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
            "    {}_home={} path={}",
            report.agent.label(),
            home_state,
            plain_value(&report.path.to_string_lossy())
        )?;
        writeln!(stdout, "    {}_store={store_state}", report.agent.label())?;
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
        "{label}_home={home_state} path={}",
        plain_value(&report.path.to_string_lossy())
    );

    let store_state = if report.readable {
        "readable"
    } else {
        "unavailable"
    };
    print!(
        "{label}_store={store_state} projects={} threads={} active={}",
        report.projects, report.threads, report.active_threads
    );
    if let Some(error) = report.error.as_deref() {
        print!(" reason={}", plain_value(error));
    }
    println!();
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
            config_dir: None,
        });
    }

    let base_config = Config::discover();
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

    let (scan, start_guard) = if args.all {
        (scan_config(&base_config, now), true)
    } else if let Some(project) = explicit.as_deref() {
        (scoped_scan(&base_config, project, now), false)
    } else if let Some(project) = remembered.as_deref() {
        let remembered_scan = scoped_scan(&base_config, project, now);
        if remembered_scan.world.office_count() > 0 {
            (remembered_scan, false)
        } else if let Some(current_project) =
            current.as_deref().filter(|candidate| *candidate != project)
        {
            let current_scan = scoped_scan(&base_config, current_project, now);
            if current_scan.world.office_count() > 0 {
                (current_scan, false)
            } else {
                (scan_config(&base_config, now), true)
            }
        } else {
            (scan_config(&base_config, now), true)
        }
    } else if let Some(project) = current.as_deref() {
        let current_scan = scoped_scan(&base_config, project, now);
        if current_scan.world.office_count() > 0 {
            (current_scan, false)
        } else {
            (scan_config(&base_config, now), true)
        }
    } else {
        (scan_config(&base_config, now), true)
    };

    Ok(Runtime {
        config: scan.config,
        sources: scan.sources,
        world: scan.world,
        errors: scan.errors,
        now,
        demo: false,
        start_guard,
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

fn select_project(runtime: &mut Runtime, project: &str) {
    let scan = scoped_scan(&runtime.config, project, runtime.now);
    runtime.config = scan.config;
    runtime.sources = scan.sources;
    runtime.world = scan.world;
    runtime.errors = scan.errors;
    runtime.start_guard = false;
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
    if !config_dir.is_dir() {
        return Err(anyhow!(
            "config directory {} is not available",
            config_dir.display()
        ));
    }

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
            std::env::remove_var("NO_COLOR");
            std::env::set_var("THEYWORK_COLOR", "true");
        }
        Some(ColorMode::Palette256) => {
            std::env::remove_var("NO_COLOR");
            std::env::set_var("THEYWORK_COLOR", "256");
        }
        Some(ColorMode::None) => {
            std::env::remove_var("THEYWORK_COLOR");
            std::env::set_var("NO_COLOR", "1");
        }
    }
}

fn configure_ui(ui: &mut Ui, args: &Args, start_guard: bool) {
    if start_guard {
        ui.handle_key(key(KeyCode::Char('0')));
    }

    let cycles = match args.view {
        Some(StartView::Iso) => 1,
        Some(StartView::Top) => 2,
        Some(StartView::Side) => 3,
        None => 0,
    };
    for _ in 0..cycles {
        ui.handle_key(key(KeyCode::Char('c')));
    }

    if args.light {
        ui.handle_key(key(KeyCode::Char('s')));
        ui.handle_key(key(KeyCode::Down));
        ui.handle_key(key(KeyCode::Enter));
        ui.handle_key(key(KeyCode::Char('s')));
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn run_headless(runtime: &mut Runtime, duration: Duration, rss_before: Option<u64>) -> Result<()> {
    let started = Instant::now();
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
    let mut next_poll = Instant::now();

    loop {
        let frame_started = Instant::now();
        let now = now_ms();
        runtime.now = now;
        if Instant::now() >= next_poll {
            polls += 1;
            for source in &mut runtime.sources {
                match source.poll(now) {
                    Ok(source_events) => {
                        events += source_events.len();
                        for event in source_events {
                            runtime.world.apply(event);
                        }
                    }
                    Err(error) => {
                        poll_errors += 1;
                        errors.insert(format!("{}: {}", error.source_name, error.message));
                    }
                }
            }
            next_poll = Instant::now() + POLL_INTERVAL;
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

    let elapsed = started.elapsed();
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

fn resident_bytes() -> Option<u64> {
    let statm = fs::read_to_string("/proc/self/statm").ok()?;
    let pages = statm.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    pages.checked_mul(4_096)
}

fn optional_metric(value: Option<u64>) -> String {
    value.map_or_else(|| "unavailable".to_string(), |value| value.to_string())
}

fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    runtime: &mut Runtime,
    args: &Args,
) -> Result<()> {
    let mut ui = Ui::new();
    configure_ui(&mut ui, args, runtime.start_guard);
    let mut next_poll = Instant::now();

    loop {
        let now = now_ms();

        if runtime.demo {
            for event in theywork_core::demo::events(now) {
                runtime.world.apply(event);
            }
        } else if Instant::now() >= next_poll {
            for source in &mut runtime.sources {
                if let Ok(events) = source.poll(now) {
                    for event in events {
                        runtime.world.apply(event);
                    }
                }
            }
            next_poll = Instant::now() + POLL_INTERVAL;
        }
        runtime.world.tick(now);
        ui.tick(now);

        terminal.draw(|frame| ui.draw(frame, &runtime.world))?;

        if event::poll(FRAME)? {
            if let TermEvent::Key(input) = event::read()? {
                let previous_view = ui.view();
                if ui.handle_key(input) == Some(UiCommand::Quit) {
                    return Ok(());
                }
                if previous_view == View::Cameras && ui.view() == View::Office {
                    persist_selected_office(runtime, ui.selected_office(), now)?;
                }
            }
        }
    }
}
fn persist_selected_office(runtime: &Runtime, selected: usize, now: Millis) -> Result<()> {
    let (Some(config_dir), Some(project)) = (
        runtime.config_dir.as_deref(),
        selected_office_path(&runtime.world, selected, now),
    ) else {
        return Ok(());
    };
    write_selection(config_dir, &project)
}

fn selected_office_path(world: &World, selected: usize, now: Millis) -> Option<String> {
    let mut offices: Vec<_> = world.offices().collect();
    offices.sort_by(|left, right| {
        office_rank(left, now)
            .cmp(&office_rank(right, now))
            .then_with(|| left.path.cmp(&right.path))
    });
    offices.get(selected).map(|office| office.path.clone())
}

fn print_once(runtime: &Runtime) -> bool {
    let mut errors = runtime.errors.clone();
    if !runtime.demo {
        for report in theywork_collect::inspect(&runtime.config, runtime.now) {
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

    let has_errors = !errors.is_empty();
    for error in errors {
        println!("collector_error={}", plain_value(&error));
    }
    has_errors
}

fn print_worker(worker: &Worker, now: Millis) {
    let status = worker.status_at(now);
    print!(
        "  worker name={} agent={} status={} activity={} idle_age={} tokens={}",
        quoted_value(&worker.name),
        worker.agent.label(),
        status.label(),
        worker.activity.label(),
        format_age(now.saturating_sub(worker.last_seen)),
        worker.tokens_used
    );
    if status == WorkerStatus::Blocked {
        let waiting_on = worker_detail(worker).unwrap_or("no recent output");
        print!(" waiting_on={}", quoted_value(waiting_on));
    } else if let Some(detail) = worker_detail(worker) {
        print!(" detail={}", quoted_value(detail));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
