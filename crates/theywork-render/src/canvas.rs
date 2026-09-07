//! A tiny pixel canvas for terminal art.
//!
//! A terminal cell can carry more pixels than it can carry colours.  The
//! encoding below keeps that trade-off explicit: dense cells are quantized to
//! two colours, while the half-block floor remains exact and widely portable.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

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
    pub(crate) fn environment_selection() -> (Self, bool, String) {
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let forced = std::env::var("THEYWORK_COLOR").ok();
        let colorterm = std::env::var("COLORTERM").ok();
        let depth = Self::resolve(no_color, forced.as_deref(), colorterm.as_deref());
        let reason = if no_color {
            "colour disabled by NO_COLOR".to_string()
        } else if let Some(value) = forced.as_deref() {
            format!("colour selected by THEYWORK_COLOR={value}")
        } else if depth == Self::TrueColor {
            "truecolor selected from COLORTERM".to_string()
        } else {
            "256-colour palette selected because truecolor was not advertised".to_string()
        };
        (depth, no_color || forced.is_some(), reason)
    }

    pub(crate) fn from_environment() -> Self {
        Self::environment_selection().0
    }

    fn resolve(no_color: bool, forced: Option<&str>, colorterm: Option<&str>) -> Self {
        if no_color {
            return Self::None;
        }
        match forced {
            Some(value) if value.eq_ignore_ascii_case("none") => Self::None,
            Some(value) if value.eq_ignore_ascii_case("true") => Self::TrueColor,
            Some(value)
                if value.eq_ignore_ascii_case("truecolor")
                    || value.eq_ignore_ascii_case("24bit") =>
            {
                Self::TrueColor
            }
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

/// The pixel pattern packed into one terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelEncoding {
    /// Unicode's six-pixel block characters: two columns by three rows.
    Sextants,
    /// Unicode quadrant block characters: two columns by two rows.
    Quadrants,
    /// The broadly supported upper/lower half block: one column by two rows.
    HalfBlocks,
}

impl PixelEncoding {
    /// The encodings in preference order, from most to least dense.
    pub(crate) const ALL: [Self; 3] = [Self::Sextants, Self::Quadrants, Self::HalfBlocks];

    pub(crate) fn environment_selection() -> (Self, bool, String) {
        let forced = std::env::var("THEYWORK_ENCODING").ok();
        let terminal = std::env::var("TERM").ok();
        let terminal_program = std::env::var("TERM_PROGRAM").ok();
        let windows_terminal_session = std::env::var("WT_SESSION").ok();
        let sextants_hint = std::env::var("THEYWORK_SEXTANTS").ok();
        let quadrants_hint = std::env::var("THEYWORK_QUADRANTS").ok();
        Self::select(
            forced.as_deref(),
            terminal.as_deref(),
            terminal_program.as_deref(),
            windows_terminal_session.as_deref(),
            sextants_hint.as_deref(),
            quadrants_hint.as_deref(),
        )
    }

    pub(crate) fn from_environment() -> Self {
        Self::environment_selection().0
    }

    fn resolve(forced: Option<&str>, capabilities: EncodingCapabilities) -> Self {
        match forced.and_then(parse_encoding) {
            Some(encoding) => encoding,
            None if capabilities.sextants => Self::Sextants,
            None if capabilities.quadrants => Self::Quadrants,
            None => Self::HalfBlocks,
        }
    }

    fn select(
        forced: Option<&str>,
        terminal: Option<&str>,
        terminal_program: Option<&str>,
        windows_terminal_session: Option<&str>,
        sextants_hint: Option<&str>,
        quadrants_hint: Option<&str>,
    ) -> (Self, bool, String) {
        if let Some(encoding) = forced.and_then(parse_encoding) {
            return (
                encoding,
                true,
                format!("{} selected by THEYWORK_ENCODING", encoding.label()),
            );
        }

        let terminal_is_usable = terminal.is_none_or(|value| {
            !value.eq_ignore_ascii_case("dumb") && !value.eq_ignore_ascii_case("cons25")
        });
        let sextants = truthy(sextants_hint)
            || windows_terminal_session.is_some_and(|session| !session.is_empty())
            || terminal_program.is_some_and(sextant_terminal);
        // Apple Terminal commonly uses font glyphs whose vertical halves do
        // not cover the line height. Half blocks with a top-colour background
        // avoid repeated gaps along every vertical edge. Explicit hints win.
        let apple_font_safe = terminal_program
            .is_some_and(|program| program.eq_ignore_ascii_case("Apple_Terminal"))
            && !sextants
            && !truthy(quadrants_hint);
        let quadrants = truthy(quadrants_hint) || (terminal_is_usable && !apple_font_safe);
        let encoding = Self::resolve(
            None,
            EncodingCapabilities {
                sextants,
                quadrants,
            },
        );
        let reason = match encoding {
            Self::Sextants if truthy(sextants_hint) => {
                "sextants selected by THEYWORK_SEXTANTS terminal hint".to_string()
            }
            Self::Sextants
                if windows_terminal_session.is_some_and(|session| !session.is_empty()) =>
            {
                "sextants selected from the Windows Terminal capability signal".to_string()
            }
            Self::Sextants => "sextants selected for a known compatible terminal".to_string(),
            Self::Quadrants => concat!(
                "quadrants selected for a usable terminal; sextant glyph coverage cannot be ",
                "queried; compare encodings in Settings (s)"
            )
            .to_string(),
            Self::HalfBlocks if apple_font_safe => concat!(
                "half-blocks selected for Apple Terminal's font geometry; sextant glyph coverage ",
                "cannot be queried; compare encodings in Settings (s)"
            )
            .to_string(),
            Self::HalfBlocks => concat!(
                "half-blocks selected because TERM is dumb or cons25; sextant glyph coverage ",
                "cannot be queried; compare encodings in Settings (s)"
            )
            .to_string(),
        };
        (
            encoding,
            forced.is_some() || sextants_hint.is_some() || quadrants_hint.is_some(),
            reason,
        )
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Sextants => "sextants",
            Self::Quadrants => "quadrants",
            Self::HalfBlocks => "half-blocks",
        }
    }

    pub(crate) fn next(self, forward: bool) -> Self {
        let index = Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0);
        let next = if forward {
            index.saturating_add(1) % Self::ALL.len()
        } else {
            (index + Self::ALL.len() - 1) % Self::ALL.len()
        };
        Self::ALL[next]
    }

    pub(crate) const fn width_per_cell(self) -> usize {
        match self {
            Self::Sextants | Self::Quadrants => 2,
            Self::HalfBlocks => 1,
        }
    }

    pub(crate) const fn height_per_cell(self) -> usize {
        match self {
            Self::Sextants => 3,
            Self::Quadrants | Self::HalfBlocks => 2,
        }
    }

    const fn sample_count(self) -> usize {
        match self {
            Self::Sextants => 6,
            Self::Quadrants => 4,
            Self::HalfBlocks => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EncodingCapabilities {
    sextants: bool,
    quadrants: bool,
}

fn parse_encoding(value: &str) -> Option<PixelEncoding> {
    if value.eq_ignore_ascii_case("sextant") || value.eq_ignore_ascii_case("sextants") {
        Some(PixelEncoding::Sextants)
    } else if value.eq_ignore_ascii_case("quadrant") || value.eq_ignore_ascii_case("quadrants") {
        Some(PixelEncoding::Quadrants)
    } else if value.eq_ignore_ascii_case("half")
        || value.eq_ignore_ascii_case("halfblock")
        || value.eq_ignore_ascii_case("half-block")
        || value.eq_ignore_ascii_case("half-blocks")
    {
        Some(PixelEncoding::HalfBlocks)
    } else {
        None
    }
}

fn truthy(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn sextant_terminal(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "contour" | "foot" | "ghostty" | "kitty" | "rio" | "wezterm"
    )
}

type QuantizationKey = (PixelEncoding, [Option<Color>; 6]);
type QuantizedCache = RefCell<HashMap<QuantizationKey, QuantizedCell>>;

/// An owned RGBA8 snapshot of a canvas and its terminal-cell destination.
///
/// The snapshot owns its bytes, so callers can encode or retain it after the
/// canvas is changed for a later frame. Transparent canvas pixels use an alpha
/// value of zero. `cell_area` is `None` until the canvas has been rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelFrame {
    width: usize,
    height: usize,
    rgba: Arc<Vec<u8>>,
    cell_area: Option<Rect>,
}

impl PixelFrame {
    /// Pixel width of this frame.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Pixel height of this frame.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Tightly packed RGBA8 pixels in row-major order.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Tightly packed RGB8 pixels in row-major order, with transparent pixels black.
    /// Use `rgba` instead when the consumer supports transparency.
    pub fn rgb(&self) -> Vec<u8> {
        self.rgba
            .chunks_exact(4)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect()
    }

    /// The exact terminal-cell rectangle last covered by this canvas.
    pub fn cell_area(&self) -> Option<Rect> {
        self.cell_area
    }
}

/// An in-memory pixel surface whose pixels are terminal colours or transparent.
#[derive(Debug, Clone)]
pub struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<Option<Color>>,
    rgba: Arc<Vec<u8>>,
    depth: ColorDepth,
    encoding: PixelEncoding,
    cell_pixel_size: Option<(usize, usize)>,
    light_mode: bool,
    quantized_cache: QuantizedCache,
    last_rendered_area: Cell<Option<Rect>>,
}

