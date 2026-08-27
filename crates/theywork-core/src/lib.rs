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
pub use model::{Activity, Agent, Office, OfficeId, Worker, WorkerId, WorkerStatus};
pub use source::{Source, SourceError};
pub use world::World;

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
pub const OFFLINE_AFTER_MS: Millis = 15 * 60_000;
