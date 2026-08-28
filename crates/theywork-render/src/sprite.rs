//! Editable pixel sprites and deterministic, time-driven animations.
//!
//! Sprites keep their source rows until a pixel is requested.  That makes the
//! declarations below easy to edit while ensuring a steady-state frame only
//! walks the already-parsed colour buffer.

use std::sync::{Arc, OnceLock};

use ratatui::style::Color;
use theywork_core::{Activity, Agent, Millis};

const WORKER_WIDTH: usize = 24;
const WORKER_HEIGHT: usize = 19;
const ACTIVITY_COUNT: usize = 9;

/// A compact sprite made from rows of palette keys.
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

    /// Select the frame for a timestamp.
    pub fn frame_at(&self, now: Millis) -> &Sprite {
        &self.frames[self.frame_index_at(now)]
    }
}

/// The nine visual activity poses used by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    fn index(self) -> usize {
        match self {
            Self::Typing => 0,
            Self::Reading => 1,
            Self::Editing => 2,
            Self::Searching => 3,
            Self::Thinking => 4,
            Self::Talking => 5,
            Self::Waiting => 6,
            Self::Idle => 7,
            Self::Error => 8,
        }
    }
}

/// All reusable art owned by one `Ui` instance.
#[derive(Clone)]
pub(crate) struct SpriteSet {
    codex: [Animation; ACTIVITY_COUNT],
    claude: [Animation; ACTIVITY_COUNT],
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
            codex: std::array::from_fn(|index| worker_animation(Agent::Codex, index)),
            claude: std::array::from_fn(|index| worker_animation(Agent::Claude, index)),
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

    pub(crate) fn worker_animation(&self, agent: Agent, activity: &Activity) -> &Animation {
        let animations = match agent {
            Agent::Codex => &self.codex,
            Agent::Claude => &self.claude,
        };
        &animations[ActivityKind::from_activity(activity).index()]
    }

    pub(crate) fn manager_animation(&self, needs_attention: bool) -> &Animation {
        if needs_attention {
            &self.manager_attention
        } else {
            &self.manager_walk
        }
    }
}

fn worker_animation(agent: Agent, kind_index: usize) -> Animation {
    let kind = [
        ActivityKind::Typing,
        ActivityKind::Reading,
        ActivityKind::Editing,
        ActivityKind::Searching,
        ActivityKind::Thinking,
        ActivityKind::Talking,
        ActivityKind::Waiting,
        ActivityKind::Idle,
        ActivityKind::Error,
    ][kind_index];
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
            .map(|frame| worker_frame(agent, kind, frame))
            .collect(),
        duration,
    )
}

fn worker_frame(agent: Agent, kind: ActivityKind, frame: usize) -> Sprite {
    let mut rows: Vec<String> = BASE_WORKER_ROWS
        .iter()
        .map(|row| (*row).to_string())
        .collect();
    if matches!(kind, ActivityKind::Thinking) && frame == 1 {
        // A single closed eye is more readable as a blink than a full-body
        // pose change at the small scale used by the office floor.
        replace_char(&mut rows[3], 11, '.');
    }
    if matches!(kind, ActivityKind::Typing | ActivityKind::Editing) {
        let hand = if frame == 0 { 'A' } else { 'B' };
        replace_char(&mut rows[9], 7, hand);
        replace_char(&mut rows[9], 14, if hand == 'A' { 'B' } else { 'A' });
    }

    let props = activity_props(kind, frame);
    let palette = worker_palette(agent);
    for (row, prop_row) in rows.iter_mut().zip(props) {
        for (column, key) in prop_row.chars().enumerate() {
            if key != '.' && row.chars().nth(column) == Some('.') {
                replace_char(row, column, key);
            }
        }
    }
    Sprite::from_owned_rows(rows, &palette)
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

fn worker_palette(agent: Agent) -> Vec<(char, Color)> {
    let shirt = match agent {
        Agent::Codex => (Color::Rgb(47, 133, 211), Color::Rgb(111, 202, 255)),
        Agent::Claude => (Color::Rgb(211, 83, 143), Color::Rgb(255, 151, 193)),
    };
    vec![
        ('H', Color::Rgb(58, 39, 52)),
        ('S', Color::Rgb(255, 193, 137)),
        ('E', Color::Rgb(50, 42, 62)),
        ('C', shirt.0),
        ('W', shirt.1),
        ('P', Color::Rgb(65, 69, 105)),
        ('A', Color::Rgb(255, 236, 170)),
        ('B', Color::Rgb(255, 218, 150)),
        ('M', Color::Rgb(39, 44, 76)),
        ('k', Color::Rgb(230, 181, 93)),
        ('D', Color::Rgb(255, 237, 184)),
        ('O', Color::Rgb(255, 213, 89)),
        ('L', Color::Rgb(255, 89, 103)),
        ('U', Color::Rgb(255, 236, 170)),
        ('!', Color::Rgb(255, 205, 113)),
        ('c', Color::Rgb(155, 103, 75)),
        ('u', Color::Rgb(242, 155, 92)),
        ('z', Color::Rgb(166, 189, 223)),
        ('Z', Color::Rgb(205, 218, 241)),
        ('W', shirt.1),
        ('│', Color::Rgb(65, 69, 105)),
        ('╱', Color::Rgb(207, 218, 230)),
        ('╲', Color::Rgb(207, 218, 230)),
        ('╰', Color::Rgb(255, 193, 137)),
        ('[', Color::Rgb(255, 237, 184)),
        (']', Color::Rgb(255, 237, 184)),
        ('h', Color::Rgb(255, 237, 184)),
        ('i', Color::Rgb(255, 237, 184)),
        ('O', Color::Rgb(255, 213, 89)),
        ('K', Color::Rgb(255, 237, 184)),
        ('^', Color::Rgb(191, 198, 217)),
        ('~', Color::Rgb(191, 198, 217)),
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
        ('!', Color::Rgb(255, 205, 113)),
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
            "...DDDDDDDDDD...",
            "..DMMMMMMMMMM D..",
            "..DMOOOOOOOOMD..",
            "..DMOOOOOOOOMD..",
            "..DMMMMMMMMMMD..",
            "......DDDD......",
            ".....DDDDDD.....",
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
                assert_eq!(sprites.worker_animation(agent, &activity).frame_count(), 2);
            }
        }
    }

    #[test]
    fn waiting_and_manager_attention_poses_have_clear_markers() {
        let sprites = SpriteSet::new();
        let waiting = Activity::Waiting {
            detail: "approve command".into(),
        };
        let waiting_frame = sprites.worker_animation(Agent::Codex, &waiting).frame_at(0);
        assert_eq!(waiting_frame.pixel(17, 0), Some(Color::Rgb(255, 205, 113)));

        let walk = sprites.manager_animation(false).frame_at(0);
        let attention = sprites.manager_animation(true).frame_at(0);
        assert_ne!(walk.pixels(), attention.pixels());
        assert_eq!(attention.pixel(11, 0), Some(Color::Rgb(255, 205, 113)));
    }
}
