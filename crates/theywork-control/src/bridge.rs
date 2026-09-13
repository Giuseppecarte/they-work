use crate::{ControlEvent, ControlSnapshot, ManagedThread};
use serde_json::Value;
use std::collections::BTreeMap;
use theywork_core::{
    Activity, Beat, CollaborationEvent, CollaborationKind, CoverageLevel, Event, EventKind,
    Evidence, OfficeId, Relationship, RelationshipKind, SequenceRange, SourceCoverage, SourceId,
    StreamContinuity, ThreadIdentity, WaitReason, WorkerLifecycle,
};

pub const DEFERRED_EVENT_LIMIT: usize = 256;
pub const DEFERRED_EVENT_BYTE_LIMIT: usize = 1024 * 1024;
pub const MISSING_RANGE_LIMIT: usize = 16;

#[derive(Debug, Clone)]
struct DeferredEvent {
    event: ControlEvent,
    bytes: usize,
}

/// Runtime observation state. It does not contain a provider connection and
/// cannot execute or authorize any user instruction.
#[derive(Debug, Clone, Default)]
pub struct BridgeCursor {
    pub applied_sequence: u64,
    pub continuity: StreamContinuity,
    initialized: bool,
    legacy_generation: String,
    deferred: BTreeMap<u64, DeferredEvent>,
    deferred_bytes: usize,
}

impl BridgeCursor {
    pub fn deferred_bytes(&self) -> usize {
        self.deferred_bytes
    }
}

#[derive(Debug)]
pub struct BridgeBatch {
    pub events: Vec<Event>,
    pub next_cursor: BridgeCursor,
    pub continuity: StreamContinuity,
}

/// Reconcile one bounded snapshot after independently collected observations.
/// Commit `next_cursor` only after applying `events`. Native event IDs provide
/// semantic deduplication; sequence IDs describe provider-stream coverage only.
pub fn reconcile_snapshot(snapshot: &ControlSnapshot, cursor: &BridgeCursor) -> BridgeBatch {
    let source = SourceId(snapshot.codex_home.to_string_lossy().into_owned());
    let window = crate::stream::valid_window(snapshot)
        .then_some(snapshot.event_window.as_ref())
        .flatten();
    let stream_id = window.map(|window| window.stream_id.clone());
    let same_lineage = cursor.initialized
        && cursor.continuity.source == source
        && cursor.continuity.stream_id == stream_id
        && (stream_id.is_some() || cursor.legacy_generation == snapshot.generation);
    if same_lineage
        && window.is_some_and(|window| window.last_assigned_sequence < cursor.applied_sequence)
    {
        let mut events = Vec::new();
        if !snapshot.connected {
            roster_events(snapshot, Some(&cursor.continuity), &mut events);
            events.retain(|event| matches!(event.kind, EventKind::Coverage(_)));
        }
        return BridgeBatch {
            events,
            next_cursor: cursor.clone(),
            continuity: cursor.continuity.clone(),
        };
    }
    let mut next = if same_lineage {
        cursor.clone()
    } else {
        BridgeCursor {
            initialized: true,
            legacy_generation: snapshot.generation.clone(),
            continuity: StreamContinuity {
                source: source.clone(),
                stream_id,
                lineage_known: window.is_some_and(|window| !window.prior_lineage_unknown),
                prior_stream_unknown: cursor.initialized
                    || window.is_none_or(|window| window.prior_lineage_unknown),
                ..StreamContinuity::default()
            },
            ..BridgeCursor::default()
        }
    };
    let after = next.applied_sequence;
    if let Some(window) = window {
        // A legacy first window cannot establish its unobserved prefix. Once
        // its high-water mark has been observed, subsequent gaps are exact.
        if same_lineage || !window.prior_lineage_unknown {
            let unavailable_through = window
                .first_retained_sequence
                .map_or(window.last_assigned_sequence, |first| {
                    first.saturating_sub(1)
                });
            if unavailable_through > after {
                add_gap(&mut next.continuity, after + 1, unavailable_through);
            }
        }
    }

    let mut projected = Vec::new();
    let resolved = next
        .deferred
        .iter()
        .filter_map(|(sequence, deferred)| {
            known_actor(snapshot, &source, &deferred.event).map(|thread| {
                project_event(thread, &deferred.event, true, &mut projected);
                *sequence
            })
        })
        .collect::<Vec<_>>();
    for sequence in resolved {
        if let Some(deferred) = next.deferred.remove(&sequence) {
            next.deferred_bytes = next.deferred_bytes.saturating_sub(deferred.bytes);
        }
    }
    // A malformed legacy snapshot may not be ordered. Sequence order is the
    // deterministic traversal, but absent window metadata never proves a gap.
    let mut observations = snapshot
        .events
        .iter()
        .filter(|event| event.sequence > after)
        .collect::<Vec<_>>();
    observations.sort_by_key(|event| event.sequence);
    observations.dedup_by_key(|event| event.sequence);
    for event in observations {
        if let Some(thread) = known_actor(snapshot, &source, event) {
            let historical = !same_lineage
                || !snapshot.connected
                || event.turn_id.as_ref().is_some_and(|turn| {
                    thread
                        .active_turn_id
                        .as_ref()
                        .is_some_and(|current| current != turn)
                });
            project_event(thread, event, historical, &mut projected);
        } else if event.thread_id.is_some() {
            defer(&mut next, event);
        }
        next.applied_sequence = next.applied_sequence.max(event.sequence);
    }
    if let Some(window) = window {
        next.applied_sequence = next.applied_sequence.max(window.last_assigned_sequence);
    }
    next.continuity.deferred_events = next.deferred.len();
    let mut events = Vec::new();
    roster_events(snapshot, Some(&next.continuity), &mut events);
    events.extend(projected);
    current_roster_state(snapshot, &source, &mut events);
    // Only the current snapshot can make a request actionable. Old recorded
    // requests, stream gaps and deferred replay never resolve or answer it.
    pending_events(snapshot, Some(&source), &mut events);
    BridgeBatch {
        events,
        continuity: next.continuity.clone(),
        next_cursor: next,
    }
}

