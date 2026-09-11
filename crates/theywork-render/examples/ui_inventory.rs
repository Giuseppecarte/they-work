//! Bounded audit matrix using real Ui routes. This produces evidence, not a
//! claim that snapshots certify real terminals, providers or human usability.
#[allow(dead_code)]
#[path = "tower_overview.rs"]
mod tower_fixture;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{backend::TestBackend, Terminal};
use serde_json::{json, Value};
use std::{fs, path::Path};
use theywork_core::{
    Activity, Beat, CollaborationEvent, CollaborationKind, Event, EventKind, Evidence,
    SourceCoverage, WaitReason, WorkerId, World,
};
use theywork_render::views::control::{Choice, ControlStatus, Request, TaskAccess};
use theywork_render::{
    interaction::Action, observation::ObservationSummary, work_brief::WorkTab, RendererPreferences,
    Ui,
};

const SIZES: [(u16, u16); 6] = [
    (32, 14),
    (80, 24),
    (120, 36),
    (192, 58),
    (110, 80),
    (240, 70),
];
const SURFACES: &[&str] = &[
    "tower",
    "office-auto",
    "office-side",
    "office-iso",
    "office-top",
    "office-list",
    "inspector-now",
    "inspector-activity",
    "inspector-team",
    "inspector-details",
    "attention",
    "deliveries",
    "changes",
    "team",
    "finder",
    "connections",
    "new-task",
    "task-controls",
    "request",
    "project-picker",
    "sources",
    "character",
    "design",
    "settings",
    "advanced",
    "help",
    "phone-now",
    "phone-attention",
    "phone-edits",
    "phone-messages",
    "more",
];

pub(crate) fn draw(ui: &mut Ui, terminal: &mut Terminal<TestBackend>, world: &World) {
    terminal.draw(|frame| ui.draw(frame, world)).unwrap();
    ui.frame_presented();
}
fn key(ui: &mut Ui, terminal: &mut Terminal<TestBackend>, world: &World, code: KeyCode) {
    ui.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    draw(ui, terminal, world);
}
fn click(ui: &mut Ui, terminal: &mut Terminal<TestBackend>, world: &World, action: Action) -> bool {
    let Some(hit) = ui
        .hit_regions()
        .iter()
        .rev()
        .find(|hit| hit.action == action)
        .cloned()
    else {
        return false;
    };
    ui.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: hit.area.x,
        row: hit.area.y,
        modifiers: KeyModifiers::NONE,
    });
    draw(ui, terminal, world);
    true
}
pub(crate) fn status() -> ControlStatus {
    let worker = WorkerId("floor-00-person-0".into());
    ControlStatus {
        can_start_codex:true, can_start_claude:true,
        codex:"Fixture: local managed capability".into(), claude:"Fixture: official console available".into(),
        tasks:std::collections::BTreeMap::from([(worker.0.clone(),TaskAccess{send:true,interrupt:true,native:false,reconnect:true,description:"Synthetic managed task; no provider process runs.".into()})]),
        requests:vec![Request{id:"fixture-request-1".into(),worker,origin:"Checkout / account interface".into(),title:"Review command".into(),detail:"Command: cargo test\nProject: /projects/00-checkout\nA recorded fixture request; no command will execute.".into(),choices:vec![Choice{label:"Allow once".into(),response:json!({"decision":"accept"})},Choice{label:"Decline".into(),response:json!({"decision":"decline"})}],questions:vec![]}],
        notice:String::new(),
    }
}

