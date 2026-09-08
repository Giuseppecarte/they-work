//! Shared observed context and lossless text wrapping for task surfaces.
use super::{safe_display, worker_status};
use ratatui::text::Span;
use theywork_core::{Activity, Millis, Worker, WorkerStatus};

/// A current observation, kept separate from historical events in both views.
pub(super) struct InspectionSummary {
    pub detail: String,
}

pub(super) fn inspection_summary(worker: &Worker, now: Millis) -> InspectionSummary {
    let detail = worker
        .activity
        .detail()
        .filter(|value| !value.trim().is_empty());
    if worker.coverage.observed_at > 0
        && (!worker.coverage.available || worker.coverage.is_stale_at(now))
    {
        return InspectionSummary {
            detail: safe_display(&worker.coverage.detail),
        };
    }
    if worker
        .wait_reason
        .filter(|reason| {
            matches!(
                reason,
                theywork_core::WaitReason::AutomaticReview
                    | theywork_core::WaitReason::Child
                    | theywork_core::WaitReason::Process
            )
        })
        .is_some()
    {
        return InspectionSummary {
            detail: safe_display(detail.unwrap_or("The provider recorded this wait.")),
        };
    }
    match worker_status(worker, now) {
        WorkerStatus::Blocked if matches!(worker.activity, Activity::Waiting { .. }) => {
            InspectionSummary {
                detail: safe_display(
                    detail.unwrap_or("The source recorded a request without details."),
                ),
            }
        }
        WorkerStatus::Blocked => InspectionSummary {
            detail: "No recent activity. No approval was identified.".into(),
        },
        WorkerStatus::Failed => InspectionSummary {
            detail: safe_display(detail.unwrap_or("The source reported an error without details.")),
        },
        WorkerStatus::Idle => InspectionSummary {
            detail: safe_display(detail.unwrap_or("No task is currently running.")),
        },
        WorkerStatus::Running => InspectionSummary {
            detail: detail
                .map(safe_display)
                .unwrap_or_else(|| worker.activity.label().into()),
        },
    }
}

// Keep every character available when text wraps, including command whitespace.
pub(super) fn wrapped_lines(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut used = 0;
    for character in safe_display(text).chars() {
        let size = Span::raw(character.to_string()).width();
        if used + size > width.max(1) && used > 0 {
            let current = lines.last_mut().expect("one line");
            let tail = current
                .rfind(' ')
                .filter(|index| *index > 0)
                .map(|index| current.split_off(index + 1))
                .unwrap_or_default();
            used = Span::raw(tail.clone()).width();
            lines.push(tail);
        }
        lines.last_mut().expect("one line").push(character);
        used += size;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapped_context_preserves_title_and_unicode_at_narrow_widths() {
        let title = "Review producción café 東京 with the final distinguishing suffix";
        for width in [1, 8, 25, 60] {
            let lines = wrapped_lines(title, width);
            assert_eq!(lines.concat(), title);
            assert!(lines
                .iter()
                .all(|line| Span::raw(line).width() <= width.max(2)));
        }
    }
}
