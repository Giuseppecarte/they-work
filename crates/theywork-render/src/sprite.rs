//! Editable pixel sprites and deterministic, time-driven animations.
//!
//! Sprites keep their source rows until a pixel is requested.  That makes the
//! declarations below easy to edit while ensuring a steady-state frame only
//! walks the already-parsed colour buffer.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, OnceLock};

use ratatui::style::Color;
use theywork_core::{Activity, Agent, Millis, Worker, WorkerStatus};

const WORKER_WIDTH: usize = 24;
const WORKER_HEIGHT: usize = 19;

/// The six stable wardrobe slots used by every surface that draws a worker.
/// Values are deliberately small indexes rather than colors or glyphs so the
/// look can be rendered at any camera scale without changing its identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct WorkerLook {
    pub(crate) head: u8,
    pub(crate) face: u8,
    pub(crate) top: u8,
    pub(crate) desk_prop: u8,
    pub(crate) skin: u8,
    pub(crate) hair: u8,
    pub(crate) contractor: bool,
}

impl WorkerLook {
    pub(crate) fn silhouette(self) -> (u8, u8, u8, bool) {
        (self.head, self.hair, self.top, self.contractor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct WorkerRenderKey {
    agent: Agent,
    status: u8,
    activity: ActivityKind,
    look: WorkerLook,
}
/// Derive a worker's base wardrobe from only its stable thread id.
/// The hash is explicit so the result does not depend on Rust's randomized
/// hashers or on machine word size.
pub(crate) fn worker_look(worker: &Worker) -> WorkerLook {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in worker.id.0.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let mut next = || {
        hash ^= hash >> 30;
        hash = hash.wrapping_mul(0xbf58476d1ce4e5b9);
        hash ^= hash >> 27;
        hash = hash.wrapping_mul(0x94d049bb133111eb);
        hash ^= hash >> 31;
        hash
    };
    WorkerLook {
        head: (next() % 6) as u8,
        face: (next() % 5) as u8,
        top: (next() % 6) as u8,
        desk_prop: (next() % 6) as u8,
        skin: (next() % 6) as u8,
        hair: (next() % 6) as u8,
        contractor: is_contractor(worker),
    }
}
/// Resolve looks in office order, making silhouettes unique for the ten
/// desks that fit on a floor while preserving the id-derived base choice.
pub(crate) fn worker_looks(workers: &[Worker]) -> Vec<WorkerLook> {
    let mut used = BTreeSet::new();
    workers
        .iter()
        .map(|worker| {
            let base = worker_look(worker);
            let mut look = base;
            let mut attempt = 0u16;
            while used.contains(&look.silhouette()) {
                attempt = attempt.saturating_add(1);
                look.head = (base.head + attempt as u8) % 6;
                look.hair = (base.hair + attempt as u8 / 6) % 6;
                look.top = (base.top + attempt as u8 / 36) % 6;
                if attempt >= 216 {
                    break;
                }
            }
            used.insert(look.silhouette());
            look
        })
        .collect()
}

/// A compact sprite made from rows of palette keys.
pub(crate) fn look_for_worker(workers: &[Worker], worker: &Worker) -> WorkerLook {
    workers
        .iter()
        .position(|candidate| candidate.id == worker.id)
        .and_then(|index| worker_looks(workers).get(index).copied())
        .unwrap_or_else(|| worker_look(worker))
}

fn is_contractor(worker: &Worker) -> bool {
    let id = worker.id.0.to_ascii_lowercase();
    id.contains("contract")
}
#[derive(Clone)]
pub struct Sprite {
    rows: Arc<[String]>,
    palette: Arc<[(char, Color)]>,
    parsed: Arc<OnceLock<ParsedSprite>>,
}

impl Sprite {
    /// Store editable rows and palette entries. Parsing is deferred until the
    /// first dimension or pixel lookup.
    pub fn from_rows(rows: &[&str], palette: &[(char, Color)]) -> Self {
        Self::from_owned_rows(rows.iter().map(|row| (*row).to_string()).collect(), palette)
    }

    /// Parse a row-and-palette sprite lazily.
    pub fn parse(rows: &[&str], palette: &[(char, Color)]) -> Self {
        Self::from_rows(rows, palette)
    }

    /// Alias with a name that reads naturally at call sites.
    pub fn new(rows: &[&str], palette: &[(char, Color)]) -> Self {
        Self::from_rows(rows, palette)
    }

    /// Build a sprite from owned rows. This is used for the small composited
    /// worker poses where an activity prop is layered over the body.
    pub fn from_owned_rows(rows: Vec<String>, palette: &[(char, Color)]) -> Self {
        Self {
            rows: Arc::from(rows),
            palette: Arc::from(palette.to_vec()),
            parsed: Arc::new(OnceLock::new()),
        }
    }

    /// An entirely transparent sprite, useful as a safe empty animation frame.
    pub fn empty() -> Self {
        Self::from_owned_rows(Vec::new(), &[])
    }

    /// Width in pixels after parsing the source rows.
    pub fn width(&self) -> usize {
        self.parsed().width
    }

    /// Height in pixels after parsing the source rows.
    pub fn height(&self) -> usize {
        self.parsed().height
    }

    /// Read one pixel. `None` means transparent or outside the sprite.
    pub fn pixel(&self, x: usize, y: usize) -> Option<Color> {
        let parsed = self.parsed();
        parsed
            .pixels
            .get(y.checked_mul(parsed.width)?.checked_add(x)?)
            .copied()
            .flatten()
    }

    /// Return the parsed pixels for callers building custom compositions.
    pub fn pixels(&self) -> &[Option<Color>] {
        &self.parsed().pixels
    }

    fn parsed(&self) -> &ParsedSprite {
        let rows = &self.rows;
        let palette = &self.palette;
        self.parsed.get_or_init(|| parse_rows(rows, palette))
    }

    #[cfg(test)]
    fn is_parsed(&self) -> bool {
        self.parsed.get().is_some()
    }
}

#[derive(Debug)]
struct ParsedSprite {
    width: usize,
    height: usize,
    pixels: Vec<Option<Color>>,
}

fn parse_rows(rows: &[String], palette: &[(char, Color)]) -> ParsedSprite {
    let width = rows
        .iter()
        .map(|row| row.chars().count())
        .max()
        .unwrap_or(0);
    let height = rows.len();
    let mut pixels = Vec::with_capacity(width.saturating_mul(height));

    for row in rows {
        let mut row_pixels = row.chars().map(|key| palette_pixel(key, palette));
        for _ in 0..width {
            pixels.push(row_pixels.next().flatten().flatten());
        }
    }

    ParsedSprite {
        width,
        height,
        pixels,
    }
}

fn palette_pixel(key: char, palette: &[(char, Color)]) -> Option<Option<Color>> {
    if key == '.' {
        return Some(None);
    }
    palette
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, color)| Some(*color))
}

/// A deterministic animation. The selected frame depends only on `now` and
/// the configured frame duration, not on how many draw calls have happened.
#[derive(Clone)]
pub struct Animation {
    frames: Arc<[Sprite]>,
    frame_duration: Millis,
}

impl Animation {
    /// Construct an animation, using a transparent frame for an empty input.
    pub fn new(frames: Vec<Sprite>, frame_duration: Millis) -> Self {
        let frames = if frames.is_empty() {
            vec![Sprite::empty()]
        } else {
            frames
        };
        Self {
            frames: Arc::from(frames),
            frame_duration: frame_duration.max(1),
        }
    }

