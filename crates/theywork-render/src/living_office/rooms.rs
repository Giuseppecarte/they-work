//! Material and furniture recipes on each scene's authored grid. Wall height,
//! circulation and native labels have independent budgets from character size.
use super::{
    art::{darken, lighten, rgb, Raster},
    RoomKey,
};
use crate::{design::OfficePreset, sprite::Sprite};
use ratatui::style::Color;

pub(super) fn background(key: RoomKey) -> Sprite {
    let mut a = Raster::new(key.width, key.height);
    let (w, h, f, s) = (
        key.width as i32,
        key.height as i32,
        key.floor as i32,
        key.shaft as i32,
    );
    let warm = key.preset == OfficePreset::Studio;
    let wall = match (key.preset, key.light) {
        (OfficePreset::Studio, false) => rgb(85, 91, 98),
        (OfficePreset::Studio, true) => rgb(223, 213, 189),
        (OfficePreset::Workshop, false) => rgb(83, 91, 90),
        (OfficePreset::Workshop, true) => rgb(207, 202, 176),
        (OfficePreset::Laboratory, false) => rgb(61, 88, 100),
        (OfficePreset::Laboratory, true) => rgb(203, 226, 220),
    };
    let accent = [
        rgb(103, 141, 144),
        rgb(109, 139, 171),
        rgb(129, 153, 107),
        rgb(164, 120, 143),
    ][key.palette];
    let trim = if key.light {
        rgb(113, 128, 130)
    } else {
        rgb(36, 48, 61)
    };
    let wood = match key.preset {
        OfficePreset::Studio => rgb(141, 112, 86),
        OfficePreset::Workshop => rgb(111, 107, 89),
        OfficePreset::Laboratory => rgb(106, 135, 138),
    };
    a.rect(0, 0, w, h, rgb(31, 42, 55));
    a.rect(s, 2, w - s - 2, f, wall);
    a.rect(s, f, w - s, h - f, wood);
    // Floor joints remain subordinate to people and do not expand the wall.
    let plank = if key.overview { 10 } else { 16 };
    for y in (f..h).step_by(plank as usize) {
        a.rect(s, y, w - s, 1, lighten(wood, 7));
        let start = s + if ((y - f) / plank) % 2 == 0 { 0 } else { 27 };
        for x in (start..w).step_by(54) {
            a.rect(x, y, 1, plank, darken(wood, 8));
        }
    }
    a.rect(s, f - 3, w - s, 3, trim);
    a.rect(s, f, w - s, 1, lighten(trim, 25));
    a.rect(0, 0, w, 2, rgb(38, 46, 55));
    a.rect(0, 2, w, 1, rgb(135, 129, 111));
    a.rect(0, h - 2, w, 2, rgb(33, 40, 50));
    let title = key.title as i32;
    a.rect(s + 6, 4, w - s - 12, title + 2, trim);
    a.rect(s + 7, 4, w - s - 14, 1, lighten(trim, 17));
    // Every preset changes its structure, not merely its hue.
    let top = (title + 10).max(f - if key.overview { 42 } else { 86 });
    let window_h = if key.overview { 17 } else { 33 };
    match key.preset {
        OfficePreset::Studio => {
            for x in (s + 9..w - 42).step_by(((w - s - 25) as usize / 3).max(if key.overview {
                78
            } else {
                116
            })) {
                glazing(
                    &mut a,
                    x,
                    top,
                    if key.overview { 47 } else { 70 },
                    window_h,
                    darken(accent, 18),
                );
            }
            if !key.overview {
                a.rect(w - 42, top + 5, 27, 4, rgb(99, 76, 58));
                books(&mut a, w - 40, top - 8, 24, 13);
            }
        }
        OfficePreset::Workshop => {
            let peg_w = (w - s - 26).min(if key.overview { 110 } else { 190 });
            a.rect(s + 10, top, peg_w, window_h + 5, rgb(119, 109, 82));
            for yy in (top + 3..top + window_h).step_by(5) {
                for xx in (s + 13..s + peg_w).step_by(7) {
                    a.rect(xx, yy, 1, 1, rgb(77, 78, 68));
                }
            }
            for x in (s + 18..s + peg_w - 7).step_by(24) {
                a.rect(x, top + 6, 2, 12, rgb(179, 181, 156));
                a.rect(x - 3, top + 6, 8, 3, accent);
            }
            a.rect(s + 8, top + window_h + 5, peg_w + 4, 3, rgb(73, 75, 67));
            a.rect(
                w - 21,
                title + 10,
                3,
                (f - title - 10).max(0),
                rgb(56, 68, 68),
            );
        }
        OfficePreset::Laboratory => {
            for x in (s + 9..w - 24).step_by(((w - s - 25) as usize / 3).max(if key.overview {
                57
            } else {
                89
            })) {
                glazing(
                    &mut a,
                    x,
                    top,
                    if key.overview { 40 } else { 65 },
                    window_h,
                    rgb(91, 135, 139),
                );
                a.rect(
                    x + 4,
                    top + window_h + 4,
                    if key.overview { 31 } else { 56 },
                    3,
                    rgb(154, 183, 173),
                );
            }
            for x in (s + 8..w - 8).step_by(28) {
                a.rect(x, f - 17, 1, 14, darken(wall, 9));
            }
        }
    }
    // A small fitted door occupies the shaft, with one style-specific detail.
    a.rect(0, 3, s - 1, h - 5, rgb(39, 53, 65));
    a.rect(s - 3, 3, 3, h - 5, rgb(25, 37, 50));
    let door_w = s - 14;
    let door_h = if key.overview { 37 } else { 65 };
    let door_y = f - door_h;
    a.rect(6, door_y - 3, door_w + 2, door_h + 4, rgb(24, 36, 46));
    let metal = if key.zones[0] % 3 == 1 {
        accent
    } else {
        rgb(128, 152, 159)
    };
    if key.elevator {
        a.rect(8, door_y, door_w - 2, door_h, metal);
        a.rect(
            9,
            door_y + 1,
            (door_w - 4) / 2,
            door_h - 1,
            lighten(metal, 20),
        );
        a.rect(s / 2 - 1, door_y, 2, door_h, rgb(62, 84, 94));
        a.rect(11, door_y + 3, 2, door_h - 6, lighten(metal, 33));
        a.rect(s / 2 - 4, door_y - 5, 8, 2, rgb(171, 218, 180));
    } else {
        a.rect(8, door_y, door_w - 2, door_h, rgb(154, 115, 78));
        a.rect(11, door_y + 4, door_w - 8, door_h / 2, accent);
        a.rect(12, door_y + 5, 2, door_h / 2 - 2, lighten(accent, 40));
        a.rect(s - 12, door_y + door_h / 2 + 6, 3, 2, rgb(232, 202, 133));
    }
    a.rect(6, f, door_w + 2, 2, rgb(174, 182, 165));
    if key.zones[0] % 3 == 2 {
        a.rect(8, door_y - 11, door_w - 4, 3, rgb(202, 166, 105));
    }
    if key.zones[0].is_multiple_of(3) && door_y > 16 {
        a.rect(8, 7, s - 18, 7, trim);
        a.rect(10, 9, s - 24, 1, rgb(192, 210, 178));
    }
    // Equipment and people share the chair's authored seat height.
    for seat in 0..key.seats {
        super::workstation::chair(&mut a, key, key.first_x + seat * key.slot);
    }
    // A usable resting corner lives in the foreground aisle. Its anchor has a
    // real destination; the decorative actor does not walk to an arbitrary offset.
    let rest_x = (w - 34).max(s + 16);
    let rest_floor = key.rest_floor() as i32;
    match key.zones[3] % 3 {
        0 => coffee_station(&mut a, rest_x, rest_floor, key.overview, warm),
        1 => {
            let width = if key.overview { 21 } else { 32 };
            a.rect(rest_x - 12, rest_floor - 14, width, 12, accent);
            a.rect(
                rest_x - 10,
                rest_floor - 13,
                width - 4,
                6,
                lighten(accent, 18),
            );
            a.rect(rest_x - 12, rest_floor - 3, width, 3, darken(accent, 20));
            books(&mut a, rest_x - 9, rest_floor - 6, 12, 3);
        }
        _ => super::plant(&mut a, rest_x, rest_floor, if key.overview { 0 } else { 1 }),
    }
    // Small shared facilities remain available in every rest arrangement, so
    // a style's planned destination always corresponds to a visible object.
    let shelf_x = s + 7;
    a.rect(
        shelf_x,
        rest_floor - 4,
        if key.overview { 16 } else { 26 },
        4,
        rgb(108, 91, 76),
    );
    books(
        &mut a,
        shelf_x + 2,
        rest_floor - if key.overview { 7 } else { 10 },
        if key.overview { 12 } else { 20 },
        if key.overview { 3 } else { 6 },
    );
    a.rect(
        shelf_x + 2,
        rest_floor - 2,
        if key.overview { 9 } else { 16 },
        1,
        rgb(225, 194, 132),
    );
    if !key.zones[3].is_multiple_of(3) {
        a.rect(w - 20, rest_floor - 8, 12, 8, rgb(101, 109, 98));
        a.rect(w - 17, rest_floor - 12, 4, 4, rgb(230, 214, 168));
    }
    if key.zones[3] % 3 != 2 {
        super::plant(&mut a, w - 11, rest_floor, 0);
    }
    a.sprite()
}

