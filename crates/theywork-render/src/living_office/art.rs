//! Original 48×64 character art. Every costume changes its outline; none is a
//! resized legacy worker. Costumes and props are decorative, never job claims.

use ratatui::style::Color;

use crate::sprite::Sprite;

pub const WIDTH: usize = 48;
pub const HEIGHT: usize = 64;
pub const COSTUMES: [&str; 12] = [
    "Headphones",
    "Knitwear",
    "Overshirt",
    "Denim",
    "Hoodie",
    "Striped tee",
    "Cardigan",
    "Blazer",
    "Oxford shirt",
    "Athleisure",
    "Turtleneck",
    "Crewneck",
];

// Shared clothing colors keep both independently authored sprite sizes coherent.
pub(super) const CLOTHING: [Color; 12] = [
    rgb(79, 103, 135),
    rgb(219, 209, 189),
    rgb(162, 143, 118),
    rgb(109, 139, 163),
    rgb(178, 183, 180),
    rgb(198, 185, 168),
    rgb(147, 135, 147),
    rgb(65, 73, 87),
    rgb(195, 210, 216),
    rgb(91, 128, 119),
    rgb(64, 66, 70),
    rgb(143, 155, 128),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Character {
    pub costume: u8,
    pub skin: u8,
    pub hair: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Facing {
    Front,
    Left,
    Right,
}

pub(super) fn face(sprite: Sprite, facing: Facing) -> Sprite {
    if facing != Facing::Left {
        return sprite;
    }
    let (w, h) = (sprite.width(), sprite.height());
    let pixels = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| sprite.pixel(w - x - 1, y))
        .collect();
    Sprite::from_pixels(w, h, pixels)
}

/// Foreground hands/held objects cross the tabletop while the seated body
/// remains behind it. This is an occlusion layer, not a second actor or cue.
pub(super) fn gesture_layer(
    sprite: &Sprite,
    pose: Pose,
    facing: Facing,
    overview: bool,
) -> Option<Sprite> {
    let rect = if overview {
        match pose {
            Pose::Read => Some((7, 20, 19, 29)),
            Pose::FolderIn => Some((2, 19, 11, 28)),
            Pose::Coffee | Pose::WaterPlant => Some((18, 19, 24, 28)),
            Pose::Work | Pose::ScreenRead | Pose::Search => Some((12, 20, 24, 28)),
            _ => None,
        }
    } else {
        match pose {
            Pose::Read => Some((12, 41, 35, 56)),
            Pose::FolderIn => Some((0, 29, 17, 47)),
            Pose::Coffee | Pose::WaterPlant => Some((33, 39, 48, 53)),
            Pose::Work | Pose::ScreenRead | Pose::Search => Some((26, 42, 48, 52)),
            _ => None,
        }
    }?;
    let (w, h) = (sprite.width(), sprite.height());
    let pixels = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| {
            let sx = if facing == Facing::Left { w - x - 1 } else { x };
            (sx >= rect.0 && sx < rect.2 && y >= rect.1 && y < rect.3)
                .then(|| sprite.pixel(x, y))
                .flatten()
        })
        .collect();
    Some(Sprite::from_pixels(w, h, pixels))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pose {
    Rest,
    SeatedRest,
    ScreenRead,
    Work,
    Read,
    Search,
    Waiting,
    Error,
    Walk,
    Coffee,
    Stretch,
    WaterPlant,
    PaperPlane,
    FolderOut,
    FolderIn,
    Message,
}

pub fn identity(id: &str, wardrobe: Option<usize>) -> Character {
    let hash = stable_hash(id);
    Character {
        costume: wardrobe.map_or((hash % 12) as u8, |value| (value % 12) as u8),
        skin: ((hash >> 17) % 6) as u8,
        hair: ((hash >> 29) % 6) as u8,
    }
}

pub(super) fn stable_hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

pub(super) const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// A small signed-coordinate rasterizer used by the authored asset recipes.
/// Half-open rectangles and clipped writes keep neighbouring material seams exact.
pub(super) struct Raster {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Option<Color>>,
}

impl Raster {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![None; width * height],
        }
    }
    pub fn set(&mut self, x: i32, y: i32, color: Color) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.pixels[y as usize * self.width + x as usize] = Some(color);
        }
    }
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        for yy in y.max(0)..(y + h).min(self.height as i32) {
            for xx in x.max(0)..(x + w).min(self.width as i32) {
                self.set(xx, yy, color);
            }
        }
    }
    pub fn line(&mut self, a: (i32, i32), b: (i32, i32), color: Color) {
        let (mut x, mut y) = a;
        let dx = (b.0 - x).abs();
        let dy = -(b.1 - y).abs();
        let sx = if x < b.0 { 1 } else { -1 };
        let sy = if y < b.1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.set(x, y, color);
            if (x, y) == b {
                break;
            }
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
        }
    }
    pub fn ellipse(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        for yy in 0..h {
            for xx in 0..w {
                let dx = i64::from(2 * xx + 1 - w);
                let dy = i64::from(2 * yy + 1 - h);
                if dx * dx * i64::from(h * h) + dy * dy * i64::from(w * w)
                    <= i64::from(w * w) * i64::from(h * h)
                {
                    self.set(x + xx, y + yy, color);
                }
            }
        }
    }
    pub fn poly(&mut self, points: &[(i32, i32)], color: Color) {
        let min_y = points.iter().map(|p| p.1).min().unwrap_or(0).max(0);
        let max_y = points
            .iter()
            .map(|p| p.1)
            .max()
            .unwrap_or(0)
            .min(self.height as i32 - 1);
        for y in min_y..=max_y {
            let mut intersections = Vec::new();
            for i in 0..points.len() {
                let a = points[i];
                let b = points[(i + 1) % points.len()];
                if (a.1 <= y && b.1 > y) || (b.1 <= y && a.1 > y) {
                    intersections.push(a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1));
                }
            }
            intersections.sort_unstable();
            for pair in intersections.chunks_exact(2) {
                self.rect(pair[0], y, pair[1] - pair[0] + 1, 1, color);
            }
        }
        for i in 0..points.len() {
            self.line(points[i], points[(i + 1) % points.len()], color);
        }
    }
    pub fn sprite(self) -> Sprite {
        Sprite::from_pixels(self.width, self.height, self.pixels)
    }
}

