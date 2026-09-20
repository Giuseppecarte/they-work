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
    let materials = super::materials::for_room(key);
    let wall = materials.wall;
    let accent = materials.equipment;
    let trim = materials.trim;
    let wood = materials.floor;
    a.rect(0, 0, w, h, trim);
    a.rect(s, 2, w - s - 2, f, wall);
    a.rect(s, f, w - s, h - f, wood);
    // Floor joints remain subordinate to people and do not expand the wall.
    let plank = if key.overview { 20 } else { 36 };
    for y in (f..h).step_by(plank as usize) {
        a.rect(s, y, w - s, 1, lighten(wood, 3));
        let start = s + if ((y - f) / plank) % 2 == 0 { 0 } else { 53 };
        for x in (start..w).step_by(106) {
            a.rect(x, y, 1, plank, darken(wood, 4));
        }
    }
    a.rect(s, f - 1, w - s, 1, trim);
    a.rect(s, f, w - s, 1, lighten(trim, 25));
    a.rect(0, 0, w, 1, trim);
    a.rect(0, 1, w, 1, lighten(wall, 4));
    a.rect(0, h - 2, w, 2, trim);
    let title = key.title as i32;
    a.rect(s + 6, 4, w - s - 12, title + 2, wall);
    a.rect(s + 7, 4, w - s - 14, 1, lighten(wall, 3));
    // Every preset changes its structure, not merely its hue.
    let top = (title + 10).max(f - if key.overview { 42 } else { 86 });
    let window_h = (f - top - 7).max(8);
    let pane_w = (w - s - 24).max(4);
    // Broad glazing and slender mullions frame the workers without filling
    // their circulation space. Collaboration and research keep distinct zones.
    match key.preset {
        OfficePreset::Studio => {
            glazing(&mut a, s + 12, top, pane_w, window_h, materials.glazing);
            for x in (s + 12 + pane_w / 3..w - 10).step_by((pane_w / 3).max(1) as usize) {
                a.rect(x, top, 1, window_h, materials.metal);
            }
        }
        OfficePreset::Workshop => {
            glazing(&mut a, s + 12, top, pane_w, window_h, materials.glazing);
            let board_w = (pane_w / 3).max(8);
            a.rect(s + 18, top + 5, board_w, window_h - 10, materials.storage);
            a.rect(
                s + 19,
                top + 6,
                board_w - 2,
                window_h - 12,
                lighten(wall, 8),
            );
            // A quiet planning board, with abstract cards rather than fake work.
            for n in 0..3 {
                a.rect(
                    s + 23 + n * (board_w / 4),
                    top + 10,
                    (board_w / 5).max(2),
                    4,
                    materials.supplies[n as usize],
                );
            }
            a.rect(
                s + 17,
                top + window_h - 4,
                board_w + 2,
                2,
                materials.desktop,
            );
        }
        OfficePreset::Laboratory => {
            glazing(&mut a, s + 12, top, pane_w, window_h, materials.glazing);
            let bay = (pane_w / 3).max(8);
            for x in (s + 12..w - 12).step_by(bay as usize) {
                a.rect(x, top, 1, window_h, materials.metal);
                a.rect(x + 3, top + window_h - 12, bay - 7, 10, materials.storage);
                a.rect(x + 3, top + window_h - 13, bay - 7, 1, materials.desktop);
                a.rect(x + 5, top + window_h - 8, bay - 11, 1, materials.metal);
            }
        }
    }
    // A small fitted door occupies the shaft, with one style-specific detail.
    a.rect(0, 3, s - 1, h - 5, darken(wall, 8));
    a.rect(s - 3, 3, 3, h - 5, trim);
    let door_w = s - 14;
    let door_h = if key.overview { 37 } else { 65 };
    let door_y = f - door_h;
    a.rect(6, door_y - 3, door_w + 2, door_h + 4, trim);
    let metal = if key.zones[0] % 3 == 1 {
        accent
    } else {
        materials.metal
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
        a.rect(s / 2 - 1, door_y, 2, door_h, darken(materials.metal, 24));
        a.rect(11, door_y + 3, 2, door_h - 6, lighten(metal, 33));
        a.rect(s / 2 - 4, door_y - 5, 8, 2, rgb(171, 218, 180));
    } else {
        a.rect(8, door_y, door_w - 2, door_h, materials.desktop);
        a.rect(11, door_y + 4, door_w - 8, door_h / 2, materials.glazing);
        a.rect(12, door_y + 5, 2, door_h / 2 - 2, lighten(accent, 40));
        a.rect(s - 12, door_y + door_h / 2 + 6, 3, 2, rgb(232, 202, 133));
    }
    a.rect(6, f, door_w + 2, 2, materials.metal);
    if key.zones[0] % 3 == 2 {
        a.rect(8, door_y - 11, door_w - 4, 3, materials.prop);
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
        0 => coffee_station(&mut a, rest_x, rest_floor, key.overview, materials),
        1 => {
            let width = if key.overview { 21 } else { 32 };
            a.rect(
                rest_x - 12,
                rest_floor - 14,
                width,
                11,
                materials.upholstery,
            );
            a.rect(
                rest_x - 10,
                rest_floor - 13,
                width - 4,
                6,
                lighten(materials.upholstery, 18),
            );
            a.rect(rest_x - 14, rest_floor - 10, 3, 8, materials.upholstery);
            a.rect(
                rest_x + width - 13,
                rest_floor - 10,
                3,
                8,
                materials.upholstery,
            );
            a.rect(rest_x - 10, rest_floor - 3, 2, 3, materials.metal);
            a.rect(rest_x + width - 16, rest_floor - 3, 2, 3, materials.metal);
            a.rect(
                rest_x - 11 + width / 2,
                rest_floor - 13,
                1,
                10,
                darken(materials.upholstery, 12),
            );
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
        materials.storage,
    );
    books(
        &mut a,
        shelf_x + 2,
        rest_floor - if key.overview { 7 } else { 10 },
        if key.overview { 12 } else { 20 },
        if key.overview { 3 } else { 6 },
        materials.supplies,
    );
    a.rect(
        shelf_x + 2,
        rest_floor - 2,
        if key.overview { 9 } else { 16 },
        1,
        materials.prop,
    );
    if !key.zones[3].is_multiple_of(3) {
        a.rect(w - 20, rest_floor - 8, 12, 8, materials.storage);
        a.rect(w - 17, rest_floor - 12, 4, 4, materials.prop);
    }
    if key.zones[3] % 3 != 2 {
        super::plant(&mut a, w - 11, rest_floor, 0);
    }
    a.sprite()
}

