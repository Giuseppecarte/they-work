//! Explicit local control. Reading a transcript never grants control authority.
//! The detached host owns its provider connection; dropping a client only
//! detaches the view. Neither startup nor recovery replays a user instruction.

mod bridge;
mod client;
mod model;
pub mod native;
mod rpc;
mod security;
mod supervisor;

pub use bridge::snapshot_events;
pub use client::ControlClient;
pub use model::*;
pub use supervisor::{maybe_run_supervisor, run_supervisor};

pub fn operation_id() -> anyhow::Result<String> {
    security::random_token()
}
