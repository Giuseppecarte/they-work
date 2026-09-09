//! Explicit local control. Reading a transcript never grants control authority.
//! The detached host owns its provider connection; dropping a client only
//! detaches the view. Neither startup nor recovery replays a user instruction.

mod bridge;
mod client;
mod model;
pub mod native;
mod rpc;
mod security;
mod state_storage;
mod stream;
mod supervisor;

pub use bridge::{
    reconcile_snapshot, snapshot_events, BridgeBatch, BridgeCursor, DEFERRED_EVENT_BYTE_LIMIT,
    DEFERRED_EVENT_LIMIT, MISSING_RANGE_LIMIT,
};
pub use client::ControlClient;
pub use model::*;
pub use supervisor::{maybe_run_supervisor, run_supervisor};

pub fn operation_id() -> anyhow::Result<String> {
    security::random_token()
}
