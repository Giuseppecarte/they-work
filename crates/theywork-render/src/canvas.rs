//! A tiny two-pixels-per-cell canvas for terminal pixel art.
//!
//! The terminal has only one character position for every pair of vertical
//! pixels.  An upper half block carries the top colour in its foreground and
//! the bottom colour in its background, which gives the art twice the useful
//! vertical resolution without asking the terminal for any special features.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::sprite::Sprite;

/// The colour representation selected once when a canvas is created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    /// Keep RGB colours intact for terminals advertising true colour.
    TrueColor,
    /// Approximate colours with the terminal's 256-colour palette.
    Palette256,
}

impl ColorDepth {
    fn from_environment() -> Self {
        match std::env::var("COLORTERM") {
            Ok(value) if value.eq_ignore_ascii_case("truecolor") || value == "24bit" => {
                Self::TrueColor
            }
            _ => Self::Palette256,
        }
    }
}

/// An in-memory pixel surface whose pixels are terminal colours or transparent.
#[derive(Debug, Clone)]
pub struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<Option<Color>>,
    depth: ColorDepth,
}

impl Canvas {
    /// Create a canvas and detect colour support once for its lifetime.
    pub fn new(width_px: usize, height_px: usize) -> Self {
        Self::with_color_depth(width_px, height_px, ColorDepth::from_environment())
    }

    /// Create a canvas with an explicit colour depth.
    ///
    /// This is useful for deterministic render tests and for callers that have
    /// already negotiated terminal capabilities.
    pub fn with_color_depth(width_px: usize, height_px: usize, depth: ColorDepth) -> Self {
        let mut canvas = Self {
            width: 0,
            height: 0,
            pixels: Vec::new(),
            depth,
        };
        canvas.resize(width_px, height_px);
        canvas
    }

    /// Number of horizontal pixels in the surface.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Number of vertical pixels in the surface.
    pub fn height(&self) -> usize {
        self.height
    }

    /// The colour mode captured at construction.
    pub fn color_depth(&self) -> ColorDepth {
        self.depth
    }

    /// Resize the backing surface, retaining the allocation when possible.
    pub fn resize(&mut self, width_px: usize, height_px: usize) {
        self.width = width_px;
        self.height = height_px;
        let len = width_px.checked_mul(height_px).unwrap_or(0);
        self.pixels.resize(len, None);
        self.pixels.fill(None);
    }

    /// Make every pixel transparent.
    pub fn clear(&mut self) {
        self.pixels.fill(None);
    }

    /// Fill the surface with one opaque colour.
    pub fn fill(&mut self, color: Color) {
        let color = self.convert_color(color);
        self.pixels.fill(Some(color));
    }

    /// Set one opaque pixel. Out-of-bounds writes are deliberately ignored so
    /// a sprite can be clipped at a terminal edge without a special branch.
    pub fn set(&mut self, x: usize, y: usize, color: Color) {
        let color = self.convert_color(color);
        if let Some(pixel) = self.pixel_mut(x, y) {
            *pixel = Some(color);
        }
    }

    /// Clear one pixel without disturbing its neighbours.
    pub fn clear_pixel(&mut self, x: usize, y: usize) {
        if let Some(pixel) = self.pixel_mut(x, y) {
            *pixel = None;
        }
    }

    /// Inspect a pixel, primarily for tests and small custom view effects.
    pub fn pixel(&self, x: usize, y: usize) -> Option<Color> {
        self.pixels.get(self.index(x, y)?).copied().flatten()
    }

    /// Blit a sprite at a pixel coordinate, preserving transparent sprite
    /// pixels so furniture and employees can overlap naturally.
    pub fn blit(&mut self, sprite: &Sprite, x: usize, y: usize) {
        for sy in 0..sprite.height() {
            for sx in 0..sprite.width() {
                if let Some(color) = sprite.pixel(sx, sy) {
                    let Some(dx) = x.checked_add(sx) else {
                        continue;
                    };
                    let Some(dy) = y.checked_add(sy) else {
                        continue;
                    };
                    self.set(dx, dy, color);
                }
            }
        }
    }

    /// Draw a sprite with nearest-neighbour scaling and transparent pixels.
    pub fn blit_scaled(
        &mut self,
        sprite: &Sprite,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) {
        if width == 0 || height == 0 || sprite.width() == 0 || sprite.height() == 0 {
            return;
        }
        for dy in 0..height {
            let sy = dy.saturating_mul(sprite.height()) / height;
            for dx in 0..width {
                let sx = dx.saturating_mul(sprite.width()) / width;
                if let Some(color) = sprite.pixel(sx, sy) {
                    let Some(px) = x.checked_add(dx) else {
                        continue;
                    };
                    let Some(py) = y.checked_add(dy) else {
                        continue;
                    };
                    self.set(px, py, color);
                }
            }
        }
    }

