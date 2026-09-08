//! Exact-grid workstation pilot. No terminal font or image protocol is simulated.
use std::{collections::BTreeMap, fs, io::Write, path::Path};
use theywork_core::{Activity, Agent, Office, OfficeId, WaitReason, Worker, WorkerId};
use theywork_render::{
    canvas::Canvas,
    design::OfficeDesign,
    living_office::{MeetingGroup, SceneOptions, Studio},
};
fn main() {
    let out_arg = std::env::args().nth(1).expect("audit output directory");
    let out = Path::new(&out_arg);
    fs::create_dir_all(out).unwrap();
    let mut office = Office::new(OfficeId("pilot".into()), "/pilot".into());
    let mut wardrobe = BTreeMap::new();
    for (index, costume) in [0, 1, 11].into_iter().enumerate() {
        let id = WorkerId(format!("pilot-{index}"));
        wardrobe.insert(id.0.clone(), costume);
        let mut w = Worker::new(
            id,
            office.id.clone(),
            Agent::Codex,
            format!("Person {index}"),
            1000,
        );
        w.turn_in_flight = true;
        w.activity = if index == 1 {
            Activity::Reading {
                detail: "README.md".into(),
            }
        } else {
            Activity::Editing {
                detail: "src/main.rs".into(),
            }
        };
        if index == 2 {
            w.wait_reason = Some(WaitReason::HumanInput);
        }
        office.workers.push(w);
    }
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
    let mut metadata = String::new();
    for variant in 0..3 {
        let design = OfficeDesign {
            desks: variant,
            ..Default::default()
        };
        for (mode, h, overview) in [("overview", 128, true), ("detail", 304, false)] {
            for (kind, meeting) in [("individual", None), ("shared", Some(&group))] {
                let mut canvas = Canvas::new(0, 0);
                canvas.set_image_cell_size(Some((8, 16)));
                canvas.resize(640, h);
                let layout = studio.paint(
                    &mut canvas,
                    &office,
                    &SceneOptions {
                        now: 1000,
                        motion: false,
                        overview,
                        design: Some(&design),
                        wardrobe: Some(&wardrobe),
                        meeting,
                        ..Default::default()
                    },
                );
                let frame = canvas.pixel_frame();
                let name = format!("{kind}-{mode}-{variant}");
                let mut f = fs::File::create(out.join(format!("{name}.ppm"))).unwrap();
                write!(f, "P6\n{} {}\n255\n", frame.width(), frame.height()).unwrap();
                f.write_all(&frame.rgb()).unwrap();
                metadata.push_str(&format!(
                    "{name}: {} seats, capacity {}, scale {}\n",
                    layout.seats.len(),
                    layout.capacity,
                    layout.scale
                ));
                for seat in layout.seats {
                    metadata.push_str(&format!("  {} {:?}\n", seat.worker_id.0, seat.workstation));
                }
            }
        }
    }
    fs::write(out.join("geometry.txt"), metadata).unwrap();
}
