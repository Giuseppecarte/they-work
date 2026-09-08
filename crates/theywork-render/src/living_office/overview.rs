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
    let shirt = shirts[costume as usize];
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
            a.rect(3, 18, 5, 10, rgb(97, 86, 59));
            a.rect(3, 19, 2, 7, rgb(150, 132, 88));
        }
        3 => {
            a.rect(5, 12, 13, 12, hair);
            a.rect(5, 21, 3, 5, hair);
        }
        4 => {
            a.rect(4, 17, 16, 10, rgb(106, 132, 146));
            a.rect(3, 19, 3, 7, rgb(172, 188, 182));
        }
        6 => {
            a.poly(&[(7, 17), (17, 18), (21, 29), (2, 29)], darken(shirt, 26));
            a.rect(4, 27, 15, 2, rgb(162, 128, 174));
        }
        9 => {
            a.ellipse(17, 5, 6, 7, hair);
            a.poly(&[(20, 8), (23, 13), (22, 21), (18, 17)], hair);
        }
        11 => {
            a.poly(
                &[(16, 24), (21, 23), (23, 19), (23, 27), (18, 29), (15, 27)],
                darken(shirt, 25),
            );
            a.rect(21, 23, 2, 2, lighten(shirt, 24));
        }
        _ => {}
    }
    a.rect(8 - step, 25, 4, 5, ink);
    a.rect(14 + step, 24, 4, 5, ink);
    a.rect(7 - step, 29, 6, 2, ink);
    a.rect(14 + step, 28, 6, 2, ink);
    a.rect(8 - step, 29, 4, 1, rgb(233, 223, 197));
    a.rect(15 + step, 28, 4, 1, rgb(186, 194, 191));
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
    match costume {
        0 => {
            a.line((10, 19), (12, 22), lighten(shirt, 42));
            a.line((15, 19), (13, 22), darken(shirt, 34));
            a.rect(10, 24, 5, 2, darken(shirt, 25));
        }
        1 => {
            a.rect(10, 19, 5, 7, rgb(250, 241, 207));
            a.rect(10, 24, 5, 2, rgb(182, 196, 178));
            a.rect(9, 27, 8, 1, rgb(173, 78, 65));
        }
        2 => {
            a.line((9, 18), (16, 25), rgb(239, 192, 102));
            a.rect(15, 23, 5, 4, rgb(109, 76, 49));
        }
        3 => {
            a.rect(9, 20, 7, 7, rgb(62, 107, 128));
            a.rect(9, 18, 1, 4, rgb(148, 180, 143));
            a.rect(15, 19, 1, 3, rgb(148, 180, 143));
        }
        4 => {
            a.rect(10, 20, 6, 4, rgb(57, 89, 112));
            a.rect(11, 21, 2, 1, rgb(133, 217, 211));
            a.rect(14, 21, 1, 2, rgb(213, 141, 94));
        }
        5 => {
            for y in [21, 24] {
                a.rect(9, y, 8, 1, rgb(249, 223, 168));
            }
            a.rect(14, 19, 2, 5, rgb(74, 116, 120));
        }
        6 => {
            a.line((9, 18), (13, 26), rgb(214, 179, 97));
            a.line((16, 19), (13, 26), rgb(214, 179, 97));
            a.rect(11, 22, 4, 1, rgb(245, 218, 128));
            a.rect(12, 21, 1, 3, rgb(245, 218, 128));
        }
        7 => {
            a.line((8, 19), (12, 23), rgb(169, 164, 155));
            a.line((16, 19), (13, 23), rgb(133, 135, 148));
            a.rect(12, 22, 1, 5, rgb(184, 176, 143));
        }
        8 => {
            a.rect(12, 19, 1, 8, rgb(119, 81, 55));
            for y in [21, 24, 26] {
                a.rect(12, y, 1, 1, rgb(244, 214, 144));
            }
            a.rect(14, 22, 3, 2, rgb(139, 94, 59));
        }
        9 => {
            a.line((8, 19), (12, 22), rgb(227, 233, 207));
            a.line((16, 19), (12, 22), rgb(227, 233, 207));
            a.rect(12, 22, 1, 5, rgb(38, 106, 111));
        }
        10 => {
            a.rect(9, 19, 2, 8, rgb(243, 164, 56));
            a.rect(15, 19, 2, 8, rgb(243, 164, 56));
            a.rect(9, 23, 8, 2, rgb(244, 236, 159));
        }
        11 => {
            a.ellipse(10, 19, 6, 8, rgb(208, 218, 148));
            a.rect(16, 21, 2, 2, rgb(221, 190, 89));
            a.rect(16, 25, 2, 2, rgb(221, 190, 89));
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
    match costume {
        0 => {
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
                    (9, 9),
                    (8, 7),
                    (8, 12),
                ],
                hair,
            );
            a.line((5, 8), (6, 3), ink);
            a.line((6, 3), (16, 3), ink);
            a.rect(4, 9, 3, 7, rgb(172, 67, 69));
            a.rect(4, 10, 1, 5, rgb(234, 134, 113));
            a.rect(18, 9, 2, 5, rgb(146, 60, 60));
        }
        1 => {
            a.ellipse(5, 3, 6, 6, rgb(206, 216, 192));
            a.ellipse(9, 0, 8, 8, rgb(251, 243, 217));
            a.ellipse(15, 3, 6, 6, rgb(224, 230, 204));
            a.rect(7, 7, 11, 3, rgb(241, 236, 204));
            a.rect(8, 7, 1, 3, rgb(255, 249, 226));
        }
        2 => {
            a.poly(
                &[(5, 9), (7, 4), (15, 3), (18, 8), (22, 10), (8, 11)],
                rgb(151, 110, 53),
            );
            a.rect(5, 9, 17, 2, rgb(106, 79, 45));
            a.rect(9, 6, 7, 1, rgb(215, 174, 100));
        }
        3 => {
            a.ellipse(6, 3, 13, 7, rgb(192, 169, 83));
            a.rect(2, 8, 21, 3, rgb(167, 137, 69));
            a.rect(3, 8, 17, 1, rgb(230, 207, 125));
            a.rect(16, 5, 3, 2, rgb(224, 122, 122));
        }
        4 => {
            a.poly(
                &[
                    (7, 2),
                    (16, 2),
                    (21, 7),
                    (21, 16),
                    (17, 20),
                    (6, 18),
                    (3, 14),
                    (3, 7),
                ],
                rgb(143, 173, 185),
            );
            a.poly(
                &[
                    (7, 4),
                    (16, 4),
                    (19, 7),
                    (19, 15),
                    (16, 18),
                    (6, 16),
                    (5, 13),
                    (5, 7),
                ],
                rgb(62, 93, 108),
            );
            a.rect(7, 7, 11, 8, rgb(174, 197, 158));
            a.rect(12, 10, 2, 2, ink);
            a.rect(17, 10, 1, 2, ink);
            a.rect(7, 7, 11, 1, rgb(213, 222, 178));
        }
        5 => {
            a.poly(
                &[(4, 8), (6, 4), (13, 3), (19, 5), (20, 8), (15, 10), (7, 10)],
                rgb(107, 73, 121),
            );
            a.rect(12, 2, 2, 3, rgb(82, 60, 98));
            a.rect(6, 8, 12, 2, rgb(70, 54, 87));
        }
        6 => {
            a.poly(
                &[(5, 9), (11, 0), (15, 5), (19, 10), (22, 12), (2, 12)],
                rgb(125, 98, 174),
            );
            a.poly(&[(6, 9), (11, 1), (11, 10)], rgb(163, 133, 202));
            a.rect(5, 10, 15, 2, rgb(199, 167, 91));
            a.rect(12, 6, 3, 1, rgb(250, 223, 131));
            a.rect(13, 5, 1, 3, rgb(250, 223, 131));
        }
        7 => {
            a.poly(
                &[
                    (5, 12),
                    (2, 7),
                    (6, 7),
                    (4, 2),
                    (10, 5),
                    (12, 1),
                    (15, 6),
                    (19, 3),
                    (19, 8),
                    (22, 9),
                    (18, 12),
                    (16, 8),
                    (12, 9),
                    (9, 7),
                    (7, 12),
                ],
                hair,
            );
            a.rect(6, 15, 2, 2, rgb(230, 193, 109));
        }
        8 => {
            a.poly(
                &[
                    (6, 11),
                    (6, 6),
                    (10, 3),
                    (16, 5),
                    (18, 9),
                    (15, 8),
                    (11, 8),
                    (8, 10),
                    (8, 14),
                ],
                hair,
            );
            if front {
                for x in [8, 14] {
                    a.rect(x, 10, 5, 4, rgb(67, 91, 94));
                    a.rect(x + 1, 11, 3, 2, rgb(182, 213, 198));
                    a.rect(x + 2, 11, 1, 2, ink);
                }
                a.rect(12, 11, 3, 1, ink);
            } else {
                a.rect(10, 10, 6, 4, rgb(67, 91, 94));
                a.rect(17, 10, 3, 4, rgb(67, 91, 94));
                a.rect(11, 11, 4, 2, rgb(182, 213, 198));
                a.rect(18, 11, 1, 2, rgb(182, 213, 198));
                a.rect(13, 11, 1, 2, ink);
                a.rect(15, 11, 3, 1, ink);
            }
        }
        9 => {
            a.poly(
                &[
                    (6, 12),
                    (5, 7),
                    (9, 3),
                    (15, 4),
                    (19, 8),
                    (16, 9),
                    (13, 8),
                    (10, 10),
                    (8, 8),
                    (8, 14),
                ],
                hair,
            );
            a.rect(6, 8, 14, 2, rgb(244, 205, 104));
            a.rect(18, 8, 4, 2, rgb(222, 168, 91));
        }
        10 => {
            a.ellipse(5, 2, 15, 9, rgb(231, 173, 63));
            a.rect(3, 9, 20, 3, rgb(193, 128, 41));
            a.rect(5, 9, 16, 1, rgb(253, 219, 118));
            a.rect(12, 2, 3, 7, rgb(252, 211, 93));
        }
        11 => {
            a.poly(
                &[
                    (4, 14),
                    (4, 6),
                    (7, 2),
                    (16, 3),
                    (20, 7),
                    (20, 17),
                    (17, 18),
                    (17, 9),
                    (8, 9),
                    (7, 16),
                    (5, 17),
                ],
                rgb(99, 149, 76),
            );
            a.rect(6, 2, 4, 4, rgb(133, 184, 89));
            a.rect(14, 3, 4, 4, rgb(133, 184, 89));
            a.rect(8, 3, 1, 1, ink);
            a.rect(16, 4, 1, 1, ink);
            a.rect(6, 8, 12, 2, rgb(185, 208, 124));
            for x in [8, 12, 16] {
                a.rect(x, 10, 1, 2, rgb(245, 230, 176));
            }
        }
        _ => {
            a.poly(
                &[
                    (6, 11),
                    (6, 6),
                    (10, 3),
                    (16, 5),
                    (18, 9),
                    (15, 8),
                    (11, 8),
                    (8, 10),
                    (8, 14),
                ],
                hair,
            );
        }
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
        Pose::Search => {
            a.ellipse(16, 17, 6, 6, rgb(111, 157, 169));
            a.ellipse(17, 18, 4, 4, rgb(199, 220, 203));
            a.line((20, 22), (22, 25), rgb(102, 69, 50));
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
