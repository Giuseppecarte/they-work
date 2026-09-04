//! Small, opt-in image transport for terminals that can display real pixels.
//!
//! The block renderer remains independent of this crate. This crate only owns
//! terminal capability probing, protocol bytes, and the lifetime of images
//! placed by its surface.
//!
//! Expected terminals: Kitty transport for kitty, Ghostty and WezTerm; inline
//! PNG for iTerm2; Sixel for Windows Terminal 1.22+. Terminal.app and terminals
//! without a positive graphics capability use the unchanged cell renderer.
//! Actual interactive testing here is limited to an xterm-256color PTY without
//! graphics replies. Protocol tests capture bytes; they are not evidence of
//! graphical playback on Windows or macOS.
//!
//! For inspectable synthetic frames, run the `measure` example in release mode
//! with `--dump /src/new-dump-directory` (and optionally `--noise`). It creates
//! a PNG and exact first-frame `.bin` transmissions for each protocol. The
//! directory must not already exist. These bytes contain terminal controls:
//! inspect them as binary data unless intentionally testing a compatible terminal.
//! Measurements exclude terminal parsing and display; a high encoder frame rate
//! does not establish end-to-end throughput. At 1600x960 the high-detail Sixel
//! baseline was 7.21 MB/frame, requiring about 72 MB/s at ten frames per second.
//! The TUI therefore caps Sixel at five updates/second and spaces large frames
//! against a conservative 8 MB/s budget, also allowing twice the observed write
//! duration. Intermediate animation frames are dropped rather than queued.
//! This policy still needs Windows Terminal playback validation; the first
//! large synchronous write can exceed the interactive frame budget.

use std::fmt;
use std::io::{self, Write};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::io::{IsTerminal, Read, Stdin, Stdout};

pub const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_millis(8);
const KITTY_CHUNK_BYTES: usize = 4_096;
const SIXEL_COLORS: usize = 216;
const MAX_PROBE_RESPONSE_BYTES: usize = 64 * 1024;

pub const KITTY_DIRECT_QUERY: &[u8] = b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=32;AAAA\x1b\\";
pub const KITTY_FILE_QUERY: &[u8] = b"\x1b_Gi=32,s=1,v=1,a=q,t=f,f=32;AAAA\x1b\\";
pub const SIXEL_QUERY: &[u8] = b"\x1b[c";
pub const CELL_SIZE_QUERY: &[u8] = b"\x1b[16t\x1b[14t\x1b[18t";
pub const TERMINAL_VERSION_QUERY: &[u8] = b"\x1b[>q";

/// The image protocol selected for a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsProtocol {
    /// Kitty graphics with direct (inline) transmission.
    Kitty { direct_transmission: bool },
    /// iTerm2 inline PNG images (OSC 1337).
    Iterm2,
    /// DEC Sixel graphics.
    Sixel,
    /// No supported image protocol was detected.
    None,
}

impl Default for GraphicsProtocol {
    fn default() -> Self {
        Self::None
    }
}

impl GraphicsProtocol {
    pub fn is_available(self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn can_transmit_pixels(self) -> bool {
        matches!(
            self,
            Self::Kitty {
                direct_transmission: true
            } | Self::Iterm2
                | Self::Sixel
        )
    }
}

/// Pixel dimensions of one terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellSize {
    pub width: u16,
    pub height: u16,
}

impl CellSize {
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    fn is_valid(self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// Terminal cell dimensions and, when known, its physical cell size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalGeometry {
    pub columns: u16,
    pub rows: u16,
    pub cell_size: Option<CellSize>,
}

impl TerminalGeometry {
    pub const fn new(columns: u16, rows: u16, cell_size: Option<CellSize>) -> Self {
        Self {
            columns,
            rows,
            cell_size,
        }
    }

    pub fn pixel_size(self, rectangle: CellRect) -> Option<(u32, u32)> {
        let cell = self.cell_size?;
        Some((
            u32::from(rectangle.width).checked_mul(u32::from(cell.width))?,
            u32::from(rectangle.height).checked_mul(u32::from(cell.height))?,
        ))
    }
}

/// A rectangle measured in terminal cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl CellRect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Clip a rectangle to a terminal geometry without overflowing u16 math.
    pub fn clipped(self, geometry: TerminalGeometry) -> Option<Self> {
        if self.is_empty() || self.x >= geometry.columns || self.y >= geometry.rows {
            return None;
        }
        let right = u32::from(self.x) + u32::from(self.width);
        let bottom = u32::from(self.y) + u32::from(self.height);
        let clipped_right = right.min(u32::from(geometry.columns));
        let clipped_bottom = bottom.min(u32::from(geometry.rows));
        let width = clipped_right.saturating_sub(u32::from(self.x));
        let height = clipped_bottom.saturating_sub(u32::from(self.y));
        if width == 0 || height == 0 {
            None
        } else {
            Some(Self {
                width: width as u16,
                height: height as u16,
                ..self
            })
        }
    }
}