fn known_actor<'a>(
    snapshot: &'a ControlSnapshot,
    source: &SourceId,
    event: &ControlEvent,
) -> Option<&'a ManagedThread> {
    let native = event.thread_id.as_ref()?;
    snapshot
        .threads
        .get(native)
        .filter(|thread| thread.identity.source == *source && thread.identity.native_id == *native)
}

fn defer(cursor: &mut BridgeCursor, event: &ControlEvent) {
    if cursor.deferred.contains_key(&event.sequence) {
        return;
    }
    // Count the entire serialized event, including actor and correlation IDs,
    // rather than only its message text. A single oversized item cannot force
    // retirement of otherwise useful pending observations.
    let bytes = serde_json::to_vec(event).map_or(usize::MAX, |bytes| bytes.len());
    if bytes > DEFERRED_EVENT_BYTE_LIMIT {
        cursor.continuity.dropped_deferred_events =
            cursor.continuity.dropped_deferred_events.saturating_add(1);
        return;
    }
    while cursor.deferred.len() >= DEFERRED_EVENT_LIMIT
        || cursor.deferred_bytes.saturating_add(bytes) > DEFERRED_EVENT_BYTE_LIMIT
    {
        let Some((_, removed)) = cursor.deferred.pop_first() else {
            break;
        };
        cursor.deferred_bytes = cursor.deferred_bytes.saturating_sub(removed.bytes);
        cursor.continuity.dropped_deferred_events =
            cursor.continuity.dropped_deferred_events.saturating_add(1);
    }
    cursor.deferred.insert(
        event.sequence,
        DeferredEvent {
            event: event.clone(),
            bytes,
        },
    );
    cursor.deferred_bytes += bytes;
}

fn add_gap(continuity: &mut StreamContinuity, first: u64, last: u64) {
    continuity.missing_events = continuity
        .missing_events
        .saturating_add(last.saturating_sub(first).saturating_add(1));
    if let Some(previous) = continuity.missing_ranges.last_mut() {
        if previous.last.checked_add(1) == Some(first) {
            previous.last = last;
            return;
        }
    }
    continuity
        .missing_ranges
        .push(SequenceRange { first, last });
    if continuity.missing_ranges.len() > MISSING_RANGE_LIMIT {
        continuity.missing_ranges.remove(0);
        continuity.omitted_ranges = continuity.omitted_ranges.saturating_add(1);
    }
}

