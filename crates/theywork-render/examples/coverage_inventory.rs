//! Complete real-Ui coverage surfaces for the iteration-9 bounded-history audit.
//! This exports compositor evidence, not physical-terminal or usability claims.
#[allow(dead_code)]
#[path = "tower_overview.rs"]
mod tower_fixture;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};
use theywork_core::{
    Activity, Agent, Beat, CollaborationEvent, CollaborationKind, Event, EventKind, Evidence,
    OfficeId, SequenceRange, SourceCoverage, SourceId, StreamContinuity, ThreadIdentity,
    ToolCorrelationCoverage, WaitReason, WorkerId, WorkerRole, World,
};
use theywork_render::views::control::{Choice, ControlStatus, Request, TaskAccess};
use theywork_render::{interaction::Action, work_brief::WorkTab, RendererPreferences, Ui};

const NOW: i64 = 1_000;
const SIZES: [(u16, u16); 6] = [
    (32, 14),
    (80, 24),
    (120, 36),
    (192, 58),
    (110, 80),
    (240, 70),
];
const SURFACES: [&str; 6] = [
    "inspector-now",
    "inspector-details",
    "attention",
    "deliveries",
    "changes",
    "coverage",
];

fn emit(world: &mut World, native: &str, project: &str, agent: Agent, at: i64, kind: EventKind) {
    world.apply(Event {
        at,
        office: OfficeId(project.into()),
        office_path: project.into(),
        worker: WorkerId(native.into()),
        agent,
        kind,
    });
}

fn fixture() -> World {
    let mut world = World::new();
    for (native, project, agent, title) in [
        (
            "account-owner",
            "/projects/accounts",
            Agent::Codex,
            "Review the account permission changes",
        ),
        (
            "account-review",
            "/projects/accounts",
            Agent::Claude,
            "Check the account migration outcomes",
        ),
        (
            "job-owner",
            "/projects/jobs",
            Agent::Codex,
            "Update the scheduled job handler",
        ),
    ] {
        emit(
            &mut world,
            native,
            project,
            agent,
            990,
            EventKind::Seen {
                name: title.into(),
                git_branch: Some("fixture/work".into()),
            },
        );
        emit(
            &mut world,
            native,
            project,
            agent,
            990,
            EventKind::Identity {
                identity: ThreadIdentity::new(
                    agent,
                    SourceId(format!("/fixture/{}", agent.label())),
                    native,
                ),
                role: WorkerRole::Main,
            },
        );
        emit(
            &mut world,
            native,
            project,
            agent,
            991,
            EventKind::Turn { in_flight: true },
        );
        emit(
            &mut world,
            native,
            project,
            agent,
            995,
            EventKind::Did(Beat {
                at: 995,
                activity: Activity::Editing {
                    detail: "src/accounts.rs".into(),
                },
                outcome: None,
            }),
        );
        let mut coverage = SourceCoverage { available: true, observed_at: NOW, detail: "Synthetic local observation fixture. No authenticated provider or real command is used.".into(), ..SourceCoverage::default() };
        if agent == Agent::Codex {
            coverage.stream = Some(StreamContinuity {
                source: SourceId("/fixture/codex".into()),
                stream_id: Some("fixture-stream".into()),
                lineage_known: true,
                missing_events: 300,
                missing_ranges: vec![SequenceRange {
                    first: 2,
                    last: 301,
                }],
                ..StreamContinuity::default()
            });
        } else {
            coverage.tool_correlation = Some(ToolCorrelationCoverage {
                epoch: 1,
                evicted: 2,
                oversized: 1,
                observed_at: NOW,
                ..ToolCorrelationCoverage::default()
            });
        }
        emit(
            &mut world,
            native,
            project,
            agent,
            NOW,
            EventKind::Coverage(coverage),
        );
    }
    for index in 0..514 {
        let kind = if index == 512 {
            CollaborationKind::Result
        } else if index == 513 {
            CollaborationKind::HumanRequest
        } else {
            CollaborationKind::Message
        };
        let text = match kind {
            CollaborationKind::Result => "Recorded result: permission fixture updated; review is still required.".to_owned(),
            CollaborationKind::HumanRequest => "Review the simulated command for the account project. Opening this request does not approve it.".to_owned(),
            _ => format!("Recorded fixture update {index}; no model-generated summary."),
        };
        let at = if index >= 512 {
            997 + (index - 512) as i64
        } else {
            index as i64 + 1
        };
        emit(
            &mut world,
            "account-owner",
            "/projects/accounts",
            Agent::Codex,
            at,
            EventKind::Collaboration(CollaborationEvent {
                id: format!("fixture-{index}"),
                at,
                actor: WorkerId("account-owner".into()),
                recipient: None,
                kind,
                text: Some(text),
                correlation_id: Some(format!("fixture-item-{index}")),
                native_turn_id: Some("fixture-turn".into()),
                native_item_id: Some(format!("fixture-item-{index}")),
                evidence: Evidence::Demo,
            }),
        );
    }
    emit(
        &mut world,
        "account-owner",
        "/projects/accounts",
        Agent::Codex,
        999,
        EventKind::Wait(Some(WaitReason::HumanApproval)),
    );
    assert_eq!(world.collaboration().count(), 512);
    assert_eq!(
        world
            .history_window(&OfficeId("/projects/accounts".into()))
            .evicted_count,
        2
    );
    world
}

