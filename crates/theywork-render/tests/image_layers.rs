//! Exercise the public UI/image boundary across real overlay transitions.
use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, style::Color, Terminal};
use theywork_core::{Activity, Agent, Event, EventKind, OfficeId, WorkerId, World};
use theywork_render::{PixelFrame, RendererPreferences, Ui};

fn world() -> World {
    let mut world = World::new();
    for index in 0..3 {
        for kind in [
            EventKind::Seen {
                name: format!("Review customer workflow {index}"),
                git_branch: Some("main".into()),
            },
            EventKind::Turn { in_flight: true },
            EventKind::Acted(Activity::Waiting {
                detail: format!("Review the production request for account {index}"),
            }),
        ] {
            world.apply(Event {
                at: 1,
                office: OfficeId("/project".into()),
                office_path: "/project".into(),
                worker: WorkerId(format!("thread-{index}")),
                agent: Agent::Codex,
                kind,
            });
        }
    }
    world
}

fn draw_and_check(
    ui: &mut Ui,
    terminal: &mut Terminal<TestBackend>,
    world: &World,
    now: &mut i64,
) -> PixelFrame {
    *now += 1_000;
    ui.tick(*now);
    terminal.draw(|frame| ui.draw(frame, world)).unwrap();
    let pixels = ui.pixel_frame();
    let area = pixels.cell_area().expect("visible scene or portrait");
    let native: HashSet<_> = pixels
        .text_cells()
        .iter()
        .map(|(x, y, _)| (*x, *y))
        .collect();
    let buffer = terminal.backend().buffer();
    assert!(
        buffer.content.iter().all(|cell| !cell.skip),
        "image masking must not leak into ordinary terminal output"
    );
    // Canvas block characters are image samples. Native letters over that
    // rectangle must be replayed after graphics, never discarded with samples.
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &buffer[(x, y)];
            if cell.symbol().chars().any(char::is_alphanumeric) {
                assert!(
                    native.contains(&(x, y)),
                    "native text at {x},{y} is missing from the image overlay"
                );
            }
        }
    }
    let composed = pixels.clone().with_text_backgrounds();
    let cell_width = pixels.width() / usize::from(area.width);
    let cell_height = pixels.height() / usize::from(area.height);
    for (x, y, cell) in pixels.text_cells() {
        if let Color::Rgb(r, g, b) = cell.bg {
            let px = usize::from(x - area.x) * cell_width + cell_width / 2;
            let py = usize::from(y - area.y) * cell_height + cell_height / 2;
            let offset = (py * pixels.width() + px) * 4;
            assert_eq!(
                &composed.rgba()[offset..offset + 4],
                &[r, g, b, 255],
                "opaque text background must cover the image, including blank cells"
            );
        }
    }
    pixels
}

#[test]
fn image_layers_survive_settings_phone_finder_and_return_to_office() {
    let world = world();
    for light in [false, true] {
        let mut ui = Ui::new();
        ui.restore_preferences(&RendererPreferences {
            light,
            motion: false,
            encoding: Some("half-blocks".into()),
            ..RendererPreferences::default()
        });
        ui.set_image_cell_size(Some((8, 16)));
        let mut terminal = Terminal::new(TestBackend::new(120, 36)).unwrap();
        let mut now = 0;
        let office = draw_and_check(&mut ui, &mut terminal, &world, &mut now);
        assert!(
            !office.text_cells().is_empty(),
            "office labels must remain native text"
        );
        for key in ['s', 'p', '/'] {
            ui.handle_key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE));
            let overlay = draw_and_check(&mut ui, &mut terminal, &world, &mut now);
            if key == '/' {
                assert!(
                    overlay.text_cells().len() > office.text_cells().len(),
                    "the finder is an opaque native panel over the room"
                );
                assert!(overlay
                    .text_cells()
                    .iter()
                    .any(|(_, _, cell)| cell.symbol() == " "));
            }
            ui.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
            let restored = draw_and_check(&mut ui, &mut terminal, &world, &mut now);
            assert_eq!(
                restored.cell_area(),
                office.cell_area(),
                "{key} close left a stale portrait/preview rectangle"
            );
            assert_eq!(
                restored.text_cells(),
                office.text_cells(),
                "{key} close left overlay text in the next image frame"
            );
        }
    }
}
