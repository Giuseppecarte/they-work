//! Exact authored workstation specimens; fictional fixtures, no terminal emulation.
use std::{collections::BTreeMap, fs, io::Write, path::Path};
use theywork_core::{Activity, Agent, Office, OfficeId, WaitReason, Worker, WorkerId};
use theywork_render::{
    canvas::Canvas,
    design::{CharacterProfile, CharacterStyle, OfficeDesign, OfficePreset},
    living_office::{MeetingGroup, SceneOptions, Studio},
};
fn save(canvas: &Canvas, path: &Path) {
    let f = canvas.pixel_frame();
    let mut out = fs::File::create(path).unwrap();
    write!(out, "P6\n{} {}\n255\n", f.width(), f.height()).unwrap();
    out.write_all(&f.rgb()).unwrap();
}
fn canvas(w: usize, h: usize) -> Canvas {
    let mut c = Canvas::new(0, 0);
    c.set_image_cell_size(Some((8, 16)));
    c.resize(w, h);
    c
}
fn main() {
    let arg = std::env::args().nth(1).expect("audit output directory");
    let out = Path::new(&arg);
    fs::create_dir_all(out.join("motion")).unwrap();
    let mut office = Office::new(OfficeId("specimen".into()), "/specimen".into());
    let mut wardrobe = BTreeMap::new();
    for i in 0..3 {
        let id = WorkerId(format!("gallery-{i}"));
        let mut w = Worker::new(
            id.clone(),
            office.id.clone(),
            Agent::Codex,
            format!("Fixture {i}"),
            1000,
        );
        w.turn_in_flight = true;
        w.activity = Activity::Editing {
            detail: "file.rs".into(),
        };
        office.workers.push(w);
        wardrobe.insert(id.0, i);
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
    for (mode, h, overview) in [("detail", 304, false), ("overview", 128, true)] {
        for (kind, meeting) in [("individual", None), ("shared", Some(&group))] {
            let mut sheet = canvas(640, h * 4);
            for batch in 0..4 {
                for i in 0..3 {
                    wardrobe.insert(office.workers[i].id.0.clone(), batch * 3 + i);
                }
                let mut c = canvas(640, h);
                let l = studio.paint(
                    &mut c,
                    &office,
                    &SceneOptions {
                        now: 1000,
                        motion: false,
                        overview,
                        meeting,
                        wardrobe: Some(&wardrobe),
                        ..Default::default()
                    },
                );
                sheet.blit_canvas(&c, 0, batch * h);
                metadata.push_str(&format!(
                    "cast-{kind}-{mode} batch{batch}: {} seats @{}x\n",
                    l.seats.len(),
                    l.scale
                ));
            }
            save(&sheet, &out.join(format!("cast-{kind}-{mode}.ppm")));
        }
    }
    for (i, costume) in [0, 1, 11].into_iter().enumerate() {
        wardrobe.insert(office.workers[i].id.0.clone(), costume);
    }
    office.workers[1].activity = Activity::Reading {
        detail: "README.md".into(),
    };
    office.workers[2].wait_reason = Some(WaitReason::HumanInput);
    for (name, preset) in [
        ("studio", OfficePreset::Studio),
        ("workshop", OfficePreset::Workshop),
        ("laboratory", OfficePreset::Laboratory),
    ] {
        for light in [false, true] {
            let mut sheet = canvas(640, 304 * 3);
            for desks in 0..3 {
                let d = OfficeDesign {
                    preset,
                    desks,
                    ..Default::default()
                };
                let mut c = canvas(640, 304);
                studio.paint(
                    &mut c,
                    &office,
                    &SceneOptions {
                        now: 1000,
                        motion: false,
                        light,
                        design: Some(&d),
                        meeting: Some(&group),
                        wardrobe: Some(&wardrobe),
                        ..Default::default()
                    },
                );
                sheet.blit_canvas(&c, 0, desks as usize * 304);
            }
            save(
                &sheet,
                &out.join(format!(
                    "materials-{name}-{}.ppm",
                    if light { "light" } else { "dark" }
                )),
            );
        }
    }
    let mut motifs = canvas(640, 128 * 5);
    let cases = [
        (
            "Editing",
            Activity::Editing {
                detail: "file.rs".into(),
            },
            None,
        ),
        (
            "Command",
            Activity::Typing {
                detail: "command".into(),
            },
            None,
        ),
        (
            "Reading",
            Activity::Reading {
                detail: "README".into(),
            },
            None,
        ),
        (
            "Searching",
            Activity::Searching {
                detail: "symbol".into(),
            },
            None,
        ),
        ("Thinking", Activity::Thinking, None),
        (
            "Automatic wait",
            Activity::Waiting {
                detail: "review".into(),
            },
            Some(WaitReason::AutomaticReview),
        ),
        (
            "Human input",
            Activity::Waiting {
                detail: "question".into(),
            },
            Some(WaitReason::HumanInput),
        ),
        (
            "Error",
            Activity::Error {
                detail: "failed".into(),
            },
            None,
        ),
        ("Quiet", Activity::Idle, None),
        (
            "Unavailable",
            Activity::Editing {
                detail: "last activity".into(),
            },
            None,
        ),
    ];
    let mut one = office.clone();
    one.workers.truncate(1);
    for (i, (label, activity, reason)) in cases.into_iter().enumerate() {
        one.workers[0].activity = activity;
        one.workers[0].wait_reason = reason;
        one.workers[0].coverage.observed_at = if i == 9 { 1 } else { 0 };
        let mut c = canvas(320, 128);
        studio.paint(
            &mut c,
            &one,
            &SceneOptions {
                now: 1000,
                motion: false,
                overview: true,
                wardrobe: Some(&wardrobe),
                ..Default::default()
            },
        );
        motifs.blit_canvas(&c, i % 2 * 320, i / 2 * 128);
        metadata.push_str(&format!("motif{i}: {label}\n"));
    }
    save(&motifs, &out.join("screen-categories.ppm"));
    let profiles = office
        .workers
        .iter()
        .enumerate()
        .map(|(i, w)| {
            (
                w.id.0.clone(),
                CharacterProfile {
                    name: format!("Fixture{i}"),
                    style: [
                        CharacterStyle::Curious,
                        CharacterStyle::Energetic,
                        CharacterStyle::Calm,
                    ][i],
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut frames = String::from("frame,time_ms,active_gags,scale\n");
    for frame in 0..129 {
        let now = 8000 + frame * 125;
        let mut c = canvas(800, 480);
        let l = studio.paint(
            &mut c,
            &office,
            &SceneOptions {
                now,
                profiles: Some(&profiles),
                wardrobe: Some(&wardrobe),
                ..Default::default()
            },
        );
        save(&c, &out.join(format!("motion/frame-{frame:03}.ppm")));
        frames.push_str(&format!("{frame},{now},{},{}\n", l.active_gags, l.scale));
    }
    fs::write(out.join("motion.csv"), frames).unwrap();
    fs::write(out.join("geometry.txt"), metadata).unwrap();
}