fn controls() -> ControlStatus {
    let worker = WorkerId("account-owner".into());
    ControlStatus {
        tasks: BTreeMap::from([(worker.0.clone(), TaskAccess { send: true, interrupt: true, native: false, reconnect: false, description: "Simulated local control; this exporter has no provider connection.".into() })]),
        requests: vec![Request { id: "fixture-exact-request".into(), worker, origin: "Accounts / Review the account permission changes".into(), title: "Review simulated command".into(), detail: "Command: fixture-only-check\nProject: /projects/accounts\nA simulated request; no command will execute.".into(), choices: vec![Choice { label: "Allow once".into(), response: json!({"decision":"accept"}) }, Choice { label: "Decline".into(), response: json!({"decision":"decline"}) }], questions: Vec::new() }],
        ..ControlStatus::default()
    }
}

fn draw(ui: &mut Ui, terminal: &mut Terminal<TestBackend>, world: &World) {
    terminal.draw(|frame| ui.draw(frame, world)).unwrap();
    ui.frame_presented();
}

fn key(ui: &mut Ui, terminal: &mut Terminal<TestBackend>, world: &World, code: KeyCode) {
    ui.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    draw(ui, terminal, world);
}

fn route(surface: &str, ui: &mut Ui, terminal: &mut Terminal<TestBackend>, world: &World) -> bool {
    if surface.starts_with("inspector-") {
        let original = terminal.backend().buffer().area;
        let enlarged =
            surface == "inspector-details" && (original.width < 60 || original.height < 18);
        if enlarged {
            terminal.backend_mut().resize(80, 24);
            terminal.resize(Rect::new(0, 0, 80, 24)).unwrap();
            ui.invalidate_pointer();
            draw(ui, terminal, world);
        }
        key(ui, terminal, world, KeyCode::Enter);
        key(ui, terminal, world, KeyCode::Enter);
        if surface == "inspector-details" {
            let Some(hit) = ui
                .hit_regions()
                .iter()
                .find(|hit| hit.action == Action::WorkTab(WorkTab::Details))
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
        }
        if enlarged {
            terminal
                .backend_mut()
                .resize(original.width, original.height);
            terminal.resize(original).unwrap();
            ui.invalidate_pointer();
            draw(ui, terminal, world);
        }
    } else {
        key(ui, terminal, world, KeyCode::Char('b'));
        key(
            ui,
            terminal,
            world,
            KeyCode::Char(match surface {
                "deliveries" => '2',
                "changes" => '3',
                _ => '1',
            }),
        );
        if surface == "coverage" {
            key(ui, terminal, world, KeyCode::Char('h'));
        }
    }
    true
}

