use std::fmt;

use crate::event::Event;

/// Anything that can produce [`Event`]s: a Claude transcript tailer, a Codex
/// SQLite reader, or a fake source for tests and demos.
///
/// Polling rather than async on purpose. The TUI already has a frame loop, and
/// a poll-per-frame keeps the whole program single-threaded and easy to reason
/// about.
pub trait Source: Send {
    /// Stable name, shown in the status bar when a source is unhealthy.
    fn name(&self) -> &'static str;

    /// Return every event observed since the previous call.
    ///
    /// Must not block for long; the frame loop calls this on a timer. Returning
    /// an empty vec is the normal quiet case, not an error.
    fn poll(&mut self, now: crate::Millis) -> Result<Vec<Event>, SourceError>;
}

#[derive(Debug)]
pub struct SourceError {
    pub source_name: &'static str,
    pub message: String,
}

impl SourceError {
    pub fn new(source_name: &'static str, message: impl Into<String>) -> Self {
        Self { source_name, message: message.into() }
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.source_name, self.message)
    }
}

impl std::error::Error for SourceError {}