    /// Emit the surface as half-block cells into a ratatui buffer.
    pub fn render(&self, buffer: &mut Buffer, area: Rect) {
        let pixel_width = self.width.min(area.width as usize);
        let cell_height = self.height.div_ceil(2).min(area.height as usize);

        for cell_y in 0..cell_height {
            let top_y = cell_y.saturating_mul(2);
            let bottom_y = top_y.saturating_add(1);
            for cell_x in 0..pixel_width {
                let top = self.pixel(cell_x, top_y);
                let bottom = if bottom_y < self.height {
                    self.pixel(cell_x, bottom_y)
                } else {
                    None
                };
                if top.is_none() && bottom.is_none() {
                    continue;
                }

                let Some(cell) = buffer.cell_mut((area.x + cell_x as u16, area.y + cell_y as u16))
                else {
                    continue;
                };
                match (top, bottom) {
                    (Some(top), Some(bottom)) => {
                        cell.set_char('▀').set_fg(top).set_bg(bottom);
                    }
                    (Some(top), None) => {
                        cell.set_char('▀').set_fg(top);
                    }
                    (None, Some(bottom)) => {
                        cell.set_char('▄').set_bg(bottom);
                    }
                    (None, None) => {}
                }
            }
        }
    }

    fn pixel_mut(&mut self, x: usize, y: usize) -> Option<&mut Option<Color>> {
        let index = self.index(x, y)?;
        self.pixels.get_mut(index)
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        (x < self.width && y < self.height).then(|| y * self.width + x)
    }

    fn convert_color(&self, color: Color) -> Color {
        match self.depth {
            ColorDepth::TrueColor => color,
            ColorDepth::Palette256 => palette_color(color),
        }
    }
}

fn palette_color(color: Color) -> Color {
    match color {
        Color::Reset => Color::Indexed(0),
        Color::Black => Color::Indexed(0),
        Color::Red => Color::Indexed(1),
        Color::Green => Color::Indexed(2),
        Color::Yellow => Color::Indexed(3),
        Color::Blue => Color::Indexed(4),
        Color::Magenta => Color::Indexed(5),
        Color::Cyan => Color::Indexed(6),
        Color::Gray => Color::Indexed(7),
        Color::DarkGray => Color::Indexed(8),
        Color::LightRed => Color::Indexed(9),
        Color::LightGreen => Color::Indexed(10),
        Color::LightYellow => Color::Indexed(11),
        Color::LightBlue => Color::Indexed(12),
        Color::LightMagenta => Color::Indexed(13),
        Color::LightCyan => Color::Indexed(14),
        Color::White => Color::Indexed(15),
        Color::Indexed(index) => Color::Indexed(index),
        Color::Rgb(red, green, blue) => Color::Indexed(nearest_xterm_index(red, green, blue)),
    }
}

fn nearest_xterm_index(red: u8, green: u8, blue: u8) -> u8 {
    let cube = [0_u8, 95, 135, 175, 215, 255];
    let red_cube = ((red as u16 * 5 + 127) / 255) as usize;
    let green_cube = ((green as u16 * 5 + 127) / 255) as usize;
    let blue_cube = ((blue as u16 * 5 + 127) / 255) as usize;
    let cube_color = (cube[red_cube], cube[green_cube], cube[blue_cube]);
    let cube_distance = color_distance((red, green, blue), cube_color);

    let gray_level = ((red as u16 + green as u16 + blue as u16) / 3).clamp(8, 238) as u8;
    let gray_step = ((gray_level.saturating_sub(8) as u16 + 5) / 10).min(23) as u8;
    let gray = 8 + gray_step * 10;
    let gray_distance = color_distance((red, green, blue), (gray, gray, gray));

    if cube_distance <= gray_distance {
        16 + (36 * red_cube + 6 * green_cube + blue_cube) as u8
    } else {
        232 + gray_step
    }
}

fn color_distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let red = a.0 as i32 - b.0 as i32;
    let green = a.1 as i32 - b.1 as i32;
    let blue = a.2 as i32 - b.2 as i32;
    (red * red + green * green + blue * blue) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn half_block_packs_top_into_foreground_and_bottom_into_background() {
        let mut canvas = Canvas::with_color_depth(1, 2, ColorDepth::TrueColor);
        let top = Color::Rgb(255, 50, 90);
        let bottom = Color::Rgb(40, 200, 150);
        canvas.set(0, 0, top);
        canvas.set(0, 1, bottom);

        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let area = buffer.area;
        canvas.render(&mut buffer, area);
        let cell = buffer.cell((0, 0)).expect("one cell");
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, top);
        assert_eq!(cell.bg, bottom);
    }

    #[test]
    fn transparent_pixels_leave_existing_cells_untouched() {
        let mut canvas = Canvas::with_color_depth(2, 2, ColorDepth::TrueColor);
        let sprite = Sprite::from_rows(&["A."], &[("A".chars().next().unwrap(), Color::Blue)]);
        canvas.blit(&sprite, 0, 1);

        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
        let original = buffer.cell((1, 0)).expect("cell").clone();
        let area = buffer.area;
        canvas.render(&mut buffer, area);
        assert_eq!(buffer.cell((1, 0)).expect("cell"), &original);
        assert_eq!(buffer.cell((0, 0)).expect("cell").symbol(), "▄");
    }

    #[test]
    fn palette_mode_is_fixed_and_maps_rgb_to_indexed_colour() {
        let mut canvas = Canvas::with_color_depth(1, 1, ColorDepth::Palette256);
        canvas.set(0, 0, Color::Rgb(255, 0, 0));
        assert!(matches!(canvas.pixel(0, 0), Some(Color::Indexed(_))));
    }
}
