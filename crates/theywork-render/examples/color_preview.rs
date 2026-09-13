//! Complete fixed-time color comparisons using the existing real-UI routes.
#[allow(dead_code)]
#[path = "ui_inventory.rs"]
mod inventory;

use ratatui::{backend::TestBackend, Terminal};
use serde_json::json;
use std::{collections::BTreeMap, fs, path::Path};
use theywork_core::{
    Activity, Agent, Event, EventKind, Evidence, OfficeId, Relationship, RelationshipKind,
    SourceCoverage, WaitReason, WorkerId, World,
};
use theywork_render::{
    design::{OfficeDesign, OfficePreset},
    RendererPreferences, Ui,
};

fn pilot(team: bool) -> World {
    let mut world = World::new();
    for index in 0..3 {
        let worker = WorkerId(format!("floor-00-person-{index}"));
        let mut emit = |kind| {
            world.apply(Event {
                at: 1_000,
                office: OfficeId("/projects/00-checkout".into()),
                office_path: "/projects/00-checkout".into(),
                worker: worker.clone(),
                agent: if index == 1 {
                    Agent::Claude
                } else {
                    Agent::Codex
                },
                kind,
            })
        };
        emit(EventKind::Seen {
            name: [
                "Build the account form",
                "Read the permission checks",
                "Implement field validation",
            ][index]
                .into(),
            git_branch: Some("preview/account-form".into()),
        });
        emit(EventKind::Turn { in_flight: true });
        emit(EventKind::Acted(if index == 1 {
            Activity::Reading {
                detail: "src/permissions.rs".into(),
            }
        } else {
            Activity::Editing {
                detail: "src/account.rs".into(),
            }
        }));
        emit(EventKind::Coverage(SourceCoverage {
            available: true,
            observed_at: 1_000,
            detail: "Synthetic preview; no provider is connected.".into(),
            ..Default::default()
        }));
        if index == 0 {
            emit(EventKind::Wait(Some(WaitReason::HumanApproval)));
        }
        if team && index > 0 {
            emit(EventKind::Relationship(Relationship {
                parent: WorkerId("floor-00-person-0".into()),
                child: worker.clone(),
                kind: RelationshipKind::Delegation,
                evidence: Evidence::Demo,
                at: 1_000,
                correlation_id: None,
            }));
        }
    }
    world
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let out = Path::new(args.get(1).expect("provide audit output directory"));
    fs::create_dir_all(out).unwrap();
    let mut cases = Vec::new();
    let mut capture = |name: String,
                       surface: &str,
                       world: &World,
                       size: (u16, u16),
                       cells: (u16, u16),
                       mode: &str,
                       design: OfficeDesign,
                       palette: usize| {
        let mut ui = Ui::new();
        let designs = world
            .offices()
            .map(|o| (o.id.0.clone(), design.clone()))
            .collect();
        let palettes = world.offices().map(|o| (o.id.0.clone(), palette)).collect();
        ui.restore_preferences(&RendererPreferences {
            motion: false,
            light: mode == "light",
            color_depth: Some(
                match mode {
                    "mono" => "none",
                    "256" => "256",
                    _ => "truecolor",
                }
                .into(),
            ),
            office_designs: designs,
            office_palettes: palettes,
            wardrobe: BTreeMap::from([
                ("floor-00-person-0".into(), 0),
                ("floor-00-person-1".into(), 1),
                ("floor-00-person-2".into(), 11),
            ]),
            ..Default::default()
        });
        if mode != "native" {
            ui.set_image_cell_size(Some(cells));
        }
        ui.set_control_status(inventory::status());
        ui.tick(1_000);
        ui.open_tower();
        let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
        inventory::draw(&mut ui, &mut terminal, world);
        let reached = inventory::route(surface, &mut ui, &mut terminal, world);
        let mut record =
            inventory::record(out, &name, surface, mode, &ui, &terminal, (reached, cells));
        record["preset"] = json!(format!("{:?}", design.preset));
        record["palette"] = json!(palette);
        cases.push(record);
    };
    for (preset_name, preset) in [
        ("studio", OfficePreset::Studio),
        ("workshop", OfficePreset::Workshop),
        ("lab", OfficePreset::Laboratory),
    ] {
        for palette in 0..4 {
            for team in [false, true] {
                for surface in ["tower", "office-auto"] {
                    let name = format!(
                        "{preset_name}-p{palette}-{}-{surface}-80x24",
                        if team { "team" } else { "desks" }
                    );
                    capture(
                        name,
                        surface,
                        &pilot(team),
                        (80, 24),
                        (8, 16),
                        "image",
                        OfficeDesign {
                            preset,
                            ..Default::default()
                        },
                        palette,
                    );
                }
            }
        }
    }
    let world = inventory::fixture();
    for surface in [
        "tower",
        "office-auto",
        "inspector-now",
        "attention",
        "connections",
    ] {
        for size in [(32, 14), (80, 24), (120, 36), (192, 58)] {
            for cells in [(8, 16), (10, 20)] {
                for mode in ["image", "light", "256", "mono", "native"] {
                    if mode != "image" && size != (80, 24) {
                        continue;
                    }
                    let name = format!(
                        "{surface}-{mode}-{}x{}-{}x{}",
                        size.0, size.1, cells.0, cells.1
                    );
                    capture(
                        name,
                        surface,
                        &world,
                        size,
                        cells,
                        mode,
                        OfficeDesign::default(),
                        0,
                    );
                }
            }
        }
    }
    assert!(!cases.is_empty());
    assert!(cases
        .iter()
        .all(|c| c["automatic"]["route"] == true && c["automatic"]["hit_bounds"] == true));
    fs::write(out.join("inventory.json"), serde_json::to_string_pretty(&json!({"schema":1,"kind":"Real Ui / TestBackend compositor; fixed time 1000, reduced motion, synthetic facts. Not terminal paint.","cases":cases})).unwrap()).unwrap();
    println!(
        "{} complete color preview cases; routes and hit bounds passed",
        cases.len()
    );
}
