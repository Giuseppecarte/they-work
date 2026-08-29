use serde::{Deserialize, Serialize};

use crate::model::{Activity, Agent, Beat, OfficeId, WorkerId};
use crate::Millis;

/// A single normalised observation from one of the agent trails.
///
/// Collectors emit these in roughly chronological order. [`crate::World`]
/// folds them into state; nothing downstream parses agent-specific formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub at: Millis,
    pub office: OfficeId,
    /// Absolute project path. Used to create the office on first sight.
    pub office_path: String,
    pub worker: WorkerId,
    pub agent: Agent,
    pub kind: EventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    /// A worker exists, with this display name. Safe to emit repeatedly.
    Seen {
        name: String,
        git_branch: Option<String>,
    },
    /// The worker changed what it is doing.
    ///
    /// Use [`EventKind::Did`] instead where the moment is worth remembering;
    /// this remains for activity that is only ever a current state.
    Acted(Activity),
    /// The worker did something worth keeping in their history.
    ///
    /// Sets the current activity *and* appends to the timeline, so a collector
    /// never has to emit the same moment twice.
    Did(Beat),
    /// Cumulative token count for this worker.
    Tokens(u64),
    /// A request/response turn started or finished.
    Turn { in_flight: bool },
    /// The worker's session ended.
    Left,
}
