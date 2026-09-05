use ratatui::{backend::TestBackend, Terminal};
use theywork_core::World;
use theywork_render::Ui;

fn main() {
    let now = 192_000i64;
    let mut world = World::new();
    for e in theywork_core::demo::events(now) {
        world.apply(e);
    }
    world.tick(now);
    // 8x16 cells keeps the frame near 1280x688 so it stays embeddable.
    let mut ui = Ui::new();
    ui.set_image_cell_size(Some((8, 16)));
    ui.tick(now);
    let mut term = Terminal::new(TestBackend::new(160, 43)).unwrap();
    term.draw(|f| ui.draw(f, &world)).unwrap();
    let frame = ui.pixel_frame();
    println!("image path: {}x{}", frame.width(), frame.height());
    std::fs::write("/src/.cap/hires.rgba", frame.rgba()).unwrap();
    std::fs::write(
        "/src/.cap/hires.dim",
        format!("{} {}", frame.width(), frame.height()),
    )
    .unwrap();
}
