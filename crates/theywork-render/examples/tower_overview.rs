//! Deterministic tower interaction scenes: twenty projects, fifty conversations.
//! Export real Ui buffers and physical images for a native-font layout replay.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    backend::TestBackend,
    style::{Color, Modifier},
    Terminal,
};
use std::{collections::HashSet, fs, io::Write, path::Path};
use theywork_core::{
    Activity, Agent, Event, EventKind, Evidence, OfficeId, Relationship, RelationshipKind,
    WaitReason, WorkerId, WorkerLifecycle, World,
};
use theywork_render::{RendererPreferences, Ui};

fn quoted(text: &str) -> String {
    let mut result = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c < ' ' => result.push_str(&format!("\\u{:04x}", c as u32)),
            c => result.push(c),
        }
    }
    result.push('"');
    result
}
fn rgb(color: Color) -> (u8, u8, u8) {
    let index = match color {
        Color::Rgb(r, g, b) => return (r, g, b),
        Color::Indexed(i) => i,
        Color::Black | Color::Reset => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
    };
    let basic = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    if index < 16 {
        basic[index as usize]
    } else if index >= 232 {
        let gray = 8 + (index - 232) * 10;
        (gray, gray, gray)
    } else {
        let n = index - 16;
        let levels = [0, 95, 135, 175, 215, 255];
        (
            levels[(n / 36) as usize],
            levels[(n / 6 % 6) as usize],
            levels[(n % 6) as usize],
        )
    }
}
fn color(color: Color, foreground: bool) -> String {
    if color == Color::Reset {
        return if foreground { "#d3d7cf" } else { "#10141c" }.into();
    }
    let (r, g, b) = rgb(color);
    format!("#{r:02x}{g:02x}{b:02x}")
}
fn save(ui: &Ui, terminal: &Terminal<TestBackend>, out: &Path, name: &str) {
    let buffer = terminal.backend().buffer();
    let pixels = ui.pixel_frame().with_text_backgrounds();
    let area = pixels.cell_area();
    let native = pixels
        .text_cells()
        .iter()
        .map(|(x, y, _)| (*x, *y))
        .collect::<HashSet<_>>();
    fs::write(out.join(format!("{name}.rgba")), pixels.rgba()).unwrap();
    let mut file = fs::File::create(out.join(format!("{name}.cells.json"))).unwrap();
    let image=area.map_or("null".into(),|r|format!("{{\"x\":{},\"y\":{},\"width\":{},\"height\":{},\"pixel_width\":{},\"pixel_height\":{}}}",r.x,r.y,r.width,r.height,pixels.width(),pixels.height()));
    write!(
        file,
        "{{\"columns\":{},\"rows\":{},\"cell_width\":8,\"cell_height\":16,\"image\":{},\"cells\":[",
        buffer.area.width, buffer.area.height, image
    )
    .unwrap();
    for y in 0..buffer.area.height {
        if y > 0 {
            write!(file, ",").unwrap();
        }
        write!(file, "[").unwrap();
        for x in 0..buffer.area.width {
            if x > 0 {
                write!(file, ",").unwrap();
            }
            let cell = &buffer[(x, y)];
            let is_native =
                area.is_none_or(|r| !r.contains((x, y).into())) || native.contains(&(x, y));
            let (fg, bg) = if cell.modifier.contains(Modifier::REVERSED) {
                (cell.bg, cell.fg)
            } else {
                (cell.fg, cell.bg)
            };
            write!(
                file,
                "{{\"text\":{},\"fg\":\"{}\",\"bg\":\"{}\",\"bold\":{},\"native\":{}}}",
                quoted(cell.symbol()),
                color(fg, true),
                color(bg, false),
                cell.modifier.contains(Modifier::BOLD),
                is_native
            )
            .unwrap();
        }
        write!(file, "]").unwrap();
    }
    writeln!(file, "]}}").unwrap();
}
fn emit(world: &mut World, floor: usize, person: usize, kind: EventKind) {
    let path = format!(
        "/projects/{floor:02}-{}",
        [
            "checkout",
            "design-system",
            "billing-service",
            "documentation"
        ][floor % 4]
    );
    world.apply(Event {
        at: 1_000,
        office: OfficeId(path.clone()),
        office_path: path,
        worker: WorkerId(format!("floor-{floor:02}-person-{person}")),
        agent: if floor.is_multiple_of(2) {
            Agent::Codex
        } else {
            Agent::Claude
        },
        kind,
    });
}
fn fixture(floors: usize) -> World {
    let mut world = World::new();
    for floor in 0..floors {
        let count = match floor {
            0 => 7,
            18 => 4,
            19 => 5,
            _ => 2,
        };
        for person in 0..count {
            emit(
                &mut world,
                floor,
                person,
                EventKind::Seen {
                    name: format!(
                        "Build the shared account interface — task {person} in project {floor:02}"
                    ),
                    git_branch: None,
                },
            );
            if person % 2 == 0 {
                emit(
                    &mut world,
                    floor,
                    person,
                    EventKind::Turn { in_flight: true },
                );
                emit(
                    &mut world,
                    floor,
                    person,
                    EventKind::Acted(Activity::Editing {
                        detail: "src/account.rs".into(),
                    }),
                );
            }
        }
    }
    for (parent, child) in [(0, 1), (0, 2), (3, 4), (3, 5)] {
        emit(
            &mut world,
            0,
            child,
            EventKind::Relationship(Relationship {
                parent: WorkerId(format!("floor-00-person-{parent}")),
                child: WorkerId(format!("floor-00-person-{child}")),
                kind: RelationshipKind::Delegation,
                evidence: Evidence::Demo,
                at: 1_000,
                correlation_id: None,
            }),
        );
        emit(
            &mut world,
            0,
            child,
            EventKind::Lifecycle(WorkerLifecycle::Active),
        );
    }
    emit(
        &mut world,
        0,
        2,
        EventKind::Wait(Some(WaitReason::HumanInput)),
    );
    world
}
fn key(ui: &mut Ui, code: KeyCode) {
    ui.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}
