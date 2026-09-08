//! Export the actual Ui + TestBackend buffers and their physical image layer.
//! A companion script replays native text with Menlo; this is not a terminal.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    backend::TestBackend,
    style::{Color, Modifier},
    Terminal,
};
use std::{collections::HashSet, fs, io::Write, path::Path};
use theywork_core::{demo, World};
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
fn click(ui: &mut Ui, action: impl Fn(&theywork_render::interaction::Action) -> bool) {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    ui.frame_presented();
    let hit = ui
        .hit_regions()
        .iter()
        .rev()
        .find(|h| action(&h.action))
        .expect("visible action")
        .clone();
    assert!(ui
        .handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.area.x,
            row: hit.area.y,
            modifiers: KeyModifiers::NONE
        })
        .is_none());
}
fn main() {
    use theywork_render::interaction::Action;
    use theywork_render::views::control::{Choice, ControlStatus, Request, TaskAccess};
    let output = std::env::args().nth(1).expect("output scratch directory");
    let out = Path::new(&output);
    fs::create_dir_all(out).unwrap();
    let mut world = World::new();
    for event in demo::events(6000) {
        world.apply(event);
    }
    for (width, height) in [(80, 24), (120, 36), (192, 58)] {
        let mut ui = Ui::new();
        ui.restore_preferences(&RendererPreferences {
            motion: false,
            color_depth: Some("truecolor".into()),
            ..Default::default()
        });
        ui.set_image_cell_size(Some((8, 16)));
        ui.tick(6000);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| ui.draw(f, &world)).unwrap();
        save(&ui, &terminal, out, &format!("office-{width}x{height}"));
        let worker = ui
            .hit_regions()
            .iter()
            .find_map(|h| {
                if let Action::Inspect(id) = &h.action {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .unwrap();
        ui.set_control_status(ControlStatus {
            tasks:std::collections::BTreeMap::from([(worker.0.clone(),TaskAccess {send:true,interrupt:true,description:"Managed fixture · verified local connection".into(),..Default::default()})]),
            requests:vec![Request {id:"fixture-review".into(),worker:worker.clone(),origin:"Checkout / Review release plan".into(),title:"Allow a command?".into(),detail:"Run cargo test before preparing the release. The task is waiting for your decision.".into(),choices:vec![Choice{label:"Allow once".into(),response:serde_json::json!({"decision":"accept"})},Choice{label:"Decline".into(),response:serde_json::json!({"decision":"decline"})}],questions:vec![]}],..Default::default()
        });
        click(&mut ui, |a| matches!(a,Action::Inspect(id) if *id==worker));
        terminal.draw(|f| ui.draw(f, &world)).unwrap();
        save(&ui, &terminal, out, &format!("inspector-{width}x{height}"));
        click(&mut ui, |a| matches!(a, Action::Review(_)));
        terminal.draw(|f| ui.draw(f, &world)).unwrap();
        save(&ui, &terminal, out, &format!("request-{width}x{height}"));
        ui.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        ui.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        terminal.draw(|f| ui.draw(f, &world)).unwrap();
        save(&ui, &terminal, out, &format!("character-{width}x{height}"));
        ui.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        ui.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        terminal.draw(|f| ui.draw(f, &world)).unwrap();
        save(&ui, &terminal, out, &format!("design-{width}x{height}"));
        ui.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        ui.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        terminal.draw(|f| ui.draw(f, &world)).unwrap();
        save(
            &ui,
            &terminal,
            out,
            &format!("connections-{width}x{height}"),
        );
        ui.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        ui.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
        terminal.draw(|f| ui.draw(f, &world)).unwrap();
        save(&ui, &terminal, out, &format!("attention-{width}x{height}"));
    }
}
