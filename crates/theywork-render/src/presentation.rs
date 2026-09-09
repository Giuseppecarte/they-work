//! Shared language for observations. Presentation never turns silence into consent.

use theywork_core::{
    CollaborationEvent, CollaborationKind, SourceCoverage, WaitReason, Worker, WorkerStatus,
};

pub fn human_request(worker: &Worker) -> bool {
    matches!(
        worker.wait_reason,
        Some(WaitReason::HumanApproval | WaitReason::HumanInput)
    )
}

pub fn state_label(worker: &Worker, now: i64) -> &'static str {
    if worker.coverage.observed_at > 0 {
        if !worker.coverage.available {
            return "Source unavailable";
        }
        if worker.coverage.is_stale_at(now) {
            return "Last known state";
        }
    }
    if worker.status_at(now) == WorkerStatus::Failed {
        return "Error reported";
    }
    match worker.wait_reason {
        Some(WaitReason::HumanApproval) => "Approval needed",
        Some(WaitReason::HumanInput) => "Question for you",
        Some(WaitReason::AutomaticReview) => "Automatic review",
        Some(WaitReason::Child) => "Waiting for the team",
        Some(WaitReason::Process) => "Waiting for a process",
        _ => match worker.status_at(now) {
            WorkerStatus::Running => "Working",
            WorkerStatus::Idle => "Ready",
            WorkerStatus::Blocked => "Needs a follow-up",
            WorkerStatus::Failed => "Error reported",
        },
    }
}

pub fn needs_attention(worker: &Worker, now: i64) -> bool {
    matches!(
        state_label(worker, now),
        "Approval needed" | "Question for you" | "Needs a follow-up" | "Error reported"
    )
}

pub fn observation_age(coverage: &SourceCoverage, now: i64) -> String {
    record_age(
        (coverage.observed_at > 0).then_some(coverage.observed_at),
        now,
    )
}

pub fn record_age(at: Option<i64>, now: i64) -> String {
    let Some(at) = at else {
        return "age unknown".into();
    };
    let seconds = now.saturating_sub(at).max(0) / 1000;
    let amount = if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h", seconds / 3600)
    } else if seconds / 86400 > 999 {
        ">999d".into()
    } else {
        format!("{}d", seconds / 86400)
    };
    format!("{amount} ago")
}

pub fn wait_description(reason: Option<WaitReason>) -> &'static str {
    match reason {
        Some(WaitReason::HumanApproval) => {
            "The source recorded an approval request. Review the current request before deciding."
        }
        Some(WaitReason::HumanInput) => {
            "The source recorded a question. Review its current form to answer."
        }
        Some(WaitReason::AutomaticReview) => {
            "An automatic reviewer is checking this work. No human approval is implied."
        }
        Some(WaitReason::Child) => {
            "This task is waiting for another agent. Its recorded family appears below."
        }
        Some(WaitReason::Process) => {
            "A process is still pending. This is not a request for your permission."
        }
        Some(WaitReason::Unknown) => {
            "A wait was recorded, but its cause is unknown. Inspect the original conversation."
        }
        None => "No explicit request for your input is recorded.",
    }
}

pub fn coverage_label(coverage: &SourceCoverage, now: i64) -> &'static str {
    if coverage.observed_at == 0 {
        "Source not checked"
    } else if !coverage.available {
        "Source unavailable"
    } else if coverage.is_stale_at(now) {
        "Source needs refreshing"
    } else if coverage.incomplete {
        "Partial local history"
    } else {
        "Available history"
    }
}

pub fn coverage_has_loss(coverage: &SourceCoverage) -> bool {
    coverage
        .stream
        .as_ref()
        .is_some_and(|stream| stream.has_limitation())
        || coverage
            .tool_correlation
            .as_ref()
            .is_some_and(|tools| tools.has_loss())
}

pub fn coverage_headline(coverage: &SourceCoverage, now: i64, local_loss: bool) -> String {
    if coverage_has_loss(coverage) || local_loss {
        let health = if coverage.observed_at == 0 {
            "Unchecked"
        } else if !coverage.available {
            "Unavailable"
        } else if coverage.is_stale_at(now) {
            "Stale"
        } else {
            "History"
        };
        format!("{health} · limits in Details")
    } else {
        coverage_label(coverage, now).into()
    }
}