/// A validated, tightly-packed RGBA8 image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, ImageError> {
        let expected = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(ImageError::DimensionsOverflow)? as usize;
        if width == 0 || height == 0 {
            return Err(ImageError::EmptyImage);
        }
        if pixels.len() != expected {
            return Err(ImageError::PixelDataLength {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Result<Self, ImageError> {
        let pixel_count = width
            .checked_mul(height)
            .ok_or(ImageError::DimensionsOverflow)? as usize;
        let mut pixels = Vec::with_capacity(pixel_count.saturating_mul(4));
        for _ in 0..pixel_count {
            pixels.extend_from_slice(&rgba);
        }
        Self::new(width, height, pixels)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    fn resized_nearest(&self, width: u32, height: u32) -> Result<Self, ImageError> {
        if width == self.width && height == self.height {
            return Ok(self.clone());
        }
        if width == 0 || height == 0 {
            return Err(ImageError::EmptyImage);
        }
        let output_len = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(ImageError::DimensionsOverflow)? as usize;
        let mut pixels = vec![0_u8; output_len];
        for output_y in 0..height {
            let source_y = output_y.saturating_mul(self.height) / height;
            for output_x in 0..width {
                let source_x = output_x.saturating_mul(self.width) / width;
                let source = ((source_y * self.width + source_x) * 4) as usize;
                let destination = ((output_y * width + output_x) * 4) as usize;
                pixels[destination..destination + 4]
                    .copy_from_slice(&self.pixels[source..source + 4]);
            }
        }
        Self::new(width, height, pixels)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageError {
    EmptyImage,
    DimensionsOverflow,
    PixelDataLength { expected: usize, actual: usize },
    InvalidMeasurementCount,
    UnsupportedProtocol,
    PngEncoding(String),
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyImage => formatter.write_str("image dimensions must be non-zero"),
            Self::DimensionsOverflow => formatter.write_str("image dimensions overflow usize"),
            Self::PixelDataLength { expected, actual } => {
                write!(formatter, "image needs {expected} RGBA bytes, got {actual}")
            }
            Self::InvalidMeasurementCount => {
                formatter.write_str("measurement count must be positive")
            }
            Self::UnsupportedProtocol => formatter.write_str("protocol cannot transmit pixels"),
            Self::PngEncoding(error) => write!(formatter, "PNG encoding failed: {error}"),
        }
    }
}

impl std::error::Error for ImageError {}

/// A terminal's response to the bounded capability probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub graphics: GraphicsProtocol,
    pub cell_size: Option<CellSize>,
    pub terminal_cells: Option<(u16, u16)>,
}

impl Capabilities {
    pub const fn none() -> Self {
        Self {
            graphics: GraphicsProtocol::None,
            cell_size: None,
            terminal_cells: None,
        }
    }

    pub fn geometry(self) -> Option<TerminalGeometry> {
        let (columns, rows) = self.terminal_cells?;
        Some(TerminalGeometry::new(columns, rows, self.cell_size))
    }
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::none()
    }
}

/// I/O needed by the probe. Implementations must make `receive` bounded by
/// the supplied timeout; this keeps a terminal that ignores queries harmless.
pub trait ProbeIo {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn receive(&mut self, timeout: Duration) -> io::Result<Vec<u8>>;
}

/// A capability detector with a deliberately small deadline.
#[derive(Debug, Clone, Copy)]
pub struct CapabilityDetector {
    timeout: Duration,
}

impl CapabilityDetector {
    pub const fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    pub const fn default() -> Self {
        Self::new(DEFAULT_PROBE_TIMEOUT)
    }

    pub fn timeout(self) -> Duration {
        self.timeout
    }

    pub fn detect<I: ProbeIo>(&self, io: &mut I) -> io::Result<Capabilities> {
        let mut query = Vec::with_capacity(
            KITTY_DIRECT_QUERY.len()
                + KITTY_FILE_QUERY.len()
                + SIXEL_QUERY.len()
                + CELL_SIZE_QUERY.len(),
        );
        query.extend_from_slice(KITTY_DIRECT_QUERY);
        query.extend_from_slice(KITTY_FILE_QUERY);
        query.extend_from_slice(SIXEL_QUERY);
        query.extend_from_slice(CELL_SIZE_QUERY);
        query.extend_from_slice(TERMINAL_VERSION_QUERY);
        io.send(&query)?;

        let started = Instant::now();
        let mut response = Vec::new();
        loop {
            let elapsed = started.elapsed();
            if elapsed >= self.timeout {
                break;
            }
            let bytes = io.receive(self.timeout.saturating_sub(elapsed))?;
            if bytes.is_empty() {
                break;
            }
            let remaining = MAX_PROBE_RESPONSE_BYTES.saturating_sub(response.len());
            response.extend(bytes.into_iter().take(remaining));
            if response.len() >= MAX_PROBE_RESPONSE_BYTES {
                break;
            }
        }
        Ok(parse_capabilities(&response))
    }
}

impl Default for CapabilityDetector {
    fn default() -> Self {
        Self::new(DEFAULT_PROBE_TIMEOUT)
    }
}

pub fn parse_capabilities(response: &[u8]) -> Capabilities {
    let (kitty_seen, kitty_direct) = parse_kitty_replies(response);
    let sixel = parse_sixel_device_attributes(response);
    let cell_size = parse_cell_size(response).or_else(|| parse_derived_cell_size(response));
    let terminal_cells = parse_terminal_cells(response);
    let graphics = if kitty_direct {
        GraphicsProtocol::Kitty {
            direct_transmission: true,
        }
    } else if response
        .windows(b"\x1bP>|iTerm2".len())
        .any(|window| window == b"\x1bP>|iTerm2")
    {
        GraphicsProtocol::Iterm2
    } else if sixel {
        GraphicsProtocol::Sixel
    } else if kitty_seen {
        GraphicsProtocol::Kitty {
            direct_transmission: false,
        }
    } else {
        GraphicsProtocol::None
    };
    Capabilities {
        graphics,
        cell_size,
        terminal_cells,
    }
}

pub fn parse_cell_size(response: &[u8]) -> Option<CellSize> {
    for body in csi_bodies(response, b't') {
        let mut fields = body.split(';');
        if fields.next().is_none_or(|field| field.trim() != "6") {
            continue;
        }
        let Some(height) = fields.next().and_then(|value| value.parse::<u16>().ok()) else {
            continue;
        };
        let Some(width) = fields.next().and_then(|value| value.parse::<u16>().ok()) else {
            continue;
        };
        let cell = CellSize::new(width, height);
        if cell.is_valid() {
            return Some(cell);
        }
    }
    None
}

fn parse_derived_cell_size(response: &[u8]) -> Option<CellSize> {
    let mut window = None;
    let mut cells = None;
    for body in csi_bodies(response, b't') {
        let mut fields = body.split(';');
        let Some(kind) = fields.next().map(str::trim) else {
            continue;
        };
        let Some(first) = fields.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Some(second) = fields.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        match kind {
            "4" => window = Some((first, second)),
            "8" => cells = Some((first, second)),
            _ => {}
        }
    }
    let (window_height, window_width) = window?;
    let (rows, columns) = cells?;
    if rows == 0 || columns == 0 {
        return None;
    }
    let width = window_width / columns;
    let height = window_height / rows;
    let cell = CellSize::new(width.try_into().ok()?, height.try_into().ok()?);
    cell.is_valid().then_some(cell)
}

fn parse_terminal_cells(response: &[u8]) -> Option<(u16, u16)> {
    for body in csi_bodies(response, b't') {
        let mut fields = body.split(';');
        if fields.next().is_none_or(|field| field.trim() != "8") {
            continue;
        }
        let Some(rows) = fields.next().and_then(|value| value.parse::<u16>().ok()) else {
            continue;
        };
        let Some(columns) = fields.next().and_then(|value| value.parse::<u16>().ok()) else {
            continue;
        };
        if rows > 0 && columns > 0 {
            return Some((columns, rows));
        }
    }
    None
}

fn parse_kitty_replies(response: &[u8]) -> (bool, bool) {
    let mut seen = false;
    let mut direct = false;
    let mut cursor = 0;
    while cursor + 3 <= response.len() {
        let Some(relative) = response[cursor..]
            .windows(3)
            .position(|window| window == b"\x1b_G")
        else {
            break;
        };
        let start = cursor + relative + 3;
        let Some(end_relative) = response[start..]
            .windows(2)
            .position(|window| window == b"\x1b\\")
        else {
            break;
        };
        let end = start + end_relative;
        let body = String::from_utf8_lossy(&response[start..end]);
        let id = body.split(',').chain(body.split(';')).find_map(|field| {
            field
                .strip_prefix("i=")
                .and_then(|value| value.parse::<u32>().ok())
        });
        let status = body.rsplit(';').next().map(str::trim);
        let response_status = status.is_some_and(|value| {
            value == "OK" || value == "EINVAL" || value == "ENOTSUP" || value == "ENOSYS"
        });
        if response_status && matches!(id, Some(31 | 32)) {
            seen = true;
            if id == Some(31) && status == Some("OK") {
                direct = true;
            }
        }
        cursor = end + 2;
    }
    (seen, direct)
}

fn parse_sixel_device_attributes(response: &[u8]) -> bool {
    csi_bodies(response, b'c').any(|body| {
        body.strip_prefix('?')
            .is_some_and(|parameters| parameters.split(';').any(|value| value == "4"))
    })
}

fn csi_bodies(response: &[u8], final_byte: u8) -> impl Iterator<Item = &str> {
    let mut bodies = Vec::new();
    let mut cursor = 0;
    while cursor + 2 < response.len() {
        let Some(relative) = response[cursor..]
            .windows(2)
            .position(|window| window == b"\x1b[")
        else {
            break;
        };
        let start = cursor + relative + 2;
        let Some(end_relative) = response[start..]
            .iter()
            .position(|byte| (0x40..=0x7e).contains(byte))
        else {
            break;
        };
        let end = start + end_relative;
        if response[end] == final_byte {
            if let Ok(body) = std::str::from_utf8(&response[start..end]) {
                bodies.push(body);
            }
        }
        cursor = end + 1;
    }
    bodies.into_iter()
}

#[cfg(unix)]
pub fn detect_terminal() -> io::Result<Capabilities> {
    detect_terminal_with_timeout(DEFAULT_PROBE_TIMEOUT)
}

#[cfg(not(unix))]
pub fn detect_terminal() -> io::Result<Capabilities> {
    Ok(Capabilities::none())
}

#[cfg(unix)]
pub fn detect_terminal_with_timeout(timeout: Duration) -> io::Result<Capabilities> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    if !stdin.is_terminal() || !stdout.is_terminal() {
        return Ok(Capabilities::none());
    }

    crossterm::terminal::enable_raw_mode()?;
    let _raw_mode = RawModeGuard;
    let mut terminal = NativeProbeIo { stdin, stdout };
    let mut capabilities = CapabilityDetector::new(timeout).detect(&mut terminal)?;
    // Older iTerm2 versions identify themselves through the environment rather
    // than XTVERSION. Do not infer passthrough support inside multiplexers.
    if std::env::var("TERM_PROGRAM").as_deref() == Ok("iTerm.app")
        && std::env::var_os("TMUX").is_none()
        && std::env::var_os("STY").is_none()
        && !matches!(
            capabilities.graphics,
            GraphicsProtocol::Kitty {
                direct_transmission: true
            }
        )
    {
        capabilities.graphics = GraphicsProtocol::Iterm2;
    }
    Ok(capabilities)
}