fn current_roster_state(snapshot: &ControlSnapshot, source: &SourceId, events: &mut Vec<Event>) {
    if !snapshot.connected {
        return;
    }
    for (native, thread) in &snapshot.threads {
        if thread.identity.source != *source || thread.identity.native_id != *native {
            continue;
        }
        let lifecycle = match thread.status.as_str() {
            "working" | "inProgress" | "active" | "running" => WorkerLifecycle::Active,
            "completed" => WorkerLifecycle::Completed,
            "failed" => WorkerLifecycle::Failed,
            "interrupted" => WorkerLifecycle::Cancelled,
            _ => continue,
        };
        events.push(make(
            thread,
            thread.updated_at,
            EventKind::Turn {
                in_flight: thread.active_turn_id.is_some(),
            },
        ));
        events.push(make(
            thread,
            thread.updated_at,
            EventKind::Lifecycle(lifecycle),
        ));
    }
}

/// Compatibility projection for consumers that do not retain reconciliation
/// state. New consumers should use `reconcile_snapshot` to disclose gaps and
/// retain unresolved actors. This wrapper cannot offer deferred recovery.
pub fn snapshot_events(snapshot: &ControlSnapshot, after_sequence: u64) -> Vec<Event> {
    let mut events = Vec::new();
    roster_events(snapshot, None, &mut events);
    for event in snapshot
        .events
        .iter()
        .filter(|event| event.sequence > after_sequence)
    {
        if let Some(thread) = event
            .thread_id
            .as_ref()
            .and_then(|id| snapshot.threads.get(id))
        {
            project_event(thread, event, false, &mut events);
        }
    }
    pending_events(snapshot, None, &mut events);
    events
}

fn roster_events(
    snapshot: &ControlSnapshot,
    continuity: Option<&StreamContinuity>,
    events: &mut Vec<Event>,
) {
    for (native, thread) in &snapshot.threads {
        if continuity.is_some_and(|continuity| {
            thread.identity.source != continuity.source || thread.identity.native_id != *native
        }) {
            continue;
        }
        let available = snapshot.connected
            && !matches!(
                thread.status.as_str(),
                "disconnected" | "connection unavailable"
            );
        events.push(make(
            thread,
            thread.updated_at,
            EventKind::Seen {
                name: clean(&thread.title),
                git_branch: None,
            },
        ));
        events.push(make(
            thread,
            thread.updated_at,
            EventKind::Identity {
                identity: thread.identity.clone(),
                role: thread.role,
            },
        ));
        events.push(make(
            thread,
            thread.updated_at,
            EventKind::Coverage(SourceCoverage {
                relationships: CoverageLevel::Partial,
                messages: CoverageLevel::Partial,
                lifecycle: if available {
                    CoverageLevel::Supported
                } else {
                    CoverageLevel::Unavailable
                },
                available,
                stream: continuity.cloned(),
                observed_at: snapshot.observed_at,
                detail: if available {
                    "Live managed connection; relationship coverage depends on provider events"
                } else {
                    "Control connection unavailable; no instruction was restarted"
                }
                .into(),
                ..SourceCoverage::default()
            }),
        ));
    }
}

