use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::Millis;

/// Which coding agent a worker belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub fn label(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }
}

/// Identifies an office. This is the absolute path of the project directory
/// the agent is working in, which is the one thing both agents agree on.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OfficeId(pub String);

/// Identifies a single worker: one Claude session or one Codex thread.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkerId(pub String);

/// What a worker is doing right now.
///
/// The renderer maps each variant to a sprite and an animation, so adding a
/// variant here is a deliberate, coordinated change to both sides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activity {
    /// Running a shell command. `detail` is the command, already truncated.
    Typing { detail: String },
    /// Reading a file. `detail` is a display path.
    Reading { detail: String },
    /// Editing or writing a file. `detail` is a display path.
    Editing { detail: String },
    /// Searching the codebase or the web.
    Searching { detail: String },
    /// Model is reasoning, no tool in flight.
    Thinking,
    /// Produced prose for the human.
    Talking { detail: String },
    /// Blocked on a human approving something.
    Waiting { detail: String },
    /// Alive but quiet.
    Idle,
    /// Something failed.
    Error { detail: String },
}

impl Activity {
    /// Short human label, e.g. for the desk view and the camera-grid caption.
    pub fn label(&self) -> &'static str {
        match self {
            Activity::Typing { .. } => "typing",
            Activity::Reading { .. } => "reading",
            Activity::Editing { .. } => "editing",
            Activity::Searching { .. } => "searching",
            Activity::Thinking => "thinking",
            Activity::Talking { .. } => "talking",
            Activity::Waiting { .. } => "waiting",
            Activity::Idle => "idle",
            Activity::Error { .. } => "error",
        }
    }

    /// The free-text detail, if this activity carries one.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Activity::Typing { detail }
            | Activity::Reading { detail }
            | Activity::Editing { detail }
            | Activity::Searching { detail }
            | Activity::Talking { detail }
            | Activity::Waiting { detail }
            | Activity::Error { detail } => Some(detail),
            Activity::Thinking | Activity::Idle => None,
        }
    }

    /// Whether this counts as actively working, for the "busy desks" tally.
    pub fn is_busy(&self) -> bool {
        !matches!(self, Activity::Idle | Activity::Error { .. })
    }
}

/// What a manager would say about a worker if you asked how they were doing.
///
/// Derived from activity and timing rather than stored, so it can never drift
/// out of sync with what the office is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerStatus {
    /// A turn is open and the worker is producing output.
    Running,
    /// No turn open. Finished, and ready to be given something new.
    Idle,
    /// A turn is open but nothing has come out of it for a long time. Almost
    /// always a command waiting on a human to approve it, or a question asked
    /// and never answered. These are the ones worth interrupting your day for.
    Blocked,
    /// Something failed.
    Failed,
}

impl WorkerStatus {
    pub fn label(self) -> &'static str {
        match self {
            WorkerStatus::Running => "running",
            WorkerStatus::Idle => "idle",
            WorkerStatus::Blocked => "blocked",
            WorkerStatus::Failed => "failed",
        }
    }

    /// Whether a human needs to do something before this worker can continue.
    pub fn needs_attention(self) -> bool {
        matches!(self, WorkerStatus::Blocked | WorkerStatus::Failed)
    }
}

/// How something a worker did turned out.
///
/// Separate from [`Activity`] because an activity is a state you can be in,
/// while an outcome is news that arrives once and then stops changing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// A command finished with this exit status.
    Exited(i32),
    /// A file was edited by this many lines.
    Changed { added: u32, removed: u32 },
}

/// One thing a worker did, at the time they did it.
///
/// The desk view reads a worker's recent beats as a timeline, which is why the
/// timestamp comes from the agent's own record rather than from when we noticed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Beat {
    pub at: Millis,
    pub activity: Activity,
    pub outcome: Option<Outcome>,
}

/// One agent thread, drawn as one employee at one desk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    pub id: WorkerId,
    pub office: OfficeId,
    pub agent: Agent,
    /// Display name. Codex thread titles and Claude session titles land here;
    /// collectors fall back to a short id when there is no better name.
    pub name: String,
    pub activity: Activity,
    pub git_branch: Option<String>,
    pub tokens_used: u64,
    /// When we last saw any sign of life from this worker.
    pub last_seen: Millis,
    /// Whether the agent reports a turn actively in flight.
    pub turn_in_flight: bool,
    /// What this worker has been doing lately, oldest first.
    ///
    /// Bounded: a thread that has run for hours must not cost more to remember
    /// than one that just started.
    pub history: VecDeque<Beat>,
}

impl Worker {
    pub fn new(id: WorkerId, office: OfficeId, agent: Agent, name: String, at: Millis) -> Self {
        Self {
            id,
            office,
            agent,
            name,
            activity: Activity::Idle,
            git_branch: None,
            tokens_used: 0,
            last_seen: at,
            turn_in_flight: false,
            history: VecDeque::new(),
        }
    }

    /// How this worker is doing, in the sense a manager would mean it.
    pub fn status_at(&self, now: Millis) -> WorkerStatus {
        if matches!(self.activity, Activity::Error { .. }) {
            return WorkerStatus::Failed;
        }
        if !self.turn_in_flight {
            return WorkerStatus::Idle;
        }
        // An open turn that has gone silent is not working, it is waiting.
        if now - self.last_seen > crate::BLOCKED_AFTER_MS {
            return WorkerStatus::Blocked;
        }
        WorkerStatus::Running
    }

    /// Record something this worker did, dropping the oldest beat when full.
    pub fn remember(&mut self, beat: Beat) {
        if self.history.len() >= crate::HISTORY_LEN {
            self.history.pop_front();
        }
        self.history.push_back(beat);
    }

    /// The most recent beats, newest last.
    pub fn recent(&self) -> impl Iterator<Item = &Beat> {
        self.history.iter()
    }

    /// Quiet for long enough that we should stop animating them.
    pub fn is_idle_at(&self, now: Millis) -> bool {
        now - self.last_seen > crate::IDLE_AFTER_MS
    }

    /// Quiet for long enough that they should leave the building.
    pub fn is_offline_at(&self, now: Millis) -> bool {
        now - self.last_seen > crate::OFFLINE_AFTER_MS
    }
}

/// One project directory, drawn as one office floor / one camera feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Office {
    pub id: OfficeId,
    /// Absolute path of the project directory.
    pub path: String,
    /// Last path segment, used as the floor's name on the sign.
    pub name: String,
    pub workers: Vec<Worker>,
}

impl Office {
    pub fn new(id: OfficeId, path: String) -> Self {
        let name = path
            .rsplit('/')
            .find(|s| !s.is_empty())
            .unwrap_or("office")
            .to_string();
        Self {
            id,
            path,
            name,
            workers: Vec::new(),
        }
    }

    pub fn busy_count(&self) -> usize {
        self.workers.iter().filter(|w| w.activity.is_busy()).count()
    }
}
