//! Original 48×64 character art. Every costume changes its outline; none is a
//! resized legacy worker. Costumes and props are decorative, never job claims.

use ratatui::style::Color;

use crate::sprite::Sprite;

pub const WIDTH: usize = 48;
pub const HEIGHT: usize = 64;
pub const COSTUMES: [&str; 12] = [
    "Headphones",
    "Chef",
    "Explorer",
    "Gardener",
    "Astronaut",
    "Artist",
    "Wizard",
    "Rocker",
    "Bookworm",
    "Runner",
    "Hard hat",
    "Dinosaur",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Character {
    pub costume: u8,
    pub skin: u8,
    pub hair: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pose {
    Rest,
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
    let shirts = [
        rgb(83, 123, 170),
        rgb(237, 232, 207),
        rgb(178, 119, 65),
        rgb(98, 153, 100),
        rgb(220, 230, 226),
        rgb(210, 102, 86),
        rgb(112, 88, 168),
        rgb(63, 64, 85),
        rgb(186, 132, 72),
        rgb(76, 160, 151),
        rgb(233, 182, 70),
        rgb(112, 163, 88),
    ];
    let (skin, skin_shadow) = skins[character.skin as usize % skins.len()];
    let hair = hairs[character.hair as usize % hairs.len()];
    let ink = rgb(42, 39, 52);
    let shirt = shirts[costume];
    let shade = darken(shirt, 34);
    let shine = lighten(shirt, 24);
    let walking = pose == Pose::Walk;
    let step = if walking {
        [0, 2, 3, 1, 0, -2, -3, -1][phase as usize % 8]
    } else {
        0
    };
    let bob = if walking && phase % 4 == 1 { -1 } else { 0 };
    // Behind the body: long hair, backpack, cape, ponytail and dinosaur tail.
    match costume {
        2 => {
            art.rect(6, 35 + bob, 9, 19, ink);
            art.rect(6, 36 + bob, 7, 15, rgb(101, 86, 65));
        }
        3 => {
            art.rect(10, 23 + bob, 28, 23, hair);
            art.rect(11, 39 + bob, 4, 9, hair);
        }
        4 => {
            art.rect(7, 32 + bob, 34, 21, ink);
            art.rect(8, 34 + bob, 31, 15, rgb(160, 174, 183));
        }
        6 => {
            art.poly(
                &[(14, 33 + bob), (33, 33 + bob), (40, 58), (7, 58)],
                darken(shirt, 25),
            );
            art.rect(9, 54, 29, 3, rgb(164, 126, 176));
        }
        7 => {
            art.poly(
                &[
                    (11, 22),
                    (4, 19),
                    (9, 15),
                    (5, 11),
                    (14, 10),
                    (12, 5),
                    (23, 9),
                    (28, 4),
                    (33, 11),
                    (39, 9),
                    (37, 19),
                ],
                hair,
            );
        }
        9 => {
            art.ellipse(33, 13 + bob, 11, 13, hair);
            art.poly(
                &[
                    (39, 19 + bob),
                    (47, 26 + bob),
                    (44, 39 + bob),
                    (37, 31 + bob),
                ],
                hair,
            );
        }
        11 => {
            art.poly(
                &[(31, 49), (42, 47), (46, 39), (47, 53), (39, 57), (30, 56)],
                shade,
            );
            art.rect(41, 46, 3, 3, shine);
        }
        _ => {}
    }
    // Shoes and individually articulated legs. Neither is a scaled rectangle body.
    art.rect(15 - step / 2, 51 + bob, 7, 10 - bob, ink);
    art.rect(27 + step / 2, 51 + bob, 7, 10 - bob, ink);
    art.rect(16 - step / 2, 51 + bob, 5, 7, rgb(76, 83, 99));
    art.rect(28 + step / 2, 51 + bob, 5, 7, rgb(62, 67, 84));
    art.rect(12 - step / 2, 59, 10, 4, ink);
    art.rect(27 + step / 2, 59, 11, 4, ink);
    art.rect(13 - step / 2, 59, 8, 2, rgb(234, 226, 205));
    art.rect(28 + step / 2, 59, 8, 2, rgb(196, 201, 196));
    // Rounded shoulders, a shaped hem, and edge shadows give clothing volume.
    art.poly(
        &[
            (15, 34 + bob),
            (31, 34 + bob),
            (36, 39 + bob),
            (34, 52 + bob),
            (29, 55 + bob),
            (15, 54 + bob),
            (11, 48 + bob),
            (11, 40 + bob),
        ],
        ink,
    );
    art.poly(
        &[
            (16, 35 + bob),
            (30, 35 + bob),
            (34, 40 + bob),
            (32, 52 + bob),
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
    // Costume cuts and identifying items remain visible beneath every face.
    match costume {
        0 => {
            art.poly(&[(16, 34 + bob), (23, 40 + bob), (30, 34 + bob)], shade);
            art.rect(19, 38 + bob, 1, 7, rgb(218, 231, 227));
            art.rect(27, 38 + bob, 1, 6, rgb(218, 231, 227));
            art.rect(19, 47 + bob, 10, 3, shade);
        }
        1 => {
            art.rect(18, 36 + bob, 11, 17, rgb(247, 240, 218));
            art.rect(20, 44 + bob, 7, 5, rgb(198, 212, 203));
            art.rect(15, 49 + bob, 18, 2, rgb(186, 90, 80));
        }
        2 => {
            art.line((16, 35 + bob), (29, 52 + bob), rgb(69, 59, 48));
            art.line((17, 35 + bob), (30, 52 + bob), rgb(223, 179, 103));
            art.rect(27, 45 + bob, 9, 9, rgb(96, 66, 45));
            art.rect(28, 46 + bob, 7, 3, rgb(164, 112, 63));
        }
        3 => {
            art.rect(17, 39 + bob, 13, 14, rgb(69, 104, 133));
            art.rect(17, 35 + bob, 3, 9, rgb(83, 121, 150));
            art.rect(27, 35 + bob, 3, 9, rgb(83, 121, 150));
            art.rect(20, 43 + bob, 7, 5, rgb(52, 82, 111));
            art.rect(19, 40 + bob, 1, 1, rgb(233, 198, 110));
        }
        4 => {
            art.rect(18, 38 + bob, 12, 10, rgb(80, 100, 118));
            art.rect(20, 40 + bob, 3, 3, rgb(140, 213, 215));
            art.rect(25, 40 + bob, 3, 2, rgb(216, 123, 90));
            art.rect(21, 46 + bob, 7, 1, rgb(186, 198, 193));
            art.rect(15, 51 + bob, 17, 2, rgb(145, 165, 174));
        }
        5 => {
            for y in [39, 43, 47] {
                art.rect(15, y + bob, 16, 2, rgb(237, 215, 175));
            }
            art.poly(
                &[
                    (16, 33 + bob),
                    (32, 33 + bob),
                    (28, 38 + bob),
                    (18, 37 + bob),
                ],
                rgb(82, 114, 118),
            );
            art.rect(27, 37 + bob, 4, 12, rgb(78, 111, 114));
        }
        6 => {
            art.line((16, 35 + bob), (23, 53 + bob), rgb(181, 147, 89));
            art.line((31, 35 + bob), (24, 53 + bob), rgb(224, 188, 102));
            star(&mut art, 20, 44 + bob, rgb(242, 211, 135));
        }
        7 => {
            art.poly(
                &[(16, 35 + bob), (22, 43 + bob), (17, 40 + bob)],
                rgb(136, 137, 144),
            );
            art.poly(
                &[(31, 35 + bob), (25, 43 + bob), (30, 41 + bob)],
                rgb(104, 105, 121),
            );
            art.rect(23, 39 + bob, 2, 13, rgb(166, 164, 151));
            art.rect(15, 45 + bob, 5, 2, ink);
        }
        8 => {
            art.rect(22, 36 + bob, 3, 17, rgb(108, 75, 65));
            for y in [40, 45, 49] {
                art.rect(23, y + bob, 1, 1, rgb(237, 207, 154));
            }
            art.rect(28, 42 + bob, 5, 4, rgb(124, 87, 59));
        }
        9 => {
            art.poly(
                &[(15, 35 + bob), (24, 42 + bob), (31, 35 + bob)],
                rgb(204, 221, 216),
            );
            art.rect(13, 39 + bob, 2, 10, rgb(226, 236, 223));
            art.rect(30, 39 + bob, 2, 11, rgb(226, 236, 223));
            art.rect(22, 41 + bob, 2, 11, rgb(34, 100, 110));
        }
        10 => {
            art.rect(16, 36 + bob, 5, 16, rgb(238, 168, 62));
            art.rect(27, 36 + bob, 5, 16, rgb(239, 168, 62));
            art.rect(16, 43 + bob, 16, 3, rgb(240, 233, 175));
            art.rect(17, 36 + bob, 2, 16, rgb(252, 225, 132));
        }
        11 => {
            art.ellipse(18, 36 + bob, 12, 17, rgb(207, 218, 150));
            for y in [37, 43, 49] {
                art.rect(32, y + bob, 5, 3, rgb(213, 189, 95));
            }
        }
        _ => {}
    }
    // Ears, neck and the stepped oval of the face; one light source upper-left.
    art.rect(20, 31 + bob, 9, 7, skin_shadow);
    art.ellipse(10, 20 + bob, 6, 9, skin_shadow);
    art.ellipse(33, 20 + bob, 6, 9, skin_shadow);
    art.poly(
        &[
            (16, 11 + bob),
            (29, 11 + bob),
            (35, 16 + bob),
            (35, 29 + bob),
            (30, 35 + bob),
            (18, 35 + bob),
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
    art.rect(23, 25 + bob, 3, 3, skin_shadow);
    art.rect(23, 25 + bob, 2, 1, lighten(skin, 17));
    art.rect(22, 30 + bob, 6, 1, darken(skin_shadow, 27));
    let eye_h = if phase == 7 && !matches!(pose, Pose::Error | Pose::Waiting) {
        1
    } else {
        3
    };
    art.rect(18, 22 + bob, 3, eye_h, ink);
    art.rect(28, 22 + bob, 3, eye_h, ink);
    if eye_h > 1 {
        art.rect(18, 22 + bob, 1, 1, rgb(255, 249, 229));
        art.rect(28, 22 + bob, 1, 1, rgb(255, 249, 229));
    }
    art.rect(17, 19 + bob, 5, 1, darken(hair, 10));
    art.rect(27, 19 + bob, 5, 1, darken(hair, 10));
    // Every crown is drawn independently at this resolution.
    match costume {
        0 => {
            hair_cap(&mut art, hair, bob);
            art.line((9, 18 + bob), (9, 11 + bob), ink);
            art.rect(10, 9 + bob, 27, 3, ink);
            art.rect(8, 18 + bob, 6, 12, rgb(178, 75, 77));
            art.rect(35, 18 + bob, 6, 12, rgb(146, 56, 68));
            art.rect(8, 19 + bob, 2, 8, rgb(233, 134, 110));
        }
        1 => {
            art.ellipse(8, 5 + bob, 13, 14, rgb(206, 213, 198));
            art.ellipse(15, 1 + bob, 16, 17, rgb(247, 241, 220));
            art.ellipse(28, 5 + bob, 13, 14, rgb(225, 229, 211));
            art.rect(12, 14 + bob, 24, 6, rgb(234, 229, 207));
            art.rect(14, 15 + bob, 2, 4, rgb(250, 246, 228));
        }
        2 => {
            hair_cap(&mut art, hair, bob);
            art.poly(
                &[
                    (12, 14 + bob),
                    (14, 8 + bob),
                    (31, 8 + bob),
                    (36, 17 + bob),
                    (10, 17 + bob),
                ],
                rgb(139, 104, 59),
            );
            art.rect(8, 16 + bob, 32, 3, rgb(104, 77, 48));
            art.rect(14, 12 + bob, 19, 3, rgb(202, 159, 86));
            art.rect(30, 18 + bob, 8, 2, rgb(202, 159, 86));
        }
        3 => {
            art.ellipse(8, 6 + bob, 31, 13, rgb(210, 178, 96));
            art.rect(4, 15 + bob, 40, 4, rgb(126, 113, 65));
            art.rect(6, 14 + bob, 36, 3, rgb(225, 197, 122));
            art.rect(12, 12 + bob, 23, 3, rgb(108, 145, 100));
            art.ellipse(31, 8 + bob, 5, 5, rgb(218, 135, 134));
        }
        4 => {
            art.poly(
                &[
                    (13, 8 + bob),
                    (32, 8 + bob),
                    (40, 15 + bob),
                    (40, 30 + bob),
                    (33, 37 + bob),
                    (13, 37 + bob),
                    (6, 29 + bob),
                    (6, 16 + bob),
                ],
                rgb(193, 211, 214),
            );
            art.poly(
                &[
                    (14, 11 + bob),
                    (31, 11 + bob),
                    (36, 16 + bob),
                    (36, 29 + bob),
                    (31, 34 + bob),
                    (14, 34 + bob),
                    (10, 28 + bob),
                    (10, 17 + bob),
                ],
                rgb(70, 105, 129),
            );
            art.rect(14, 15 + bob, 18, 15, rgb(191, 191, 155));
            art.rect(18, 22 + bob, 3, 3, ink);
            art.rect(28, 22 + bob, 3, 3, ink);
            art.rect(14, 14 + bob, 16, 2, rgb(193, 232, 228));
            art.rect(12, 17 + bob, 2, 10, rgb(172, 220, 224));
            art.rect(37, 21 + bob, 5, 8, rgb(226, 154, 76));
        }
        5 => {
            hair_cap(&mut art, hair, bob);
            art.ellipse(8, 6 + bob, 30, 12, rgb(81, 62, 89));
            art.rect(17, 6 + bob, 22, 6, rgb(112, 85, 119));
            art.rect(26, 3 + bob, 4, 5, rgb(62, 48, 70));
            art.rect(12, 16 + bob, 22, 2, rgb(61, 47, 69));
        }
        6 => {
            art.poly(
                &[
                    (6, 18 + bob),
                    (15, 14 + bob),
                    (24, 1 + bob),
                    (29, 9 + bob),
                    (34, 14 + bob),
                    (43, 18 + bob),
                    (41, 21 + bob),
                    (7, 21 + bob),
                ],
                rgb(101, 76, 149),
            );
            art.poly(
                &[(16, 14 + bob), (24, 2 + bob), (24, 16 + bob)],
                rgb(144, 113, 190),
            );
            art.rect(13, 16 + bob, 25, 3, rgb(195, 153, 84));
            star(&mut art, 25, 11 + bob, rgb(248, 219, 144));
        }
        7 => {
            art.poly(
                &[
                    (11, 20 + bob),
                    (13, 13 + bob),
                    (17, 8 + bob),
                    (23, 11 + bob),
                    (28, 5 + bob),
                    (32, 14 + bob),
                    (36, 18 + bob),
                    (31, 18 + bob),
                    (26, 13 + bob),
                    (22, 17 + bob),
                    (17, 14 + bob),
                ],
                hair,
            );
            art.rect(33, 27 + bob, 3, 3, rgb(216, 190, 116));
        }
        8 => {
            hair_cap(&mut art, hair, bob);
            art.rect(14, 20 + bob, 10, 7, rgb(76, 90, 99));
            art.rect(26, 20 + bob, 10, 7, rgb(76, 90, 99));
            art.rect(16, 21 + bob, 6, 4, rgb(195, 218, 207));
            art.rect(28, 21 + bob, 6, 4, rgb(195, 218, 207));
            art.rect(18, 22 + bob, 2, 3, ink);
            art.rect(29, 22 + bob, 2, 3, ink);
            art.rect(23, 22 + bob, 4, 1, ink);
        }
        9 => {
            hair_cap(&mut art, hair, bob);
            art.rect(12, 16 + bob, 24, 3, rgb(241, 210, 121));
            art.rect(33, 16 + bob, 7, 3, rgb(244, 188, 113));
        }
        10 => {
            hair_cap(&mut art, hair, bob);
            art.ellipse(10, 5 + bob, 28, 16, rgb(231, 171, 57));
            art.rect(7, 16 + bob, 35, 4, rgb(195, 135, 41));
            art.rect(11, 16 + bob, 26, 2, rgb(255, 211, 94));
            art.rect(23, 5 + bob, 5, 11, rgb(255, 213, 103));
        }
        11 => {
            art.poly(
                &[
                    (8, 22 + bob),
                    (7, 11 + bob),
                    (14, 5 + bob),
                    (32, 6 + bob),
                    (39, 14 + bob),
                    (39, 32 + bob),
                    (35, 35 + bob),
                    (34, 16 + bob),
                    (15, 17 + bob),
                    (12, 32 + bob),
                    (8, 30 + bob),
                ],
                rgb(95, 142, 77),
            );
            art.rect(13, 5 + bob, 6, 6, rgb(119, 171, 93));
            art.rect(27, 6 + bob, 6, 6, rgb(119, 171, 93));
            art.rect(15, 7 + bob, 2, 2, ink);
            art.rect(29, 8 + bob, 2, 2, ink);
            art.rect(8, 16 + bob, 28, 3, rgb(167, 196, 119));
            for x in [13, 19, 25, 31] {
                art.rect(x, 19 + bob, 2, 3, rgb(247, 235, 186));
            }
        }
        _ => {}
    }
    // Props are foreground decorations, not labels or provider events.
    match pose {
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
        Pose::Search => {
            art.ellipse(29, 31, 11, 11, rgb(114, 154, 166));
            art.ellipse(31, 33, 7, 7, rgb(184, 216, 212));
            art.line((37, 40), (42, 46), rgb(92, 66, 51));
            art.line((38, 40), (43, 46), rgb(92, 66, 51));
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

fn star(art: &mut Raster, x: i32, y: i32, color: Color) {
    art.rect(x, y - 2, 1, 5, color);
    art.rect(x - 2, y, 5, 1, color);
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