    /// Construct an animation from borrowed frame descriptions.
    pub fn from_frames(frames: &[Sprite], frame_duration: Millis) -> Self {
        Self::new(frames.to_vec(), frame_duration)
    }

    /// Number of frames in this animation.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Duration of one frame in milliseconds.
    pub fn frame_duration(&self) -> Millis {
        self.frame_duration
    }

    /// Exact frame index for a timestamp.
    pub fn frame_index_at(&self, now: Millis) -> usize {
        let elapsed = now.max(0) as u64;
        let duration = self.frame_duration as u64;
        ((elapsed / duration) as usize) % self.frames.len()
    }

    #[cfg(test)]
    fn parsed_count(&self) -> usize {
        self.frames
            .iter()
            .filter(|sprite| sprite.is_parsed())
            .count()
    }

    /// Select the frame for a timestamp.
    pub fn frame_at(&self, now: Millis) -> &Sprite {
        &self.frames[self.frame_index_at(now)]
    }
}

/// The nine visual activity poses used by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ActivityKind {
    Typing,
    Reading,
    Editing,
    Searching,
    Thinking,
    Talking,
    Waiting,
    Idle,
    Error,
}

impl ActivityKind {
    pub(crate) fn from_activity(activity: &Activity) -> Self {
        match activity {
            Activity::Typing { .. } => Self::Typing,
            Activity::Reading { .. } => Self::Reading,
            Activity::Editing { .. } => Self::Editing,
            Activity::Searching { .. } => Self::Searching,
            Activity::Thinking => Self::Thinking,
            Activity::Talking { .. } => Self::Talking,
            Activity::Waiting { .. } => Self::Waiting,
            Activity::Idle => Self::Idle,
            Activity::Error { .. } => Self::Error,
        }
    }
}

