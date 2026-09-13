//! Stage-five pilot: exact authored grids, viewed before extending the cast.
use ratatui::style::Color;
use std::{fs, io::Write, path::Path};
use theywork_render::{
    canvas::Canvas,
    design::{OfficeDesign, OfficePreset},
    living_office::{
        art::{self, Character, Pose},
        overview, MeetingGroup, SceneOptions, Studio,
    },
};
fn save(canvas: &Canvas, path: &Path) {
    let f = canvas.pixel_frame();
    let mut out = fs::File::create(path).unwrap();
    write!(out, "P6\n{} {}\n255\n", f.width(), f.height()).unwrap();
    out.write_all(&f.rgb()).unwrap();
}
fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("audit scratch output directory");
    let out = Path::new(&path);
    fs::create_dir_all(out).unwrap();
    let mut canvas = Canvas::new(840, 450);
    canvas.fill(Color::Rgb(28, 34, 45));
    for (column, costume) in [0, 1, 11].into_iter().enumerate() {
        let person = Character {
            costume,
            skin: column as u8,
            hair: column as u8,
        };
        for (row, pose) in [Pose::Rest, Pose::Waiting].into_iter().enumerate() {
            canvas.blit_scaled(
                &art::character(person, pose, 0),
                column * 280 + 12,
                row * 210 + 10,
                144,
                192,
            );
            canvas.blit_scaled(
                &overview::character(person, pose, 0),
                column * 280 + 176,
                row * 210 + 38,
                72,
                96,
            );
            canvas.blit_scaled(
                &overview::character(person, pose, 0),
                column * 280 + 190,
                row * 210 + 150,
                24,
                32,
            );
        }
    }
    save(&canvas, &out.join("pilot-three-cast.ppm"));
    let mut office =
        theywork_core::Office::new(theywork_core::OfficeId("pilot".into()), "/pilot".into());
    let mut wardrobe = std::collections::BTreeMap::new();
    for (index, costume) in [0, 1, 11].into_iter().enumerate() {
        let id = theywork_core::WorkerId(format!("pilot-{index}"));
        wardrobe.insert(id.0.clone(), costume);
        office.workers.push(theywork_core::Worker::new(
            id,
            office.id.clone(),
            theywork_core::Agent::Codex,
            format!("Person {index}"),
            0,
        ));
    }
    office.workers[1].activity = theywork_core::Activity::Waiting {
        detail: "Pilot question".into(),
    };
    office.workers[1].wait_reason = Some(theywork_core::WaitReason::HumanInput);
    let group = MeetingGroup {
        parent: office.workers[0].id.clone(),
        members: office
            .workers
            .iter()
            .skip(1)
            .map(|w| w.id.clone())
            .collect(),
    };
    let mut studio = Studio::new();
    for (label, preset) in [
        ("studio", OfficePreset::Studio),
        ("workshop", OfficePreset::Workshop),
        ("laboratory", OfficePreset::Laboratory),
    ] {
        let design = OfficeDesign {
            preset,
            ..OfficeDesign::default()
        };
        for (mode, w, h, overview) in [("overview", 640, 128, true), ("detail", 640, 304, false)] {
            let mut room = Canvas::new(0, 0);
            room.set_image_cell_size(Some((8, 16)));
            room.resize(w, h);
            let layout = studio.paint(
                &mut room,
                &office,
                &SceneOptions {
                    motion: false,
                    overview,
                    design: Some(&design),
                    wardrobe: Some(&wardrobe),
                    meeting: Some(&group),
                    ..SceneOptions::default()
                },
            );
            save(&room, &out.join(format!("pilot-{label}-{mode}.ppm")));
            println!(
                "{label} {mode}: {} seats @{}x sign={}px",
                layout.seats.len(),
                layout.scale,
                layout.sign.height
            );
        }
    }
}