pub fn character(character: Character, pose: Pose, phase: u8) -> Sprite {
    character_facing(character, pose, phase, Facing::Right)
}

pub fn character_facing(character: Character, pose: Pose, phase: u8, facing: Facing) -> Sprite {
    face(
        character_pose(character, pose, phase, facing == Facing::Front),
        facing,
    )
}

fn character_pose(character: Character, pose: Pose, phase: u8, front: bool) -> Sprite {
    let mut art = Raster::new(WIDTH, HEIGHT);
    let costume = character.costume as usize % COSTUMES.len();
    let skins = [
        (rgb(248, 204, 161), rgb(220, 161, 123)),
        (rgb(223, 168, 123), rgb(187, 125, 89)),
        (rgb(183, 124, 88), rgb(143, 86, 62)),
        (rgb(124, 77, 57), rgb(90, 54, 45)),
        (rgb(239, 183, 151), rgb(203, 139, 116)),
        (rgb(160, 103, 72), rgb(119, 72, 53)),
    ];
    let hairs = [
        rgb(58, 38, 38),
        rgb(105, 66, 42),
        rgb(191, 139, 67),
        rgb(187, 77, 50),
        rgb(214, 211, 192),
        rgb(81, 64, 114),
    ];
    let (skin, skin_shadow) = skins[character.skin as usize % skins.len()];
    let hair = hairs[character.hair as usize % hairs.len()];
    let ink = rgb(42, 39, 52);
    let shirt = CLOTHING[costume];
    let shade = darken(shirt, 34);
    let shine = lighten(shirt, 24);
    let walking = pose == Pose::Walk;
    let step = if walking {
        [0, 2, 3, 1, 0, -2, -3, -1][phase as usize % 8]
    } else {
        0
    };
    let seated = matches!(
        pose,
        Pose::Work | Pose::ScreenRead | Pose::Search | Pose::SeatedRest
    );
    let bob = if seated {
        2
    } else if walking && phase % 4 == 1 {
        -1
    } else {
        0
    };
    // Contemporary layers and hair are drawn behind the animated body.
    match costume {
        2 => {
            art.rect(7, 36 + bob, 8, 17, rgb(106, 94, 80));
            art.rect(7, 38 + bob, 2, 12, rgb(149, 135, 113));
        }
        3 => {
            art.rect(10, 23 + bob, 28, 21, hair);
            art.rect(11, 39 + bob, 4, 8, hair);
        }
        4 => {
            art.ellipse(8, 27 + bob, 31, 17, shade);
        }
        6 => {
            art.poly(&[(12, 34 + bob), (34, 34 + bob), (36, 57), (10, 57)], shade);
        }
        9 => {
            art.ellipse(33, 13 + bob, 10, 12, hair);
            art.poly(
                &[
                    (39, 19 + bob),
                    (44, 27 + bob),
                    (42, 37 + bob),
                    (36, 30 + bob),
                ],
                hair,
            );
        }
        _ => {}
    }
    if seated {
        // Hips meet the chair; bent knees and level shoes make sitting distinct.
        art.poly(
            &[
                (15, 52),
                (23, 53),
                (24, 57),
                (29, 58),
                (28, 62),
                (19, 61),
                (16, 57),
            ],
            ink,
        );
        art.poly(
            &[(27, 52), (33, 52), (34, 56), (39, 57), (38, 61), (29, 60)],
            ink,
        );
        art.rect(18, 54, 5, 4, rgb(83, 92, 108));
        art.rect(29, 54, 4, 3, rgb(68, 78, 95));
        art.rect(19, 60, 10, 3, ink);
        art.rect(30, 59, 10, 3, ink);
        art.rect(20, 60, 8, 1, rgb(234, 226, 205));
        art.rect(31, 59, 8, 1, rgb(196, 201, 196));
    } else {
        // Shoes and individually articulated legs. Neither is a scaled rectangle body.
        art.rect(15 - step / 2, 51 + bob, 7, 10 - bob, ink);
        art.rect(27 + step / 2, 51 + bob, 7, 10 - bob, ink);
        art.rect(16 - step / 2, 51 + bob, 5, 7, rgb(76, 83, 99));
        art.rect(28 + step / 2, 51 + bob, 5, 7, rgb(62, 67, 84));
        art.rect(12 - step / 2, 59, 10, 4, ink);
        art.rect(27 + step / 2, 57, 11, 4, ink);
        art.rect(13 - step / 2, 59, 8, 2, rgb(234, 226, 205));
        art.rect(28 + step / 2, 57, 8, 2, rgb(196, 201, 196));
    }
    // Rounded shoulders, a shaped hem, and edge shadows give clothing volume.
    art.poly(
        &[
            (15, 34 + bob),
            (29, 35 + bob),
            (34, 40 + bob),
            (32, 52 + bob),
            (26, 55 + bob),
            (15, 54 + bob),
            (11, 48 + bob),
            (11, 40 + bob),
        ],
        ink,
    );
    art.poly(
        &[
            (16, 35 + bob),
            (28, 36 + bob),
            (32, 41 + bob),
            (30, 52 + bob),
            (16, 52 + bob),
            (13, 47 + bob),
            (13, 40 + bob),
        ],
        shirt,
    );
    art.rect(14, 39 + bob, 3, 12, shine);
    art.rect(30, 40 + bob, 3, 12, shade);
    art.rect(17, 50 + bob, 13, 3, shade);
    let arm_y = match pose {
        Pose::Stretch => 23,
        Pose::Waiting => 26,
        Pose::Coffee if phase % 8 >= 3 => 20,
        Pose::PaperPlane => 26,
        Pose::FolderOut => 20,
        Pose::Message => 24,
        _ => 39,
    } + bob;
    let working = matches!(pose, Pose::Work | Pose::Read | Pose::Search);
    let wrist = if pose == Pose::FolderIn {
        34
    } else if pose == Pose::Stretch {
        23
    } else if working {
        43 + i32::from(phase % 2)
    } else {
        48
    } + bob;
    if matches!(pose, Pose::Work | Pose::ScreenRead | Pose::Search) {
        art.poly(
            &[
                (13, 38 + bob),
                (10, 42 + bob),
                (17, 48),
                (32, 48),
                (33, 44),
                (18, 43),
            ],
            shade,
        );
        art.poly(
            &[
                (31, 38 + bob),
                (35, 42),
                (44, 44),
                (44, 48),
                (34, 48),
                (29, 43),
            ],
            shade,
        );
    } else {
        art.poly(
            &[
                (13, 37 + bob),
                (9, 39 + bob),
                (8, wrist),
                (13, wrist + 3),
                (16, 41 + bob),
            ],
            shade,
        );
        art.rect(9, wrist, 5, 5, skin_shadow);
        art.rect(9, wrist, 4, 3, skin);
        art.poly(
            &[
                (31, 36 + bob),
                (36, arm_y),
                (39, arm_y + 8),
                (34, arm_y + 10),
                (30, 41 + bob),
            ],
            shade,
        );
        art.rect(35, arm_y + 7, 5, 5, skin_shadow);
        art.rect(35, arm_y + 7, 4, 3, skin);
    }
    // Costume cuts and identifying items remain visible beneath every face.
    match costume {
        0 | 4 => {
            art.poly(&[(16, 34 + bob), (23, 40 + bob), (30, 34 + bob)], shade);
            art.rect(19, 38 + bob, 1, 7, rgb(226, 229, 227));
            art.rect(27, 38 + bob, 1, 6, rgb(226, 229, 227));
            art.rect(19, 47 + bob, 10, 3, shade);
        }
        1 => {
            for x in (16..32).step_by(3) {
                art.rect(x, 39 + bob, 1, 13, darken(shirt, 12));
            }
            art.rect(16, 51 + bob, 16, 2, shade);
        }
        2 | 3 | 8 => {
            art.line((16, 35 + bob), (23, 41 + bob), lighten(shirt, 22));
            art.line((31, 35 + bob), (24, 41 + bob), shade);
            art.rect(23, 40 + bob, 2, 13, shade);
            art.rect(28, 42 + bob, 5, 4, shade);
            for y in [42, 47, 51] {
                art.rect(24, y + bob, 1, 1, rgb(224, 226, 224));
            }
        }
        5 => {
            for y in [39, 44, 49] {
                art.rect(15, y + bob, 17, 2, rgb(93, 106, 120));
            }
        }
        6 | 7 => {
            art.rect(20, 36 + bob, 8, 16, rgb(220, 222, 216));
            art.poly(
                &[(15, 34 + bob), (22, 43 + bob), (17, 40 + bob)],
                lighten(shirt, 25),
            );
            art.poly(&[(32, 34 + bob), (26, 43 + bob), (31, 41 + bob)], shade);
            art.rect(15, 46 + bob, 5, 2, shade);
        }
        9 => {
            art.rect(23, 36 + bob, 1, 16, rgb(205, 218, 214));
            art.rect(14, 39 + bob, 2, 11, shade);
        }
        10 => {
            art.rect(18, 33 + bob, 13, 6, shirt);
            art.rect(17, 39 + bob, 15, 1, shade);
        }
        11 => {
            art.poly(&[(17, 34 + bob), (24, 39 + bob), (30, 34 + bob)], shade);
            art.rect(16, 51 + bob, 16, 2, shade);
        }
        _ => {}
    }
    if front {
        // A separate frontal construction: level shoulders, equal eye sizes,
        // centred nose/collar and a balanced face. Costume crowns remain shared.
        art.rect(15, 36 + bob, 18, 5, shirt);
        art.rect(15, 36 + bob, 3, 5, shine);
        art.rect(29, 36 + bob, 4, 5, shade);
        art.poly(
            &[(19, 35 + bob), (24, 39 + bob), (29, 35 + bob)],
            skin_shadow,
        );
        art.rect(20, 31 + bob, 8, 5, skin_shadow);
        art.ellipse(10, 20 + bob, 6, 8, skin_shadow);
        art.ellipse(32, 20 + bob, 6, 8, skin_shadow);
        art.poly(
            &[
                (16, 11 + bob),
                (31, 11 + bob),
                (35, 17 + bob),
                (35, 27 + bob),
                (30, 33 + bob),
                (24, 35 + bob),
                (17, 33 + bob),
                (12, 27 + bob),
                (12, 17 + bob),
            ],
            skin_shadow,
        );
        art.poly(
            &[
                (17, 12 + bob),
                (29, 12 + bob),
                (33, 17 + bob),
                (33, 27 + bob),
                (29, 31 + bob),
                (24, 33 + bob),
                (18, 31 + bob),
                (14, 26 + bob),
                (14, 17 + bob),
            ],
            skin,
        );
        art.rect(15, 17 + bob, 2, 7, lighten(skin, 15));
        let eye_h = if phase == 7 && !matches!(pose, Pose::Waiting | Pose::Error) {
            1
        } else {
            3
        };
        for x in [18, 28] {
            art.rect(x, 22 + bob, 3, eye_h, ink);
            if eye_h > 1 {
                art.rect(x, 22 + bob, 1, 1, rgb(255, 249, 229));
            }
            art.rect(x - 1, 19 + bob, 5, 1, darken(hair, 10));
        }
        art.rect(23, 25 + bob, 3, 3, skin_shadow);
        art.rect(22, 30 + bob, 6, 1, darken(skin_shadow, 27));
    } else {
        // Ears, neck and the stepped oval of the face; one light source upper-left.
        art.rect(20, 31 + bob, 9, 7, skin_shadow);
        art.ellipse(10, 20 + bob, 6, 9, skin_shadow);
        art.ellipse(33, 20 + bob, 4, 8, skin_shadow);
        art.poly(
            &[
                (16, 11 + bob),
                (29, 11 + bob),
                (35, 16 + bob),
                (35, 22 + bob),
                (39, 25 + bob),
                (35, 27 + bob),
                (34, 31 + bob),
                (29, 35 + bob),
                (19, 34 + bob),
                (12, 29 + bob),
                (12, 17 + bob),
            ],
            skin_shadow,
        );
        art.poly(
            &[
                (16, 12 + bob),
                (28, 12 + bob),
                (32, 16 + bob),
                (32, 30 + bob),
                (28, 33 + bob),
                (18, 32 + bob),
                (14, 27 + bob),
                (14, 17 + bob),
            ],
            skin,
        );
        art.rect(16, 16 + bob, 3, 6, lighten(skin, 15));
        art.rect(33, 25 + bob, 4, 2, skin_shadow);
        art.rect(33, 25 + bob, 3, 1, lighten(skin, 17));
        art.rect(28, 30 + bob, 6, 1, darken(skin_shadow, 27));
        let eye_h = if phase == 7 && !matches!(pose, Pose::Error | Pose::Waiting) {
            1
        } else {
            3
        };
        art.rect(23, 22 + bob, 3, eye_h, ink);
        art.rect(32, 22 + bob, 2, eye_h, ink);
        if eye_h > 1 {
            art.rect(23, 22 + bob, 1, 1, rgb(255, 249, 229));
            art.rect(32, 22 + bob, 1, 1, rgb(255, 249, 229));
        }
        art.rect(22, 19 + bob, 5, 1, darken(hair, 10));
        art.rect(31, 19 + bob, 4, 1, darken(hair, 10));
    }
    // Every crown is drawn independently at this resolution.
    hair_cap(&mut art, hair, bob);
    match costume {
        0 => {
            art.rect(10, 9 + bob, 27, 3, ink);
            art.line((10, 11 + bob), (9, 20 + bob), ink);
            art.line((36, 11 + bob), (38, 20 + bob), ink);
            art.rect(8, 18 + bob, 6, 12, rgb(134, 145, 154));
            art.rect(35, 18 + bob, 6, 12, rgb(109, 121, 132));
        }
        1 => {
            art.ellipse(12, 4 + bob, 22, 13, hair);
            art.ellipse(9, 9 + bob, 8, 10, hair);
        }
        2 => {
            art.poly(
                &[
                    (10, 16 + bob),
                    (12, 7 + bob),
                    (31, 6 + bob),
                    (36, 12 + bob),
                    (27, 13 + bob),
                ],
                hair,
            );
        }
        3 => {
            art.rect(10, 15 + bob, 4, 18, hair);
            art.rect(34, 14 + bob, 4, 20, hair);
        }
        4 => {
            art.ellipse(14, 5 + bob, 19, 9, hair);
            art.rect(12, 9 + bob, 22, 4, hair);
        }
        5 => {
            art.ellipse(24, 2 + bob, 12, 10, hair);
            art.rect(12, 11 + bob, 3, 13, hair);
        }
        6 => {
            art.rect(9, 14 + bob, 5, 22, hair);
            art.rect(34, 17 + bob, 5, 18, hair);
        }
        7 => {
            art.poly(
                &[
                    (11, 16 + bob),
                    (13, 8 + bob),
                    (20, 4 + bob),
                    (32, 7 + bob),
                    (36, 14 + bob),
                ],
                hair,
            );
            art.rect(33, 27 + bob, 2, 3, rgb(189, 179, 152));
        }
        8 => {
            for x in if front { [15, 26] } else { [19, 30] } {
                art.rect(x, 20 + bob, 9, 7, rgb(76, 84, 92));
                art.rect(x + 1, 21 + bob, 6, 4, skin);
                art.rect(x + 4, 22 + bob, 2, 3, ink);
            }
            art.rect(if front { 24 } else { 28 }, 22 + bob, 3, 1, ink);
        }
        9 => {
            art.rect(12, 16 + bob, 24, 2, rgb(174, 194, 186));
        }
        10 => {
            art.poly(
                &[
                    (10, 16 + bob),
                    (9, 8 + bob),
                    (15, 5 + bob),
                    (30, 5 + bob),
                    (36, 12 + bob),
                    (31, 15 + bob),
                ],
                hair,
            );
        }
        11 => {
            art.ellipse(8, 7 + bob, 12, 12, hair);
            art.ellipse(16, 3 + bob, 14, 13, hair);
            art.ellipse(27, 7 + bob, 12, 11, hair);
        }
        _ => {}
    }
    // Props are foreground decorations, not labels or provider events.
    match pose {
        Pose::Waiting => {
            art.poly(
                &[(32, 37), (40, 29), (41, 16), (45, 16), (45, 32), (36, 43)],
                shade,
            );
            art.rect(41, 7, 5, 11, skin_shadow);
            art.rect(41, 7, 3, 9, skin);
            art.rect(40, 12, 2, 6, skin);
            art.rect(42, 5, 1, 4, skin);
            art.rect(44, 6, 1, 4, skin);
        }
        Pose::FolderOut => {
            art.rect(32, 27, 15, 12, rgb(172, 126, 69));
            art.rect(32, 25, 7, 3, rgb(228, 190, 118));
            art.rect(33, 29, 13, 3, rgb(245, 228, 183));
            art.rect(32, 32, 15, 8, rgb(216, 169, 94));
            art.rect(34, 34, 7, 1, rgb(248, 219, 155));
        }
        Pose::FolderIn => {
            art.rect(1, 32, 14, 12, rgb(116, 156, 136));
            art.rect(2, 30, 6, 3, rgb(167, 207, 168));
            art.rect(3, 33, 10, 3, rgb(241, 230, 193));
            art.rect(1, 36, 14, 9, rgb(133, 176, 146));
            art.rect(3, 38, 6, 1, rgb(199, 224, 178));
        }
        Pose::Message => {
            art.rect(33, 30, 14, 11, rgb(160, 204, 213));
            art.rect(34, 31, 12, 8, rgb(237, 236, 207));
            art.line((34, 31), (40, 35), rgb(130, 166, 177));
            art.line((40, 35), (46, 31), rgb(130, 166, 177));
            art.line((34, 38), (38, 35), rgb(169, 194, 191));
            art.line((42, 35), (46, 38), rgb(169, 194, 191));
        }
        Pose::Read => {
            art.poly(
                &[(13, 42), (23, 44), (33, 42), (33, 53), (23, 55), (13, 52)],
                rgb(101, 82, 127),
            );
            art.rect(15, 44, 7, 7, rgb(237, 221, 176));
            art.rect(24, 45, 7, 7, rgb(211, 200, 162));
        }
        Pose::Work | Pose::ScreenRead | Pose::Search => {
            let press = if pose == Pose::Work {
                i32::from(phase % 2)
            } else {
                0
            };
            art.rect(31, 44 + press, 5, 4, skin_shadow);
            art.rect(31, 44 + press, 4, 2, skin);
            art.rect(41, 45 - press, 5, 4, skin_shadow);
            art.rect(41, 45 - press, 4, 2, skin);
        }
        Pose::Coffee => {
            let y = if phase % 8 >= 3 { 28 } else { 43 };
            art.rect(34, y, 8, 8, rgb(226, 225, 198));
            art.rect(35, y, 6, 2, rgb(91, 62, 45));
            art.rect(42, y + 2, 3, 4, rgb(194, 198, 179));
            art.rect(36, y + 2, 2, 5, rgb(249, 241, 211));
        }
        Pose::WaterPlant => {
            art.rect(34, 43, 9, 7, rgb(74, 151, 142));
            art.line((42, 45), (47, 41), rgb(111, 190, 175));
            art.rect(35, 40, 5, 2, rgb(111, 190, 175));
        }
        Pose::PaperPlane => {
            art.poly(
                &[(31, 37), (46, 32), (39, 42), (37, 37)],
                rgb(245, 239, 209),
            );
            art.line((37, 37), (46, 32), rgb(150, 188, 194));
        }
        Pose::Error => {
            art.rect(20, 30 + bob, 7, 2, darken(skin_shadow, 20));
        }
        Pose::Stretch => {
            art.rect(8, 21 + bob, 5, 7, skin);
            art.rect(35, 20 + bob, 5, 8, skin);
        }
        _ => {}
    }
    art.sprite()
}