fn capture(
    out: &Path,
    name: &str,
    ui: &Ui,
    terminal: &Terminal<TestBackend>,
    cells: (u16, u16),
) -> Value {
    tower_fixture::save_with_cells(ui, terminal, out, name, cells);
    let buffer = terminal.backend().buffer();
    let text = buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(out.join(format!("{name}.txt")), &text).unwrap();
    let hits = ui.hit_regions().iter().map(|hit| json!({"x":hit.area.x,"y":hit.area.y,"width":hit.area.width,"height":hit.area.height,"action":format!("{:?}",hit.action)})).collect::<Vec<_>>();
    fs::write(
        out.join(format!("{name}.hits.json")),
        serde_json::to_string_pretty(&hits).unwrap(),
    )
    .unwrap();
    json!({"hit_bounds":ui.hit_regions().iter().all(|hit| hit.area.width>0 && hit.area.height>0 && buffer.area.intersection(hit.area)==hit.area),"text":format!("{name}.txt"),"cells":format!("{name}.cells.json"),"rgba":format!("{name}.rgba"),"hits":format!("{name}.hits.json")})
}

fn main() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/audit/iteration-9/captures");
    fs::create_dir_all(&out).unwrap();
    let filter = std::env::args().nth(1).unwrap_or_default();
    let world = fixture();
    let mut cases = Vec::new();
    for cells in [(8, 16), (10, 20)] {
        for surface in SURFACES {
            for (width, height) in SIZES {
                for mode in ["image", "light", "mono"] {
                    if mode != "image" && (width, height) != (80, 24) {
                        continue;
                    }
                    let name = format!("{surface}-{mode}-{width}x{height}-{}x{}", cells.0, cells.1);
                    if !name.contains(&filter) {
                        continue;
                    }
                    let mut ui = Ui::new();
                    ui.restore_preferences(&RendererPreferences {
                        projection: "side".into(),
                        motion: false,
                        light: mode == "light",
                        color_depth: Some(if mode == "mono" { "none" } else { "truecolor" }.into()),
                        ..RendererPreferences::default()
                    });
                    if mode != "mono" {
                        ui.set_image_cell_size(Some(cells));
                    }
                    ui.set_control_status(controls());
                    ui.tick(NOW);
                    ui.open_tower();
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    draw(&mut ui, &mut terminal, &world);
                    let reached = route(surface, &mut ui, &mut terminal, &world);
                    let artifacts = capture(&out, &name, &ui, &terminal, cells);
                    cases.push(json!({"id":name,"surface":surface,"columns":width,"rows":height,"cell_pixels":cells,"mode":mode,"route_reached":reached,"compact_limitation":if surface.starts_with("inspector-") && (width<60 || height<18) {Some("Compact brief is the rendered fallback; Details was selected at80x24 before resizing and does not imply tabs are available at32x14.")}else{None},"artifacts":artifacts,"visual_review":"not-tested","terminal_transport":"not-tested"}));
                }
            }
        }
    }
    let all_ok = cases
        .iter()
        .all(|case| case["route_reached"] == true && case["artifacts"]["hit_bounds"] == true);
    fs::write(out.join("coverage-manifest.json"), serde_json::to_string_pretty(&json!({"schema_version":1,"kind":"full Ui composition export","fixed_time":NOW,"motion":false,"fixture":{"provider_stream_events_missing":300,"local_retention_evictions":2,"tool_correlation":{"evicted":2,"oversized":1},"current_requests":1,"retained_collaboration_records":512},"cases":cases,"automatic_pass":all_ok,"limitations":["Synthetic current request: no provider connection or command execution.","Mouse events select the Details tab; all other routes use keyboard events with redraw after each input.","Mono is an image-free native view.","These captures do not measure terminal transport, authenticated integration or user understanding."]})).unwrap()).unwrap();
    assert!(
        all_ok,
        "Some Ui routes or hit bounds failed; inspect coverage-manifest.json"
    );
    println!(
        "Exported {} complete coverage surfaces to {}",
        cases.len(),
        out.display()
    );
}