fn project_event(
    thread: &ManagedThread,
    event: &ControlEvent,
    historical: bool,
    events: &mut Vec<Event>,
) {
    let start = events.len();
    let item = event.params.get("item");
    match event.method.as_str() {
        "turn/started" => {
            events.push(make(thread, event.at, EventKind::Wait(None)));
            events.push(make(thread, event.at, EventKind::Turn { in_flight: true }));
            events.push(make(
                thread,
                event.at,
                EventKind::Lifecycle(WorkerLifecycle::Active),
            ));
        }
        "turn/completed" => {
            events.push(make(thread, event.at, EventKind::Turn { in_flight: false }));
            events.push(make(thread, event.at, EventKind::Wait(None)));
            let status = event
                .params
                .pointer("/turn/status")
                .and_then(Value::as_str)
                .unwrap_or("");
            let lifecycle = match status {
                "completed" => WorkerLifecycle::Completed,
                "failed" => WorkerLifecycle::Failed,
                "interrupted" => WorkerLifecycle::Cancelled,
                _ => WorkerLifecycle::Unknown,
            };
            events.push(make(thread, event.at, EventKind::Lifecycle(lifecycle)));
            let activity = if lifecycle == WorkerLifecycle::Failed {
                Activity::Error {
                    detail: clean(
                        event
                            .params
                            .pointer("/turn/error/message")
                            .and_then(Value::as_str)
                            .unwrap_or("Provider reported a failed turn"),
                    ),
                }
            } else {
                Activity::Idle
            };
            events.push(make(thread, event.at, EventKind::Acted(activity)));
        }
        "item/started" | "item/completed" => {
            if let Some(item) = item {
                item_events(thread, event, item, events);
            }
        }
        "item/agentMessage/delta" => {
            events.push(make(
                thread,
                event.at,
                EventKind::Acted(Activity::Talking {
                    detail: clean(&thread.latest_text),
                }),
            ));
        }
        "serverRequest/resolved" => {
            events.push(make(thread, event.at, EventKind::Wait(None)));
        }
        "item/commandExecution/requestApproval"
        | "item/fileChange/requestApproval"
        | "item/tool/requestUserInput"
        | "mcpServer/elicitation/request"
        | "item/permissions/requestApproval" => {
            if let Some(native_item) = event
                .item_id
                .as_deref()
                .or_else(|| event.params.get("itemId").and_then(Value::as_str))
            {
                events.push(make(
                    thread,
                    event.at,
                    EventKind::Collaboration(CollaborationEvent {
                        id: format!(
                            "{}:{native_item}:request",
                            event.turn_id.as_deref().unwrap_or("")
                        ),
                        at: event.at,
                        actor: thread.identity.worker_id(),
                        recipient: None,
                        kind: CollaborationKind::HumanRequest,
                        text: Some(request_text(&event.method, &event.params)),
                        correlation_id: Some(native_item.into()),
                        native_turn_id: event.turn_id.clone(),
                        native_item_id: Some(native_item.into()),
                        evidence: Evidence::NativeEvent,
                    }),
                ));
            }
        }
        _ => {}
    }

    if historical {
        let projected = events
            .drain(start..)
            .filter_map(|mut event| {
                event.kind = match event.kind {
                    EventKind::Did(beat) => EventKind::HistoricalBeat(beat),
                    kind @ (EventKind::Collaboration(_) | EventKind::Relationship(_)) => kind,
                    _ => return None,
                };
                Some(event)
            })
            .collect::<Vec<_>>();
        events.extend(projected);
    }
}

fn pending_events(snapshot: &ControlSnapshot, source: Option<&SourceId>, events: &mut Vec<Event>) {
    if source.is_some() && !snapshot.connected {
        return;
    }
    for pending in &snapshot.pending_requests {
        let Some(thread) = snapshot.threads.get(&pending.thread_id) else {
            continue;
        };
        if source.is_some_and(|source| {
            thread.identity.source != *source || thread.identity.native_id != pending.thread_id
        }) {
            continue;
        }
        let input = pending.method == "item/tool/requestUserInput"
            || pending.method == "mcpServer/elicitation/request";
        events.push(make(
            thread,
            pending.received_at,
            EventKind::Wait(Some(if input {
                WaitReason::HumanInput
            } else {
                WaitReason::HumanApproval
            })),
        ));
        let detail = request_text(&pending.method, &pending.params);
        let native_item = pending
            .params
            .get("itemId")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| pending.native_id.to_string());
        events.push(make(
            thread,
            pending.received_at,
            EventKind::Collaboration(CollaborationEvent {
                id: format!(
                    "{}:{native_item}:request",
                    pending.turn_id.as_deref().unwrap_or("")
                ),
                at: pending.received_at,
                actor: thread.identity.worker_id(),
                recipient: None,
                kind: CollaborationKind::HumanRequest,
                text: Some(detail.clone()),
                correlation_id: Some(native_item.clone()),
                native_turn_id: pending.turn_id.clone(),
                native_item_id: Some(native_item),
                evidence: Evidence::NativeEvent,
            }),
        ));
        events.push(make(
            thread,
            pending.received_at,
            EventKind::Acted(Activity::Waiting { detail }),
        ));
    }
}

fn request_text(method: &str, params: &Value) -> String {
    params
        .get("reason")
        .or_else(|| params.get("message"))
        .or_else(|| params.get("command"))
        .and_then(Value::as_str)
        .map(clean)
        .unwrap_or_else(|| {
            if matches!(
                method,
                "item/tool/requestUserInput" | "mcpServer/elicitation/request"
            ) {
                "Input requested in control panel"
            } else {
                "Approval requested in control panel"
            }
            .into()
        })
}

