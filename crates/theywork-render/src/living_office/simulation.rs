//! Reproducible stage directions. This module never writes to a worker or
//! invents a provider event; real status is read only by the caller.

use theywork_core::{Millis, Worker, WorkerId};

use super::art::{stable_hash, Pose};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoration {
    pub pose: Pose,
    /// Offset in authored pixels, along the clear aisle in front of the desk.
    pub x_offset: i32,
    pub walking: bool,
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
