//! Provider-independent, evidence-backed relationships and collaboration.
//! A transcript can identify a family without identifying an immediate parent.

use serde::{Deserialize, Serialize};

use crate::{Agent, Millis, WorkerId};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceId(pub String);

/// Native identity is preserved verbatim for a future authenticated adapter.
/// It is never a display label, command, or proof that a session is controllable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadIdentity {
    pub provider: Agent,
    pub source: SourceId,
    pub native_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

impl ThreadIdentity {
    pub fn new(provider: Agent, source: SourceId, native_id: impl Into<String>) -> Self {
        Self {
            provider,
            source,
            native_id: native_id.into(),
            session_id: None,
        }
    }

    pub fn worker_id(&self) -> WorkerId {
        // Length framing avoids collisions involving separators or identical
        // IDs in different homes. A Claude agent ID is scoped to its session.
        let session = self.session_id.as_deref().unwrap_or("");
        WorkerId(format!(
            "{}:{}:{}:{}:{}:{}:{}",
            self.provider.label(),
            self.source.0.len(),
            self.source.0,
            session.len(),
            session,
            self.native_id.len(),
            self.native_id
        ))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerRole {
    #[default]
    Main,
    Subagent,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerLifecycle {
    #[default]
    Unknown,
    Active,
    Completed,
    Failed,
    Cancelled,
    /// Removed from the source's current roster; not proof of completion.
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaitReason {
    HumanApproval,
    HumanInput,
    AutomaticReview,
    Child,
    Process,
    Unknown,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoverageLevel {
    #[default]
    Unknown,
    /// The source format exposes only a subset; absence is not evidence of none.
    Partial,
    Supported,
    Unavailable,
}

/// A range in one provider's numbered observation stream, not a result count.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceRange {
    pub first: u64,
    pub last: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamContinuity {
    pub source: SourceId,
    pub stream_id: Option<String>,
    pub lineage_known: bool,
    pub prior_stream_unknown: bool,
    pub missing_events: u64,
    pub missing_ranges: Vec<SequenceRange>,
    pub omitted_ranges: u64,
    pub deferred_events: usize,
    pub dropped_deferred_events: u64,
}

impl StreamContinuity {
    pub fn has_limitation(&self) -> bool {
        !self.lineage_known
            || self.prior_stream_unknown
            || self.missing_events > 0
            || self.deferred_events > 0
            || self.dropped_deferred_events > 0
    }

    fn merge_previous(&mut self, previous: &Self) {
        if self.source == previous.source && self.stream_id == previous.stream_id {
            // The bridge supplies its bounded range history; an older producer
            // snapshot must not remove already observed gaps.
            if self.missing_events < previous.missing_events {
                self.missing_ranges = previous.missing_ranges.clone();
            }
            self.missing_events = self.missing_events.max(previous.missing_events);
            self.omitted_ranges = self.omitted_ranges.max(previous.omitted_ranges);
            self.dropped_deferred_events = self
                .dropped_deferred_events
                .max(previous.dropped_deferred_events);
            self.prior_stream_unknown |= previous.prior_stream_unknown;
        } else {
            self.prior_stream_unknown |= previous.has_limitation();
        }
        self.bound_ranges();
    }

    fn bound_ranges(&mut self) {
        if self.missing_ranges.len() > 16 {
            let removed = self.missing_ranges.len() - 16;
            self.missing_ranges.drain(..removed);
            self.omitted_ranges = self.omitted_ranges.saturating_add(removed as u64);
        }
    }
}

/// Observer-local correlation losses. Categories are not distinct lost results.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolCorrelationCoverage {
    pub epoch: u64,
    pub evicted: u64,
    pub oversized: u64,
    pub ambiguous: u64,
    pub reset_discarded: u64,
    pub prior_loss: bool,
    pub observed_at: Millis,
}

impl ToolCorrelationCoverage {
    pub fn has_loss(&self) -> bool {
        self.prior_loss
            || self.evicted > 0
            || self.oversized > 0
            || self.ambiguous > 0
            || self.reset_discarded > 0
    }

    fn merge_previous(&mut self, previous: &Self) {
        if self.epoch == previous.epoch {
            self.evicted = self.evicted.max(previous.evicted);
            self.oversized = self.oversized.max(previous.oversized);
            self.ambiguous = self.ambiguous.max(previous.ambiguous);
            self.reset_discarded = self.reset_discarded.max(previous.reset_discarded);
            self.prior_loss |= previous.prior_loss;
            self.observed_at = self.observed_at.max(previous.observed_at);
        } else {
            self.prior_loss |= previous.has_loss();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceCoverage {
    pub available: bool,
    pub incomplete: bool,
    pub relationships: CoverageLevel,
    pub messages: CoverageLevel,
    pub lifecycle: CoverageLevel,
    pub observed_at: Millis,
    /// Static explanation of limitations, never a transcript-derived command.
    pub detail: String,
    /// Omitted updates preserve the managed stream producer's prior facts.
    pub stream: Option<StreamContinuity>,
    /// Omitted updates preserve the collector's prior correlation facts.
    pub tool_correlation: Option<ToolCorrelationCoverage>,
}

impl Default for SourceCoverage {
    fn default() -> Self {
        Self {
            available: false,
            incomplete: true,
            relationships: CoverageLevel::Unknown,
            messages: CoverageLevel::Unknown,
            lifecycle: CoverageLevel::Unknown,
            observed_at: 0,
            detail: "Source coverage unknown.".into(),
            stream: None,
            tool_correlation: None,
        }
    }
}

impl SourceCoverage {
    pub fn merge_observation(&mut self, mut incoming: Self) {
        match (&mut incoming.stream, &self.stream) {
            (Some(next), Some(old)) => next.merge_previous(old),
            (None, old) => incoming.stream = old.clone(),
            _ => {}
        }
        if let Some(stream) = &mut incoming.stream {
            stream.bound_ranges();
        }
        match (&mut incoming.tool_correlation, &self.tool_correlation) {
            (Some(next), Some(old)) => next.merge_previous(old),
            (None, old) => incoming.tool_correlation = old.clone(),
            _ => {}
        }
        *self = incoming;
    }

    pub fn is_stale_at(&self, now: Millis) -> bool {
        now.saturating_sub(self.observed_at) > crate::BLOCKED_AFTER_MS
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RelationshipKind {
    Delegation,
    SessionMembership,
    Fork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Evidence {
    NativeMetadata,
    NativeEvent,
    TranscriptLayout,
    Demo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    pub parent: WorkerId,
    pub child: WorkerId,
    pub kind: RelationshipKind,
    pub evidence: Evidence,
    pub at: Millis,
    #[serde(default)]
    pub correlation_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationKind {
    Delegated,
    Message,
    Waiting,
    Result,
    HumanRequest,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationEvent {
    /// Native item/message/tool ID plus a stable discriminator for each event.
    pub id: String,
    pub at: Millis,
    pub actor: WorkerId,
    #[serde(default)]
    pub recipient: Option<WorkerId>,
    pub kind: CollaborationKind,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub native_turn_id: Option<String>,
    #[serde(default)]
    pub native_item_id: Option<String>,
    pub evidence: Evidence,
}

/// Stable pre-order projection. Missing endpoints are retained as rows, so the
/// UI can say "parent unavailable" without inventing a worker or losing a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub worker: WorkerId,
    pub depth: usize,
    pub via: Option<RelationshipKind>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_do_not_collide_between_providers_homes_or_session_scopes() {
        let identity = ThreadIdentity::new(Agent::Codex, SourceId("/one".into()), "same");
        let mut other = identity.clone();
        other.provider = Agent::Claude;
        assert_ne!(identity.worker_id(), other.worker_id());
        other = identity.clone();
        other.source = SourceId("/two".into());
        assert_ne!(identity.worker_id(), other.worker_id());
        other = identity.clone();
        other.session_id = Some("root".into());
        assert_ne!(identity.worker_id(), other.worker_id());
        assert_ne!(
            ThreadIdentity::new(Agent::Codex, SourceId("a:1".into()), "b").worker_id(),
            ThreadIdentity::new(Agent::Codex, SourceId("a".into()), "1:b").worker_id()
        );
    }
}