#[cfg(not(unix))]
pub fn detect_terminal_with_timeout(_: Duration) -> io::Result<Capabilities> {
    Ok(Capabilities::none())
}

#[cfg(unix)]
struct RawModeGuard;

#[cfg(unix)]
impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

#[cfg(unix)]
struct NativeProbeIo {
    stdin: Stdin,
    stdout: Stdout,
}

#[cfg(unix)]
impl ProbeIo for NativeProbeIo {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.stdout.write_all(bytes)?;
        self.stdout.flush()
    }

    fn receive(&mut self, timeout: Duration) -> io::Result<Vec<u8>> {
        use std::os::fd::AsRawFd;

        let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        let mut descriptor = libc::pollfd {
            fd: self.stdin.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut descriptor, 1, timeout_ms) };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        if result == 0 || descriptor.revents & libc::POLLIN == 0 {
            return Ok(Vec::new());
        }

        let mut bytes = vec![0_u8; 8 * 1024];
        let count = self.stdin.read(&mut bytes)?;
        bytes.truncate(count);
        Ok(bytes)
    }
}

/// The result of encoding one image frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameReport {
    pub encoded_bytes: usize,
    pub written_bytes: usize,
    pub encode_time: Duration,
}

/// A measured encoder cost over one or more frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodingMeasurement {
    pub iterations: usize,
    pub bytes_per_frame: usize,
    pub total_bytes: usize,
    pub total_time: Duration,
}

