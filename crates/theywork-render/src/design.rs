//! Local presentation choices. None of these values are model instructions.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CharacterStyle {
    #[default]
    Calm,
    Curious,
    Energetic,
}
impl CharacterStyle {
    pub fn label(self) -> &'static str {
        match self {
            Self::Calm => "Serene",
            Self::Curious => "Curious",
            Self::Energetic => "Energetic",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Calm => Self::Curious,
            Self::Curious => Self::Energetic,
            Self::Energetic => Self::Calm,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterProfile {
    pub name: String,
    pub style: CharacterStyle,
}

fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

pub fn profile_for(id: &str, profiles: &BTreeMap<String, CharacterProfile>) -> CharacterProfile {
    profiles
        .get(id)
        .filter(|profile| !profile.name.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| {
            const NAMES: &[&str] = &[
                "Alex", "Sam", "Charlie", "Robin", "Dani", "Ari", "Nico", "Remy", "Jamie", "Kai",
                "Morgan", "Riley", "Sasha", "Noa", "Emery", "Jules", "Cameron", "Taylor", "Casey",
                "Quinn", "Rowan", "Sky", "Rene", "Blair",
            ];
            let hash = stable_hash(id);
            CharacterProfile {
                name: format!(
                    "{} {}.",
                    NAMES[(hash % NAMES.len() as u64) as usize],
                    (b'A' + ((hash >> 29) % 26) as u8) as char
                ),
                style: match (hash >> 17) % 3 {
                    0 => CharacterStyle::Calm,
                    1 => CharacterStyle::Curious,
                    _ => CharacterStyle::Energetic,
                },
            }
        })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OfficePreset {
    #[default]
    Studio,
    Workshop,
    Laboratory,
}
impl OfficePreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::Studio => "Open workspace",
            Self::Workshop => "Collaboration studio",
            Self::Laboratory => "Research lab",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Studio => Self::Workshop,
            Self::Workshop => Self::Laboratory,
            Self::Laboratory => Self::Studio,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct OfficeDesign {
    pub preset: OfficePreset,
    pub entrance: u8,
    pub desks: u8,
    pub meeting: u8,
    pub rest: u8,
}
pub fn office_design_for(id: &str, designs: &BTreeMap<String, OfficeDesign>) -> OfficeDesign {
    designs.get(id).cloned().unwrap_or_else(|| OfficeDesign {
        preset: match stable_hash(id) % 3 {
            0 => OfficePreset::Studio,
            1 => OfficePreset::Workshop,
            _ => OfficePreset::Laboratory,
        },
        ..Default::default()
    })
}
