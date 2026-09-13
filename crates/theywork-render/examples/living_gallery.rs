//! Reproducible artwork exports. PPM intermediates belong under audit scratch;
//! the companion audit script labels and stores the review PNGs.
use ratatui::style::Color;
use std::{fs, io::Write, path::Path};
use theywork_core::{Agent, Office, OfficeId, Worker, WorkerId};
use theywork_render::{
    canvas::Canvas,
    living_office::{
        art::{Character, Pose},
        MeetingGroup, SceneOptions, Studio,
    },
};

fn save(canvas: &Canvas, path: &Path) {
    let frame = canvas.pixel_frame();
    let mut file = fs::File::create(path).unwrap();
    write!(file, "P6\n{} {}\n255\n", frame.width(), frame.height()).unwrap();
    file.write_all(&frame.rgb()).unwrap();
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("provide an audit scratch output directory");
    let out = Path::new(&out);
    fs::create_dir_all(out).unwrap();
    let mut studio = Studio::new();
    let mut atlas = Canvas::new(800, 720);
    atlas.fill(Color::Rgb(24, 29, 39));
    for costume in 0..12 {
        let sprite = studio.character_frame(
            Character {
                costume,
                skin: costume % 6,
                hair: costume % 6,
            },
            Pose::Rest,
            0,
        );
        atlas.blit_scaled(
            &sprite,
            28 + (costume as usize % 4) * 200,
            18 + (costume as usize / 4) * 240,
            144,
            192,
        );
    }
    save(&atlas, &out.join("cast.ppm"));
    let mut poses = Canvas::new(960, 240);
    poses.fill(Color::Rgb(24, 29, 39));
    for (index, pose) in [
        Pose::Work,
        Pose::Read,
        Pose::Search,
        Pose::Waiting,
        Pose::Coffee,
        Pose::Stretch,
        Pose::WaterPlant,
        Pose::PaperPlane,
    ]
    .iter()
    .enumerate()
    {
        let sprite = studio.character_frame(
            Character {
                costume: index as u8,
                skin: 1,
                hair: 1,
            },
            *pose,
            3,
        );
        poses.blit_scaled(&sprite, 12 + index * 120, 35, 96, 128);
    }
    save(&poses, &out.join("poses.ppm"));
    let mut interactions = Canvas::new(480, 220);
    interactions.fill(Color::Rgb(24, 29, 39));
    for (index, pose) in [Pose::FolderOut, Pose::Message, Pose::FolderIn, Pose::Read]
        .iter()
        .enumerate()
    {
        let sprite = studio.character_frame(
            Character {
                costume: index as u8,
                skin: 1,
                hair: 1,
            },
            *pose,
            2,
        );
        interactions.blit_scaled(&sprite, 12 + index * 120, 32, 96, 128);
    }
    save(&interactions, &out.join("interactions.ppm"));
    for &(count, cols, rows) in &[(3, 80, 24), (6, 120, 32), (20, 192, 48)] {
        let mut office = Office::new(OfficeId("lumen".into()), "/projects/lumen".into());
        let mut wardrobe = std::collections::BTreeMap::new();
        for index in 0..count {
            let id = WorkerId(format!("member-{index}"));
            wardrobe.insert(id.0.clone(), index);
            office.workers.push(Worker::new(
                id,
                office.id.clone(),
                Agent::Codex,
                format!("Task {index}"),
                0,
            ));
        }
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize_for_cells(cols, rows);
        if count == 3 && std::env::args().any(|argument| argument == "--motion") {
            for frame in 0..48 {
                studio.paint(
                    &mut canvas,
                    &office,
                    &SceneOptions {
                        now: 8_000 + frame * 125,
                        motion: true,
                        wardrobe: Some(&wardrobe),
                        ..SceneOptions::default()
                    },
                );
                save(&canvas, &out.join(format!("motion-{frame:02}.ppm")));
            }
        }
        for (label, now, motion, meeting) in [
            ("office", 0, false, None),
            ("adjoining-room", 0, false, None),
            ("gags", 14_000, true, None),
            (
                "meeting",
                0,
                false,
                Some(MeetingGroup {
                    parent: office.workers[0].id.clone(),
                    members: office
                        .workers
                        .iter()
                        .skip(1)
                        .map(|worker| worker.id.clone())
                        .collect(),
                }),
            ),
        ] {
            let options = SceneOptions {
                now,
                motion,
                wardrobe: Some(&wardrobe),
                selected_worker: Some(&office.workers[0].id),
                meeting: meeting.as_ref(),
                show_elevator: label != "adjoining-room",
                ..SceneOptions::default()
            };
            let layout = studio.paint(&mut canvas, &office, &options);
            save(
                &canvas,
                &out.join(format!("{label}-{count}-{cols}x{rows}.ppm")),
            );
            println!(
                "{label} {count}: {} seats, {}/{} pages, scale {}, {} gags",
                layout.seats.len(),
                layout.page + 1,
                layout.page_count,
                layout.scale,
                layout.active_gags
            );
        }
    }
}