/// All reusable art owned by one `Ui` instance.
#[derive(Clone)]
pub(crate) struct SpriteSet {
    wardrobe_cache: RefCell<HashMap<WorkerRenderKey, Animation>>,
    animation_time: Cell<Option<Millis>>,
    pub(crate) manager_walk: Animation,
    pub(crate) manager_attention: Animation,
    pub(crate) desk: Sprite,
    pub(crate) monitor: Sprite,
    pub(crate) plant: Sprite,
    pub(crate) water_cooler: Sprite,
    pub(crate) floor_tile: Sprite,
    pub(crate) wall_tile: Sprite,
}

impl SpriteSet {
    pub(crate) fn new() -> Self {
        Self {
            wardrobe_cache: RefCell::new(HashMap::new()),
            animation_time: Cell::new(None),
            manager_walk: manager_animation(false),
            manager_attention: manager_animation(true),
            desk: desk(),
            monitor: monitor(),
            plant: plant(),
            water_cooler: water_cooler(),
            floor_tile: floor_tile(),
            wall_tile: wall_tile(),
        }
    }

    pub(crate) fn manager_animation(&self, needs_attention: bool) -> &Animation {
        if needs_attention {
            &self.manager_attention
        } else {
            &self.manager_walk
        }
    }

    pub(crate) fn set_animation_time(&self, now: Option<Millis>) {
        self.animation_time.set(now);
    }

    pub(crate) fn worker_frame(&self, worker: &Worker, look: WorkerLook, now: Millis) -> Sprite {
        let activity = ActivityKind::from_activity(&worker.activity);
        let status = worker.status_at(now);
        let key = WorkerRenderKey {
            agent: worker.agent,
            status: status_index(status),
            activity,
            look,
        };
        let mut cache = self.wardrobe_cache.borrow_mut();
        let animation_now = self.animation_time.get().unwrap_or(now);
        cache
            .entry(key)
            .or_insert_with(|| worker_animation_for_look(worker.agent, look, activity, status))
            .frame_at(animation_now)
            .clone()
    }

