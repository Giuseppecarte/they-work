//! Authored 24×32 cast for the building overview. Coordinates, silhouettes and
//! gestures are designed on this grid; no detail sprite is sampled down.
use super::art::{darken, lighten, rgb, Character, Facing, Pose, Raster};
use crate::sprite::Sprite;

pub const WIDTH: usize = 24;
pub const HEIGHT: usize = 32;

pub fn character(person: Character, pose: Pose, phase: u8) -> Sprite {
    character_facing(person, pose, phase, Facing::Right)
}
pub fn character_facing(person: Character, pose: Pose, phase: u8, facing: Facing) -> Sprite {
    super::art::face(
        character_pose(person, pose, phase, facing == Facing::Front),
        facing,
    )
}
fn character_pose(person: Character, pose: Pose, phase: u8, front: bool) -> Sprite {
    let mut a = Raster::new(WIDTH, HEIGHT);
    let costume = person.costume % 12;
    let skins = [
        rgb(248, 204, 161),
        rgb(223, 168, 123),
        rgb(183, 124, 88),
        rgb(124, 77, 57),
        rgb(239, 183, 151),
        rgb(160, 103, 72),
    ];
    let skin = skins[person.skin as usize % skins.len()];
    let shade = darken(skin, 34);
    let hair = [
        rgb(58, 38, 38),
        rgb(105, 66, 42),
        rgb(191, 139, 67),
        rgb(187, 77, 50),
        rgb(214, 211, 192),
        rgb(81, 64, 114),
    ][person.hair as usize % 6];
    let shirt = super::art::CLOTHING[costume as usize];
    let ink = rgb(40, 39, 50);
    let step = if pose == Pose::Walk {
        [0, 1, 1, 0, 0, -1, -1, 0][phase as usize % 8]
    } else {
        0
    };
    // The far leg is shorter and higher; the near shoulder is wider. This is
    // a three-quarter stance, not a symmetric miniature of the large portrait.
    match costume {
        2 => {
            a.rect(3, 18, 5, 10, rgb(106, 94, 80));
        }
        3 => {
            a.rect(5, 12, 13, 12, hair);
            a.rect(5, 21, 3, 4, hair);
        }
        4 => {
            a.ellipse(4, 14, 15, 9, darken(shirt, 25));
        }
        6 => {
            a.poly(&[(6, 18), (17, 18), (18, 29), (5, 29)], darken(shirt, 25));
        }
        9 => {
            a.ellipse(17, 5, 6, 7, hair);
            a.poly(&[(20, 8), (23, 13), (21, 19), (18, 16)], hair);
        }
        _ => {}
    }
    let seated = matches!(
        pose,
        Pose::Work | Pose::ScreenRead | Pose::Search | Pose::SeatedRest
    );
    if seated {
        a.poly(
            &[(8, 25), (12, 26), (13, 28), (16, 29), (15, 31), (10, 30)],
            ink,
        );
        a.poly(
            &[(14, 25), (17, 25), (18, 28), (21, 29), (20, 31), (15, 30)],
            ink,
        );
        a.rect(10, 29, 5, 1, rgb(233, 223, 197));
        a.rect(16, 29, 4, 1, rgb(186, 194, 191));
    } else {
        a.rect(8 - step, 25, 4, 5, ink);
        a.rect(14 + step, 24, 4, 5, ink);
        a.rect(7 - step, 29, 6, 2, ink);
        a.rect(14 + step, 28, 6, 2, ink);
        a.rect(8 - step, 29, 4, 1, rgb(233, 223, 197));
        a.rect(15 + step, 28, 4, 1, rgb(186, 194, 191));
    }
    a.poly(
        &[
            (9, 17),
            (16, 18),
            (19, 21),
            (17, 27),
            (10, 28),
            (6, 24),
            (7, 20),
        ],
        ink,
    );
    a.poly(
        &[
            (9, 18),
            (15, 19),
            (17, 21),
            (16, 26),
            (10, 26),
            (8, 24),
            (8, 21),
        ],
        shirt,
    );
    a.rect(8, 21, 2, 4, lighten(shirt, 27));
    a.rect(15, 21, 2, 5, darken(shirt, 30));
    // Near elbow and palm are visible below the cheek, rather than hidden at
    // the body's edge. The raised hand reaches above the head for human help.
    let hand_y = match pose {
        Pose::Waiting => 5,
        Pose::FolderOut | Pose::Message => 16,
        Pose::Coffee => {
            if phase % 8 >= 3 {
                14
            } else {
                21
            }
        }
        Pose::Stretch => 8,
        _ => 23,
    };
    if matches!(pose, Pose::Work | Pose::ScreenRead | Pose::Search) {
        a.poly(
            &[(8, 20), (6, 22), (10, 25), (17, 25), (18, 22), (11, 22)],
            darken(shirt, 25),
        );
        a.poly(
            &[(16, 20), (19, 21), (22, 23), (21, 26), (16, 24)],
            darken(shirt, 25),
        );
    } else {
        a.poly(
            &[
                (16, 19),
                (20, hand_y + 2),
                (22, hand_y + 4),
                (19, 24),
                (16, 23),
            ],
            darken(shirt, 25),
        );
        a.rect(20, hand_y, 3, 4, shade);
        a.rect(20, hand_y, 2, 3, skin);
        a.rect(7, 23, 3, 3, shade);
        a.rect(7, 23, 2, 2, skin);
    }
    match costume {
        0 | 4 => {
            a.line((10, 19), (12, 22), lighten(shirt, 30));
            a.line((15, 19), (13, 22), darken(shirt, 30));
            a.rect(10, 24, 5, 2, darken(shirt, 25));
        }
        1 => {
            for x in [10, 12, 14] {
                a.rect(x, 20, 1, 6, darken(shirt, 14));
            }
        }
        2 | 3 | 8 => {
            a.line((9, 18), (12, 21), lighten(shirt, 22));
            a.rect(12, 21, 1, 6, darken(shirt, 25));
            a.rect(15, 22, 3, 2, darken(shirt, 25));
        }
        5 => {
            for y in [21, 24, 27] {
                a.rect(9, y, 8, 1, rgb(93, 106, 120));
            }
        }
        6 | 7 => {
            a.rect(11, 19, 3, 8, rgb(220, 222, 216));
            a.line((8, 19), (12, 23), lighten(shirt, 25));
            a.line((16, 19), (13, 23), darken(shirt, 25));
        }
        9 => {
            a.rect(12, 19, 1, 8, rgb(205, 218, 214));
        }
        10 => {
            a.rect(10, 17, 6, 3, shirt);
        }
        11 => {
            a.line((9, 18), (12, 21), darken(shirt, 25));
            a.rect(9, 26, 8, 1, darken(shirt, 25));
        }
        _ => {}
    }
    if front {
        a.rect(8, 19, 9, 3, shirt);
        a.rect(8, 19, 2, 3, lighten(shirt, 27));
        a.rect(16, 19, 2, 3, darken(shirt, 30));
        a.poly(&[(10, 18), (12, 20), (15, 18)], shade);
        a.rect(10, 16, 5, 3, shade);
        a.poly(
            &[
                (9, 5),
                (15, 5),
                (18, 8),
                (19, 12),
                (17, 16),
                (13, 18),
                (8, 16),
                (6, 12),
                (7, 8),
            ],
            shade,
        );
        a.poly(
            &[
                (9, 6),
                (15, 6),
                (17, 9),
                (17, 13),
                (15, 16),
                (11, 17),
                (8, 14),
                (8, 9),
            ],
            skin,
        );
        a.rect(8, 9, 1, 4, lighten(skin, 18));
        let eye_h = if phase == 7 && !matches!(pose, Pose::Waiting | Pose::Error) {
            1
        } else {
            2
        };
        a.rect(10, 11, 2, eye_h, ink);
        a.rect(15, 11, 2, eye_h, ink);
        a.rect(12, 13, 2, 2, shade);
        a.rect(11, 16, 4, 1, darken(shade, 24));
    } else {
        a.rect(11, 15, 4, 5, shade);
        a.poly(
            &[
                (9, 5),
                (15, 5),
                (18, 8),
                (18, 11),
                (20, 12),
                (18, 14),
                (17, 17),
                (12, 18),
                (8, 16),
                (6, 12),
                (7, 8),
            ],
            shade,
        );
        a.poly(
            &[
                (9, 6),
                (15, 6),
                (17, 9),
                (17, 14),
                (15, 17),
                (11, 16),
                (8, 13),
                (8, 9),
            ],
            skin,
        );
        a.rect(8, 9, 1, 4, lighten(skin, 18));
        a.rect(7, 11, 2, 3, shade);
        let eye_h = if phase == 7 && !matches!(pose, Pose::Waiting | Pose::Error) {
            1
        } else {
            2
        };
        a.rect(12, 11, 2, eye_h, ink);
        a.rect(17, 11, 1, eye_h, ink);
        a.rect(15, 14, 2, 1, shade);
        a.rect(14, 16, 3, 1, darken(shade, 24));
    }
    a.poly(
        &[
            (6, 11),
            (6, 6),
            (9, 3),
            (15, 4),
            (18, 7),
            (18, 10),
            (15, 8),
            (11, 8),
            (8, 10),
            (8, 12),
        ],
        hair,
    );
    match costume {
        0 => {
            a.line((5, 8), (6, 3), ink);
            a.line((6, 3), (16, 3), ink);
            a.line((16, 3), (19, 9), ink);
            a.rect(4, 9, 3, 7, rgb(134, 145, 154));
            a.rect(18, 9, 2, 6, rgb(109, 121, 132));
        }
        1 => {
            a.ellipse(6, 2, 11, 6, hair);
            a.ellipse(4, 5, 5, 5, hair);
        }
        2 => {
            a.poly(&[(5, 8), (6, 3), (16, 2), (19, 6), (14, 7)], hair);
        }
        3 => {
            a.rect(5, 8, 3, 9, hair);
            a.rect(17, 7, 2, 10, hair);
        }
        4 => {
            a.ellipse(8, 2, 9, 5, hair);
        }
        5 => {
            a.ellipse(13, 0, 6, 5, hair);
            a.rect(6, 6, 2, 6, hair);
        }
        6 => {
            a.rect(4, 7, 3, 11, hair);
            a.rect(17, 9, 3, 9, hair);
        }
        7 => {
            a.poly(&[(5, 8), (7, 4), (10, 1), (16, 3), (19, 7)], hair);
            a.rect(17, 14, 1, 2, rgb(189, 179, 152));
        }
        8 => {
            for x in if front { [8, 14] } else { [10, 16] } {
                a.rect(x, 10, 5, 4, rgb(76, 84, 92));
                a.rect(x + 1, 11, 3, 2, skin);
                a.rect(x + 2, 11, 1, 2, ink);
            }
            a.rect(if front { 12 } else { 14 }, 11, 3, 1, ink);
        }
        9 => {
            a.rect(6, 8, 12, 1, rgb(174, 194, 186));
        }
        10 => {
            a.poly(&[(5, 8), (4, 4), (8, 2), (15, 2), (18, 6), (16, 8)], hair);
        }
        11 => {
            a.ellipse(4, 4, 6, 6, hair);
            a.ellipse(9, 1, 7, 6, hair);
            a.ellipse(15, 4, 5, 5, hair);
        }
        _ => {}
    }
    match pose {
        Pose::Read => {
            a.poly(
                &[(8, 22), (13, 23), (18, 21), (18, 27), (13, 28), (8, 26)],
                rgb(104, 76, 124),
            );
            a.rect(9, 23, 3, 3, rgb(238, 221, 175));
            a.rect(14, 23, 3, 3, rgb(205, 196, 163));
        }
        Pose::FolderOut => {
            a.rect(17, 18, 7, 6, rgb(215, 164, 87));
            a.rect(17, 17, 3, 2, rgb(231, 190, 110));
            a.rect(18, 19, 5, 1, rgb(246, 224, 172));
        }
        Pose::FolderIn => {
            a.rect(3, 21, 7, 6, rgb(126, 175, 140));
            a.rect(3, 20, 3, 2, rgb(169, 204, 160));
            a.rect(4, 22, 5, 1, rgb(244, 227, 183));
        }
        Pose::Message => {
            a.rect(17, 18, 7, 5, rgb(226, 233, 207));
            a.line((17, 18), (20, 21), rgb(103, 161, 173));
            a.line((20, 21), (23, 18), rgb(103, 161, 173));
        }
        Pose::Coffee => {
            a.rect(19, hand_y + 1, 4, 4, rgb(237, 228, 190));
            a.rect(20, hand_y + 1, 2, 1, rgb(90, 61, 47));
        }
        Pose::WaterPlant => {
            a.rect(18, 22, 5, 4, rgb(79, 159, 145));
            a.line((21, 23), (23, 20), rgb(135, 195, 173));
        }
        Pose::PaperPlane => a.poly(
            &[(16, 19), (23, 16), (20, 22), (19, 19)],
            rgb(245, 235, 201),
        ),
        _ => {}
    }
    if matches!(pose, Pose::Work | Pose::ScreenRead | Pose::Search) {
        let press = if pose == Pose::Work {
            i32::from(phase % 2)
        } else {
            0
        };
        a.rect(15, 22 + press, 3, 3, shade);
        a.rect(15, 22 + press, 2, 1, skin);
        a.rect(20, 23 - press, 3, 3, shade);
        a.rect(20, 23 - press, 2, 1, skin);
    }
    a.sprite()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_twelve_costumes_are_authored_on_the_overview_grid_with_distinct_outlines() {
        let mut masks = std::collections::HashSet::new();
        for costume in 0..12 {
            let sprite = character(
                Character {
                    costume,
                    skin: 1,
                    hair: 1,
                },
                Pose::Rest,
                0,
            );
            assert_eq!((sprite.width(), sprite.height()), (24, 32));
            masks.insert(
                (0..32)
                    .flat_map(|y| (0..24).map(move |x| (x, y)))
                    .map(|(x, y)| sprite.pixel(x, y).is_some())
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(masks.len(), 12);
    }
}
