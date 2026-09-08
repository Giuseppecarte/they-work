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

pub fn coverage_text(coverage: &SourceCoverage, now: i64) -> String {
    format!("{}\n{}", coverage_label(coverage, now), coverage.detail)
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