    #[cfg(test)]
    pub(crate) fn parse_all_cached_frames(&self) {
        for animation in [&self.manager_walk, &self.manager_attention] {
            for frame in animation.frames.iter() {
                let _ = frame.pixels();
            }
        }
        for sprite in [
            &self.desk,
            &self.monitor,
            &self.plant,
            &self.water_cooler,
            &self.floor_tile,
            &self.wall_tile,
        ] {
            let _ = sprite.pixels();
        }
        for animation in self.wardrobe_cache.borrow().values() {
            for frame in animation.frames.iter() {
                let _ = frame.pixels();
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn parsed_count(&self) -> usize {
        self.manager_walk.parsed_count()
            + self.manager_attention.parsed_count()
            + usize::from(self.desk.is_parsed())
            + usize::from(self.monitor.is_parsed())
            + usize::from(self.plant.is_parsed())
            + usize::from(self.water_cooler.is_parsed())
            + usize::from(self.floor_tile.is_parsed())
            + usize::from(self.wall_tile.is_parsed())
            + self
                .wardrobe_cache
                .borrow()
                .values()
                .map(Animation::parsed_count)
                .sum::<usize>()
    }
}

fn status_index(status: WorkerStatus) -> u8 {
    match status {
        WorkerStatus::Running => 0,
        WorkerStatus::Idle => 1,
        WorkerStatus::Blocked => 2,
        WorkerStatus::Failed => 3,
    }
}

fn replace_char(row: &mut String, index: usize, replacement: char) {
    let mut chars: Vec<char> = row.chars().collect();
    if let Some(slot) = chars.get_mut(index) {
        *slot = replacement;
        *row = chars.into_iter().collect();
    }
}

fn activity_props(kind: ActivityKind, frame: usize) -> Vec<String> {
    let mut rows = vec![".".repeat(WORKER_WIDTH); WORKER_HEIGHT];
    let put = |rows: &mut [String], row: usize, column: usize, text: &str| {
        if let Some(target) = rows.get_mut(row) {
            for (offset, key) in text.chars().enumerate() {
                replace_char(target, column + offset, key);
            }
        }
    };

    match kind {
        ActivityKind::Typing => {
            put(&mut rows, 6, 18, if frame == 0 { "MM" } else { "MO" });
            put(&mut rows, 7, 18, if frame == 0 { "MM" } else { "OM" });
            put(&mut rows, 8, 19, "│");
            put(&mut rows, 9, 18, "KK");
            put(&mut rows, 10, 2 + frame, "kkkk");
        }
        ActivityKind::Reading => {
            put(&mut rows, 5, 1 + frame, "[DD]");
            put(&mut rows, 6, 2 + frame, "[DD]");
        }
        ActivityKind::Editing => {
            put(&mut rows, 5, 18, if frame == 0 { "╱" } else { "╲" });
            put(&mut rows, 6, 17, "W");
            put(&mut rows, 7, 18, "W");
            put(&mut rows, 8, 19, "W");
        }
        ActivityKind::Searching => {
            put(&mut rows, 7, 1, if frame == 0 { "OO" } else { " O" });
            put(&mut rows, 8, 1, "O╲");
            put(&mut rows, 9, 3, "╲");
        }
        ActivityKind::Thinking => {
            put(&mut rows, 1, 17, if frame == 0 { "(o)" } else { "( )" });
            put(&mut rows, 2, 19, "·");
        }
        ActivityKind::Talking => {
            put(&mut rows, 1, 15, if frame == 0 { "[hi]" } else { "[OK]" });
            put(&mut rows, 2, 17, "╰");
        }
        ActivityKind::Waiting => {
            put(&mut rows, 0, 17, "!");
            put(&mut rows, 1, 17, "!");
            put(&mut rows, 2, 17, "!");
            put(&mut rows, 4 + frame.min(1), 2, "U");
            put(&mut rows, 5 + frame.min(1), 1, "U");
            put(&mut rows, 6 + frame.min(1), 2, "U");
        }
        ActivityKind::Idle => {
            put(&mut rows, 1, 17, if frame == 0 { "z" } else { "Z" });
            put(&mut rows, 2, 19, "z");
            put(&mut rows, 10, 18, "c");
            put(&mut rows, 11, 18, "u");
        }
        ActivityKind::Error => {
            put(&mut rows, 2, 18, if frame == 0 { "^^" } else { "~~" });
            put(&mut rows, 9, 18, "L");
            put(&mut rows, 10, 18, "L");
        }
    }
    rows
}

fn worker_animation_for_look(
    agent: Agent,
    look: WorkerLook,
    kind: ActivityKind,
    status: WorkerStatus,
) -> Animation {
    let duration = match kind {
        ActivityKind::Typing => 260,
        ActivityKind::Reading => 620,
        ActivityKind::Editing => 300,
        ActivityKind::Searching => 480,
        ActivityKind::Thinking => 900,
        ActivityKind::Talking => 700,
        ActivityKind::Waiting => 450,
        ActivityKind::Idle => 1_000,
        ActivityKind::Error => 520,
    };
    Animation::new(
        (0..2)
            .map(|frame| wardrobe_frame(agent, look, kind, status, frame))
            .collect(),
        duration,
    )
}
fn wardrobe_frame(
    agent: Agent,
    look: WorkerLook,
    kind: ActivityKind,
    status: WorkerStatus,
    frame: usize,
) -> Sprite {
    let mut rows: Vec<String> = BASE_WORKER_ROWS
        .iter()
        .map(|row| (*row).to_string())
        .collect();
    let hair_key = ['H', 'J', 'N', 'R', 'T', 'V'][look.hair as usize];
    let skin_key = ['S', 'a', 'd', 'n', 'r', 'y'][look.skin as usize];
    let face_key = ['E', 'e', 'X', 'Y', 'Z'][look.face as usize];
    replace_key(&mut rows, 'H', hair_key);
    replace_key(&mut rows, 'S', skin_key);
    replace_key(&mut rows, 'E', face_key);
    let (top_base, top_light) = if look.contractor {
        ('G', 'g')
    } else {
        ('C', 'W')
    };
    replace_key(&mut rows, 'C', top_base);
    replace_key(&mut rows, 'W', top_light);
    apply_hair_shape(&mut rows, hair_key, look.hair);
    apply_headwear(&mut rows, hair_key, look.head);
    apply_top_pattern(&mut rows, top_base, look.top);
    if matches!(kind, ActivityKind::Thinking) && frame == 1 {
        replace_char(&mut rows[3], 11, '.');
    }
    if matches!(kind, ActivityKind::Typing | ActivityKind::Editing) {
        let hand = if frame == 0 { 'A' } else { 'B' };
        replace_char(&mut rows[9], 7, hand);
        replace_char(&mut rows[9], 14, if hand == 'A' { 'B' } else { 'A' });
    }
    let props = activity_props(kind, frame);
    for (row, prop_row) in rows.iter_mut().zip(props) {
        for (column, key) in prop_row.chars().enumerate() {
            if key != '.' && row.chars().nth(column) == Some('.') {
                replace_char(row, column, key);
            }
        }
    }
    let prop_key = ['1', '2', '3', '4', '5', '6'][look.desk_prop as usize];
    replace_char(&mut rows[14], 2 + usize::from(look.desk_prop % 4), prop_key);
    if look.contractor {
        replace_char(&mut rows[0], 17, 'b');
        replace_char(&mut rows[1], 17, 'b');
    }
    let palette = wardrobe_palette(agent, status);
    Sprite::from_owned_rows(rows, &palette)
}
fn replace_key(rows: &mut [String], from: char, to: char) {
    for row in rows {
        let mut chars: Vec<char> = row.chars().collect();
        for slot in &mut chars {
            if *slot == from {
                *slot = to;
            }
        }
        *row = chars.into_iter().collect();
    }
}
fn apply_hair_shape(rows: &mut [String], key: char, variant: u8) {
    match variant {
        0 => {}
        1 => {
            replace_char(&mut rows[0], 8, '.');
            replace_char(&mut rows[0], 15, '.');
        }
        2 => {
            replace_char(&mut rows[1], 7, key);
            replace_char(&mut rows[1], 16, key);
        }
        3 => {
            replace_char(&mut rows[2], 7, key);
            replace_char(&mut rows[2], 16, key);
        }
        4 => {
            replace_char(&mut rows[4], 8, key);
            replace_char(&mut rows[4], 15, key);
        }
        _ => {
            replace_char(&mut rows[3], 8, key);
            replace_char(&mut rows[3], 15, key);
        }
    }
}
fn apply_headwear(rows: &mut [String], key: char, variant: u8) {
    match variant {
        0 => {}
        1 => {
            replace_char(&mut rows[0], 10, key);
            replace_char(&mut rows[0], 13, key);
        }
        2 => {
            replace_char(&mut rows[0], 8, key);
            replace_char(&mut rows[0], 15, key);
        }
        3 => {
            replace_char(&mut rows[0], 9, key);
            replace_char(&mut rows[1], 8, key);
        }
        4 => {
            replace_char(&mut rows[0], 14, key);
            replace_char(&mut rows[1], 15, key);
        }
        _ => {
            replace_char(&mut rows[1], 7, key);
            replace_char(&mut rows[1], 16, key);
        }
    }
}
fn apply_top_pattern(rows: &mut [String], base: char, variant: u8) {
    for column in 9..15 {
        if (column + usize::from(variant)) % 3 == 0 {
            replace_char(&mut rows[8], column, base);
        }
    }
    if variant % 2 == 1 {
        replace_char(&mut rows[9], 10, base);
        replace_char(&mut rows[9], 13, base);
    }
}
const BASE_WORKER_ROWS: &[&str] = &[
    ".........HHHHHH.........",
    "........HHHHHHHH........",
    "........HSSSSSSH........",
    "........HSEEEESH........",
    ".........SSSSSS.........",
    "..........SSSS..........",
    ".........CCCCCC.........",
    "........CCCCCCCC........",
    "........CWWWWWWC........",
    "........CWWWWWWC........",
    ".........CCCCCC.........",
    "..........PPPP..........",
    ".........PPPPPP.........",
    ".........PPPPPP.........",
    "..........PPPP..........",
    "........................",
    "........................",
    "........................",
    "........................",
];

fn wardrobe_palette(agent: Agent, status: WorkerStatus) -> Vec<(char, Color)> {
    let (shirt, shade) = match agent {
        Agent::Codex => (Color::Rgb(79, 158, 232), Color::Rgb(58, 124, 189)),
        Agent::Claude => (Color::Rgb(232, 131, 74), Color::Rgb(201, 106, 55)),
    };
    let marker = if status == WorkerStatus::Blocked {
        Color::Rgb(240, 180, 41)
    } else {
        Color::Rgb(232, 226, 214)
    };
    vec![
        ('H', Color::Rgb(43, 37, 66)),
        ('J', Color::Rgb(58, 51, 88)),
        ('N', Color::Rgb(84, 51, 31)),
        ('R', Color::Rgb(107, 68, 41)),
        ('T', Color::Rgb(138, 90, 56)),
        ('V', Color::Rgb(138, 130, 153)),
        ('S', Color::Rgb(242, 192, 154)),
        ('a', Color::Rgb(217, 157, 120)),
        ('d', Color::Rgb(185, 120, 88)),
        ('n', Color::Rgb(155, 98, 77)),
        ('r', Color::Rgb(125, 73, 61)),
        ('y', Color::Rgb(240, 207, 173)),
        ('E', Color::Rgb(42, 36, 64)),
        ('e', Color::Rgb(58, 53, 44)),
        ('X', Color::Rgb(13, 11, 20)),
        ('Y', Color::Rgb(138, 130, 153)),
        ('Z', Color::Rgb(232, 226, 214)),
        ('C', shirt),
        ('W', shade),
        ('G', Color::Rgb(138, 130, 153)),
        ('g', Color::Rgb(232, 226, 214)),
        ('P', Color::Rgb(42, 36, 64)),
        ('A', Color::Rgb(244, 239, 228)),
        ('B', Color::Rgb(232, 226, 214)),
        ('M', Color::Rgb(88, 214, 232)),
        ('k', Color::Rgb(138, 90, 56)),
        ('D', Color::Rgb(232, 226, 214)),
        ('O', Color::Rgb(90, 169, 201)),
        ('L', Color::Rgb(232, 52, 44)),
        ('U', Color::Rgb(232, 226, 214)),
        ('!', marker),
        ('c', Color::Rgb(84, 51, 31)),
        ('u', Color::Rgb(138, 90, 56)),
        ('z', Color::Rgb(138, 130, 153)),
        ('│', Color::Rgb(42, 36, 64)),
        ('╱', Color::Rgb(232, 226, 214)),
        ('╲', Color::Rgb(232, 226, 214)),
        ('╰', Color::Rgb(242, 192, 154)),
        ('[', Color::Rgb(232, 226, 214)),
        (']', Color::Rgb(232, 226, 214)),
        ('h', Color::Rgb(232, 226, 214)),
        ('i', Color::Rgb(232, 226, 214)),
        ('K', Color::Rgb(232, 226, 214)),
        ('^', Color::Rgb(138, 130, 153)),
        ('~', Color::Rgb(138, 130, 153)),
        ('(', Color::Rgb(138, 130, 153)),
        (')', Color::Rgb(138, 130, 153)),
        ('o', Color::Rgb(232, 226, 214)),
        ('1', Color::Rgb(88, 214, 232)),
        ('2', shirt),
        ('3', shade),
        ('4', Color::Rgb(138, 90, 56)),
        ('5', Color::Rgb(90, 169, 201)),
        ('6', Color::Rgb(232, 226, 214)),
        ('b', Color::Rgb(232, 226, 214)),
    ]
}
fn static_palette() -> Vec<(char, Color)> {
    vec![
        ('D', Color::Rgb(47, 38, 59)),
        ('H', Color::Rgb(58, 39, 52)),
        ('S', Color::Rgb(255, 193, 137)),
        ('E', Color::Rgb(50, 42, 62)),
        ('Y', Color::Rgb(224, 166, 63)),
        ('Q', Color::Rgb(65, 69, 105)),
        ('!', Color::Rgb(240, 180, 41)),
        ('W', Color::Rgb(186, 119, 74)),
        ('L', Color::Rgb(91, 186, 126)),
        ('l', Color::Rgb(133, 221, 148)),
        ('P', Color::Rgb(222, 132, 100)),
        ('M', Color::Rgb(42, 47, 77)),
        ('O', Color::Rgb(100, 211, 224)),
        ('F', Color::Rgb(188, 145, 93)),
        ('f', Color::Rgb(224, 181, 115)),
        ('B', Color::Rgb(91, 82, 112)),
        ('b', Color::Rgb(117, 103, 139)),
        ('C', Color::Rgb(108, 185, 213)),
        ('c', Color::Rgb(210, 218, 230)),
    ]
}

fn make_desk_sprite() -> Sprite {
    cached_sprite(
        "desk",
        &[
            "........................",
            "....DDDDDDDDDDDDDDDD....",
            "...DWWWWWWWWWWWWWWWWD...",
            "...DWWWWWWWWWWWWWWWWD...",
            "....DDDDDDDDDDDDDDDD....",
            "...........D............",
            "..........DD............",
            ".........DDD............",
            "........................",
        ],
    )
}

fn make_monitor_sprite() -> Sprite {
    cached_sprite(
        "monitor",
        &[
            "....DDDDD....",
            "...DMMMMMD...",
            "...DMOOOMD...",
            "...DMOOOMD...",
            "...DMOOOMD...",
            "...DMOOOMD...",
            "...DMMMMMD...",
            ".....DDD.....",
            "....DDDDD....",
            ".....DDD.....",
        ],
    )
}

fn make_plant_sprite() -> Sprite {
    cached_sprite(
        "plant",
        &[
            "....L.....",
            "...LLL....",
            "..LL.LL...",
            ".LLL.LLL..",
            "...LLLL...",
            "....L.....",
            "....L.....",
            "...DDD....",
            "..DWWWD...",
            "..DWWWD...",
            "...DDD....",
            "..........",
        ],
    )
}

fn make_water_cooler_sprite() -> Sprite {
    cached_sprite(
        "cooler",
        &[
            "...DDDD...",
            "..DCCCCD..",
            "..DCCCCD..",
            "...DDDD...",
            "....DD....",
            "...DDDD...",
            "..DMMMMD..",
            "..DMOO MD..",
            "..DMMMMD..",
            "...DDDD...",
            "..DWWWWD..",
            "..DWWWWD..",
            "...DDDD...",
        ],
    )
}

fn make_floor_tile() -> Sprite {
    cached_sprite("floor", &["FFFFFFFF", "Ff..f..F", "F..f..fF", "FFFFFFFF"])
}

fn make_wall_tile() -> Sprite {
    cached_sprite(
        "wall",
        &[
            "BBBBBBBBBBBB",
            "BbbBbbBbbBbb",
            "BbbbbbbbbbbB",
            "BBBBBBBBBBBB",
        ],
    )
}

fn cached_sprite(_name: &'static str, rows: &[&str]) -> Sprite {
    // The caller owns the `OnceLock`; keeping this helper small makes each
    // declaration readable. The source rows themselves are cheap to retain,
    // while Sprite still defers palette expansion until first use.
    let palette = static_palette();
    Sprite::from_rows(rows, &palette)
}

/// Shared floor tile for custom renderers.
pub fn floor_tile() -> Sprite {
    static SPRITE: OnceLock<Sprite> = OnceLock::new();
    SPRITE.get_or_init(make_floor_tile).clone()
}

/// Shared wall tile for custom renderers.
pub fn wall_tile() -> Sprite {
    static SPRITE: OnceLock<Sprite> = OnceLock::new();
    SPRITE.get_or_init(make_wall_tile).clone()
}

/// Shared desk sprite for custom renderers.
pub fn desk() -> Sprite {
    static SPRITE: OnceLock<Sprite> = OnceLock::new();
    SPRITE.get_or_init(make_desk_sprite).clone()
}

/// Shared monitor sprite for custom renderers.
pub fn monitor() -> Sprite {
    static SPRITE: OnceLock<Sprite> = OnceLock::new();
    SPRITE.get_or_init(make_monitor_sprite).clone()
}

/// Shared office plant sprite for custom renderers.
pub fn plant() -> Sprite {
    static SPRITE: OnceLock<Sprite> = OnceLock::new();
    SPRITE.get_or_init(make_plant_sprite).clone()
}

/// Shared water-cooler sprite for custom renderers.
pub fn water_cooler() -> Sprite {
    static SPRITE: OnceLock<Sprite> = OnceLock::new();
    SPRITE.get_or_init(make_water_cooler_sprite).clone()
}

fn manager_animation(needs_attention: bool) -> Animation {
    let frame_duration = if needs_attention { 900 } else { 260 };
    Animation::new(
        (0..2)
            .map(|frame| manager_frame(needs_attention, frame))
            .collect(),
        frame_duration,
    )
}

fn manager_frame(needs_attention: bool, frame: usize) -> Sprite {
    let mut rows: Vec<String> = BASE_MANAGER_ROWS
        .iter()
        .map(|row| (*row).to_string())
        .collect();
    if frame == 1 {
        replace_char(&mut rows[15], 5, 'P');
        replace_char(&mut rows[15], 10, 'P');
    }
    if needs_attention {
        replace_char(&mut rows[0], 11, '!');
        replace_char(&mut rows[6], 11, 'D');
        replace_char(&mut rows[7], 11, 'D');
    }

    Sprite::from_owned_rows(rows, &static_palette())
}

const BASE_MANAGER_ROWS: &[&str] = &[
    "......HHHH......",
    ".....HSSSSH.....",
    ".....HSEESH.....",
    "......SSSS......",
    "......YYYY......",
    ".....YYYYYY.....",
    "....YYYYYYYY....",
    "....YYYYYYYY....",
    ".....YYYYYY.....",
    "......YQQY......",
    ".....YQQQQY.....",
    ".....QQQQQQ.....",
    "......QQQQ......",
    "................",
    ".....PP..PP.....",
    "....PP....PP....",
    "................",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_parse_with_dot_as_transparent_and_unknown_keys_as_transparent() {
        let sprite = Sprite::from_rows(&["A.", "?A"], &[('A', Color::Red)]);
        assert_eq!(sprite.width(), 2);
        assert_eq!(sprite.height(), 2);
        assert_eq!(sprite.pixel(0, 0), Some(Color::Red));
        assert_eq!(sprite.pixel(1, 0), None);
        assert_eq!(sprite.pixel(0, 1), None);
        assert_eq!(sprite.pixel(1, 1), Some(Color::Red));
    }

    #[test]
    fn animation_frame_selection_is_deterministic() {
        let first = Sprite::from_rows(&["A"], &[('A', Color::Red)]);
        let second = Sprite::from_rows(&["B"], &[('B', Color::Blue)]);
        let animation = Animation::new(vec![first, second], 100);
        assert_eq!(animation.frame_index_at(0), 0);
        assert_eq!(animation.frame_index_at(99), 0);
        assert_eq!(animation.frame_index_at(100), 1);
        assert_eq!(animation.frame_index_at(200), 0);
        assert_eq!(animation.frame_index_at(-1), 0);
    }

    #[test]
    fn every_activity_has_two_frames_for_each_agent() {
        let sprites = SpriteSet::new();
        for agent in [Agent::Codex, Agent::Claude] {
            for activity in [
                Activity::Typing {
                    detail: String::new(),
                },
                Activity::Reading {
                    detail: String::new(),
                },
                Activity::Editing {
                    detail: String::new(),
                },
                Activity::Searching {
                    detail: String::new(),
                },
                Activity::Thinking,
                Activity::Talking {
                    detail: String::new(),
                },
                Activity::Waiting {
                    detail: String::new(),
                },
                Activity::Idle,
                Activity::Error {
                    detail: String::new(),
                },
            ] {
                let mut worker = test_worker(agent);
                worker.activity = activity;
                let look = worker_look(&worker);
                assert_eq!(
                    sprites.worker_frame(&worker, look, 0).height(),
                    WORKER_HEIGHT
                );
            }
        }
    }

    #[test]
    fn waiting_and_manager_attention_poses_have_clear_markers() {
        let sprites = SpriteSet::new();
        let mut waiting = test_worker(Agent::Codex);
        waiting.activity = Activity::Waiting {
            detail: "approve command".into(),
        };
        waiting.turn_in_flight = true;
        waiting.last_seen = 0;
        let look = worker_look(&waiting);
        let waiting_frame =
            sprites.worker_frame(&waiting, look, theywork_core::BLOCKED_AFTER_MS + 1);
        assert_eq!(waiting_frame.pixel(17, 0), Some(Color::Rgb(240, 180, 41)));

        let walk = sprites.manager_animation(false).frame_at(0);
        let attention = sprites.manager_animation(true).frame_at(0);
        assert_ne!(walk.pixels(), attention.pixels());
        assert_eq!(attention.pixel(11, 0), Some(Color::Rgb(240, 180, 41)));
    }

    #[test]
    fn wardrobe_is_stable_by_id_and_unique_at_guard_scale() {
        let mut worker = test_worker(Agent::Claude);
        let initial = worker_look(&worker);
        worker.name = "Renamed worker".into();
        assert_eq!(worker_look(&worker), initial);
        let workers = (0..10)
            .map(|index| {
                let mut worker = test_worker(if index % 2 == 0 {
                    Agent::Codex
                } else {
                    Agent::Claude
                });
                worker.id = theywork_core::WorkerId(format!("worker-{index}"));
                worker
            })
            .collect::<Vec<_>>();
        let looks = worker_looks(&workers);
        let silhouettes = looks
            .iter()
            .map(|look| look.silhouette())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(silhouettes.len(), looks.len());
        assert!(looks.iter().all(|look| look.head < 6
            && look.face < 5
            && look.top < 6
            && look.desk_prop < 6
            && look.skin < 6
            && look.hair < 6));
    }
    #[test]
    fn wardrobe_tops_use_agent_hues_and_amber_only_for_blocked_markers() {
        let look = worker_look(&test_worker(Agent::Claude));
        let frame = |agent, contractor, status| {
            wardrobe_frame(
                agent,
                WorkerLook { contractor, ..look },
                ActivityKind::Waiting,
                status,
                0,
            )
        };
        assert_eq!(
            frame(Agent::Claude, false, WorkerStatus::Idle).pixel(9, 6),
            Some(Color::Rgb(232, 131, 74))
        );
        assert_eq!(
            frame(Agent::Codex, false, WorkerStatus::Idle).pixel(9, 6),
            Some(Color::Rgb(79, 158, 232))
        );
        assert_eq!(
            frame(Agent::Codex, true, WorkerStatus::Idle).pixel(9, 6),
            Some(Color::Rgb(138, 130, 153))
        );
        assert_eq!(
            frame(Agent::Codex, false, WorkerStatus::Idle).pixel(17, 0),
            Some(Color::Rgb(232, 226, 214))
        );
        assert_eq!(
            frame(Agent::Codex, false, WorkerStatus::Blocked).pixel(17, 0),
            Some(Color::Rgb(240, 180, 41))
        );
    }
    #[test]
    fn motion_setting_freezes_worker_animation_frames() {
        let sprites = SpriteSet::new();
        let mut worker = test_worker(Agent::Codex);
        worker.activity = Activity::Typing {
            detail: "cargo test".into(),
        };
        let look = worker_look(&worker);
        let first = sprites.worker_frame(&worker, look, 0);
        let later = sprites.worker_frame(&worker, look, 260);
        assert_ne!(first.pixels(), later.pixels());
        sprites.set_animation_time(Some(0));
        let frozen = sprites.worker_frame(&worker, look, 260);
        assert_eq!(first.pixels(), frozen.pixels());
    }
    fn test_worker(agent: Agent) -> Worker {
        Worker::new(
            theywork_core::WorkerId("sprite-test".into()),
            theywork_core::OfficeId("sprite-office".into()),
            agent,
            "Sprite test".into(),
            0,
        )
    }
}