pub(crate) fn fixture() -> World {
    let mut world = tower_fixture::fixture(20);
    let workers = world
        .offices()
        .flat_map(|office| {
            office
                .workers
                .iter()
                .map(move |worker| (office.path.clone(), worker.clone()))
        })
        .collect::<Vec<_>>();
    for (path, worker) in workers {
        let event = |at, kind| Event {
            at,
            office: worker.office.clone(),
            office_path: path.clone(),
            worker: worker.id.clone(),
            agent: worker.agent,
            kind,
        };
        for (at, activity) in [
            (
                700,
                Activity::Reading {
                    detail: "src/account.rs".into(),
                },
            ),
            (
                800,
                Activity::Searching {
                    detail: "Find existing permission checks".into(),
                },
            ),
            (900, worker.activity.clone()),
        ] {
            world.apply(event(
                at,
                EventKind::Did(Beat {
                    at,
                    activity,
                    outcome: None,
                }),
            ));
        }
        world.apply(event(
            1_000,
            EventKind::Coverage(SourceCoverage {
                available: true,
                observed_at: 1_000,
                detail: "Synthetic local history; live provider compatibility is not exercised."
                    .into(),
                ..Default::default()
            }),
        ));
        if worker.id.0 == "floor-00-person-0" {
            world.apply(event(1_000,EventKind::Collaboration(CollaborationEvent{id:"fixture-result".into(),at:1_000,actor:worker.id.clone(),recipient:None,kind:CollaborationKind::Result,text:Some("Implemented the account form and checked the required fields. The next step is a local review.".into()),correlation_id:None,native_turn_id:Some("turn-fixture".into()),native_item_id:Some("item-result".into()),evidence:Evidence::Demo})));
            world.apply(event(
                1_000,
                EventKind::Wait(Some(WaitReason::HumanApproval)),
            ));
        }
    }
    world
}

pub(crate) fn route(
    surface: &str,
    ui: &mut Ui,
    terminal: &mut Terminal<TestBackend>,
    world: &World,
) -> bool {
    let key =
        |ui: &mut Ui, terminal: &mut Terminal<TestBackend>, code| key(ui, terminal, world, code);
    match surface {
        "tower" => {}
        name if name.starts_with("office-") => key(ui, terminal, KeyCode::Enter),
        name if name.starts_with("inspector-") => {
            let original = terminal.backend().buffer().area;
            let emergency = original.width < 60 || original.height < 18;
            if emergency && name != "inspector-now" {
                terminal.backend_mut().resize(80, 24);
                terminal
                    .resize(ratatui::layout::Rect::new(0, 0, 80, 24))
                    .unwrap();
                ui.invalidate_pointer();
                draw(ui, terminal, world);
            }
            key(ui, terminal, KeyCode::Enter);
            key(ui, terminal, KeyCode::Enter);
            let tab = match name {
                "inspector-activity" => WorkTab::Activity,
                "inspector-team" => WorkTab::Team,
                "inspector-details" => WorkTab::Details,
                _ => WorkTab::Now,
            };
            if tab != WorkTab::Now {
                let reached = click(ui, terminal, world, Action::WorkTab(tab));
                if emergency {
                    terminal
                        .backend_mut()
                        .resize(original.width, original.height);
                    terminal.resize(original).unwrap();
                    ui.invalidate_pointer();
                    draw(ui, terminal, world);
                }
                return reached;
            }
        }
        "attention" | "deliveries" | "changes" | "team" => {
            key(ui, terminal, KeyCode::Char('b'));
            key(
                ui,
                terminal,
                KeyCode::Char(match surface {
                    "deliveries" => '2',
                    "changes" => '3',
                    "team" => '4',
                    _ => '1',
                }),
            );
        }
        "finder" => {
            key(ui, terminal, KeyCode::Char('/'));
            ui.handle_paste("account");
            draw(ui, terminal, world);
        }
        "connections" => key(ui, terminal, KeyCode::Char('c')),
        "new-task" | "project-picker" => {
            key(ui, terminal, KeyCode::Char('n'));
            if surface == "project-picker" {
                key(ui, terminal, KeyCode::F(7));
            }
        }
        "task-controls" => {
            key(ui, terminal, KeyCode::Enter);
            key(ui, terminal, KeyCode::Char('m'));
        }
        "request" => {
            key(ui, terminal, KeyCode::Enter);
            key(ui, terminal, KeyCode::Enter);
            let reached = click(
                ui,
                terminal,
                world,
                Action::Review(WorkerId("floor-00-person-0".into())),
            );
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            return reached && text.contains("REVIEW REQUEST");
        }
        "character" => {
            key(ui, terminal, KeyCode::Enter);
            key(ui, terminal, KeyCode::Enter);
            key(ui, terminal, KeyCode::Char('a'));
        }
        "design" => key(ui, terminal, KeyCode::Char('d')),
        "settings" | "advanced" => {
            key(ui, terminal, KeyCode::Char('s'));
            if surface == "advanced" {
                return click(ui, terminal, world, Action::Setting(5));
            }
        }
        "help" => key(ui, terminal, KeyCode::Char('?')),
        name if name.starts_with("phone-") => {
            key(ui, terminal, KeyCode::Char('p'));
            key(
                ui,
                terminal,
                KeyCode::Char(match name {
                    "phone-attention" => '2',
                    "phone-edits" => '3',
                    "phone-messages" => '4',
                    _ => '1',
                }),
            );
        }
        "more" => {
            // Overflow is conditional, so reach it in a narrow viewport first
            // and then verify the open menu survives the requested resize.
            let original = terminal.backend().buffer().area;
            if !ui
                .hit_regions()
                .iter()
                .any(|hit| hit.action == Action::More)
            {
                terminal.backend_mut().resize(32, original.height);
                terminal
                    .resize(ratatui::layout::Rect::new(0, 0, 32, original.height))
                    .unwrap();
                ui.invalidate_pointer();
                draw(ui, terminal, world);
            }
            let reached = click(ui, terminal, world, Action::More);
            terminal
                .backend_mut()
                .resize(original.width, original.height);
            terminal.resize(original).unwrap();
            ui.invalidate_pointer();
            draw(ui, terminal, world);
            return reached;
        }
        _ => return false,
    }
    true
}

