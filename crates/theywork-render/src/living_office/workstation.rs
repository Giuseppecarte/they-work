//! Authored workstations and their interaction geometry. Screen motifs classify
//! observations; they contain no invented source text, percentages or results.
use super::{
    art::{darken, lighten, rgb, Raster},
    PixelRect, RoomKey,
};
use theywork_core::{Activity, Worker, WorkerStatus};

/// Physical hit anchors in the same canvas coordinates as the native nameplate.
/// The home and station remain fixed while a decorative actor visits the aisle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkstationLayout {
    pub computer: PixelRect,
    pub keyboard: PixelRect,
    pub chair: PixelRect,
    pub selection: PixelRect,
    pub actor_home: PixelRect,
}
impl WorkstationLayout {
    pub(super) fn scaled(self, scale: usize) -> Self {
        Self {
            computer: self.computer.scaled(scale),
            keyboard: self.keyboard.scaled(scale),
            chair: self.chair.scaled(scale),
            selection: self.selection.scaled(scale),
            actor_home: self.actor_home.scaled(scale),
        }
    }
    pub(super) fn translate(&mut self, x: usize, y: usize) {
        for rect in [
            &mut self.computer,
            &mut self.keyboard,
            &mut self.chair,
            &mut self.selection,
            &mut self.actor_home,
        ] {
            rect.x += x;
            rect.y += y;
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Screen {
    Code,
    Terminal,
    Document,
    Search,
    Thinking,
    Waiting,
    Human,
    Quiet,
    Error,
    Unknown,
}
pub(super) fn screen(worker: &Worker, now: i64) -> Screen {
    if super::observation_unavailable(worker, now) {
        return Screen::Unknown;
    }
    if worker.status_at(now) == WorkerStatus::Failed {
        return Screen::Error;
    }
    if crate::presentation::human_request(worker) {
        return Screen::Human;
    }
    if worker.wait_reason.is_some() {
        return Screen::Waiting;
    }
    if worker.status_at(now) != WorkerStatus::Running {
        return Screen::Quiet;
    }
    match worker.activity {
        Activity::Editing { .. } => Screen::Code,
        Activity::Typing { .. } => Screen::Terminal,
        Activity::Reading { .. } => Screen::Document,
        Activity::Searching { .. } => Screen::Search,
        Activity::Thinking => Screen::Thinking,
        Activity::Waiting { .. } => Screen::Waiting,
        _ => Screen::Quiet,
    }
}
fn rect(x: usize, y: usize, w: usize, h: usize) -> PixelRect {
    PixelRect {
        x,
        y,
        width: w,
        height: h,
    }
}
fn laptop(key: RoomKey) -> bool {
    key.meeting || key.zones[1] % 3 == 2
}
pub(super) fn geometry(key: RoomKey, x: usize) -> WorkstationLayout {
    let f = key.floor;
    if key.overview {
        WorkstationLayout {
            computer: if laptop(key) {
                rect(x + 26, f - 20, 16, 13)
            } else {
                rect(x + 26, f - 22, 16, 12)
            },
            keyboard: if laptop(key) {
                rect(x + 16, f - 9, 24, 3)
            } else {
                rect(x + 15, f - 9, 14, 3)
            },
            chair: rect(x + 6, f - 18, 18, 19),
            selection: rect(x, f - 31, 44, 32),
            actor_home: rect(x + 2, f - 31, 24, 32),
        }
    } else {
        WorkstationLayout {
            computer: if laptop(key) {
                rect(x + 52, f - 37, 28, 23)
            } else {
                rect(x + 52, f - 42, 28, 20)
            },
            keyboard: if laptop(key) {
                rect(x + 33, f - 19, 42, 5)
            } else {
                rect(x + 31, f - 19, 28, 4)
            },
            chair: rect(x + 12, f - 34, 36, 35),
            selection: rect(x, f - 63, 84, 64),
            actor_home: rect(x + 4, f - 63, 48, 64),
        }
    }
}
/// Chair backs, seat pans and casters are composed before the actor.
pub(super) fn chair(a: &mut Raster, key: RoomKey, x: usize) {
    let r = geometry(key, x).chair;
    let (x, y, w, h) = (r.x as i32, r.y as i32, r.width as i32, r.height as i32);
    let small = key.overview;
    let ink = rgb(27, 40, 50);
    let materials = super::materials::for_room(key);
    let upholstery = materials.upholstery;
    let seat = if small { 5 } else { 9 };
    a.ellipse(x - 2, y + h - 3, w + 4, 4, darken(materials.floor, 14));
    a.rect(x + 2, y, w - 4, h - seat - 1, ink);
    a.rect(x + 3, y + 1, w - 6, h - seat - 3, upholstery);
    a.rect(x + 4, y + 2, w - 8, 1, lighten(upholstery, 25));
    for row in (y + 4..y + h - seat - 3).step_by(3) {
        a.rect(x + 4, row, w - 8, 1, darken(upholstery, 9));
    }
    a.rect(x, y + h - seat, w, if small { 3 } else { 4 }, ink);
    a.rect(x + 1, y + h - seat, w - 2, 1, lighten(upholstery, 22));
    a.rect(
        x + w / 2 - 1,
        y + h - seat + 2,
        2,
        seat - 3,
        materials.metal,
    );
    a.rect(x + 2, y + h - 2, w - 4, 1, ink);
    for cx in [x + 2, x + w - 5] {
        a.rect(cx, y + h - 2, 3, 2, ink);
    }
}
/// The desk plane, then a complete computer, then hands drawn by the caller.
pub(super) fn furniture(
    a: &mut Raster,
    key: RoomKey,
    x: usize,
    selected: bool,
    motif: Screen,
    phase: u8,
) {
    let f = key.floor as i32;
    let x = x as i32;
    let small = key.overview;
    let sw = key.slot as i32;
    let materials = super::materials::for_room(key);
    let edge = materials.desk_edge;
    let y = f - if small { 7 } else { 16 };
    let th = if small { 2 } else { 3 };
    let inset = if key.meeting && key.zones[2].is_multiple_of(3) && x == key.first_x as i32 {
        2
    } else {
        0
    };
    let end = if key.meeting
        && key.zones[2].is_multiple_of(3)
        && x == key.first_x as i32 + (key.seats.saturating_sub(1) * key.slot) as i32
    {
        2
    } else {
        0
    };
    let width = if key.meeting {
        sw - inset - end
    } else {
        sw - 4
    };
    a.rect(x + inset, y, width, th, edge);
    a.rect(x + inset, y, width, 1, lighten(materials.desktop, 22));
    a.rect(x + inset, y + th - 1, width, 1, darken(edge, 18));
    for leg in [x + 5, x + sw - 10] {
        a.rect(leg, y + th, 2, f - y - th, materials.metal);
        a.rect(leg, f - 1, 5, 2, darken(materials.metal, 18));
    }
    if key.zones[1] % 3 == 1 {
        let dw = if small { 9 } else { 17 };
        a.rect(x + 2, y + th, dw, f - y - th, materials.storage);
        a.rect(x + 3, y + th + 1, dw - 2, 1, lighten(materials.storage, 28));
        a.rect(x + dw / 2, y + th + 3, 2, 1, rgb(37, 53, 62));
    }
    let g = geometry(key, x as usize);
    let c = g.computer;
    let (cx, cy, cw, ch) = (c.x as i32, c.y as i32, c.width as i32, c.height as i32);
    let portable = laptop(key);
    let variant = key.zones[1] % 3;
    let shell = materials.bezel;
    let ink = rgb(27, 40, 52);
    let screen_rect;
    if portable {
        let lid_h = ch - if small { 3 } else { 6 };
        a.rect(cx, cy, cw, lid_h, ink);
        a.rect(
            cx + 1,
            cy + 1,
            cw - 2,
            lid_h - 2,
            if variant == 1 {
                materials.equipment
            } else {
                shell
            },
        );
        screen_rect = rect(c.x + 2, c.y + 2, c.width - 4, lid_h as usize - 4);
        if variant == 2 {
            a.rect(cx + 1, cy, cw - 2, 1, materials.equipment);
        }
        // The base meets the hinge and projects toward the working hands.
        let k = g.keyboard;
        let (kx, ky, kw, kh) = (k.x as i32, k.y as i32, k.width as i32, k.height as i32);
        a.poly(
            &[
                (cx, cy + lid_h - 1),
                (cx + cw - 1, cy + lid_h - 1),
                (kx + kw - 1, ky + kh),
                (kx, ky + kh),
                (kx + 2, ky),
            ],
            shell,
        );
        a.rect(kx, ky + kh - 1, kw, 1, ink);
    } else {
        a.rect(cx, cy, cw, ch, ink);
        a.rect(cx + 1, cy + 1, cw - 2, ch - 2, shell);
        screen_rect = rect(
            c.x + 2,
            c.y + 2,
            c.width - 4,
            c.height - if small { 5 } else { 6 },
        );
        let stand_w = if small { 2 } else { 3 };
        a.rect(cx + cw / 2 - 1, cy + ch, stand_w, y - (cy + ch), ink);
        a.rect(
            cx + cw / 2,
            cy + ch,
            1,
            y - (cy + ch),
            lighten(materials.metal, 16),
        );
        let bw = if small { 9 } else { 16 };
        a.rect(cx + cw / 2 - bw / 2, y - 1, bw, 2, ink);
        a.rect(cx + cw / 2 - bw / 2 + 1, y - 1, bw - 2, 1, materials.metal);
        a.rect(
            cx + cw - 4,
            cy + ch - 2,
            1,
            1,
            if motif == Screen::Unknown {
                rgb(90, 109, 114)
            } else {
                rgb(164, 198, 155)
            },
        );
    }
    // A small colored bezel edge belongs to the computer case, not its state.
    a.rect(cx + 1, cy + 1, cw - 2, 1, materials.equipment);
    screen_art(a, screen_rect, motif, phase);
    let k = g.keyboard;
    let (kx, ky, kw, kh) = (k.x as i32, k.y as i32, k.width as i32, k.height as i32);
    a.rect(kx, ky, kw, kh, ink);
    a.rect(
        kx + 1,
        ky + 1,
        kw - 2,
        kh - 2,
        darken(materials.equipment, 17),
    );
    for xx in (kx + 2..kx + kw - 2).step_by(if small { 3 } else { 4 }) {
        a.rect(xx, ky + 1, if small { 1 } else { 2 }, 1, materials.bezel);
    }
    if !portable {
        a.rect(x + sw - 7, y - 3, 3, 3, ink);
        a.rect(x + sw - 6, y - 3, 1, 2, materials.prop);
    }
    if key.meeting && key.zones[2] % 3 == 2 {
        a.rect(x + 2, y - 4, 3, 4, materials.prop);
        a.rect(x + 3, y - 4, 2, 1, rgb(226, 220, 193));
    }
    let py = key.plate_y() as i32;
    a.rect(x, py, sw - 4, key.plate as i32, rgb(34, 48, 59));
    if selected {
        a.rect(x, py, 1, key.plate as i32, rgb(171, 215, 195));
    }
}
fn screen_art(target: &mut Raster, r: PixelRect, motif: Screen, phase: u8) {
    // The overview screen is independently small: clip every motif to its glass.
    let mut clipped = Raster::new(r.width, r.height);
    screen_motif(&mut clipped, motif, phase);
    for (index, color) in clipped.pixels.into_iter().enumerate() {
        if let Some(color) = color {
            target.set(
                (r.x + index % r.width) as i32,
                (r.y + index / r.width) as i32,
                color,
            );
        }
    }
}
fn screen_motif(a: &mut Raster, motif: Screen, phase: u8) {
    let (x, y, w, h) = (0, 0, a.width as i32, a.height as i32);
    a.rect(x, y, w, h, rgb(40, 67, 83));
    let quiet = rgb(89, 123, 136);
    let line = rgb(179, 216, 197);
    let cool = rgb(119, 169, 183);
    a.rect(x, y, w, 1, quiet);
    match motif {
        Screen::Code => {
            a.rect(x + 2, y + 3, 1, h - 4, quiet);
            for n in 0..3 {
                a.rect(
                    x + 4 + (n % 2),
                    y + 3 + n * 3,
                    (w - 6 - (n % 2) * 3).max(1),
                    1,
                    if n == 1 { cool } else { line },
                );
            }
        }
        Screen::Terminal => {
            a.rect(x + 2, y + 3, 2, 1, line);
            a.rect(x + 4, y + 4, 2, 1, line);
            a.rect(x + 2, y + 5, 2, 1, line);
            if phase % 4 < 2 {
                a.rect(x + 7, y + (h - 3).max(3), 2, 1, line);
            }
        }
        Screen::Document => {
            a.rect(x + w / 3, y + 2, (w / 2).max(3), h - 3, line);
            for n in 0..2 {
                a.rect(x + w / 3 + 1, y + 3 + n * 3, (w / 2 - 2).max(1), 1, quiet);
            }
        }
        Screen::Search => {
            a.rect(x + 2, y + 2, w - 4, 3, cool);
            a.rect(x + 3, y + 3, w - 7, 1, rgb(45, 73, 89));
            a.rect(x + 3, y + 6, (w - 6).max(2), 1, line);
        }
        Screen::Thinking | Screen::Waiting => {
            for n in 0..3 {
                a.rect(
                    x + w / 2 - 4 + n * 3,
                    y + h / 2,
                    1,
                    1,
                    if motif == Screen::Thinking && phase % 3 == n as u8 {
                        line
                    } else {
                        quiet
                    },
                );
            }
        }
        Screen::Human => {
            a.rect(x + 2, y + 2, w - 4, h - 4, quiet);
            a.rect(x + w / 2 - 1, y + 3, 2, (h - 6).max(1), rgb(241, 196, 120));
            a.rect(x + w / 2 - 1, y + h - 2, 2, 1, rgb(241, 196, 120));
        }
        Screen::Error => {
            for n in 0..(h - 3).min(w - 4) {
                a.rect(x + 2 + n, y + 2 + n, 1, 1, rgb(224, 141, 125));
                a.rect(x + 2 + n, y + h - 2 - n, 1, 1, rgb(224, 141, 125));
            }
        }
        Screen::Quiet => {
            a.rect(x + 2, y + h - 3, (w - 4).max(1), 1, quiet);
        }
        Screen::Unknown => {
            a.rect(x + 2, y + 2, 2, 1, quiet);
            a.rect(x + w - 4, y + h - 2, 2, 1, quiet);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::living_office::art::{self, Character, Facing, Pose};
    use crate::living_office::overview;
    fn key(small: bool, meeting: bool, variant: u8) -> RoomKey {
        RoomKey {
            width: 320,
            height: if small { 64 } else { 152 },
            palette: 0,
            light: false,
            seats: 3,
            meeting,
            elevator: true,
            overview: small,
            preset: crate::design::OfficePreset::Studio,
            zones: [0, variant, 0, 0],
            floor: if small { 52 } else { 112 },
            shaft: if small { 36 } else { 52 },
            slot: if small { 44 } else { 84 },
            title: 8,
            plate: if small { 8 } else { 16 },
            first_x: 60,
        }
    }
    #[test]
    fn office_palettes_recolor_equipment_and_chairs_without_changing_any_hit_anchor() {
        use crate::design::OfficePreset;
        use std::collections::HashSet;

        for preset in [
            OfficePreset::Studio,
            OfficePreset::Workshop,
            OfficePreset::Laboratory,
        ] {
            for light in [false, true] {
                for small in [false, true] {
                    for meeting in [false, true] {
                        for variant in 0..3 {
                            let base = RoomKey {
                                preset,
                                light,
                                ..key(small, meeting, variant)
                            };
                            let station = geometry(base, base.first_x);
                            let mut chairs = HashSet::new();
                            let mut computers = HashSet::new();
                            let mut edges = HashSet::new();
                            let mut masks = HashSet::new();
                            for palette in 0..4 {
                                let key = RoomKey { palette, ..base };
                                assert_eq!(geometry(key, key.first_x), station);
                                let mut a = Raster::new(key.width, key.height);
                                chair(&mut a, key, key.first_x);
                                furniture(&mut a, key, key.first_x, true, Screen::Code, 0);
                                let crop = |r: PixelRect| {
                                    (r.y..r.y + r.height)
                                        .flat_map(|y| (r.x..r.x + r.width).map(move |x| (x, y)))
                                        .map(|(x, y)| a.pixels[y * a.width + x])
                                        .collect::<Vec<_>>()
                                };
                                chairs.insert(crop(station.chair));
                                computers.insert(crop(station.computer));
                                edges.insert(crop(rect(
                                    key.first_x,
                                    key.floor - if small { 7 } else { 16 },
                                    key.slot - 4,
                                    if small { 3 } else { 4 },
                                )));
                                masks.insert(
                                    a.pixels.iter().map(Option::is_some).collect::<Vec<_>>(),
                                );
                            }
                            assert_eq!((chairs.len(), computers.len(), edges.len()), (4, 4, 4));
                            assert_eq!(masks.len(), 1, "materials must not change drawn geometry");
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn every_screen_motif_is_clipped_to_the_glass_at_both_authored_sizes() {
        for (w, h) in [(12, 7), (13, 6), (24, 14), (28, 13)] {
            for motif in [
                Screen::Code,
                Screen::Terminal,
                Screen::Document,
                Screen::Search,
                Screen::Thinking,
                Screen::Waiting,
                Screen::Human,
                Screen::Quiet,
                Screen::Error,
                Screen::Unknown,
            ] {
                for phase in 0..8 {
                    let mut a = Raster::new(40, 30);
                    let r = rect(5, 6, w, h);
                    screen_art(&mut a, r, motif, phase);
                    for (i, pixel) in a.pixels.iter().enumerate() {
                        let (x, y) = (i % a.width, i / a.width);
                        assert_eq!(
                            pixel.is_some(),
                            x >= r.x && x < r.x + w && y >= r.y && y < r.y + h,
                            "{motif:?} ({x},{y})"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn faces_and_raised_hands_are_not_occluded_and_working_hands_reach_keys() {
        for small in [false, true] {
            for meeting in [false, true] {
                for variant in 0..3 {
                    let key = key(small, meeting, variant);
                    let station = geometry(key, 60);
                    let mut foreground = Raster::new(key.width, key.height);
                    furniture(&mut foreground, key, 60, false, Screen::Code, 0);
                    let foreground = foreground.sprite();
                    for costume in 0..12 {
                        for pose in [Pose::Work, Pose::Waiting] {
                            for phase in 0..2 {
                                let c = Character {
                                    costume,
                                    skin: costume % 6,
                                    hair: costume % 6,
                                };
                                let facing = if pose == Pose::Waiting {
                                    Facing::Front
                                } else {
                                    Facing::Right
                                };
                                let actor = if small {
                                    overview::character_facing(c, pose, phase, facing)
                                } else {
                                    art::character_facing(c, pose, phase, facing)
                                };
                                // The full upper silhouette includes hats and the raised hand.
                                for y in 0..if small { 17 } else { 34 } {
                                    for x in 0..actor.width() {
                                        if actor.pixel(x, y).is_some() {
                                            assert!(foreground.pixel(station.actor_home.x+x,station.actor_home.y+y).is_none(),"small={small} meeting={meeting} variant={variant} costume={costume} pose={pose:?} x={x} y={y}");
                                        }
                                    }
                                }
                                if pose == Pose::Work {
                                    let hands =
                                        art::gesture_layer(&actor, pose, facing, small).unwrap();
                                    let k = station.keyboard;
                                    assert!((k.y..k.y+k.height).any(|y|(k.x..k.x+k.width).any(|x|hands.pixel(x.saturating_sub(station.actor_home.x),y.saturating_sub(station.actor_home.y)).is_some())),"hands must touch keys at each typing phase: {small} {costume} {phase}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn desktop_stem_reaches_the_table_and_team_variants_change_the_equipment() {
        for small in [false, true] {
            let k = key(small, false, 0);
            let g = geometry(k, 60);
            let mut a = Raster::new(k.width, k.height);
            furniture(&mut a, k, 60, false, Screen::Code, 0);
            let s = a.sprite();
            let stem_x = g.computer.x + g.computer.width / 2;
            for y in g.computer.y + g.computer.height..k.floor - if small { 6 } else { 15 } {
                assert!(
                    s.pixel(stem_x, y).is_some(),
                    "monitor must be visibly supported"
                );
            }
            let mut variants = std::collections::HashSet::new();
            for variant in 0..3 {
                let k = key(small, true, variant);
                let mut a = Raster::new(k.width, k.height);
                furniture(&mut a, k, 60, false, Screen::Code, 0);
                variants.insert(a.pixels);
            }
            assert_eq!(
                variants.len(),
                3,
                "Desk choices remain visible at a shared table"
            );
        }
    }
}
