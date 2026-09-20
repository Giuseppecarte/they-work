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
        (rgb(91, 128, 127), rgb(171, 146, 121)),
        (rgb(93, 123, 157), rgb(177, 157, 135)),
        (rgb(115, 135, 105), rgb(168, 152, 124)),
        (rgb(133, 120, 145), rgb(176, 150, 146)),
    ][key.palette % 4];
    let (wall, floor, desktop) = match (key.preset, key.light) {
        (OfficePreset::Studio, true) => {
            (rgb(245, 244, 240), rgb(226, 214, 193), rgb(224, 204, 170))
        }
        (OfficePreset::Workshop, true) => {
            (rgb(239, 239, 235), rgb(217, 211, 201), rgb(219, 199, 164))
        }
        (OfficePreset::Laboratory, true) => {
            (rgb(242, 245, 245), rgb(218, 224, 223), rgb(227, 225, 216))
        }
        (OfficePreset::Studio, false) => (rgb(66, 68, 69), rgb(104, 96, 82), rgb(181, 161, 130)),
        (OfficePreset::Workshop, false) => (rgb(62, 65, 67), rgb(96, 93, 87), rgb(174, 155, 128)),
        (OfficePreset::Laboratory, false) => {
            (rgb(60, 67, 72), rgb(89, 98, 101), rgb(170, 181, 180))
        }
    };
    Materials {
        wall,
        floor,
        trim: if key.light {
            rgb(191, 195, 195)
        } else {
            rgb(47, 53, 58)
        },
        upholstery: mix(rgb(93, 101, 107), accent, 1, 3),
        desktop: mix(desktop, companion, 1, 10),
        desk_edge: mix(desktop, companion, 1, 4),
        storage: if key.light {
            rgb(221, 223, 219)
        } else {
            rgb(112, 119, 120)
        },
        equipment: mix(rgb(158, 167, 173), accent, 1, 5),
        bezel: mix(
            if key.light {
                rgb(207, 213, 215)
            } else {
                rgb(174, 183, 190)
            },
            accent,
            1,
            8,
        ),
        metal: if key.light {
            rgb(179, 187, 193)
        } else {
            rgb(137, 148, 158)
        },
        prop: companion,
        glazing: if key.light {
            rgb(190, 215, 220)
        } else {
            rgb(83, 111, 126)
        },
        supplies: [companion, accent, rgb(180, 183, 169), rgb(163, 167, 177)],
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
