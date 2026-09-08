//! Behavioral integration checks for the graphic view and explicit controls.
use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use theywork_core::{Agent, Event, EventKind, SourceId, ThreadIdentity, WorkerId, WorkerRole};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn fixture(projects: usize, first_workers: usize) -> World {
    let mut world = World::new();
    for floor in 0..projects {
        let project = format!("/projects/floor-{floor:02}");
        for index in 0..if floor == 0 { first_workers } else { 1 } {
            let identity = ThreadIdentity::new(
                Agent::Codex,
                SourceId("/fixtures/codex".into()),
                format!("floor-{floor:02}-task-{index:02}"),
            );
            for kind in [
                EventKind::Seen {
                    name: format!("Task {floor:02}-{index:02}"),
                    git_branch: None,
                },
                EventKind::Identity {
                    identity: identity.clone(),
                    role: WorkerRole::Main,
                },
            ] {
                world.apply(Event {
                    at: 1000,
                    office: OfficeId(project.clone()),
                    office_path: project.clone(),
                    worker: identity.worker_id(),
                    agent: Agent::Codex,
                    kind,
                });
            }
        }
    }
    world
}

#[test]
fn graphical_navigation_reaches_every_floor_and_last_of_fifty_without_losing_labels() {
    for projects in [1, 6, 20] {
        let world = fixture(projects, 50);
        for (width, height) in [(80, 24), (120, 36), (192, 58)] {
            let mut ui = Ui::new();
            ui.set_image_cell_size(Some((8, 16)));
            ui.motion = false;
            ui.tick(1000);
            ui.open_tower();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            ui.handle_key(key(KeyCode::End));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(
                ui.selected_office_id.as_ref().unwrap().0,
                format!("/projects/floor-{:02}", projects - 1)
            );
            ui.handle_key(key(KeyCode::Home));
            ui.handle_key(key(KeyCode::Enter));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            ui.handle_key(key(KeyCode::End));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            let id = ThreadIdentity::new(
                Agent::Codex,
                SourceId("/fixtures/codex".into()),
                "floor-00-task-49",
            )
            .worker_id();
            assert_eq!(ui.selected_worker_id, Some(id));
            let frame = ui.pixel_frame();
            assert!(frame.cell_area().is_some());
            assert!(
                frame
                    .text_cells()
                    .iter()
                    .any(|(_, _, cell)| cell.symbol() == "T"),
                "the selected task has a native nameplate at {width}x{height}"
            );
            let symbols: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(symbols.contains("Task 00-49"));
            ui.handle_key(key(KeyCode::Enter));
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            assert_eq!(ui.view(), View::Desk);
        }
    }
}

#[test]
fn legacy_explicit_costume_migrates_to_scoped_identity_and_survives_roster_changes() {
    let world = fixture(1, 2);
    let mut ui = Ui::new();
    ui.restore_preferences(&RendererPreferences {
        wardrobe: BTreeMap::from([("floor-00-task-00".into(), 4)]),
        ..Default::default()
    });
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
    let worker = &world.offices().next().unwrap().workers[0];
    assert_eq!(ui.preferences().wardrobe.get(&worker.id.0), Some(&4));
    assert!(!ui.preferences().wardrobe.contains_key("floor-00-task-00"));
    let mut restored = Ui::new();
    restored.restore_preferences(&ui.preferences());
    terminal
        .draw(|frame| restored.draw(frame, &fixture(1, 1)))
        .unwrap();
    assert_eq!(restored.preferences().wardrobe.get(&worker.id.0), Some(&4));
}

#[test]
fn resize_hides_and_disables_approval_actions_until_the_request_is_visible() {
    use views::control::{Choice, ControlStatus, Request};
    let world = fixture(1, 1);
    let mut ui = Ui::new();
    let worker = world.offices().next().unwrap().workers[0].id.clone();
    let mut normal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    normal.draw(|frame| ui.draw(frame, &world)).unwrap();
    ui.handle_key(key(KeyCode::Char('m')));
    normal.draw(|frame| ui.draw(frame, &world)).unwrap();
    ui.set_control_status(ControlStatus {
        requests: vec![Request {
            id: "request-a".into(),
            worker,
            origin: "Project / exact task A".into(),
            title: "Review command".into(),
            detail: "echo fixture".into(),
            choices: vec![Choice {
                label: "Allow".into(),
                response: serde_json::json!({"decision":"accept"}),
            }],
            questions: vec![],
        }],
        ..Default::default()
    });
    ui.handle_key(key(KeyCode::F(4)));
    normal.draw(|frame| ui.draw(frame, &world)).unwrap();
    let mut tiny = Terminal::new(TestBackend::new(20, 8)).unwrap();
    tiny.draw(|frame| ui.draw(frame, &world)).unwrap();
    assert!(ui.handle_key(key(KeyCode::Char('1'))).is_none());
    normal.draw(|frame| ui.draw(frame, &world)).unwrap();
    assert!(
        matches!(ui.handle_key(key(KeyCode::Char('1'))),Some(UiCommand::Control(views::control::Command::Reply{request,..})) if request=="request-a")
    );
}

#[test]
fn unbound_historical_worker_does_not_gain_send_authority() {
    let world = fixture(1, 1);
    let mut ui = Ui::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
    ui.handle_key(key(KeyCode::Char('m')));
    terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
    ui.handle_paste("Do not send without an owning connection");
    assert!(ui.handle_key(key(KeyCode::F(5))).is_none());
    assert!(ui.control_modal());
    let _: Option<WorkerId> = ui.selected_worker_id;
}
