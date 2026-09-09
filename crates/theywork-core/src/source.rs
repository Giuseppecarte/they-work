use std::fmt;

use crate::event::Event;

/// Anything that can produce [`Event`]s: a Claude transcript tailer, a Codex
/// SQLite reader, or a fake source for tests and demos.
///
/// The host owns scheduling. The native host polls sources sequentially on a
/// background thread, sends the resulting batch to the UI, then waits one second
/// before the next traversal. Polling is independent of the render frame loop;
/// time spent reading sources adds to that interval, so one-second freshness is
/// not guaranteed. One-shot consumers may call a source directly.
pub trait Source: Send {
    /// Stable name, shown in the status bar when a source is unhealthy.
    fn name(&self) -> &'static str;

    /// Return every event observed since the previous call.
    ///
    /// Keep each call bounded: a slow source delays the other sources in that
    /// traversal and shutdown waits for an in-flight call to finish. Returning
    /// an empty vec is the normal quiet case, not an error. A source reports
    /// observations; it does not schedule UI frames or acquire control authority.
    fn poll(&mut self, now: crate::Millis) -> Result<Vec<Event>, SourceError>;
}

#[derive(Debug)]
pub struct SourceError {
    pub source_name: &'static str,
    pub message: String,
}

impl SourceError {
    pub fn new(source_name: &'static str, message: impl Into<String>) -> Self {
        Self {
            source_name,
            message: message.into(),
        }
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.source_name, self.message)
    }
}

impl std::error::Error for SourceError {}
