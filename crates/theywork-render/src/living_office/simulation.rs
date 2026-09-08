//! Reproducible stage directions. This module never writes to a worker or
//! invents a provider event; real status is read only by the caller.

use crate::design::{profile_for, CharacterProfile, CharacterStyle};
use std::collections::BTreeMap;
use theywork_core::{Millis, Worker, WorkerId};

use super::art::{stable_hash, Pose};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoration {
    pub pose: Pose,
    /// Offset in authored pixels, along the clear aisle in front of the desk.
    pub x_offset: i32,
    pub walking: bool,
    /// Travel fraction, 0 at the desk and 1000 at the destination.
    pub progress: u16,
    pub returning: bool,
    pub destination: Destination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Coffee,
    Reading,
    Plant,
    PaperTray,
    StretchSpot,
}

/// Plan once per real office, then share this map with every visible room.
/// Splitting a floor cannot multiply the two-vignette budget.
pub fn plan(
    workers: &[&Worker],
    now: Millis,
    motion: bool,
    profiles: &BTreeMap<String, CharacterProfile>,
) -> BTreeMap<String, Decoration> {
    let mut result: BTreeMap<String, Decoration> = decorations(workers, now, motion)
        .into_iter()
        .map(|(id, mut decoration)| {
            let style = profile_for(&id.0, profiles).style;
            let choice =
                (stable_hash(&id.0).wrapping_add((now.max(0) / 24_000) as u64) % 2) as usize;
            let (destination, pose) = match style {
                CharacterStyle::Calm => [
                    (Destination::Reading, Pose::Read),
                    (Destination::StretchSpot, Pose::Stretch),
                ][choice],
                CharacterStyle::Curious => [
                    (Destination::Plant, Pose::WaterPlant),
                    (Destination::Reading, Pose::Read),
                ][choice],
                CharacterStyle::Energetic => [
                    (Destination::Coffee, Pose::Coffee),
                    (Destination::PaperTray, Pose::PaperPlane),
                ][choice],
            };
            decoration.destination = destination;
            if !decoration.walking {
                decoration.pose = pose;
            }
            (id.0, decoration)
        })
        .collect();
    // A floor has one left shelf and one right refreshment corner. Two people
    // do not perform the same gag on top of each other at a single facility.
    let mut occupied = Vec::new();
    for decoration in result.values_mut() {
        let mut side = match decoration.destination {
            Destination::Reading | Destination::PaperTray => 0,
            Destination::Coffee | Destination::Plant => 1,
            Destination::StretchSpot => 2,
        };
        if occupied.contains(&side) {
            side = [2, 0, 1]
                .into_iter()
                .find(|candidate| !occupied.contains(candidate))
                .expect("at most two vignettes share three facilities");
            let (destination, pose) = match side {
                0 => (Destination::Reading, Pose::Read),
                1 => (Destination::Coffee, Pose::Coffee),
                _ => (Destination::StretchSpot, Pose::Stretch),
            };
            decoration.destination = destination;
            if !decoration.walking {
                decoration.pose = pose;
            }
        }
        occupied.push(side);
    }
    result
}