impl EncodingMeasurement {
    pub fn per_frame_time(self) -> Duration {
        if self.iterations == 0 {
            Duration::ZERO
        } else {
            self.total_time / self.iterations as u32
        }
    }
}

/// Stateful image placement. Drawing a new frame retires the old placement
/// first, and `resize`/`clear` explicitly retire the current view.
pub struct ImageSurface {
    protocol: GraphicsProtocol,
    geometry: TerminalGeometry,
    next_image_id: u32,
    active: Option<ActiveImage>,
    last_frame: Option<(RgbaImage, CellRect)>,
}

#[derive(Debug, Clone, Copy)]
struct ActiveImage {
    protocol: GraphicsProtocol,
    image_id: u32,
    rectangle: CellRect,
}

impl ImageSurface {
    pub fn new(protocol: GraphicsProtocol, geometry: TerminalGeometry) -> Self {
        Self {
            protocol,
            geometry,
            next_image_id: 1,
            active: None,
            last_frame: None,
        }
    }

    pub fn protocol(&self) -> GraphicsProtocol {
        self.protocol
    }

    pub fn geometry(&self) -> TerminalGeometry {
        self.geometry
    }

    pub fn set_protocol<W: Write>(
        &mut self,
        output: &mut W,
        protocol: GraphicsProtocol,
    ) -> io::Result<usize> {
        let cleared = self.clear(output)?;
        self.protocol = protocol;
        Ok(cleared)
    }

    pub fn resize<W: Write>(
        &mut self,
        output: &mut W,
        geometry: TerminalGeometry,
    ) -> io::Result<usize> {
        let cleared = self.clear(output)?;
        self.geometry = geometry;
        Ok(cleared)
    }

    pub fn clear<W: Write>(&mut self, output: &mut W) -> io::Result<usize> {
        self.last_frame = None;
        let Some(active) = self.active else {
            return Ok(0);
        };
        let bytes = match active.protocol {
            GraphicsProtocol::Kitty {
                direct_transmission: true,
            } => kitty_delete(active.image_id),
            GraphicsProtocol::Sixel | GraphicsProtocol::Iterm2 => sixel_clear(active.rectangle),
            GraphicsProtocol::Kitty {
                direct_transmission: false,
            }
            | GraphicsProtocol::None => Vec::new(),
        };
        output.write_all(&bytes)?;
        self.active = None;
        Ok(bytes.len())
    }

    pub fn draw<W: Write>(
        &mut self,
        output: &mut W,
        image: &RgbaImage,
        rectangle: CellRect,
    ) -> Result<FrameReport, SurfaceError> {
        if self
            .last_frame
            .as_ref()
            .is_some_and(|(previous, area)| previous == image && *area == rectangle)
        {
            return Ok(FrameReport {
                encoded_bytes: 0,
                written_bytes: 0,
                encode_time: Duration::ZERO,
            });
        }
        let clear_bytes = self.clear(output).map_err(SurfaceError::Io)?;
        let Some(rectangle) = rectangle.clipped(self.geometry) else {
            return Ok(FrameReport {
                encoded_bytes: 0,
                written_bytes: clear_bytes,
                encode_time: Duration::ZERO,
            });
        };

        let image_id = self.next_image_id;
        self.next_image_id = self.next_image_id.wrapping_add(1).max(1);
        let started = Instant::now();
        let bytes = match self.protocol {
            GraphicsProtocol::Kitty {
                direct_transmission: true,
            } => kitty_encode(image, rectangle, image_id),
            GraphicsProtocol::Iterm2 => encode_iterm2(image, rectangle)?,
            GraphicsProtocol::Sixel => {
                let mut output = cursor_position(rectangle);
                output.extend(encode_sixel(image));
                output
            }
            GraphicsProtocol::Kitty {
                direct_transmission: false,
            }
            | GraphicsProtocol::None => Vec::new(),
        };
        output.write_all(&bytes).map_err(SurfaceError::Io)?;
        let encoded_bytes = bytes.len();
        let can_draw = self.protocol.can_transmit_pixels() && encoded_bytes > 0;
        self.active = can_draw.then_some(ActiveImage {
            protocol: self.protocol,
            image_id,
            rectangle,
        });
        if can_draw {
            self.last_frame = Some((image.clone(), rectangle));
        }
        Ok(FrameReport {
            encoded_bytes,
            written_bytes: clear_bytes + encoded_bytes,
            encode_time: started.elapsed(),
        })
    }