impl Canvas {
    /// Create a canvas and detect colour support once for its lifetime.
    pub fn new(width_px: usize, height_px: usize) -> Self {
        Self::with_color_depth_and_encoding(
            width_px,
            height_px,
            ColorDepth::from_environment(),
            PixelEncoding::from_environment(),
        )
    }

    /// Create a canvas with an explicit colour depth.
    ///
    /// This is useful for deterministic render tests and for callers that have
    /// already negotiated terminal capabilities.
    pub fn with_color_depth(width_px: usize, height_px: usize, depth: ColorDepth) -> Self {
        Self::with_color_depth_and_encoding(width_px, height_px, depth, PixelEncoding::HalfBlocks)
    }

    /// Create a canvas with explicit colour and pixel encodings.
    pub fn with_color_depth_and_encoding(
        width_px: usize,
        height_px: usize,
        depth: ColorDepth,
        encoding: PixelEncoding,
    ) -> Self {
        let mut canvas = Self {
            width: 0,
            height: 0,
            pixels: Vec::new(),
            rgba: Arc::new(Vec::new()),
            depth,
            encoding,
            cell_pixel_size: None,
            light_mode: false,
            quantized_cache: RefCell::new(HashMap::new()),
            last_rendered_area: Cell::new(None),
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
    pub(crate) fn set_color_depth(&mut self, depth: ColorDepth) {
        self.depth = depth;
    }

    pub(crate) fn set_light_mode(&mut self, light: bool) {
        self.light_mode = light;
    }

    pub(crate) fn is_light_mode(&self) -> bool {
        self.light_mode
    }

    /// The colour mode captured at construction.
    pub fn color_depth(&self) -> ColorDepth {
        self.depth
    }

    /// The cell encoding used when this surface is rendered.
    pub fn encoding(&self) -> PixelEncoding {
        self.encoding
    }

    pub(crate) fn set_encoding(&mut self, encoding: PixelEncoding) {
        if self.encoding != encoding {
            self.last_rendered_area.set(None);
        }
        self.encoding = encoding;
    }

    pub(crate) fn set_cell_pixel_size(&mut self, size: Option<(usize, usize)>) {
        if self.cell_pixel_size != size {
            self.last_rendered_area.set(None);
        }
        self.cell_pixel_size = size;
    }

    pub(crate) fn pixels_per_cell(&self) -> (usize, usize) {
        self.cell_pixel_size.unwrap_or((
            self.encoding.width_per_cell(),
            self.encoding.height_per_cell(),
        ))
    }

    pub(crate) fn has_image_density(&self) -> bool {
        self.cell_pixel_size.is_some()
    }

    pub(crate) fn scale_width(&self, value: usize) -> usize {
        value.saturating_mul(self.pixels_per_cell().0)
    }

    pub(crate) fn scale_half_height(&self, value: usize) -> usize {
        value
            .saturating_mul(self.pixels_per_cell().1)
            .saturating_add(1)
            / 2
    }

    #[cfg(test)]
    pub(crate) fn half_space_height(&self, value: usize) -> usize {
        value.saturating_mul(2) / self.pixels_per_cell().1
    }

    pub(crate) fn scale_image_sprite_width(&self, value: usize) -> usize {
        self.cell_pixel_size
            .map_or(value, |_| self.scale_width(value))
    }

    pub(crate) fn scale_image_sprite_height(&self, value: usize) -> usize {
        self.cell_pixel_size
            .map_or(value, |_| self.scale_half_height(value))
    }

    /// Resize the surface to exactly fill a terminal-cell rectangle.
    pub fn resize_for_cells(&mut self, width_cells: usize, height_cells: usize) {
        let (width_per_cell, height_per_cell) = self.pixels_per_cell();
        self.resize(
            width_cells.saturating_mul(width_per_cell),
            height_cells.saturating_mul(height_per_cell),
        );
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
        let rgba = Arc::make_mut(&mut self.rgba);
        rgba.resize(len.saturating_mul(4), 0);
        rgba.fill(0);
        self.last_rendered_area.set(None);
    }

    /// Make every pixel transparent.
    pub fn clear(&mut self) {
        self.pixels.fill(None);
        Arc::make_mut(&mut self.rgba).fill(0);
    }

    /// Fill the surface with one opaque colour.
    pub fn fill(&mut self, color: Color) {
        let image_color = self.themed_color(color);
        let cell_color = self.convert_color(image_color);
        self.pixels.fill(Some(cell_color));
        let (red, green, blue) = rgb_of_color(image_color);
        for pixel in Arc::make_mut(&mut self.rgba).chunks_exact_mut(4) {
            pixel.copy_from_slice(&[red, green, blue, u8::MAX]);
        }
    }

    /// Set one opaque pixel. Out-of-bounds writes are deliberately ignored so
    /// a sprite can be clipped at a terminal edge without a special branch.
    pub fn set(&mut self, x: usize, y: usize, color: Color) {
        let image_color = self.themed_color(color);
        let cell_color = self.convert_color(image_color);
        if let Some(index) = self.index(x, y) {
            let pixel = &mut self.pixels[index];
            *pixel = Some(cell_color);
            let (red, green, blue) = rgb_of_color(image_color);
            let offset = index.saturating_mul(4);
            Arc::make_mut(&mut self.rgba)[offset..offset + 4].copy_from_slice(&[
                red,
                green,
                blue,
                u8::MAX,
            ]);
        }
    }

    /// Clear one pixel without disturbing its neighbours.
    pub fn clear_pixel(&mut self, x: usize, y: usize) {
        if let Some(index) = self.index(x, y) {
            let pixel = &mut self.pixels[index];
            *pixel = None;
            let offset = index.saturating_mul(4);
            Arc::make_mut(&mut self.rgba)[offset..offset + 4].fill(0);
        }
    }

    /// Inspect a pixel, primarily for tests and small custom view effects.
    pub fn pixel(&self, x: usize, y: usize) -> Option<Color> {
        self.pixels.get(self.index(x, y)?).copied().flatten()
    }

    /// Replace exact room materials after drawing, preserving skin, clothing,
    /// status colours and alpha. Sources use the current global light theme;
    /// destinations are already selected for that theme by the room palette.
    pub(crate) fn remap_materials(&mut self, mappings: &[(Color, Color)]) {
        let mappings = mappings
            .iter()
            .map(|&(from, to)| {
                (
                    rgb_of_color(self.themed_color(from)),
                    rgb_of_color(to),
                    self.convert_color(to),
                )
            })
            .collect::<Vec<_>>();
        for (index, rgba) in Arc::make_mut(&mut self.rgba)
            .chunks_exact_mut(4)
            .enumerate()
        {
            if rgba[3] == 0 {
                continue;
            }
            if let Some((_, (r, g, b), cell_color)) = mappings
                .iter()
                .find(|(from, _, _)| *from == (rgba[0], rgba[1], rgba[2]))
            {
                rgba[..3].copy_from_slice(&[*r, *g, *b]);
                self.pixels[index] = Some(*cell_color);
            }
        }
    }

    /// Copy this canvas into an owned RGBA8 frame for a terminal-image encoder.
    ///
    /// This accessor deliberately does not select a graphics protocol or write
    /// to the terminal; callers own presenting the returned frame.
    pub fn pixel_frame(&self) -> PixelFrame {
        PixelFrame {
            width: self.width,
            height: self.height,
            rgba: Arc::clone(&self.rgba),
            cell_area: self.last_rendered_area.get(),
        }
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

    /// Emit the surface as encoded cells into a ratatui buffer.
    pub fn render(&self, buffer: &mut Buffer, area: Rect) {
        let (width_per_cell, height_per_cell) = self.pixels_per_cell();
        let pixel_width = self.width.div_ceil(width_per_cell).min(area.width as usize);
        let cell_height = self
            .height
            .div_ceil(height_per_cell)
            .min(area.height as usize);
        self.last_rendered_area.set(
            (pixel_width > 0 && cell_height > 0)
                .then(|| Rect::new(area.x, area.y, pixel_width as u16, cell_height as u16)),
        );
        for cell_y in 0..cell_height {
            for cell_x in 0..pixel_width {
                let (mut samples, sample_count) = self.samples_for_cell(cell_x, cell_y);
                if samples[..sample_count].iter().all(Option::is_none) {
                    continue;
                }

                let Some(cell) = buffer.cell_mut((
                    area.x.saturating_add(cell_x as u16),
                    area.y.saturating_add(cell_y as u16),
                )) else {
                    continue;
                };
                if self.depth == ColorDepth::None {
                    cell.set_char(monochrome_dense_symbol(
                        &samples[..sample_count],
                        self.encoding,
                    ))
                    .set_fg(Color::Reset)
                    .set_bg(Color::Reset);
                    continue;
                }
                if self.encoding == PixelEncoding::HalfBlocks {
                    match (samples[0], samples[1]) {
                        (Some(top), Some(bottom)) => {
                            // The background reaches the cell's top, including
                            // font leading. A lower block anchors to the descent;
                            // an upper block can leave a false line above it.
                            cell.set_char('▄').set_fg(bottom).set_bg(top);
                        }
                        (Some(top), None) => {
                            let bottom = cell.bg;
                            cell.set_char('▄').set_fg(bottom).set_bg(top);
                        }
                        (None, Some(bottom)) => {
                            cell.set_char('▄').set_fg(bottom);
                        }
                        (None, None) => {}
                    }
                    continue;
                }
                // A missing subpixel reveals the existing cell background, never
                // its text foreground. Resolve that colour before choosing a
                // glyph, since the quantizer may invert foreground/background.
                for sample in samples.iter_mut().take(sample_count) {
                    if sample.is_none() {
                        *sample = Some(cell.bg);
                    }
                }
                let quantized = {
                    let mut cache = self.quantized_cache.borrow_mut();
                    *cache
                        .entry((self.encoding, samples))
                        .or_insert_with(|| quantize_cell(self.encoding, &samples[..sample_count]))
                };
                cell.set_char(quantized.symbol);
                if let Some(foreground) = quantized.foreground {
                    cell.set_fg(foreground);
                }
                if let Some(background) = quantized.background {
                    cell.set_bg(background);
                }
            }
        }
    }

    fn samples_for_cell(&self, cell_x: usize, cell_y: usize) -> ([Option<Color>; 6], usize) {
        let sample_width = self.encoding.width_per_cell();
        let sample_height = self.encoding.height_per_cell();
        let (cell_width, cell_height) = self.pixels_per_cell();
        let origin_x = cell_x.saturating_mul(cell_width);
        let origin_y = cell_y.saturating_mul(cell_height);
        let mut samples = [None; 6];
        for (index, sample) in samples
            .iter_mut()
            .enumerate()
            .take(self.encoding.sample_count())
        {
            let sample_x = index % sample_width;
            let sample_y = index / sample_width;
            let x = origin_x
                + sample_x
                    .saturating_mul(cell_width)
                    .checked_div(sample_width)
                    .unwrap_or(0);
            let y = origin_y
                + sample_y
                    .saturating_mul(cell_height)
                    .checked_div(sample_height)
                    .unwrap_or(0);
            *sample = self.pixel(x, y);
        }
        (samples, self.encoding.sample_count())
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        (x < self.width && y < self.height).then(|| y * self.width + x)
    }

    fn themed_color(&self, color: Color) -> Color {
        if self.light_mode {
            crate::views::light_color(color)
        } else {
            color
        }
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

fn monochrome_dense_symbol(samples: &[Option<Color>], encoding: PixelEncoding) -> char {
    if encoding == PixelEncoding::HalfBlocks {
        return monochrome_symbol(samples[0], samples[1]);
    }
    let mut total = 0u32;
    let mut present = 0u32;
    for sample in samples.iter().flatten() {
        total = total.saturating_add(u32::from(luminance(*sample)));
        present += 1;
    }
    if present == 0 {
        return ' ';
    }
    let threshold = (total / present) as u8;
    let mut mask = 0u8;
    for (index, sample) in samples.iter().enumerate() {
        if sample.is_some_and(|color| luminance(color) >= threshold) {
            mask |= 1 << index;
        }
    }
    if mask == 0 || mask == (1u8 << samples.len()).saturating_sub(1) {
        shade_char(threshold)
    } else {
        glyph_for_mask(encoding, mask).unwrap_or_else(|| shade_char(threshold))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QuantizedCell {
    symbol: char,
    foreground: Option<Color>,
    background: Option<Color>,
}

fn quantize_cell(encoding: PixelEncoding, samples: &[Option<Color>]) -> QuantizedCell {
    let mut candidates = [None; 7];
    let mut candidate_count = 0usize;
    for color in samples.iter().copied().flatten() {
        if !candidates[..candidate_count].contains(&Some(color)) {
            candidates[candidate_count] = Some(color);
            candidate_count += 1;
        }
    }
    if samples.iter().any(Option::is_none) {
        candidates[candidate_count] = None;
        candidate_count += 1;
    }

    let mut best = None;
    for foreground in candidates[..candidate_count].iter().copied() {
        for background in candidates[..candidate_count].iter().copied() {
            for mask in supported_masks(encoding) {
                let error = cell_error(samples, mask, foreground, background);
                let symbol = glyph_for_mask(encoding, mask).expect("supported glyph");
                let candidate = QuantizedCell {
                    symbol,
                    foreground,
                    background,
                };
                let bit_count = mask.count_ones();
                let is_better =
                    best.as_ref()
                        .is_none_or(|(best_error, best_bits, best_mask, _)| {
                            error < *best_error
                                || (error == *best_error
                                    && (bit_count > *best_bits
                                        || (bit_count == *best_bits && mask < *best_mask)))
                        });
                if is_better {
                    best = Some((error, bit_count, mask, candidate));
                }
            }
        }
    }
    best.expect("a non-empty cell has quantization candidates")
        .3
}

fn cell_error(
    samples: &[Option<Color>],
    mask: u8,
    foreground: Option<Color>,
    background: Option<Color>,
) -> u32 {
    samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let selected = if mask & (1 << index) != 0 {
                foreground
            } else {
                background
            };
            option_color_distance(*sample, selected)
        })
        .sum()
}

fn option_color_distance(a: Option<Color>, b: Option<Color>) -> u32 {
    match (a, b) {
        (None, None) => 0,
        (Some(a), Some(b)) => color_distance(rgb_of_color(a), rgb_of_color(b)),
        _ => 1_000_000,
    }
}

fn supported_masks(encoding: PixelEncoding) -> impl Iterator<Item = u8> {
    (0..=63).filter(move |mask| glyph_for_mask(encoding, *mask).is_some())
}

fn glyph_for_mask(encoding: PixelEncoding, mask: u8) -> Option<char> {
    match encoding {
        PixelEncoding::HalfBlocks => match mask {
            0 => Some(' '),
            1 => Some('▀'),
            2 => Some('▄'),
            3 => Some('█'),
            _ => None,
        },
        PixelEncoding::Quadrants => {
            const GLYPHS: [char; 16] = [
                ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
            ];
            GLYPHS.get(mask as usize).copied()
        }
        PixelEncoding::Sextants => sextant_glyph(mask),
    }
}

fn sextant_glyph(mask: u8) -> Option<char> {
    // Unicode omits full-height columns from the sextant range because the
    // Block Elements range already contains them as LEFT/RIGHT HALF BLOCK.
    // They are still valid masks; excluding them damages every vertical edge.
    const MASKS: [u8; 60] = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 22, 23, 24, 25, 26,
        27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 43, 44, 45, 46, 47, 48, 49, 50,
        51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62,
    ];
    if mask == 0 {
        Some(' ')
    } else if mask == 21 {
        Some('▌')
    } else if mask == 42 {
        Some('▐')
    } else if mask == 63 {
        Some('█')
    } else {
        MASKS
            .iter()
            .position(|candidate| *candidate == mask)
            .and_then(|index| char::from_u32(0x1f_b00 + index as u32))
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
    let (red, green, blue) = rgb_of_color(color);
    ((red as u32 * 299 + green as u32 * 587 + blue as u32 * 114) / 1_000) as u8
}

fn rgb_of_color(color: Color) -> (u8, u8, u8) {
    match color {
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
    }
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
    fn half_block_packs_bottom_into_foreground_and_top_into_background() {
        let mut canvas = Canvas::with_color_depth(1, 2, ColorDepth::TrueColor);
        let top = Color::Rgb(255, 50, 90);
        let bottom = Color::Rgb(40, 200, 150);
        canvas.set(0, 0, top);
        canvas.set(0, 1, bottom);

        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let area = buffer.area;
        canvas.render(&mut buffer, area);
        let cell = buffer.cell((0, 0)).expect("one cell");
        assert_eq!(cell.symbol(), "▄");
        assert_eq!(cell.fg, bottom);
        assert_eq!(cell.bg, top);
    }

    #[test]
    fn half_block_bottom_only_preserves_the_existing_background() {
        let foreground = Color::Rgb(12, 80, 190);
        let background = Color::Rgb(32, 21, 40);
        let mut canvas = Canvas::with_color_depth(1, 2, ColorDepth::TrueColor);
        canvas.set(0, 1, foreground);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        buffer[(0, 0)].set_fg(Color::Red).set_bg(background);
        canvas.render(&mut buffer, Rect::new(0, 0, 1, 1));
        assert_eq!(buffer[(0, 0)].symbol(), "▄");
        assert_eq!(buffer[(0, 0)].fg, foreground);
        assert_eq!(buffer[(0, 0)].bg, background);
    }

    // Independent Unicode character-name oracle. Sextant numbers follow
    // reading order: 12 / 34 / 56. This uses the published names, not the
    // encoder's numeric mask table or its supported_masks helper.
    fn unicode_sample_mask(encoding: PixelEncoding, symbol: char) -> u8 {
        if encoding == PixelEncoding::Sextants && (0x1fb00..=0x1fb3b).contains(&(symbol as u32)) {
            const NAMES: &str = "1 2 12 3 13 23 123 4 14 24 124 34 134 234 1234 5 15 25 125 35 235 1235 45 145 245 1245 345 1345 2345 12345 6 16 26 126 36 136 236 1236 46 146 1246 346 1346 2346 12346 56 156 256 1256 356 1356 2356 12356 456 1456 2456 12456 3456 13456 23456";
            return NAMES
                .split_whitespace()
                .nth(symbol as usize - 0x1fb00)
                .unwrap()
                .bytes()
                .fold(0, |mask, digit| mask | (1 << (digit - b'1')));
        }
        match (encoding, symbol) {
            (_, ' ') => 0,
            (PixelEncoding::Sextants, '▌') => 0b010101,
            (PixelEncoding::Sextants, '▐') => 0b101010,
            (PixelEncoding::Sextants, '█') => 0b111111,
            (PixelEncoding::Quadrants, '▘') => 0b0001,
            (PixelEncoding::Quadrants, '▝') => 0b0010,
            (PixelEncoding::Quadrants, '▀') => 0b0011,
            (PixelEncoding::Quadrants, '▖') => 0b0100,
            (PixelEncoding::Quadrants, '▌') => 0b0101,
            (PixelEncoding::Quadrants, '▞') => 0b0110,
            (PixelEncoding::Quadrants, '▛') => 0b0111,
            (PixelEncoding::Quadrants, '▗') => 0b1000,
            (PixelEncoding::Quadrants, '▚') => 0b1001,
            (PixelEncoding::Quadrants, '▐') => 0b1010,
            (PixelEncoding::Quadrants, '▜') => 0b1011,
            (PixelEncoding::Quadrants, '▄') => 0b1100,
            (PixelEncoding::Quadrants, '▙') => 0b1101,
            (PixelEncoding::Quadrants, '▟') => 0b1110,
            (PixelEncoding::Quadrants, '█') => 0b1111,
            _ => panic!("unexpected glyph: {symbol:?}"),
        }
    }

    #[test]
    fn dense_encodings_preserve_every_two_colour_mask_using_unicode_names() {
        let front = Color::Rgb(220, 34, 43);
        let back = Color::Rgb(19, 18, 31);
        for encoding in [PixelEncoding::Quadrants, PixelEncoding::Sextants] {
            for mask in 0..(1 << encoding.sample_count()) {
                let samples: Vec<_> = (0..encoding.sample_count())
                    .map(|bit| Some(if mask & (1 << bit) != 0 { front } else { back }))
                    .collect();
                let cell = quantize_cell(encoding, &samples);
                let actual = unicode_sample_mask(encoding, cell.symbol);
                for (bit, expected) in samples.into_iter().enumerate() {
                    let reconstructed = if actual & (1 << bit) != 0 {
                        cell.foreground
                    } else {
                        cell.background
                    };
                    assert_eq!(
                        reconstructed, expected,
                        "{encoding:?} mask={mask} bit={bit} glyph={:?}",
                        cell.symbol
                    );
                }
            }
        }
    }

    #[test]
    fn partial_dense_pixels_reveal_background_even_when_the_glyph_is_inverted() {
        let front = Color::Rgb(220, 34, 43);
        let back = Color::Rgb(19, 18, 31);
        for encoding in [PixelEncoding::Quadrants, PixelEncoding::Sextants] {
            for mask in 1..(1 << encoding.sample_count()) {
                let mut canvas = Canvas::with_color_depth_and_encoding(
                    encoding.width_per_cell(),
                    encoding.height_per_cell(),
                    ColorDepth::TrueColor,
                    encoding,
                );
                for bit in 0..encoding.sample_count() {
                    if mask & (1 << bit) != 0 {
                        canvas.set(bit % 2, bit / 2, front);
                    }
                }
                let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
                buffer[(0, 0)].set_fg(Color::Green).set_bg(back);
                canvas.render(&mut buffer, Rect::new(0, 0, 1, 1));
                let cell = &buffer[(0, 0)];
                let actual = unicode_sample_mask(encoding, cell.symbol().chars().next().unwrap());
                for bit in 0..encoding.sample_count() {
                    let reconstructed = if actual & (1 << bit) != 0 {
                        cell.fg
                    } else {
                        cell.bg
                    };
                    assert_eq!(
                        reconstructed,
                        if mask & (1 << bit) != 0 { front } else { back },
                        "{encoding:?} mask={mask} bit={bit}"
                    );
                }
            }
        }
    }

    #[test]
    fn pixel_frame_owns_rgba_pixels_and_last_cell_area() {
        let mut canvas = Canvas::with_color_depth(2, 1, ColorDepth::TrueColor);
        canvas.set(0, 0, Color::Rgb(12, 34, 56));

        let before_render = canvas.pixel_frame();
        assert_eq!((before_render.width(), before_render.height()), (2, 1));
        assert_eq!(before_render.rgba(), [12, 34, 56, 255, 0, 0, 0, 0]);
        assert_eq!(before_render.cell_area(), None);

        let mut buffer = Buffer::empty(Rect::new(5, 7, 2, 1));
        canvas.render(&mut buffer, Rect::new(5, 7, 2, 1));
        let frame = canvas.pixel_frame();
        canvas.set(0, 0, Color::Rgb(90, 80, 70));

        assert_eq!(frame.rgba(), [12, 34, 56, 255, 0, 0, 0, 0]);
        assert_eq!(frame.cell_area(), Some(Rect::new(5, 7, 2, 1)));
    }

    #[test]
    fn requested_cell_density_controls_canvas_dimensions() {
        let mut canvas = Canvas::with_color_depth_and_encoding(
            0,
            0,
            ColorDepth::TrueColor,
            PixelEncoding::Sextants,
        );
        canvas.set_cell_pixel_size(Some((10, 20)));
        canvas.resize_for_cells(160, 48);
        assert_eq!((canvas.width(), canvas.height()), (1_600, 960));

        canvas.set_cell_pixel_size(None);
        canvas.resize_for_cells(160, 48);
        assert_eq!((canvas.width(), canvas.height()), (320, 144));
    }

    #[test]
    fn dropped_pixel_frames_reuse_the_rgba_allocation() {
        let mut canvas = Canvas::with_color_depth(1_600, 960, ColorDepth::TrueColor);
        canvas.fill(Color::Rgb(12, 34, 56));
        let first = canvas.pixel_frame();
        let allocation = first.rgba().as_ptr();
        drop(first);

        canvas.clear();
        canvas.set(0, 0, Color::Rgb(90, 80, 70));
        let second = canvas.pixel_frame();
        assert_eq!(second.rgba().as_ptr(), allocation);
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
        canvas.set(0, 0, Color::Rgb(58, 51, 88));
        assert!(matches!(canvas.pixel(0, 0), Some(Color::Indexed(_))));
        assert_eq!(canvas.pixel_frame().rgba(), [58, 51, 88, 255]);
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

    #[test]
    fn encoding_detection_prefers_dense_support_and_accepts_forcing() {
        assert_eq!(
            PixelEncoding::resolve(
                None,
                EncodingCapabilities {
                    sextants: true,
                    quadrants: true,
                },
            ),
            PixelEncoding::Sextants
        );
        assert_eq!(
            PixelEncoding::resolve(
                None,
                EncodingCapabilities {
                    sextants: false,
                    quadrants: true,
                },
            ),
            PixelEncoding::Quadrants
        );
        assert_eq!(
            PixelEncoding::resolve(
                None,
                EncodingCapabilities {
                    sextants: false,
                    quadrants: false,
                },
            ),
            PixelEncoding::HalfBlocks
        );
        assert_eq!(
            parse_encoding("half-block"),
            Some(PixelEncoding::HalfBlocks)
        );
        assert_eq!(parse_encoding("QUADRANTS"), Some(PixelEncoding::Quadrants));
    }

    #[test]
    fn locale_less_terminals_use_the_conservative_dense_fallback() {
        let (encoding, locked, reason) = PixelEncoding::select(None, None, None, None, None, None);
        assert_eq!(encoding, PixelEncoding::Quadrants);
        assert!(!locked);
        assert!(reason.contains("compare encodings in Settings (s)"));

        for terminal in ["dumb", "cons25", "DUMB"] {
            let (encoding, locked, reason) =
                PixelEncoding::select(None, Some(terminal), None, None, None, None);
            assert_eq!(encoding, PixelEncoding::HalfBlocks);
            assert!(!locked);
            assert!(reason.contains("dumb or cons25"));
        }
    }

    #[test]
    fn explicit_sextants_override_is_reported() {
        let (encoding, locked, reason) =
            PixelEncoding::select(Some("sextants"), Some("dumb"), None, None, None, None);
        assert_eq!(encoding, PixelEncoding::Sextants);
        assert!(locked);
        assert_eq!(reason, "sextants selected by THEYWORK_ENCODING");
    }

    #[test]
    fn windows_terminal_session_selects_sextants_without_a_program_name() {
        let (encoding, locked, reason) = PixelEncoding::select(
            None,
            Some("xterm-256color"),
            None,
            Some("session-id"),
            None,
            None,
        );
        assert_eq!(encoding, PixelEncoding::Sextants);
        assert!(!locked);
        assert!(reason.contains("Windows Terminal capability signal"));
    }

    #[test]
    fn apple_terminal_uses_lower_density_unless_explicitly_overridden() {
        let select = |forced, sextants, quadrants| {
            PixelEncoding::select(
                forced,
                Some("xterm-256color"),
                Some("Apple_Terminal"),
                None,
                sextants,
                quadrants,
            )
        };
        let (encoding, locked, reason) = select(None, None, None);
        assert_eq!(encoding, PixelEncoding::HalfBlocks);
        assert!(!locked);
        assert!(reason.contains("Apple Terminal's font geometry"));
        assert_eq!(
            select(Some("quadrants"), None, None).0,
            PixelEncoding::Quadrants
        );
        assert_eq!(
            select(Some("sextants"), None, None).0,
            PixelEncoding::Sextants
        );
        assert_eq!(select(None, Some("1"), None).0, PixelEncoding::Sextants);
        assert_eq!(select(None, None, Some("1")).0, PixelEncoding::Quadrants);
    }

    #[test]
    fn half_blocks_preserve_all_opaque_and_transparent_sample_pairs() {
        let backdrop = Color::Rgb(19, 18, 31);
        let red = Color::Rgb(220, 34, 43);
        let blue = Color::Rgb(14, 170, 210);
        for top in [None, Some(red), Some(blue)] {
            for bottom in [None, Some(red), Some(blue)] {
                let mut canvas = Canvas::with_color_depth(1, 2, ColorDepth::TrueColor);
                if let Some(top) = top {
                    canvas.set(0, 0, top);
                }
                if let Some(bottom) = bottom {
                    canvas.set(0, 1, bottom);
                }
                let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
                buffer[(0, 0)].set_fg(Color::Green).set_bg(backdrop);
                canvas.render(&mut buffer, Rect::new(0, 0, 1, 1));
                let cell = &buffer[(0, 0)];
                if top.is_none() && bottom.is_none() {
                    assert_eq!(cell.symbol(), " ");
                    assert_eq!(cell.bg, backdrop);
                } else {
                    assert_eq!(cell.symbol(), "▄");
                    assert_eq!(cell.bg, top.unwrap_or(backdrop));
                    assert_eq!(cell.fg, bottom.unwrap_or(backdrop));
                }
            }
        }
    }

    #[test]
    fn cell_resize_matches_the_three_encoding_ladder() {
        let expected = [
            (PixelEncoding::Sextants, (320, 144)),
            (PixelEncoding::Quadrants, (320, 96)),
            (PixelEncoding::HalfBlocks, (160, 96)),
        ];
        for (encoding, (width, height)) in expected {
            let mut canvas =
                Canvas::with_color_depth_and_encoding(0, 0, ColorDepth::TrueColor, encoding);
            canvas.resize_for_cells(160, 48);
            assert_eq!((canvas.width(), canvas.height()), (width, height));
        }
    }

    #[test]
    fn quadrant_quantizer_preserves_a_two_colour_split() {
        let red = Color::Rgb(232, 52, 44);
        let blue = Color::Rgb(79, 158, 232);
        let mut canvas = Canvas::with_color_depth_and_encoding(
            4,
            2,
            ColorDepth::TrueColor,
            PixelEncoding::Quadrants,
        );
        for x in 0..2 {
            canvas.set(x, 0, red);
            canvas.set(x, 1, blue);
        }
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
        let area = buffer.area;
        canvas.render(&mut buffer, area);
        let cell = buffer.cell((0, 0)).expect("one quadrant cell");
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, red);
        assert_eq!(cell.bg, blue);
    }

    #[test]
    fn sextant_quantizer_uses_a_supported_glyph_for_six_samples() {
        let red = Color::Rgb(232, 52, 44);
        let blue = Color::Rgb(79, 158, 232);
        let samples = [
            Some(red),
            Some(red),
            Some(blue),
            Some(blue),
            Some(blue),
            Some(blue),
        ];
        let cell = quantize_cell(PixelEncoding::Sextants, &samples);
        assert!(
            cell.symbol == char::from_u32(0x1f_b02).expect("sextant glyph")
                || cell.symbol == char::from_u32(0x1f_b39).expect("sextant glyph")
        );
        assert!(
            (cell.foreground == Some(red) && cell.background == Some(blue))
                || (cell.foreground == Some(blue) && cell.background == Some(red))
        );
        assert_eq!(sextant_glyph(21), Some('▌'));
        assert_eq!(sextant_glyph(42), Some('▐'));
    }
}