pub fn coverage_text(coverage: &SourceCoverage, now: i64) -> String {
    let mut text = format!("{}\n{}", coverage_label(coverage, now), coverage.detail);
    if let Some(stream) = &coverage.stream {
        text.push_str(&format!("\nSTREAM OBSERVATION\nSource: {}\nLineage: {}\n{} numbered events missing in this lineage; {} events awaiting an actor; {} deferred events retired locally.",
            stream.source.0, stream.stream_id.as_deref().unwrap_or("unknown"), stream.missing_events, stream.deferred_events, stream.dropped_deferred_events));
        if !stream.lineage_known {
            text.push_str(
                "\nThe initial stream prefix is unknown; no missing-event count is inferred.",
            );
        }
        if stream.prior_stream_unknown {
            text.push_str("\nEarlier stream continuity is unknown or had a recorded limitation.");
        }
        if !stream.missing_ranges.is_empty() {
            text.push_str(&format!(
                "\nMissing sequence ranges: {}",
                stream
                    .missing_ranges
                    .iter()
                    .map(|range| format!("{}–{}", range.first, range.last))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if stream.omitted_ranges > 0 {
            text.push_str(&format!(
                "\n{} older range details omitted; event total retained.",
                stream.omitted_ranges
            ));
        }
        text.push_str("\nThese are event counts, not counts of missing results or requests.");
    }
    if let Some(tools) = &coverage.tool_correlation {
        if tools.has_loss() {
            text.push_str(&format!("\nTOOL CORRELATION\n{} pending entries evicted; {} oversized starts; {} conflicting IDs; {} pending entries discarded on reset.\nCounts cover observer epoch {} and are not distinct lost results.", tools.evicted, tools.oversized, tools.ambiguous, tools.reset_discarded, tools.epoch));
            if tools.prior_loss {
                text.push_str("\nEarlier correlation loss was observed; its counts are unavailable in this epoch.");
            }
        }
    }
    text
}

pub fn history_text(window: &theywork_core::HistoryWindow) -> String {
    let mut text = format!("LOCAL PROJECT WINDOW\n{} collaboration records retained; {} local retention evictions.\nOldest retained source timestamp: {}\nLast eviction observation: {} (record source timestamp: {}).\nThe shared limit is 512 records, retired from the largest project partition first. Evictions are retention actions, not unique missing deliveries.",
        window.retained_count, window.evicted_count,
        window.oldest_retained_at.map_or_else(|| "unavailable".into(), |at| at.to_string()),
        window.last_eviction_ordinal.map_or_else(|| "none recorded".into(), |ordinal| ordinal.to_string()),
        window.last_evicted_record_at.map_or_else(|| "unavailable".into(), |at| at.to_string()));
    if window.prior_history_unknown {
        text.push_str("\nHistory before this observer's retained window is unknown.");
    }
    if window.prior_local_evictions_unknown {
        text.push_str(
            "\nEarlier local project eviction counts are unknown: bounded metadata was retired.",
        );
    }
    text
}

pub fn event_label(kind: CollaborationKind) -> &'static str {
    match kind {
        CollaborationKind::Delegated => "Work delegated",
        CollaborationKind::Message => "Message",
        CollaborationKind::Waiting => "Waiting for a teammate",
        CollaborationKind::Result => "Result delivered",
        CollaborationKind::HumanRequest => "Request recorded",
        CollaborationKind::Completed => "Work completed",
        CollaborationKind::Failed => "Error reported",
        CollaborationKind::Cancelled => "Work cancelled",
    }
}

pub fn event_summary(event: &CollaborationEvent, actor: &str, recipient: Option<&str>) -> String {
    let target = recipient.unwrap_or("a recipient not recorded by the source");
    match event.kind {
        CollaborationKind::Delegated => format!("{actor} delegated work to {target}."),
        CollaborationKind::Message => format!("{actor} sent a message to {target}."),
        CollaborationKind::Waiting => format!("{actor} recorded a wait for a teammate."),
        CollaborationKind::Result => format!("{actor} delivered a result."),
        CollaborationKind::HumanRequest => {
            format!("{actor} recorded a question or approval request.")
        }
        CollaborationKind::Completed => format!("{actor} completed the recorded work."),
        CollaborationKind::Failed => format!("{actor} reported a failure."),
        CollaborationKind::Cancelled => format!("{actor}'s recorded work was cancelled."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use theywork_core::{Activity, Agent, Evidence, OfficeId, WorkerId};

    #[test]
    fn quiet_and_automatic_waits_do_not_become_human_requests() {
        let mut worker = Worker::new(
            WorkerId("worker".into()),
            OfficeId("p".into()),
            Agent::Codex,
            "Task".into(),
            1,
        );
        worker.turn_in_flight = true;
        assert!(!human_request(&worker));
        assert_eq!(
            state_label(&worker, theywork_core::BLOCKED_AFTER_MS + 2),
            "Needs a follow-up"
        );
        worker.activity = Activity::Waiting {
            detail: "Review".into(),
        };
        worker.wait_reason = Some(WaitReason::AutomaticReview);
        assert!(!human_request(&worker));
        assert_eq!(state_label(&worker, 2), "Automatic review");
    }

    #[test]
    fn unknown_recipient_does_not_turn_into_the_user() {
        let event = CollaborationEvent {
            id: "message".into(),
            at: 1,
            actor: WorkerId("a".into()),
            recipient: None,
            kind: CollaborationKind::Message,
            text: None,
            correlation_id: None,
            native_turn_id: None,
            native_item_id: None,
            evidence: Evidence::NativeEvent,
        };
        let text = event_summary(&event, "Avery", None);
        assert!(text.contains("not recorded"));
        assert!(!text.contains("user"));
    }

    #[test]
    fn stale_or_disconnected_sources_do_not_claim_a_current_state() {
        let mut worker = Worker::new(
            WorkerId("worker".into()),
            OfficeId("p".into()),
            Agent::Codex,
            "Task".into(),
            1,
        );
        worker.turn_in_flight = true;
        worker.coverage.observed_at = 1;
        worker.coverage.available = false;
        assert_eq!(state_label(&worker, 2), "Source unavailable");
        worker.wait_reason = Some(WaitReason::HumanApproval);
        assert_eq!(state_label(&worker, 2), "Source unavailable");
        worker.coverage.available = true;
        assert_eq!(state_label(&worker, i64::MAX), "Last known state");
        assert_eq!(state_label(&worker, 2), "Approval needed");
    }
}