    pub fn capture(
        &mut self,
        image: &RgbaImage,
        rectangle: CellRect,
    ) -> Result<Vec<u8>, SurfaceError> {
        let mut output = Vec::new();
        self.draw(&mut output, image, rectangle)?;
        Ok(output)
    }

    pub fn capture_clear(&mut self) -> Result<Vec<u8>, io::Error> {
        let mut output = Vec::new();
        self.clear(&mut output)?;
        Ok(output)
    }
}

#[derive(Debug)]
pub enum SurfaceError {
    Io(io::Error),
    Image(ImageError),
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "terminal image I/O failed: {error}"),
            Self::Image(error) => write!(formatter, "terminal image encoding failed: {error}"),
        }
    }
}

impl std::error::Error for SurfaceError {}

impl From<ImageError> for SurfaceError {
    fn from(error: ImageError) -> Self {
        Self::Image(error)
    }
}

impl From<io::Error> for SurfaceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// A writer useful for tests and callers that want to inspect exact bytes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CaptureWriter {
    bytes: Vec<u8>,
}

impl CaptureWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn clear(&mut self) {
        self.bytes.clear();
    }
}

impl Write for CaptureWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn encode_kitty(image: &RgbaImage, rectangle: CellRect, image_id: u32) -> Vec<u8> {
    kitty_encode(image, rectangle, image_id)
}

pub fn encode_sixel(image: &RgbaImage) -> Vec<u8> {
    encode_sixel_scaled(image, image.width(), image.height())
        .expect("a validated image can be encoded at its own dimensions")
}

/// Encode a full-colour PNG suitable for inspection or inline transmission.
pub fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, ImageError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, image.width(), image.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder
            .write_header()
            .map_err(|error| ImageError::PngEncoding(error.to_string()))?;
        writer
            .write_image_data(image.pixels())
            .map_err(|error| ImageError::PngEncoding(error.to_string()))?;
    }
    Ok(bytes)
}

/// Place an inline PNG at a cell rectangle without advancing the text cursor.
pub fn encode_iterm2(image: &RgbaImage, rectangle: CellRect) -> Result<Vec<u8>, ImageError> {
    let png = encode_png(image)?;
    let mut output = b"\x1b7".to_vec();
    output.extend(cursor_position(rectangle));
    output.extend(format!(
        "\x1b]1337;File=inline=1;size={};width={};height={};preserveAspectRatio=0;doNotMoveCursor=1:",
        png.len(), rectangle.width, rectangle.height,
    ).as_bytes());
    output.extend(base64(&png));
    output.extend_from_slice(b"\x07\x1b8");
    Ok(output)
}