fn main() {
    let output = std::env::args()
        .nth(1)
        .expect("provide audit scratch output");
    let out = Path::new(&output);
    fs::create_dir_all(out).unwrap();
    for floors in [1, 3, 20] {
        let world = fixture(floors);
        for (width, height) in [(80, 24), (120, 36), (192, 58)] {
            let mut ui = Ui::new();
            ui.restore_preferences(&RendererPreferences {
                motion: false,
                color_depth: Some("truecolor".into()),
                ..RendererPreferences::default()
            });
            ui.set_image_cell_size(Some((8, 16)));
            ui.tick(1_000);
            ui.open_tower();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
            save(
                &ui,
                &terminal,
                out,
                &format!("tower-{floors}-{width}x{height}-first"),
            );
            if floors == 20 {
                key(&mut ui, KeyCode::End);
                terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
                assert_eq!(
                    ui.selected_office(),
                    19,
                    "End reaches the last stable floor"
                );
                save(
                    &ui,
                    &terminal,
                    out,
                    &format!("tower-{floors}-{width}x{height}-last"),
                );
                key(&mut ui, KeyCode::Home);
                terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
                key(&mut ui, KeyCode::Enter);
                terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
                save(
                    &ui,
                    &terminal,
                    out,
                    &format!("office-team-{width}x{height}"),
                );
                key(&mut ui, KeyCode::Enter);
                terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
                save(
                    &ui,
                    &terminal,
                    out,
                    &format!("inspect-team-{width}x{height}"),
                );
            }
        }
    }
    let world = fixture(20);
    for (width, height, light) in [(80, 24, false), (120, 36, true), (32, 14, false)] {
        let mut ui = Ui::new();
        ui.restore_preferences(&RendererPreferences {
            motion: false,
            light,
            color_depth: Some("truecolor".into()),
            ..RendererPreferences::default()
        });
        ui.tick(1_000);
        ui.open_tower();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        save(
            &ui,
            &terminal,
            out,
            &format!("native-{width}x{height}-first"),
        );
        key(&mut ui, KeyCode::End);
        terminal.draw(|frame| ui.draw(frame, &world)).unwrap();
        save(
            &ui,
            &terminal,
            out,
            &format!("native-{width}x{height}-last"),
        );
    }
}