pub(crate) fn record(
    out: &Path,
    name: &str,
    surface: &str,
    mode: &str,
    ui: &Ui,
    terminal: &Terminal<TestBackend>,
    capture: (bool, (u16, u16)),
) -> Value {
    let (reached, cell_pixels) = capture;
    tower_fixture::save_with_cells(ui, terminal, out, name, cell_pixels);
    let buffer = terminal.backend().buffer();
    let text = buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(out.join(format!("{name}.txt")), &text).unwrap();
    fs::write(out.join(format!("{name}.hits.json")),serde_json::to_string_pretty(&ui.hit_regions().iter().map(|hit|json!({"x":hit.area.x,"y":hit.area.y,"width":hit.area.width,"height":hit.area.height,"action":format!("{:?}",hit.action)})).collect::<Vec<_>>()).unwrap()).unwrap();
    let bounds = ui.hit_regions().iter().all(|hit| {
        hit.area.width > 0 && hit.area.height > 0 && buffer.area.intersection(hit.area) == hit.area
    });
    let emergency_brief =
        surface.starts_with("inspector-") && (buffer.area.width < 60 || buffer.area.height < 18);
    json!({"id":name,"surface":surface,"mode":mode,"columns":buffer.area.width,"rows":buffer.area.height,
        "rendered_surface":if emergency_brief {"compact-work-brief"}else{surface},
        "limitation":if emergency_brief {Some("Emergency work brief: full tabs require a larger window. Non-Now tabs were opened at 80x24 before resizing; this frame does not claim their controls are available here.")}else{None},
        "status":if reached&&bounds {"not-tested"}else{"defect"},
        "reason":if reached&&bounds {"Automatic route and hit bounds passed; visual review pending."}else{"Expected route unavailable or hit region outside viewport."},
        "automatic":{"route":reached,"hit_bounds":bounds},"visual_review":"not-tested","terminal_transport":"not-tested",
        "configuration":{"cell_pixels":cell_pixels,"effective_color":format!("{:?}",ui.diagnostics().color_depth),"effective_encoding":format!("{:?}",ui.diagnostics().encoding)},
        "native_text":format!("{name}.txt"),"native_cells":format!("{name}.cells.json"),"physical_rgba":format!("{name}.rgba")})
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let out = Path::new(
        args.get(1)
            .expect("provide repo-local audit scratch output"),
    );
    let filter = args.get(2).map(String::as_str).unwrap_or("");
    let cell_pixels = match args.get(3).map(String::as_str).unwrap_or("8x16") {
        "8x16" => (8, 16),
        "10x20" => (10, 20),
        other => panic!("Unsupported audit cell geometry: {other}"),
    };
    fs::create_dir_all(out).unwrap();
    let world = fixture();
    let mut cases = Vec::new();
    for &surface in SURFACES {
        for &(width, height) in &SIZES {
            for mode in ["image", "native", "light", "no-color"] {
                if mode != "image" && (width, height) != (80, 24) {
                    continue;
                }
                let name = format!("{surface}-{mode}-{width}x{height}");
                if !name.contains(filter) {
                    continue;
                }
                if surface == "sources" {
                    cases.push(json!({"id":name,"surface":surface,"mode":mode,"columns":width,"rows":height,"status":"not-tested","reason":"Source chooser belongs to the executable; requires the isolated PTY inventory."}));
                    continue;
                }
                let mut ui = Ui::new();
                let projection = match surface {
                    "office-side" => "side",
                    "office-iso" => "isometric",
                    "office-top" => "top-down",
                    "office-list" => "list",
                    _ => "auto",
                };
                ui.restore_preferences(&RendererPreferences {
                    projection: projection.into(),
                    motion: false,
                    light: mode == "light",
                    color_depth: Some(
                        if mode == "no-color" {
                            "none"
                        } else {
                            "truecolor"
                        }
                        .into(),
                    ),
                    ..Default::default()
                });
                if mode != "native" {
                    ui.set_image_cell_size(Some(cell_pixels));
                }
                ui.set_control_status(status());
                ui.tick(1_000);
                ui.open_tower();
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                draw(&mut ui, &mut terminal, &world);
                let reached = route(surface, &mut ui, &mut terminal, &world);
                cases.push(record(
                    out,
                    &name,
                    surface,
                    mode,
                    &ui,
                    &terminal,
                    (reached, cell_pixels),
                ));
            }
        }
    }
    for (state, summary) in [
        (
            "no-sources",
            ObservationSummary {
                enabled_sources: 0,
                ..Default::default()
            },
        ),
        (
            "scanning",
            ObservationSummary {
                scanning: true,
                ..Default::default()
            },
        ),
        (
            "source-error",
            ObservationSummary {
                errors: vec!["Fixture: selected folder is unavailable.".into()],
                ..Default::default()
            },
        ),
        (
            "filtered-empty",
            ObservationSummary {
                filtered: true,
                ..Default::default()
            },
        ),
        (
            "empty",
            ObservationSummary {
                checked_at: 1_000,
                ..Default::default()
            },
        ),
    ] {
        for &(width, height) in &SIZES {
            let name = format!("{state}-image-{width}x{height}");
            if !name.contains(filter) {
                continue;
            }
            let mut ui = Ui::new();
            ui.restore_preferences(&RendererPreferences {
                motion: false,
                color_depth: Some("truecolor".into()),
                ..Default::default()
            });
            ui.set_image_cell_size(Some(cell_pixels));
            ui.set_observation_summary(summary.clone());
            ui.tick(1_000);
            ui.open_tower();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            draw(&mut ui, &mut terminal, &World::new());
            cases.push(record(
                out,
                &name,
                state,
                "image",
                &ui,
                &terminal,
                (true, cell_pixels),
            ));
        }
    }
    for state in [
        "single-project",
        "single-worker",
        "last-floor",
        "last-person",
        "names-off",
        "source-stale",
        "source-offline",
        "mixed-waits",
        "finder-none",
        "long-names",
    ] {
        for &(width, height) in &SIZES {
            let name = format!("directed-{state}-{width}x{height}");
            if !name.contains(filter) {
                continue;
            }
            let mut directed = if state == "single-project" {
                tower_fixture::fixture(1)
            } else {
                world.clone()
            };
            if state == "single-worker" {
                directed = World::new();
                directed.apply(Event {
                    at: 1_000,
                    office: theywork_core::OfficeId("/projects/solo".into()),
                    office_path: "/projects/solo".into(),
                    worker: WorkerId("solo".into()),
                    agent: theywork_core::Agent::Codex,
                    kind: EventKind::Seen {
                        name: "A single recorded conversation".into(),
                        git_branch: None,
                    },
                });
            }
            let selected = directed
                .offices()
                .next()
                .map(|office| (office.path.clone(), office.workers.clone()));
            if let Some((path, workers)) = selected {
                for (index, worker) in workers.into_iter().enumerate() {
                    let kind=match state {
                        "source-offline"=>Some(EventKind::Coverage(SourceCoverage{available:false,observed_at:1_000,detail:"Fixture: source disconnected after the last observation.".into(),..Default::default()})),
                        "long-names"=>Some(EventKind::Seen{name:format!("東京 — Shared account permissions and billing settings with a very long repeated prefix — distinct task {index}"),git_branch:None}),
                        "mixed-waits"=>Some(EventKind::Wait(Some([theywork_core::WaitReason::HumanApproval,theywork_core::WaitReason::HumanInput,theywork_core::WaitReason::AutomaticReview,theywork_core::WaitReason::Child,theywork_core::WaitReason::Process,theywork_core::WaitReason::Unknown][index%6]))),
                        _=>None,
                    };
                    if let Some(kind) = kind {
                        directed.apply(Event {
                            at: 1_000,
                            office: worker.office,
                            office_path: path.clone(),
                            worker: worker.id,
                            agent: worker.agent,
                            kind,
                        });
                    }
                }
            }
            let mut ui = Ui::new();
            ui.restore_preferences(&RendererPreferences {
                motion: false,
                name_plates: state != "names-off",
                color_depth: Some("truecolor".into()),
                ..Default::default()
            });
            ui.set_image_cell_size(Some(cell_pixels));
            ui.tick(if state == "source-stale" {
                1_001 + theywork_core::BLOCKED_AFTER_MS
            } else {
                1_000
            });
            ui.open_tower();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            draw(&mut ui, &mut terminal, &directed);
            if state == "last-floor" {
                key(&mut ui, &mut terminal, &directed, KeyCode::End);
            }
            if state == "last-person" {
                key(&mut ui, &mut terminal, &directed, KeyCode::Enter);
                key(&mut ui, &mut terminal, &directed, KeyCode::End);
            }
            if state == "finder-none" {
                key(&mut ui, &mut terminal, &directed, KeyCode::Char('/'));
                ui.handle_paste("no-such-project-or-task");
                draw(&mut ui, &mut terminal, &directed);
            }
            cases.push(record(
                out,
                &name,
                state,
                "image",
                &ui,
                &terminal,
                (true, cell_pixels),
            ));
        }
    }
    for (state, action) in [
        ("work-expanded", Action::WorkExpand),
        ("work-actions", Action::WorkActions),
        ("work-results", Action::WorkResults),
    ] {
        for (width, height) in [(80, 24), (192, 58)] {
            let name = format!("directed-{state}-{width}x{height}");
            if !name.contains(filter) {
                continue;
            }
            let mut ui = Ui::new();
            ui.restore_preferences(&RendererPreferences {
                motion: false,
                color_depth: Some("truecolor".into()),
                ..Default::default()
            });
            ui.set_image_cell_size(Some(cell_pixels));
            ui.set_control_status(status());
            ui.tick(1_000);
            ui.open_tower();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            draw(&mut ui, &mut terminal, &world);
            let mut reached = route("inspector-now", &mut ui, &mut terminal, &world);
            if state == "work-results" {
                reached &= click(
                    &mut ui,
                    &mut terminal,
                    &world,
                    Action::WorkTab(WorkTab::Activity),
                );
            }
            reached &= click(&mut ui, &mut terminal, &world, action.clone());
            cases.push(record(
                out,
                &name,
                state,
                "image",
                &ui,
                &terminal,
                (reached, cell_pixels),
            ));
        }
    }
    fs::write(out.join("inventory.json"),serde_json::to_string_pretty(&json!({"schema":1,"kind":"Real Ui / TestBackend; no real terminal or provider was invoked.","sizes":SIZES,"surfaces":SURFACES,"cases":cases})).unwrap()).unwrap();
    println!(
        "{} cases recorded; visual and real-terminal verdicts remain explicit.",
        cases.len()
    );
}
