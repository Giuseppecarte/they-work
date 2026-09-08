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

fn click_at(ui: &mut Ui, x: u16, y: u16) -> Option<UiCommand> {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    ui.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })
}
fn click_action(
    ui: &mut Ui,
    predicate: impl Fn(&interaction::Action) -> bool,
) -> Option<UiCommand> {
    let hit = ui
        .presented_hits
        .iter()
        .rev()
        .find(|h| predicate(&h.action))
        .expect("action on presented frame")
        .clone();
    click_at(ui, hit.area.x, hit.area.y)
}

#[test]
fn pointer_uses_only_presented_geometry_and_resize_requires_an_acknowledged_frame() {
    use interaction::{Action, HitRegion};
    use ratatui::layout::Rect;
    let world = fixture(2, 1);
    let mut ui = Ui::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|f| ui.draw(f, &world)).unwrap();
    let first = world.offices().next().unwrap().workers[0].id.clone();
    let second = world.offices().nth(1).unwrap().workers[0].id.clone();
    ui.frame_hits = vec![HitRegion::new(
        Rect::new(10, 10, 4, 2),
        Action::Inspect(first.clone()),
    )];
    ui.frame_presented();
    // A newly composed frame may move or replace the person, but isn't visible yet.
    ui.frame_hits = vec![HitRegion::new(
        Rect::new(10, 10, 4, 2),
        Action::Inspect(second.clone()),
    )];
    click_at(&mut ui, 11, 11);
    assert_eq!(ui.phone_pending_worker, Some(first));
    ui.phone_pending_worker = None;
    ui.invalidate_pointer();
    click_at(&mut ui, 11, 11);
    assert_eq!(ui.phone_pending_worker, None);
    ui.frame_presented();
    click_at(&mut ui, 11, 11);
    assert_eq!(ui.phone_pending_worker, Some(second));
    ui.phone_pending_worker = None;
    ui.set_mouse_enabled(false);
    ui.frame_presented();
    click_at(&mut ui, 11, 11);
    assert_eq!(ui.phone_pending_worker, None);
}

#[test]
fn character_click_then_request_never_approves_and_expired_buttons_cannot_answer_replacements() {
    use interaction::Action;
    use views::control::{Choice, Command, ControlStatus, Request};
    let world = fixture(1, 1);
    let worker = world.offices().next().unwrap().workers[0].id.clone();
    let mut ui = Ui::new();
    ui.set_image_cell_size(Some((8, 16)));
    let mut terminal = Terminal::new(TestBackend::new(120, 36)).unwrap();
    let request = Request {
        id: "first".into(),
        worker: worker.clone(),
        origin: "Exact task".into(),
        title: "Allow this command?".into(),
        detail: "cargo test".into(),
        choices: vec![Choice {
            label: "Allow once".into(),
            response: serde_json::json!({"decision":"accept"}),
        }],
        questions: vec![],
    };
    ui.set_control_status(ControlStatus {
        requests: vec![request.clone()],
        ..Default::default()
    });
    terminal.draw(|f| ui.draw(f, &world)).unwrap();
    ui.frame_presented();
    assert!(click_action(&mut ui, |a| matches!(a,Action::Inspect(id) if *id==worker)).is_none());
    terminal.draw(|f| ui.draw(f, &world)).unwrap();
    ui.frame_presented();
    assert_eq!(ui.view(), View::Desk);
    assert!(click_action(&mut ui, |a| matches!(a,Action::Review(id) if *id==worker)).is_none());
    terminal.draw(|f| ui.draw(f, &world)).unwrap();
    ui.frame_presented();
    let stale = ui
        .presented_hits
        .iter()
        .find(|h| matches!(h.action, Action::Control(Command::Reply { .. })))
        .unwrap()
        .clone();
    let mut replacement = request;
    replacement.id = "replacement".into();
    ui.set_control_status(ControlStatus {
        requests: vec![replacement],
        ..Default::default()
    });
    assert!(click_at(&mut ui, stale.area.x, stale.area.y).is_none());
}