fn glazing(a: &mut Raster, x: i32, y: i32, w: i32, h: i32, c: Color) {
    a.rect(x - 2, y - 2, w + 4, h + 5, rgb(38, 57, 68));
    a.rect(x - 1, y - 1, w + 2, h + 2, rgb(106, 125, 128));
    a.rect(x, y, w, h, c);
    a.rect(x, y, w, h / 2, lighten(c, 12));
    for n in 0..w / 10 {
        let bh = 4 + (n * 7) % 10;
        a.rect(x + n * 10, y + h - bh, 8, bh, darken(c, 30));
        a.rect(x + n * 10 + 2, y + h - bh + 2, 1, 1, rgb(203, 213, 174));
    }
    a.poly(
        &[(x + 3, y), (x + 10, y), (x + 3, y + h), (x, y + h)],
        lighten(c, 14),
    );
    a.rect(x + w / 2, y, 2, h, rgb(65, 93, 101));
    a.rect(x - 3, y + h + 2, w + 6, 2, rgb(123, 138, 134));
}
fn books(a: &mut Raster, x: i32, y: i32, w: i32, h: i32) {
    for (i, c) in [
        rgb(182, 105, 87),
        rgb(113, 158, 158),
        rgb(198, 170, 105),
        rgb(147, 129, 167),
    ]
    .into_iter()
    .enumerate()
    {
        let bx = x + i as i32 * (w / 4);
        a.rect(bx, y + (i % 2) as i32, w / 4 - 1, h - (i % 2) as i32, c);
        a.rect(bx + 1, y + 2, 1, (h - 3).max(1), lighten(c, 18));
    }
}
fn coffee_station(a: &mut Raster, x: i32, f: i32, small: bool, warm: bool) {
    let w = if small { 20 } else { 30 };
    let h = if small { 11 } else { 19 };
    a.rect(x - 10, f - h, w, h, rgb(96, 86, 72));
    a.rect(x - 11, f - h, w + 2, 2, rgb(214, 181, 124));
    a.rect(
        x - 7,
        f - h - 10,
        10,
        10,
        if warm {
            rgb(156, 110, 78)
        } else {
            rgb(125, 152, 153)
        },
    );
    a.rect(x - 5, f - h - 8, 6, 4, rgb(35, 54, 64));
    a.rect(x - 4, f - h - 3, 4, 3, rgb(237, 225, 179));
    a.rect(x + 7, f - h - 4, 4, 4, rgb(232, 216, 163));
}