fn glazing(a: &mut Raster, x: i32, y: i32, w: i32, h: i32, c: Color) {
    a.rect(x - 1, y - 1, w + 2, h + 2, darken(c, 23));
    a.rect(x, y, w, h, c);
    a.rect(x, y, w, h / 2, lighten(c, 12));
    // A garden silhouette and broad reflections suggest daylight through glass.
    for n in 0..w / 32 {
        let bh = 3 + (n * 7) % 9;
        a.ellipse(x + n * 32, y + h - bh, 28, bh, darken(c, 13));
    }
    a.poly(
        &[
            (x + w / 3, y),
            (x + w / 3 + 12, y),
            (x + 12, y + h),
            (x, y + h),
        ],
        lighten(c, 8),
    );
    a.rect(x, y + h, w, 1, lighten(c, 20));
}
fn books(a: &mut Raster, x: i32, y: i32, w: i32, h: i32, colors: [Color; 4]) {
    for (i, c) in colors.into_iter().enumerate() {
        let bx = x + i as i32 * (w / 4);
        a.rect(bx, y + (i % 2) as i32, w / 4 - 1, h - (i % 2) as i32, c);
        a.rect(bx + 1, y + 2, 1, (h - 3).max(1), lighten(c, 18));
    }
}
fn coffee_station(
    a: &mut Raster,
    x: i32,
    f: i32,
    small: bool,
    materials: super::materials::Materials,
) {
    let w = if small { 20 } else { 30 };
    let h = if small { 11 } else { 19 };
    a.rect(x - 10, f - h, w, h, materials.storage);
    a.rect(x - 11, f - h, w + 2, 2, materials.desktop);
    a.rect(x - 7, f - h - 10, 10, 10, materials.equipment);
    a.rect(x - 5, f - h - 8, 6, 4, rgb(35, 54, 64));
    a.rect(x - 4, f - h - 3, 4, 3, rgb(237, 225, 179));
    a.rect(x + 7, f - h - 4, 4, 4, materials.prop);
}
