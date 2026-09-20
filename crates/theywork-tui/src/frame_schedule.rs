//! Decide whether to build a frame before paying for rasterization or encoding.
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum GraphicsMode {
    #[default]
    Auto,
    Images,
    Cells,
}

impl GraphicsMode {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "images" => Ok(Self::Images),
            "cells" => Ok(Self::Cells),
            _ => Err("--graphics must be auto, images, or cells".into()),
        }
    }
}

pub(crate) struct FrameSchedule {
    pub(crate) dirty: bool,
    next: Instant,
    slow_frames: u8,
    mode: GraphicsMode,
}

impl FrameSchedule {
    pub(crate) fn new(mode: GraphicsMode, now: Instant) -> Self {
        Self {
            dirty: true,
            next: now,
            slow_frames: 0,
            mode,
        }
    }

    pub(crate) fn wait(&self, now: Instant) -> Duration {
        if self.dirty {
            Duration::from_millis(1)
        } else {
            self.next
                .saturating_duration_since(now)
                .clamp(Duration::from_millis(1), Duration::from_millis(20))
        }
    }

    pub(crate) fn due(&self, now: Instant) -> bool {
        self.dirty || now >= self.next
    }

    /// Returns true once automatic graphics should give way to cells.
    pub(crate) fn presented(
        &mut self,
        now: Instant,
        cost: Duration,
        images: bool,
        animation: bool,
        image_deadline: Instant,
    ) -> bool {
        self.dirty = false;
        let interval = if !animation {
            Duration::from_secs(1)
        } else if images {
            Duration::from_millis(200)
        } else {
            Duration::from_millis(33)
        };
        self.next = now + interval;
        if images {
            self.next = self.next.max(image_deadline);
        }
        self.slow_frames = if images && cost > Duration::from_millis(75) {
            self.slow_frames.saturating_add(1)
        } else {
            0
        };
        if self.mode == GraphicsMode::Auto && self.slow_frames >= 3 {
            self.mode = GraphicsMode::Cells;
            self.dirty = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_waits_before_building_but_input_bypasses_deadlines() {
        let now = Instant::now();
        let mut schedule = FrameSchedule::new(GraphicsMode::Auto, now);
        schedule.presented(
            now,
            Duration::from_millis(20),
            true,
            true,
            now + Duration::from_secs(2),
        );
        assert!(!schedule.due(now + Duration::from_secs(1)));
        schedule.dirty = true;
        assert!(schedule.due(now));
        assert_eq!(schedule.wait(now), Duration::from_millis(1));
    }

    #[test]
    fn only_three_consecutive_slow_automatic_image_frames_trigger_fallback() {
        let now = Instant::now();
        for mode in [
            GraphicsMode::Auto,
            GraphicsMode::Images,
            GraphicsMode::Cells,
        ] {
            let mut s = FrameSchedule::new(mode, now);
            let slow = Duration::from_millis(76);
            assert!(!s.presented(now, slow, true, true, now));
            assert!(!s.presented(now, slow, true, true, now));
            assert!(!s.presented(now, Duration::from_millis(75), true, true, now));
            assert!(!s.presented(now, slow, true, true, now));
            assert!(!s.presented(now, slow, true, true, now));
            assert_eq!(
                s.presented(now, slow, true, true, now),
                mode == GraphicsMode::Auto
            );
            assert!(!s.presented(now, slow, false, true, now));
        }
    }
}