#[test]
fn inspector_switches_to_fullscreen_without_clicking_through_opaque_panels() {
    use interaction::Action;
    let world = fixture(1, 3);
    for (width, height, lateral) in [(80, 24, false), (120, 36, true), (192, 58, true)] {
        let mut ui = Ui::new();
        ui.set_image_cell_size(Some((8, 16)));
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| ui.draw(f, &world)).unwrap();
        ui.frame_presented();
        click_action(&mut ui, |a| matches!(a, Action::Inspect(_)));
        terminal.draw(|f| ui.draw(f, &world)).unwrap();
        ui.frame_presented();
        let team = ui
            .hit_regions()
            .iter()
            .find(|h| matches!(h.action, Action::Team(_)))
            .unwrap();
        assert_eq!(team.area.x >= width - 40, lateral);
        for hit in ui
            .hit_regions()
            .iter()
            .filter(|h| matches!(h.action, Action::Inspect(_)))
        {
            assert!(lateral && hit.area.right() <= width - 41);
        }
        let selected = ui.selected_worker_id.clone();
        ui.handle_key(key(KeyCode::Esc));
        terminal.draw(|f| ui.draw(f, &world)).unwrap();
        assert_eq!(ui.selected_worker_id, selected);
    }
}

#[test]
fn tab_traverses_controls_without_changing_floor_and_enter_matches_click() {
    let world = fixture(3, 2);
    let mut ui = Ui::new();
    ui.set_image_cell_size(Some((8, 16)));
    ui.open_tower();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|f| ui.draw(f, &world)).unwrap();
    let selected = ui.selected_office_id.clone();
    let target = ui
        .frame_hits
        .iter()
        .position(|h| h.action == interaction::Action::Connections)
        .unwrap();
    for _ in 0..=target {
        ui.handle_key(key(KeyCode::Tab));
    }
    assert_eq!(ui.selected_office_id, selected);
    assert!(ui.handle_key(key(KeyCode::Enter)).is_none());
    assert!(ui.controls.open);
}

#[test]
fn local_character_and_zone_changes_survive_restart_without_changing_tasks() {
    use design::{profile_for, OfficePreset};
    let world = fixture(2, 3);
    let worker = world.offices().next().unwrap().workers[0].id.clone();
    let office = world.offices().next().unwrap().id.clone();
    let original = world.worker(&worker).unwrap().clone();
    let mut ui = Ui::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|f| ui.draw(f, &world)).unwrap();
    ui.activate(interaction::Action::Character(worker.clone()));
    terminal.draw(|f| ui.draw(f, &world)).unwrap();
    ui.handle_key(key(KeyCode::Home));
    for _ in 0..32 {
        ui.handle_key(key(KeyCode::Delete));
    }
    ui.handle_paste("Álex 東京");
    ui.handle_key(key(KeyCode::Left));
    ui.handle_key(key(KeyCode::Backspace));
    ui.handle_key(key(KeyCode::Esc));
    assert_eq!(
        profile_for(&worker.0, &ui.preferences().character_profiles).name,
        "Álex 京"
    );
    ui.activate(interaction::Action::Decorate(office.clone()));
    terminal.draw(|f| ui.draw(f, &world)).unwrap();
    ui.handle_key(key(KeyCode::Right));
    ui.handle_key(key(KeyCode::Tab));
    ui.handle_key(key(KeyCode::Right));
    ui.handle_key(key(KeyCode::Esc));
    let saved: RendererPreferences =
        serde_json::from_slice(&serde_json::to_vec(&ui.preferences()).unwrap()).unwrap();
    assert_eq!(saved.office_designs[&office.0].entrance, 1);
    let _preset: OfficePreset = saved.office_designs[&office.0].preset;
    let mut restored = Ui::new();
    restored.restore_preferences(&saved);
    terminal.draw(|f| restored.draw(f, &fixture(1, 1))).unwrap();
    assert_eq!(
        restored.preferences().character_profiles,
        saved.character_profiles
    );
    assert_eq!(restored.preferences().office_designs, saved.office_designs);
    assert_eq!(world.worker(&worker).unwrap().name, original.name);
    assert_eq!(world.worker(&worker).unwrap().activity, original.activity);
}

#[test]
fn old_appearance_defaults_enable_mouse_and_preserve_explicit_wardrobe_and_palette() {
    let saved: RendererPreferences = serde_json::from_str(
        r#"{"wardrobe":{"task":4},"office_palettes":{"project":2},"motion":false}"#,
    )
    .unwrap();
    assert!(saved.mouse);
    assert!(saved.character_profiles.is_empty());
    let mut ui = Ui::new();
    ui.restore_preferences(&saved);
    assert_eq!(ui.preferences().wardrobe["task"], 4);
    assert_eq!(ui.preferences().office_palettes["project"], 2);
    assert!(!ui.preferences().motion);
}
