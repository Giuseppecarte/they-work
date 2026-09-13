//! Decorative material recipes shared by architecture and complete workstations.
//! These colors never encode an observation, request, result or selection state.
use super::{art::rgb, RoomKey};
use crate::design::OfficePreset;
use ratatui::style::Color;

#[derive(Clone, Copy)]
pub(super) struct Materials {
    pub wall: Color,
    pub floor: Color,
    pub trim: Color,
    pub upholstery: Color,
    pub desktop: Color,
    pub desk_edge: Color,
    pub storage: Color,
    pub equipment: Color,
    pub bezel: Color,
    pub metal: Color,
    pub prop: Color,
    pub glazing: Color,
    pub supplies: [Color; 4],
}

pub(super) fn for_room(key: RoomKey) -> Materials {
    // Indices retain their saved meaning: teal, blue, green and violet families.
    // Smaller surfaces carry saturation, leaving walls and flooring quieter.
    let (accent, companion) = [
        (rgb(54, 168, 171), rgb(229, 139, 114)),
        (rgb(93, 151, 212), rgb(225, 157, 119)),
        (rgb(132, 165, 87), rgb(221, 172, 88)),
        (rgb(162, 127, 197), rgb(214, 141, 177)),
    ][key.palette % 4];
    let (wall, floor, desktop, storage, glazing, upholstery) = match key.preset {
        OfficePreset::Studio => (
            if key.light {
                rgb(240, 210, 180)
            } else {
                rgb(155, 116, 98)
            },
            if key.light {
                rgb(206, 164, 113)
            } else {
                rgb(145, 105, 69)
            },
            rgb(220, 170, 103),
            mix(rgb(178, 115, 81), companion, 1, 3),
            rgb(77, 122, 138),
            accent,
        ),
        OfficePreset::Workshop => (
            if key.light {
                rgb(230, 215, 195)
            } else {
                rgb(139, 125, 116)
            },
            if key.light {
                rgb(184, 162, 135)
            } else {
                rgb(117, 99, 84)
            },
            rgb(194, 164, 122),
            mix(rgb(114, 77, 128), accent, 1, 5),
            rgb(91, 131, 145),
            mix(accent, rgb(113, 84, 145), 1, 4),
        ),
        OfficePreset::Laboratory => (
            if key.light {
                rgb(213, 234, 240)
            } else {
                rgb(101, 138, 156)
            },
            if key.light {
                rgb(184, 211, 209)
            } else {
                rgb(104, 135, 140)
            },
            rgb(193, 220, 210),
            mix(rgb(175, 192, 210), accent, 1, 4),
            rgb(86, 129, 143),
            [
                rgb(178, 146, 207),
                rgb(115, 165, 209),
                rgb(139, 180, 154),
                rgb(208, 147, 187),
            ][key.palette % 4],
        ),
    };
    Materials {
        wall,
        floor,
        // Tinted joinery, with enough dark structure to separate adjoining floors.
        trim: mix(rgb(31, 48, 67), accent, 1, 3),
        upholstery,
        desktop,
        desk_edge: mix(desktop, accent, 1, 3),
        storage,
        equipment: mix(accent, rgb(38, 130, 144), 1, 4),
        bezel: if key.light {
            rgb(234, 237, 230)
        } else {
            rgb(215, 224, 219)
        },
        metal: rgb(166, 190, 202),
        prop: companion,
        glazing,
        supplies: [companion, accent, rgb(233, 189, 100), rgb(172, 143, 199)],
    }
}

fn mix(base: Color, tint: Color, amount: u16, parts: u16) -> Color {
    let (Color::Rgb(r, g, b), Color::Rgb(tr, tg, tb)) = (base, tint) else {
        unreachable!("authored materials use RGB colors")
    };
    let channel =
        |a: u8, z: u8| ((u16::from(a) * (parts - amount) + u16::from(z) * amount) / parts) as u8;
    rgb(channel(r, tr), channel(g, tg), channel(b, tb))
}