fn item_events(
    thread: &ManagedThread,
    event: &ControlEvent,
    item: &Value,
    events: &mut Vec<Event>,
) {
    let kind = item.get("type").and_then(Value::as_str).unwrap_or("");
    let text = |key: &str| {
        item.get(key)
            .and_then(Value::as_str)
            .map(clean)
            .unwrap_or_default()
    };
    let activity = match kind {
        "commandExecution" => Some(Activity::Typing {
            detail: text("command"),
        }),
        "fileChange" => Some(Activity::Editing {
            detail: "File changes".into(),
        }),
        "webSearch" => Some(Activity::Searching {
            detail: text("query"),
        }),
        "reasoning" => Some(Activity::Thinking),
        "agentMessage" => Some(Activity::Talking {
            detail: text("text"),
        }),
        _ => None,
    };
    if let Some(activity) = activity {
        events.push(make(
            thread,
            event.at,
            EventKind::Did(Beat {
                at: event.at,
                activity,
                outcome: None,
            }),
        ));
    }
    let Some(item_id) = item.get("id").and_then(Value::as_str) else {
        return;
    };
    let turn_id = event.turn_id.as_deref().unwrap_or("");
    let collab = |kind, recipient, text, suffix: &str| {
        make(
            thread,
            event.at,
            EventKind::Collaboration(CollaborationEvent {
                id: format!("{turn_id}:{item_id}:{suffix}"),
                at: event.at,
                actor: thread.identity.worker_id(),
                recipient,
                kind,
                text,
                correlation_id: Some(item_id.into()),
                native_turn_id: event.turn_id.clone(),
                native_item_id: Some(item_id.into()),
                evidence: Evidence::NativeEvent,
            }),
        )
    };
    if kind == "agentMessage"
        && event.method == "item/completed"
        && item.get("phase").and_then(Value::as_str) == Some("final_answer")
    {
        events.push(collab(
            CollaborationKind::Result,
            None,
            Some(text("text")),
            "final",
        ));
    }
    if !matches!(kind, "collabToolCall" | "collabAgentToolCall") {
        return;
    }
    if item
        .get("senderThreadId")
        .and_then(Value::as_str)
        .is_some_and(|id| id != thread.identity.native_id)
    {
        return;
    }
    let tool = text("tool").replace(['_', '-'], "").to_ascii_lowercase();
    let kind = match tool.as_str() {
        "spawn" | "spawnagent" => CollaborationKind::Delegated,
        "sendmessage" | "sendinput" | "resume" | "resumeagent" => CollaborationKind::Message,
        "wait" | "waitagent" => CollaborationKind::Waiting,
        "close" | "closeagent" => CollaborationKind::Cancelled,
        _ => return,
    };
    let mut recipients = ["newThreadId", "receiverThreadId"]
        .into_iter()
        .filter_map(|key| item.get(key).and_then(Value::as_str))
        .collect::<Vec<_>>();
    if let Some(array) = item.get("receiverThreadIds").and_then(Value::as_array) {
        recipients.extend(array.iter().filter_map(Value::as_str));
    }
    recipients.sort_unstable();
    recipients.dedup();
    let prompt = item.get("prompt").and_then(Value::as_str).map(clean);
    for native in recipients {
        let recipient = ThreadIdentity::new(
            thread.identity.provider,
            thread.identity.source.clone(),
            native,
        )
        .worker_id();
        if recipient == thread.identity.worker_id() {
            continue;
        }
        events.push(collab(
            kind,
            Some(recipient.clone()),
            prompt.clone(),
            native,
        ));
        if kind == CollaborationKind::Delegated {
            events.push(make(
                thread,
                event.at,
                EventKind::Relationship(Relationship {
                    parent: thread.identity.worker_id(),
                    child: recipient,
                    kind: RelationshipKind::Delegation,
                    evidence: Evidence::NativeEvent,
                    at: event.at,
                    correlation_id: Some(item_id.into()),
                }),
            ));
        }
    }
    if kind == CollaborationKind::Waiting {
        let waiting = matches!(
            item.get("status").and_then(Value::as_str),
            Some("inProgress" | "running")
        );
        events.push(make(
            thread,
            event.at,
            EventKind::Wait(waiting.then_some(WaitReason::Child)),
        ));
    }
}

fn make(thread: &ManagedThread, at: i64, kind: EventKind) -> Event {
    let project = thread.project.to_string_lossy().into_owned();
    Event {
        at,
        office: OfficeId(project.clone()),
        office_path: project,
        worker: thread.identity.worker_id(),
        agent: thread.identity.provider,
        kind,
    }
}

fn clean(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
        .take(8_192)
        .collect()
}
