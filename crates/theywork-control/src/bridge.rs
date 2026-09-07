use crate::{ControlEvent, ControlSnapshot, ManagedThread};
use serde_json::Value;
use theywork_core::{
    Activity, Beat, CollaborationEvent, CollaborationKind, CoverageLevel, Event, EventKind,
    Evidence, OfficeId, Relationship, RelationshipKind, SourceCoverage, ThreadIdentity, WaitReason,
    WorkerLifecycle,
};

/// Apply returned events after collector events. Advance the caller's cursor
/// to snapshot.events.last().sequence only after applying this batch. Roster
/// observations repeat safely; native collaboration IDs match the collector.
pub fn snapshot_events(snapshot: &ControlSnapshot, after_sequence: u64) -> Vec<Event> {
    let mut events = Vec::new();
    for thread in snapshot.threads.values() {
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
                lifecycle: if snapshot.connected {
                    CoverageLevel::Supported
                } else {
                    CoverageLevel::Unavailable
                },
                available: snapshot.connected,
                observed_at: snapshot.observed_at,
                detail: if snapshot.connected {
                    "Live managed connection; relationship coverage depends on provider events"
                } else {
                    "Control connection unavailable; no instruction was restarted"
                }
                .into(),
                ..SourceCoverage::default()
            }),
        ));
    }
    for event in snapshot
        .events
        .iter()
        .filter(|event| event.sequence > after_sequence)
    {
        let Some(thread) = event
            .thread_id
            .as_ref()
            .and_then(|id| snapshot.threads.get(id))
        else {
            continue;
        };
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
                    item_events(thread, event, item, &mut events);
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
            _ => {}
        }
    }
    // Current pending requests take precedence over historical tool text. A
    // submitted reply stays pending until a provider resolution clears it.
    for pending in &snapshot.pending_requests {
        let Some(thread) = snapshot.threads.get(&pending.thread_id) else {
            continue;
        };
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
        let detail = pending
            .params
            .get("reason")
            .or_else(|| pending.params.get("message"))
            .or_else(|| pending.params.get("command"))
            .and_then(Value::as_str)
            .map(clean)
            .unwrap_or_else(|| {
                if input {
                    "Input requested in control panel"
                } else {
                    "Approval requested in control panel"
                }
                .into()
            });
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
    events
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
