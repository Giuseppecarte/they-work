//! The shared vocabulary of they-work.
//!
//! Everything else in the workspace depends on this crate and on nothing of
//! each other. Collectors turn agent-specific trails into [`Event`]s; the
//! renderer draws a [`World`]. Neither needs to know the other exists.
//!
//! This crate is deliberately I/O-free and dependency-light so it can stay
//! stable while the crates around it churn.

pub mod demo;
mod event;
mod model;
mod source;
mod world;

pub use event::{Event, EventKind};
pub use model::{
    Activity, Agent, Beat, Office, OfficeId, Outcome, Worker, WorkerId, WorkerStatus,
};
pub use source::{Source, SourceError};
pub use world::World;

/// How many beats of a worker's history the desk view can show.
///
/// Enough to read the shape of a turn, few enough that a day-old thread costs
/// no more to remember than a fresh one.
pub const HISTORY_LEN: usize = 64;

/// Milliseconds since the Unix epoch.
///
/// Every timestamp in they-work is this. Collectors normalise into it, the
/// renderer animates against it, and tests can hand-pick values.
pub type Millis = i64;

/// How long a worker may go quiet before we consider them idle rather than busy.
pub const IDLE_AFTER_MS: Millis = 30_000;

/// How long a turn may stay open with nothing coming out of it before we stop
/// calling it work and start calling it a blockage.
///
/// Generous on purpose: agents genuinely think for a while, and crying wolf
/// about a blocked worker is worse than noticing one a minute late.
pub const BLOCKED_AFTER_MS: Millis = 180_000;

/// How long a worker may go quiet before they disappear from the office.
///
/// Deliberately long. A developer who finished an hour ago and is waiting to be
/// given something else is idle, not gone, and "who is free right now" is the
/// most useful thing this program knows. Sending them home after a few quiet
/// minutes threw that away.
///
/// Collectors own the roster: they bound it by recency and emit `Left` when a
/// session really ends. This is only a backstop for a source that cannot say so.
pub const OFFLINE_AFTER_MS: Millis = 12 * 60 * 60_000;
