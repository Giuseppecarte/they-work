//! Offline input-to-composition timing; excludes terminal output and OS paint.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use std::time::Instant;
use theywork_core::{Agent, Event, EventKind, OfficeId, WorkerId, World};
use theywork_render::{RendererPreferences, Ui, View};

fn world(projects: usize) -> World {
    let mut world = World::new();
    for index in 0..50 {
        let path = format!("/fixture/project-{:02}", index % projects);
        world.apply(Event {
            at: 1000,
            office: OfficeId(path.clone()),
            office_path: path,
            worker: WorkerId(format!("task-{index:02}")),
            agent: Agent::Codex,
            kind: EventKind::Seen {
                name: format!("Task {index:02}"),
                git_branch: None,
            },
        });
    }
    world
}
fn main() {
    println!("view,projects,tasks,columns,rows,physical_width,physical_height,samples,p50_ms,p95_ms,max_ms");
    for view in ["tower", "office", "inspector"] {
        for projects in [1, 6, 20] {
            for (columns, rows) in [(80, 24), (120, 36), (192, 58)] {
                let world = world(projects);
                let mut ui = Ui::new();
                ui.set_image_cell_size(Some((8, 16)));
                ui.restore_preferences(&RendererPreferences {
                    motion: true,
                    ..Default::default()
                });
                if view == "tower" {
                    ui.open_tower();
                }
                let mut terminal = Terminal::new(TestBackend::new(columns, rows)).unwrap();
                ui.tick(9_000);
                terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
                if view == "inspector" {
                    ui.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                    terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
                    assert_eq!(
                        ui.view(),
                        View::Desk,
                        "benchmark must actually open inspection"
                    );
                }
                let mut samples = Vec::new();
                let mut dimensions = (0, 0);
                for iteration in 0..70 {
                    ui.tick(9_000 + iteration * 100);
                    let started = Instant::now();
                    let forward = iteration % 2 == 0;
                    let key = if view == "office" {
                        if forward {
                            KeyCode::Right
                        } else {
                            KeyCode::Left
                        }
                    } else if forward {
                        KeyCode::Down
                    } else {
                        KeyCode::Up
                    };
                    ui.handle_key(KeyEvent::new(key, KeyModifiers::NONE));
                    terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
                    let pixels = ui.pixel_frame().with_text_backgrounds();
                    dimensions = if pixels.cell_area().is_some() {
                        (pixels.width(), pixels.height())
                    } else {
                        (0, 0)
                    };
                    std::hint::black_box(pixels);
                    if iteration >= 10 {
                        samples.push(started.elapsed().as_secs_f64() * 1000.0);
                    }
                }
                samples.sort_by(f64::total_cmp);
                println!(
                    "{view},{projects},50,{columns},{rows},{},{},{},{:.3},{:.3},{:.3}",
                    dimensions.0,
                    dimensions.1,
                    samples.len(),
                    samples[samples.len() / 2],
                    samples[samples.len() * 95 / 100],
                    samples[samples.len() - 1]
                );
            }
        }
    }
}