fn hair_cap(art: &mut Raster, hair: Color, bob: i32) {
    art.poly(
        &[
            (11, 22 + bob),
            (11, 13 + bob),
            (16, 8 + bob),
            (30, 9 + bob),
            (36, 15 + bob),
            (35, 23 + bob),
            (31, 16 + bob),
            (25, 15 + bob),
            (20, 18 + bob),
            (18, 15 + bob),
            (14, 18 + bob),
            (14, 23 + bob),
        ],
        hair,
    );
    art.rect(16, 10 + bob, 9, 2, lighten(hair, 19));
}

pub(super) fn darken(color: Color, amount: u8) -> Color {
    match color {
        Color::Rgb(r, g, b) => rgb(
            r.saturating_sub(amount),
            g.saturating_sub(amount),
            b.saturating_sub(amount),
        ),
        _ => color,
    }
}
pub(super) fn lighten(color: Color, amount: u8) -> Color {
    match color {
        Color::Rgb(r, g, b) => rgb(
            r.saturating_add(amount),
            g.saturating_add(amount),
            b.saturating_add(amount),
        ),
        _ => color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn twelve_costumes_have_twelve_authored_outlines() {
        let masks = (0..12)
            .map(|costume| {
                let image = character(
                    Character {
                        costume,
                        skin: 1,
                        hair: 0,
                    },
                    Pose::Rest,
                    0,
                );
                assert_eq!((image.width(), image.height()), (48, 64));
                let mask = image
                    .pixels()
                    .iter()
                    .map(Option::is_some)
                    .collect::<Vec<_>>();
                assert!(mask.iter().filter(|&&v| v).count() > 750);
                mask
            })
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(masks.len(), 12);
    }
    #[test]
    fn identity_depends_only_on_id_and_explicit_choice() {
        let original = identity("conversation-a", None);
        for id in ["neighbour-z", "new-neighbour", "retired-neighbour"] {
            let _ = identity(id, None);
        }
        assert_eq!(identity("conversation-a", None), original);
        assert_eq!(identity("conversation-a", Some(11)).costume, 11);
    }
    #[test]
    fn gestures_change_pose_without_changing_costume() {
        let look = identity("same-person", Some(3));
        let rest = character(look, Pose::Rest, 0);
        for pose in [
            Pose::Read,
            Pose::Coffee,
            Pose::Walk,
            Pose::WaterPlant,
            Pose::Stretch,
            Pose::PaperPlane,
        ] {
            assert!(
                rest.pixels() != character(look, pose, 2).pixels(),
                "missing gesture: {pose:?}"
            );
        }
    }
}