pub fn encode_sixel_scaled(
    image: &RgbaImage,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, ImageError> {
    let image = image.resized_nearest(width, height)?;
    let (palette, indexed) = sixel_palette(&image);
    let width_usize = width as usize;
    let mut output = Vec::new();
    output.extend_from_slice(b"\x1bPq\"1;1;");
    append_decimal(&mut output, width);
    output.push(b';');
    append_decimal(&mut output, height);

    for (color, rgb) in palette.iter().enumerate() {
        output.push(b'#');
        append_decimal(&mut output, color as u32);
        output.extend_from_slice(b";2;");
        append_decimal(&mut output, u32::from(rgb[0]));
        output.push(b';');
        append_decimal(&mut output, u32::from(rgb[1]));
        output.push(b';');
        append_decimal(&mut output, u32::from(rgb[2]));
    }

    let mut masks = vec![0_u8; SIXEL_COLORS * width_usize];
    for band_start in (0..height).step_by(6) {
        masks.fill(0);
        let mut used_colors = [false; SIXEL_COLORS];
        for bit in 0..6 {
            let y = band_start + bit;
            if y >= height {
                break;
            }
            let row_start = y as usize * width_usize;
            for x in 0..width_usize {
                let color = indexed[row_start + x] as usize;
                masks[color * width_usize + x] |= 1 << bit;
                used_colors[color] = true;
            }
        }

        let mut wrote_color = false;
        for (color, used) in used_colors.iter().enumerate() {
            if !used {
                continue;
            }
            if wrote_color {
                output.push(b'$');
            }
            wrote_color = true;
            output.push(b'#');
            append_decimal(&mut output, color as u32);
            let start = color * width_usize;
            append_sixel_columns(&mut output, &masks[start..start + width_usize]);
        }
        if band_start + 6 < height {
            output.push(b'-');
        }
    }
    output.extend_from_slice(b"\x1b\\");
    Ok(output)
}

pub fn measure_encoding(
    protocol: GraphicsProtocol,
    image: &RgbaImage,
    rectangle: CellRect,
    _geometry: TerminalGeometry,
    iterations: usize,
) -> Result<EncodingMeasurement, ImageError> {
    if iterations == 0 {
        return Err(ImageError::InvalidMeasurementCount);
    }
    let started = Instant::now();
    let mut total_bytes: usize = 0;
    for iteration in 0..iterations {
        let bytes = match protocol {
            GraphicsProtocol::Kitty {
                direct_transmission: true,
            } => encode_kitty(image, rectangle, iteration as u32 + 1),
            GraphicsProtocol::Iterm2 => encode_iterm2(image, rectangle)?,
            GraphicsProtocol::Sixel => encode_sixel(image),
            GraphicsProtocol::Kitty {
                direct_transmission: false,
            }
            | GraphicsProtocol::None => {
                return Err(ImageError::UnsupportedProtocol);
            }
        };
        total_bytes = total_bytes.saturating_add(bytes.len());
        std::hint::black_box(bytes);
    }
    Ok(EncodingMeasurement {
        iterations,
        bytes_per_frame: total_bytes / iterations,
        total_bytes,
        total_time: started.elapsed(),
    })
}

fn kitty_encode(image: &RgbaImage, rectangle: CellRect, image_id: u32) -> Vec<u8> {
    let encoded = base64(image.pixels());
    let mut output = Vec::with_capacity(encoded.len() + 160);
    output.extend_from_slice(b"\x1b[");
    append_decimal(&mut output, u32::from(rectangle.y) + 1);
    output.push(b';');
    append_decimal(&mut output, u32::from(rectangle.x) + 1);
    output.push(b'H');

    let chunks = encoded.chunks(KITTY_CHUNK_BYTES).collect::<Vec<_>>();
    for (index, chunk) in chunks.iter().enumerate() {
        let more = index + 1 < chunks.len();
        output.extend_from_slice(b"\x1b_G");
        if index == 0 {
            output.extend_from_slice(b"a=T,f=32");
            output.extend_from_slice(b",s=");
            append_decimal(&mut output, image.width());
            output.extend_from_slice(b",v=");
            append_decimal(&mut output, image.height());
            output.extend_from_slice(b",i=");
            append_decimal(&mut output, image_id);
            output.extend_from_slice(b",c=");
            append_decimal(&mut output, u32::from(rectangle.width));
            output.extend_from_slice(b",r=");
            append_decimal(&mut output, u32::from(rectangle.height));
            output.extend_from_slice(b",m=");
        } else {
            output.extend_from_slice(b"m=");
        }
        output.push(if more { b'1' } else { b'0' });
        output.push(b';');
        output.extend_from_slice(chunk);
        output.extend_from_slice(b"\x1b\\");
    }
    output
}

fn kitty_delete(image_id: u32) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"\x1b_Ga=d,d=i,i=");
    append_decimal(&mut output, image_id);
    output.extend_from_slice(b"\x1b\\");
    output
}

fn cursor_position(rectangle: CellRect) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"\x1b[");
    append_decimal(&mut output, u32::from(rectangle.y) + 1);
    output.push(b';');
    append_decimal(&mut output, u32::from(rectangle.x) + 1);
    output.push(b'H');
    output
}

fn sixel_clear(rectangle: CellRect) -> Vec<u8> {
    let mut output = Vec::new();
    for row in rectangle.y..rectangle.y.saturating_add(rectangle.height) {
        output.extend_from_slice(b"\x1b[");
        append_decimal(&mut output, u32::from(row) + 1);
        output.push(b';');
        append_decimal(&mut output, u32::from(rectangle.x) + 1);
        output.push(b'H');
        output.extend_from_slice(b"\x1b[");
        append_decimal(&mut output, u32::from(rectangle.width));
        output.push(b'X');
    }
    output
}

fn append_sixel_columns(output: &mut Vec<u8>, masks: &[u8]) {
    let mut index = 0;
    while index < masks.len() {
        let mask = masks[index];
        let mut end = index + 1;
        while end < masks.len() && masks[end] == mask {
            end += 1;
        }
        let count = end - index;
        if count >= 4 {
            output.push(b'!');
            append_decimal(output, count as u32);
            output.push(b'?'.saturating_add(mask));
        } else {
            for _ in 0..count {
                output.push(b'?'.saturating_add(mask));
            }
        }
        index = end;
    }
}

fn sixel_palette(image: &RgbaImage) -> (Vec<[u8; 3]>, Vec<u8>) {
    // Office scenes usually fit in the register budget. Preserve their colours
    // to Sixel's percentage precision instead of always snapping to a cube.
    let mut palette = Vec::new();
    let mut lookup = std::collections::HashMap::new();
    let mut indices = Vec::with_capacity(image.pixels.len() / 4);
    for pixel in image.pixels.chunks_exact(4) {
        let rgb = [0, 1, 2].map(|channel| {
            ((u16::from(composite_channel(pixel[channel], pixel[3])) * 100 + 127) / 255) as u8
        });
        let index = if let Some(index) = lookup.get(&rgb) {
            *index
        } else {
            if palette.len() == SIXEL_COLORS {
                let cube = (0..SIXEL_COLORS)
                    .map(|color| {
                        [
                            (color / 36 * 20) as u8,
                            (color / 6 % 6 * 20) as u8,
                            (color % 6 * 20) as u8,
                        ]
                    })
                    .collect();
                return (cube, palette_indices(image));
            }
            let index = palette.len() as u8;
            palette.push(rgb);
            lookup.insert(rgb, index);
            index
        };
        indices.push(index);
    }
    (palette, indices)
}

