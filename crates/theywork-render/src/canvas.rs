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
    /// Render luminance as block characters without terminal colours.
    None,
}

impl ColorDepth {
    fn from_environment() -> Self {
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let forced = std::env::var("THEYWORK_COLOR").ok();
        let colorterm = std::env::var("COLORTERM").ok();
        Self::resolve(no_color, forced.as_deref(), colorterm.as_deref())
    }

    fn resolve(no_color: bool, forced: Option<&str>, colorterm: Option<&str>) -> Self {
        if no_color {
            return Self::None;
        }
        match forced {
            Some(value) if value.eq_ignore_ascii_case("none") => Self::None,
            Some(value) if value.eq_ignore_ascii_case("true") => Self::TrueColor,
            Some("256") => Self::Palette256,
            _ if colorterm.is_some_and(|value| {
                value.eq_ignore_ascii_case("truecolor") || value == "24bit"
            }) =>
            {
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

    #[cfg(test)]
    pub(crate) fn pixel_capacity(&self) -> usize {
        self.pixels.capacity()
    }

    /// Remove terminal colour attributes while preserving glyphs and modifiers.
    pub fn strip_colors(buffer: &mut Buffer) {
        for cell in &mut buffer.content {
            cell.set_fg(Color::Reset).set_bg(Color::Reset);
        }
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
                if self.depth == ColorDepth::None {
                    cell.set_char(monochrome_symbol(top, bottom))
                        .set_fg(Color::Reset)
                        .set_bg(Color::Reset);
                    continue;
                }
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
            ColorDepth::TrueColor | ColorDepth::None => color,
            ColorDepth::Palette256 => palette_color(color),
        }
    }
}

fn monochrome_symbol(top: Option<Color>, bottom: Option<Color>) -> char {
    match (top, bottom) {
        (Some(top), Some(bottom)) => {
            let top_luminance = luminance(top);
            let bottom_luminance = luminance(bottom);
            if top_luminance.abs_diff(bottom_luminance) >= 48 {
                if top_luminance >= bottom_luminance {
                    '▀'
                } else {
                    '▄'
                }
            } else {
                shade_char(((top_luminance as u16 + bottom_luminance as u16) / 2) as u8)
            }
        }
        (Some(color), None) | (None, Some(color)) => shade_char(luminance(color)),
        (None, None) => ' ',
    }
}

fn shade_char(luminance: u8) -> char {
    match luminance {
        0..=35 => ' ',
        36..=95 => '░',
        96..=160 => '▒',
        161..=215 => '▓',
        _ => '█',
    }
}

fn luminance(color: Color) -> u8 {
    let (red, green, blue) = match color {
        Color::Reset | Color::Black => (0, 0, 0),
        Color::DarkGray => (85, 85, 85),
        Color::Gray => (170, 170, 170),
        Color::White => (255, 255, 255),
        Color::Red => (205, 0, 0),
        Color::Green => (0, 205, 0),
        Color::Yellow => (205, 205, 0),
        Color::Blue => (0, 0, 238),
        Color::Magenta => (205, 0, 205),
        Color::Cyan => (0, 205, 205),
        Color::LightRed => (255, 0, 0),
        Color::LightGreen => (0, 255, 0),
        Color::LightYellow => (255, 255, 0),
        Color::LightBlue => (92, 92, 255),
        Color::LightMagenta => (255, 0, 255),
        Color::LightCyan => (0, 255, 255),
        Color::Rgb(red, green, blue) => (red, green, blue),
        Color::Indexed(index) => indexed_rgb(index),
    };
    ((red as u32 * 299 + green as u32 * 587 + blue as u32 * 114) / 1_000) as u8
}

fn indexed_rgb(index: u8) -> (u8, u8, u8) {
    const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        0 => (0, 0, 0),
        1 => (205, 0, 0),
        2 => (0, 205, 0),
        3 => (205, 205, 0),
        4 => (0, 0, 238),
        5 => (205, 0, 205),
        6 => (0, 205, 205),
        7 => (229, 229, 229),
        8 => (127, 127, 127),
        9 => (255, 0, 0),
        10 => (0, 255, 0),
        11 => (255, 255, 0),
        12 => (92, 92, 255),
        13 => (255, 0, 255),
        14 => (0, 255, 255),
        15 => (255, 255, 255),
        16..=231 => {
            let cube_index = index - 16;
            (
                CUBE[(cube_index / 36) as usize],
                CUBE[((cube_index / 6) % 6) as usize],
                CUBE[(cube_index % 6) as usize],
            )
        }
        _ => {
            let gray = 8 + index.saturating_sub(232) * 10;
            (gray, gray, gray)
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

    #[test]
    fn color_settings_honor_no_color_and_explicit_modes() {
        assert_eq!(
            ColorDepth::resolve(true, Some("true"), Some("truecolor")),
            ColorDepth::None
        );
        assert_eq!(
            ColorDepth::resolve(false, Some("true"), None),
            ColorDepth::TrueColor
        );
        assert_eq!(
            ColorDepth::resolve(false, Some("256"), Some("truecolor")),
            ColorDepth::Palette256
        );
        assert_eq!(
            ColorDepth::resolve(false, Some("auto"), Some("24bit")),
            ColorDepth::TrueColor
        );
        assert_eq!(
            ColorDepth::resolve(false, Some("auto"), None),
            ColorDepth::Palette256
        );
        assert_eq!(
            ColorDepth::resolve(false, Some("none"), Some("truecolor")),
            ColorDepth::None
        );
    }

    #[test]
    fn monochrome_mode_renders_luminance_glyphs_without_colour() {
        for depth in [
            ColorDepth::TrueColor,
            ColorDepth::Palette256,
            ColorDepth::None,
        ] {
            let mut canvas = Canvas::with_color_depth(2, 2, depth);
            canvas.set(0, 0, Color::Rgb(255, 255, 255));
            canvas.set(0, 1, Color::Rgb(20, 20, 20));
            canvas.set(1, 0, Color::Rgb(120, 120, 120));
            canvas.set(1, 1, Color::Rgb(230, 230, 230));
            let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
            let area = buffer.area;
            canvas.render(&mut buffer, area);
            assert!(
                buffer.content.iter().any(|cell| cell.symbol() != " "),
                "each depth should emit visible output"
            );
            if depth == ColorDepth::None {
                assert!(buffer
                    .content
                    .iter()
                    .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset));
                assert!(buffer
                    .content
                    .iter()
                    .any(|cell| matches!(cell.symbol(), "▀" | "▄" | "░" | "▒" | "▓" | "█")));
            }
        }
    }
}
