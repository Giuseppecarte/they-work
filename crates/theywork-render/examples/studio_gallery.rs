//! Exact stage-five artwork: independent authored grids, room choices and a
//! complete deterministic outbound/action/return vignette. No terminal emulation.
use ratatui::style::Color;
use std::{collections::BTreeMap, fs, io::Write, path::Path};
use theywork_core::{Agent, Office, OfficeId, Worker, WorkerId};
use theywork_render::{
    canvas::Canvas,
    design::{CharacterProfile, CharacterStyle, OfficeDesign, OfficePreset},
    living_office::{
        art::{self, Character, Facing, Pose},
        overview, SceneOptions, Studio,
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
    fs::create_dir_all(out.join("motion")).unwrap();
    let mut cast = Canvas::new(864, 600);
    cast.fill(Color::Rgb(28, 34, 45));
    for costume in 0..12 {
        let c = Character {
            costume,
            skin: costume % 6,
            hair: costume % 6,
        };
        let x = usize::from(costume % 6) * 144;
        let y = usize::from(costume / 6) * 300;
        cast.blit_scaled(&art::character(c, Pose::Rest, 0), x + 22, y + 12, 96, 128);
        cast.blit_scaled(
            &overview::character(c, Pose::Rest, 0),
            x + 22,
            y + 158,
            48,
            64,
        );
        cast.blit_scaled(
            &overview::character(c, Pose::Waiting, 0),
            x + 86,
            y + 158,
            48,
            64,
        );
        cast.blit_scaled(
            &overview::character(c, Pose::Rest, 0),
            x + 60,
            y + 240,
            24,
            32,
        );
    }
    save(&cast, &out.join("cast-twelve-authored-grids.ppm"));
    let mut directions = Canvas::new(864, 288);
    directions.fill(Color::Rgb(28, 34, 45));
    for (cast_index, costume) in [0, 1, 11].into_iter().enumerate() {
        for (direction_index, facing) in [Facing::Front, Facing::Right, Facing::Left]
            .into_iter()
            .enumerate()
        {
            let x = (cast_index * 3 + direction_index) * 96;
            let person = Character {
                costume,
                skin: cast_index as u8,
                hair: cast_index as u8,
            };
            directions.blit_scaled(
                &art::character_facing(person, Pose::Waiting, 0, facing),
                x,
                8,
                96,
                128,
            );
            directions.blit_scaled(
                &overview::character_facing(person, Pose::Waiting, 0, facing),
                x + 24,
                180,
                48,
                64,
            );
        }
    }
    save(&directions, &out.join("front-and-three-quarter-pilot.ppm"));
    let mut gestures = Canvas::new(840, 290);
    gestures.fill(Color::Rgb(28, 34, 45));
    for (i, pose) in [
        Pose::Work,
        Pose::Read,
        Pose::FolderOut,
        Pose::FolderIn,
        Pose::Message,
        Pose::Waiting,
        Pose::Coffee,
    ]
    .into_iter()
    .enumerate()
    {
        let c = Character {
            costume: i as u8,
            skin: i as u8 % 6,
            hair: i as u8 % 6,
        };
        gestures.blit_scaled(&art::character(c, pose, 3), i * 120 + 8, 8, 96, 128);
        gestures.blit_scaled(&overview::character(c, pose, 3), i * 120 + 32, 170, 48, 64);
    }
    save(&gestures, &out.join("gestures-at-double-size.ppm"));
    let mut office = Office::new(OfficeId("gallery".into()), "/gallery".into());
    let mut profiles = BTreeMap::new();
    let mut wardrobe = BTreeMap::new();
    for i in 0..3 {
        let id = WorkerId(format!("gallery-{i}"));
        profiles.insert(
            id.0.clone(),
            CharacterProfile {
                name: format!("Cast {i}"),
                style: [
                    CharacterStyle::Curious,
                    CharacterStyle::Energetic,
                    CharacterStyle::Calm,
                ][i],
            },
        );
        wardrobe.insert(id.0.clone(), [0, 1, 11][i]);
        office.workers.push(Worker::new(
            id,
            office.id.clone(),
            Agent::Codex,
            format!("Observed task {i}"),
            0,
        ));
    }
    let mut studio = Studio::new();
    for (name, preset) in [
        ("studio", OfficePreset::Studio),
        ("workshop", OfficePreset::Workshop),
        ("laboratory", OfficePreset::Laboratory),
    ] {
        for small in [true, false] {
            let mut c = Canvas::new(0, 0);
            c.set_image_cell_size(Some((8, 16)));
            c.resize(800, if small { 160 } else { 480 });
            let design = OfficeDesign {
                preset,
                ..OfficeDesign::default()
            };
            studio.paint(
                &mut c,
                &office,
                &SceneOptions {
                    overview: small,
                    motion: false,
                    design: Some(&design),
                    wardrobe: Some(&wardrobe),
                    ..SceneOptions::default()
                },
            );
            save(
                &c,
                &out.join(format!(
                    "room-{name}-{}.ppm",
                    if small { "overview" } else { "detail" }
                )),
            );
        }
    }
    // Three choices for each zone use the same cast, grid and truthful status.
    for zone in 0..4 {
        let mut sheet = Canvas::new(800, 480);
        for value in 0..3 {
            let mut design = OfficeDesign::default();
            match zone {
                0 => design.entrance = value,
                1 => design.desks = value,
                2 => design.meeting = value,
                _ => design.rest = value,
            }
            let group = theywork_render::living_office::MeetingGroup {
                parent: office.workers[0].id.clone(),
                members: office.workers[1..].iter().map(|w| w.id.clone()).collect(),
            };
            let mut room = Canvas::new(0, 0);
            room.set_image_cell_size(Some((8, 16)));
            room.resize(800, 160);
            studio.paint(
                &mut room,
                &office,
                &SceneOptions {
                    overview: true,
                    motion: false,
                    design: Some(&design),
                    wardrobe: Some(&wardrobe),
                    meeting: if zone == 2 { Some(&group) } else { None },
                    ..SceneOptions::default()
                },
            );
            sheet.blit_canvas(&room, 0, usize::from(value) * 160);
        }
        save(
            &sheet,
            &out.join(format!(
                "zone-{}.ppm",
                ["entrance", "desks", "meeting", "rest"][zone]
            )),
        );
    }
    let mut c = Canvas::new(0, 0);
    c.set_image_cell_size(Some((8, 16)));
    c.resize(800, 480);
    let mut metadata = String::from("frame,time_ms,active_gags,scale\n");
    for frame in 0..129 {
        let now = (8_000 + frame * 125).min(23_999);
        let layout = studio.paint(
            &mut c,
            &office,
            &SceneOptions {
                now,
                profiles: Some(&profiles),
                wardrobe: Some(&wardrobe),
                ..SceneOptions::default()
            },
        );
        save(&c, &out.join(format!("motion/frame-{frame:03}.ppm")));
        metadata.push_str(&format!(
            "{frame},{now},{},{}\n",
            layout.active_gags, layout.scale
        ));
    }
    fs::write(out.join("motion.csv"), metadata).unwrap();
}
