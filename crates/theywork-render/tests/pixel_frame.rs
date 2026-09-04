use ratatui::style::Color;
use theywork_render::canvas::{Canvas, ColorDepth};
use theywork_render::PixelFrame;

#[test]
fn external_consumer_can_read_owned_rgb_dimensions() {
    let mut canvas = Canvas::with_color_depth(2, 1, ColorDepth::TrueColor);
    canvas.set(0, 0, Color::Rgb(12, 34, 56));
    let frame: PixelFrame = canvas.pixel_frame();
    canvas.set(0, 0, Color::Rgb(90, 80, 70));
    assert_eq!((frame.width(), frame.height()), (2, 1));
    assert_eq!(frame.rgb(), [12, 34, 56, 0, 0, 0]);
    assert_eq!(frame.rgba(), [12, 34, 56, 255, 0, 0, 0, 0]);
}