/// At most two actors receive a decorative vignette per floor. Selection uses
/// sorted stable IDs; call order, rendering speed and page order have no effect.
pub fn decorations(workers: &[&Worker], now: Millis, motion: bool) -> Vec<(WorkerId, Decoration)> {
    if !motion || workers.is_empty() {
        return Vec::new();
    }
    let now = now.max(0) as u64;
    let cycle = now / 24_000;
    let elapsed = now % 24_000;
    if elapsed < 8_000 {
        return Vec::new();
    }
    let mut ids = workers
        .iter()
        .map(|worker| worker.id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    let start = cycle as usize % ids.len();
    (0..ids.len().min(2))
        .map(|slot| {
            let id = ids[(start + slot) % ids.len()].clone();
            let seed = stable_hash(&id.0).wrapping_add(cycle);
            let progress = (elapsed - 8_000) as i32;
            let travel = if progress < 3_000 {
                progress * 24 / 3_000
            } else if progress > 13_000 {
                (16_000 - progress) * 24 / 3_000
            } else {
                24
            };
            let walking = !(3_000..=13_000).contains(&progress);
            let pose = if walking {
                Pose::Walk
            } else {
                match seed % 5 {
                    0 => Pose::Coffee,
                    1 => Pose::Stretch,
                    2 => Pose::WaterPlant,
                    3 => Pose::PaperPlane,
                    _ => Pose::Read,
                }
            };
            (
                id,
                Decoration {
                    pose,
                    x_offset: travel * if slot % 2 == 0 { 1 } else { -1 },
                    walking,
                    progress: if progress < 3_000 {
                        (progress / 3) as u16
                    } else if progress > 13_000 {
                        ((16_000 - progress) / 3) as u16
                    } else {
                        1000
                    },
                    returning: progress > 13_000,
                    destination: match seed % 5 {
                        0 => Destination::Coffee,
                        1 => Destination::StretchSpot,
                        2 => Destination::Plant,
                        3 => Destination::PaperTray,
                        _ => Destination::Reading,
                    },
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use theywork_core::{Agent, OfficeId};
    #[test]
    fn shared_facilities_do_not_receive_two_simultaneous_actors() {
        let workers = (0..8)
            .map(|i| {
                Worker::new(
                    WorkerId(format!("person-{i}")),
                    OfficeId("floor".into()),
                    Agent::Codex,
                    "Task".into(),
                    0,
                )
            })
            .collect::<Vec<_>>();
        let refs = workers.iter().collect::<Vec<_>>();
        for style in [
            CharacterStyle::Calm,
            CharacterStyle::Curious,
            CharacterStyle::Energetic,
        ] {
            let profiles = workers
                .iter()
                .map(|w| {
                    (
                        w.id.0.clone(),
                        CharacterProfile {
                            name: String::new(),
                            style,
                        },
                    )
                })
                .collect();
            for cycle in 0..100 {
                let planned = plan(&refs, cycle * 24_000 + 14_000, true, &profiles);
                let mut sides = std::collections::BTreeSet::new();
                for decoration in planned.values() {
                    let side = match decoration.destination {
                        Destination::Reading | Destination::PaperTray => 0,
                        Destination::Coffee | Destination::Plant => 1,
                        Destination::StretchSpot => 2,
                    };
                    assert!(
                        sides.insert(side),
                        "two actors occupy one facility at cycle {cycle}"
                    );
                }
            }
        }
    }

    #[test]
    fn two_gags_max_and_reduced_motion_has_none() {
        let workers = (0..20)
            .map(|id| {
                Worker::new(
                    WorkerId(id.to_string()),
                    OfficeId("floor".into()),
                    Agent::Codex,
                    "Task".into(),
                    0,
                )
            })
            .collect::<Vec<_>>();
        let workers = workers.iter().collect::<Vec<_>>();
        for now in (0..100_000).step_by(137) {
            assert!(decorations(&workers, now, true).len() <= 2);
            assert!(decorations(&workers, now, false).is_empty());
        }
    }
    #[test]
    fn skipped_frames_and_reordering_do_not_change_simulation() {
        let workers = (0..5)
            .map(|id| {
                Worker::new(
                    WorkerId(id.to_string()),
                    OfficeId("floor".into()),
                    Agent::Codex,
                    "Task".into(),
                    0,
                )
            })
            .collect::<Vec<_>>();
        let mut refs = workers.iter().collect::<Vec<_>>();
        let expected = decorations(&refs, 14_200, true);
        for time in [0, 83, 151, 8_000, 9_433, 14_000] {
            let _ = decorations(&refs, time, true);
        }
        refs.reverse();
        assert_eq!(decorations(&refs, 14_200, true), expected);
    }
}