fn palette_indices(image: &RgbaImage) -> Vec<u8> {
    image
        .pixels
        .chunks_exact(4)
        .map(|pixel| {
            let red = composite_channel(pixel[0], pixel[3]);
            let green = composite_channel(pixel[1], pixel[3]);
            let blue = composite_channel(pixel[2], pixel[3]);
            let red = (u16::from(red) * 5 + 127) / 255;
            let green = (u16::from(green) * 5 + 127) / 255;
            let blue = (u16::from(blue) * 5 + 127) / 255;
            (red * 36 + green * 6 + blue) as u8
        })
        .collect()
}

fn composite_channel(channel: u8, alpha: u8) -> u8 {
    ((u16::from(channel) * u16::from(alpha) + 127) / 255) as u8
}

fn append_decimal(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(value.to_string().as_bytes());
}

fn base64(bytes: &[u8]) -> Vec<u8> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[(first >> 2) as usize]);
        output.push(TABLE[((first & 0b11) << 4 | second >> 4) as usize]);
        output.push(if chunk.len() > 1 {
            TABLE[((second & 0b1111) << 2 | third >> 6) as usize]
        } else {
            b'='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(third & 0b11_1111) as usize]
        } else {
            b'='
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_sixel_palettes_keep_source_colours_at_percentage_precision() {
        let image = RgbaImage::new(2, 1, vec![17, 29, 43, 255, 20, 32, 46, 255]).unwrap();
        let (palette, indices) = sixel_palette(&image);
        assert_eq!(palette, vec![[7, 11, 17], [8, 13, 18]]);
        assert_eq!(indices, vec![0, 1]);
        let bytes = encode_sixel(&image);
        assert!(bytes
            .windows(b"#0;2;7;11;17".len())
            .any(|part| part == b"#0;2;7;11;17"));
        let noise = RgbaImage::new(
            256,
            1,
            (0..256)
                .flat_map(|n| [n as u8, (n * 13) as u8, (n * 31) as u8, 255])
                .collect(),
        )
        .unwrap();
        let (palette, indices) = sixel_palette(&noise);
        assert_eq!(palette.len(), SIXEL_COLORS);
        assert_eq!(indices.len(), 256);
        assert!(indices
            .iter()
            .all(|index| usize::from(*index) < palette.len()));
    }

    #[test]
    fn iterm2_png_preserves_rgba_and_places_in_cells() {
        let image = RgbaImage::new(2, 1, vec![17, 29, 43, 255, 101, 151, 201, 255]).unwrap();
        let png = encode_png(&image).unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(&png))
            .read_info()
            .unwrap();
        let mut decoded = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut decoded).unwrap();
        assert_eq!((info.width, info.height), (2, 1));
        assert_eq!(&decoded[..info.buffer_size()], image.pixels());
        let bytes = encode_iterm2(&image, CellRect::new(3, 4, 8, 6)).unwrap();
        assert!(bytes.starts_with(b"\x1b7\x1b[5;4H\x1b]1337;File=inline=1;"));
        assert!(bytes
            .windows(b"width=8;height=6;".len())
            .any(|window| window == b"width=8;height=6;"));
        assert!(bytes.ends_with(b"\x07\x1b8"));
        assert!(bytes
            .windows(base64(&png).len())
            .any(|window| window == base64(&png)));
    }

    #[test]
    fn protocol_preference_is_kitty_then_iterm2_then_sixel() {
        let mut response = b"\x1b[?1;2;4c\x1bP>|iTerm2 3.5\x1b\\".to_vec();
        assert_eq!(
            parse_capabilities(&response).graphics,
            GraphicsProtocol::Iterm2
        );
        response.extend_from_slice(b"\x1b_Gi=31;OK\x1b\\");
        assert_eq!(
            parse_capabilities(&response).graphics,
            GraphicsProtocol::Kitty {
                direct_transmission: true
            }
        );
        assert_eq!(
            parse_capabilities(b"\x1b[?1;2;4c").graphics,
            GraphicsProtocol::Sixel
        );
        assert_eq!(parse_capabilities(b"").graphics, GraphicsProtocol::None);
    }

    #[test]
    fn identical_frames_skip_transmission_and_resize_invalidates_cache() {
        for protocol in [
            GraphicsProtocol::Iterm2,
            GraphicsProtocol::Sixel,
            GraphicsProtocol::Kitty {
                direct_transmission: true,
            },
        ] {
            let mut surface = ImageSurface::new(protocol, TerminalGeometry::new(80, 24, None));
            let image = RgbaImage::solid(2, 2, [23, 47, 91, 255]).unwrap();
            let area = CellRect::new(0, 0, 2, 2);
            assert!(!surface.capture(&image, area).unwrap().is_empty());
            assert!(surface.capture(&image, area).unwrap().is_empty());
            let mut cleared = Vec::new();
            surface
                .resize(&mut cleared, TerminalGeometry::new(40, 12, None))
                .unwrap();
            assert!(!cleared.is_empty());
            assert!(!surface.capture(&image, area).unwrap().is_empty());
        }
    }

    struct MockProbe {
        response: Vec<u8>,
        writes: Vec<u8>,
    }

    impl ProbeIo for MockProbe {
        fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.writes.extend_from_slice(bytes);
            Ok(())
        }

        fn receive(&mut self, _: Duration) -> io::Result<Vec<u8>> {
            Ok(std::mem::take(&mut self.response))
        }
    }

    #[test]
    fn kitty_reply_and_cell_queries_are_parsed() {
        let response = b"\x1b_Gi=31;OK\x1b\\\x1b[?1;2;4c\x1b[6;16;8t\x1b[8;48;160t";
        let capabilities = parse_capabilities(response);
        assert_eq!(
            capabilities.graphics,
            GraphicsProtocol::Kitty {
                direct_transmission: true
            }
        );
        assert_eq!(capabilities.cell_size, Some(CellSize::new(8, 16)));
        assert_eq!(capabilities.terminal_cells, Some((160, 48)));
    }

    #[test]
    fn sixel_and_derived_cell_size_are_detected_without_kitty() {
        let response = b"\x1b[?1;2;4c\x1b[4;768;1280t\x1b[8;48;160t";
        let capabilities = parse_capabilities(response);
        assert_eq!(capabilities.graphics, GraphicsProtocol::Sixel);
        assert_eq!(capabilities.cell_size, Some(CellSize::new(8, 16)));
    }

    #[test]
    fn unsupported_probe_is_bounded_and_emits_all_queries_once() {
        let mut probe = MockProbe {
            response: Vec::new(),
            writes: Vec::new(),
        };
        let capabilities = CapabilityDetector::new(Duration::from_millis(1))
            .detect(&mut probe)
            .unwrap();
        assert_eq!(capabilities, Capabilities::none());
        assert!(probe.writes.starts_with(KITTY_DIRECT_QUERY));
        assert!(probe
            .writes
            .windows(SIXEL_QUERY.len())
            .any(|window| window == SIXEL_QUERY));
        assert!(probe.writes.ends_with(TERMINAL_VERSION_QUERY));
    }

    #[test]
    fn kitty_bytes_are_chunked_and_placed() {
        let image = RgbaImage::solid(64, 64, [20, 40, 60, 255]).unwrap();
        let bytes = encode_kitty(&image, CellRect::new(2, 3, 10, 8), 7);
        assert!(bytes.starts_with(b"\x1b[4;3H\x1b_Ga=T,f=32,s=64,v=64,i=7,c=10,r=8,m=1;"));
        assert!(bytes.ends_with(b"\x1b\\"));
        assert!(bytes.windows(4).any(|window| window == b"m=1;"));
        assert!(bytes.windows(4).any(|window| window == b"m=0;"));
    }

    #[test]
    fn sixel_bytes_have_palette_data_and_placement_surface_clears() {
        let image = RgbaImage::solid(4, 4, [255, 0, 0, 255]).unwrap();
        let geometry = TerminalGeometry::new(80, 24, Some(CellSize::new(8, 16)));
        let mut surface = ImageSurface::new(GraphicsProtocol::Sixel, geometry);
        let first = surface.capture(&image, CellRect::new(1, 2, 2, 2)).unwrap();
        assert!(first.starts_with(b"\x1b[3;2H\x1bPq"));
        assert!(first.ends_with(b"\x1b\\"));
        let second = surface.capture(&image, CellRect::new(4, 5, 2, 2)).unwrap();
        assert!(second.starts_with(b"\x1b[3;2H\x1b[2X"));
        assert!(second.contains(&b'\x1b'));
        let cleared = surface.capture_clear().unwrap();
        assert!(cleared.starts_with(b"\x1b[6;5H"));
    }

    #[test]
    fn resize_retires_old_kitty_image_and_clips_new_target() {
        let image = RgbaImage::solid(2, 2, [0, 0, 0, 255]).unwrap();
        let geometry = TerminalGeometry::new(10, 10, None);
        let mut surface = ImageSurface::new(
            GraphicsProtocol::Kitty {
                direct_transmission: true,
            },
            geometry,
        );
        let _ = surface.capture(&image, CellRect::new(8, 8, 4, 4)).unwrap();
        let mut output = CaptureWriter::new();
        surface
            .resize(&mut output, TerminalGeometry::new(4, 4, None))
            .unwrap();
        assert_eq!(output.bytes(), b"\x1b_Ga=d,d=i,i=1\x1b\\");
        let report = surface
            .draw(&mut output, &image, CellRect::new(3, 3, 4, 4))
            .unwrap();
        assert!(report.encoded_bytes > 0);
        assert!(output
            .bytes()
            .windows(6)
            .any(|window| window == b"[4;4H\x1b"));
    }

    #[test]
    fn measurement_rejects_zero_iterations() {
        let image = RgbaImage::solid(1, 1, [0, 0, 0, 255]).unwrap();
        let error = measure_encoding(
            GraphicsProtocol::Kitty {
                direct_transmission: true,
            },
            &image,
            CellRect::new(0, 0, 1, 1),
            TerminalGeometry::new(1, 1, None),
            0,
        )
        .unwrap_err();
        assert_eq!(error, ImageError::InvalidMeasurementCount);
    }

    #[test]
    fn base64_is_standard_without_line_breaks() {
        assert_eq!(base64(b"Man"), b"TWFu");
        assert_eq!(base64(b"Ma"), b"TWE=");
        assert_eq!(base64(b"M"), b"TQ==");
    }
}
