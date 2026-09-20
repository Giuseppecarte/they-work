//! Reproducible campus review: real UI routes and buffers, not terminal playback.
use inventory::tower_fixture as export;
#[allow(dead_code)]
#[path = "ui_inventory.rs"]
mod inventory;

use ratatui::{backend::TestBackend, Terminal};
use std::{fs, path::Path};
use theywork_render::{RendererPreferences, Ui};

fn main() {
    let directory = std::env::args().nth(1).expect("output directory");
    let out = Path::new(&directory);
    fs::create_dir_all(out).unwrap();
    let world = inventory::fixture();
    for (width, height) in [(80, 24), (120, 36)] {
        for light in [true, false] {
            for graphics in [true, false] {
                for surface in [
                    "tower",
                    "office-auto",
                    "office-side",
                    "office-top",
                    "office-iso",
                    "office-list",
                    "inspector-now",
                    "inspector-activity",
                    "inspector-team",
                    "inspector-details",
                    "attention",
                    "deliveries",
                    "finder",
                    "new-task",
                    "character",
                    "design",
                    "settings",
                    "help",
                    "phone-now",
                ] {
                    let mut ui = Ui::new();
                    ui.restore_preferences(&RendererPreferences {
                        light,
                        motion: false,
                        color_depth: Some("truecolor".into()),
                        projection: match surface {
                            "office-side" => "side",
                            "office-top" => "top-down",
                            "office-iso" => "isometric",
                            "office-list" => "list",
                            _ => "auto",
                        }
                        .into(),
                        ..Default::default()
                    });
                    if graphics {
                        ui.set_image_cell_size(Some((8, 16)));
                    }
                    ui.set_control_status(inventory::status());
                    ui.tick(1000);
                    ui.open_tower();
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    inventory::draw(&mut ui, &mut terminal, &world);
                    assert!(
                        inventory::route(surface, &mut ui, &mut terminal, &world),
                        "{surface}"
                    );
                    assert!(ui.hit_regions().iter().all(|hit| hit
                        .area
                        .intersection(terminal.backend().buffer().area)
                        == hit.area));
                    let name = format!(
                        "{surface}-{}-{}-{width}x{height}",
                        if light { "light" } else { "dark" },
                        if graphics { "image" } else { "cells" }
                    );
                    export::save(&ui, &terminal, out, &name);
                }
            }
        }
    }
}
