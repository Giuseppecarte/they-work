//! Contextual, read-only task inspection with explicitly available actions.

use super::{
    control::ControlStatus, paint_opaque, safe_display, safe_multiline, wrap_text, ACCENT, INK,
    MUTED, PANEL, PANEL_HIGHLIGHT, WARNING,
};
use crate::{
    design::{profile_for, CharacterProfile},
    interaction::{Action, HitRegion},
    presentation,
};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Paragraph, Widget},
    Frame,
};
use std::collections::BTreeMap;
use theywork_core::{Outcome, WorkerId, World};

pub struct InspectorContext<'a> {
    pub world: &'a World,
    pub worker_id: &'a WorkerId,
    pub profiles: &'a BTreeMap<String, CharacterProfile>,
    pub status: &'a ControlStatus,
    pub now: i64,
    pub scroll: usize,
}

pub fn draw(frame: &mut Frame, area: Rect, context: &InspectorContext<'_>) -> Vec<HitRegion> {
    let mut hits = Vec::new();
    paint_opaque(frame, area, Style::default().fg(INK).bg(PANEL));
    if area.width < 28 || area.height < 12 {
        Paragraph::new("Inspector\nEnlarge the panel\nEsc back").render(area, frame.buffer_mut());
        return hits;
    }
    let inner = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
    let Some(worker) = context.world.worker(context.worker_id) else {
        Paragraph::new(
            "This task is no longer available.\nIts source may have been disconnected.\nEsc back",
        )
        .render(inner, frame.buffer_mut());
        return hits;
    };
    let profile = profile_for(&worker.id.0, context.profiles);
    Paragraph::new(format!(
        "{}  ·  {}",
        safe_display(&profile.name),
        profile.style.label()
    ))
    .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
    .render(
        Rect::new(inner.x, inner.y, inner.width, 1),
        frame.buffer_mut(),
    );
    let title = wrap_text(&safe_display(&worker.name), inner.width);
    let title_height = title.len().min(2) as u16;
    Paragraph::new(title.into_iter().take(2).collect::<Vec<_>>().join("\n")).render(
        Rect::new(inner.x, inner.y + 1, inner.width, title_height),
        frame.buffer_mut(),
    );
    let state_y = inner.y + 1 + title_height;
    Paragraph::new(format!(
        "{} · {}",
        worker.agent.label(),
        presentation::state_label(worker, context.now)
    ))
    .style(Style::default().fg(MUTED))
    .render(
        Rect::new(inner.x, state_y, inner.width, 1),
        frame.buffer_mut(),
    );
    Paragraph::new(presentation::coverage_label(&worker.coverage, context.now))
        .style(Style::default().fg(
            if worker.coverage.incomplete
                || !worker.coverage.available
                || worker.coverage.is_stale_at(context.now)
            {
                WARNING
            } else {
                MUTED
            },
        ))
        .render(
            Rect::new(inner.x, state_y + 1, inner.width, 1),
            frame.buffer_mut(),
        );
    let access = context.status.tasks.get(&worker.id.0);
    let mut buttons = Vec::new();
    if context
        .status
        .requests
        .iter()
        .any(|request| request.worker == worker.id)
    {
        buttons.push(("Review request", Action::Review(worker.id.clone())));
    }
    if access.is_some_and(|access| access.send) {
        buttons.push(("Send instruction", Action::Controls(worker.id.clone())));
    }
    if access.is_some_and(|access| access.native) {
        buttons.push((
            "Open conversation",
            Action::Control(super::control::Command::OpenNative {
                worker: worker.id.clone(),
            }),
        ));
    }
    if access.is_some_and(|access| access.interrupt) {
        buttons.push((
            "Stop task",
            Action::Control(super::control::Command::Interrupt {
                worker: worker.id.clone(),
            }),
        ));
    }
    if access.is_some_and(|access| access.reconnect) {
        buttons.push((
            "Reconnect",
            Action::Control(super::control::Command::Reconnect {
                worker: worker.id.clone(),
            }),
        ));
    }
    buttons.push(("Team", Action::Team(worker.id.clone())));
    buttons.push(("Character", Action::Character(worker.id.clone())));
    let mut x = inner.x;
    let mut y = state_y + 3;
    for (label, action) in buttons {
        let width = (label.len() as u16 + 4).min(inner.width);
        let (next_x, next_y) = if x + width > inner.right() {
            (inner.x, y + 1)
        } else {
            (x, y)
        };
        if next_y >= inner.bottom().saturating_sub(3) {
            break;
        }
        x = next_x;
        y = next_y;
        let button = Rect::new(x, y, width, 1);
        Paragraph::new(format!("[ {label} ]"))
            .style(Style::default().fg(INK).bg(PANEL_HIGHLIGHT))
            .render(button, frame.buffer_mut());
        hits.push(HitRegion::new(button, action));
        x += width + 1;
    }
    let body_y = y + 2;
    let body = Rect::new(
        inner.x,
        body_y,
        inner.width,
        inner.bottom().saturating_sub(body_y + 1),
    );
    let mut sections = vec![format!(
        "CURRENT WORK\n{}\n{}",
        worker.activity.detail().unwrap_or(worker.activity.label()),
        presentation::wait_description(worker.wait_reason)
    )];
    if !context.world.is_present(&worker.id) {
        sections.push(
            "This task is no longer on the current floor. Its recorded history remains available."
                .into(),
        );
    }
    if let Some(access) = access {
        sections.push(format!("CONNECTION\n{}", access.description));
    } else {
        sections.push(format!("CONNECTION\nObservation only. Open the original {} conversation to give instructions or respond. This source has not provided an available control connection.",worker.agent.label()));
    }
    if worker.tokens_used > 0 {
        sections.push(format!(
            "USAGE\n{} tokens reported by the source",
            super::human_tokens(worker.tokens_used)
        ));
    }
    sections.push(presentation::coverage_text(&worker.coverage, context.now));
    let family = context.world.family(&worker.id);
    if family.len() > 1 {
        let people = family
            .iter()
            .filter(|id| *id != &worker.id)
            .map(|id| {
                context.world.worker(id).map_or_else(
                    || "Task unavailable".into(),
                    |member| {
                        format!(
                            "{} · {}",
                            profile_for(&member.id.0, context.profiles).name,
                            member.name
                        )
                    },
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("RECORDED TEAM\n{people}"));
    }
    let events = context
        .world
        .collaboration_for(&worker.id)
        .rev()
        .take(5)
        .map(|event| {
            let actor = context.world.worker(&event.actor).map_or_else(
                || "An unavailable task".into(),
                |worker| profile_for(&worker.id.0, context.profiles).name,
            );
            let recipient = event
                .recipient
                .as_ref()
                .and_then(|id| context.world.worker(id))
                .map(|worker| profile_for(&worker.id.0, context.profiles).name);
            format!(
                "{}\n{}",
                presentation::event_summary(event, &actor, recipient.as_deref()),
                event.text.as_deref().unwrap_or("No message body recorded.")
            )
        })
        .collect::<Vec<_>>();
    if !events.is_empty() {
        sections.push(format!("RECENT COLLABORATION\n{}", events.join("\n\n")));
    }
    let history = worker
        .history
        .iter()
        .rev()
        .take(8)
        .map(|beat| {
            let outcome = match beat.outcome {
                Some(Outcome::Exited(code)) => format!(" · command exited with code {code}"),
                Some(Outcome::Changed { added, removed }) => {
                    format!(" · {added} lines added, {removed} removed")
                }
                None => String::new(),
            };
            format!(
                "{} ago · {}{}",
                super::duration_label(context.now.saturating_sub(beat.at)),
                beat.activity.detail().unwrap_or(beat.activity.label()),
                outcome,
            )
        })
        .collect::<Vec<_>>();
    if !history.is_empty() {
        sections.push(format!("RECENT ACTIVITY\n{}", history.join("\n")));
    }
    sections.push(format!(
        "CONVERSATION\n{}\nProject: {}",
        worker.name, worker.office.0
    ));
    let lines = wrap_text(&safe_multiline(&sections.join("\n\n")), body.width);
    let scroll = context
        .scroll
        .min(lines.len().saturating_sub(body.height as usize))
        .min(u16::MAX as usize) as u16;
    Paragraph::new(lines.join("\n"))
        .scroll((scroll, 0))
        .render(body, frame.buffer_mut());
    Paragraph::new("PgUp/Dn scroll · Esc back")
        .style(Style::default().fg(MUTED))
        .render(
            Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
            frame.buffer_mut(),
        );
    hits
}

#[cfg(test)]
mod tests {
    use super::super::control::TaskAccess;
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use theywork_core::{Agent, Event, EventKind, OfficeId};

    #[test]
    fn narrow_inspector_preserves_alias_real_title_and_only_available_actions() {
        let id = WorkerId("worker".into());
        let mut world = World::new();
        world.apply(Event {
            at: 1,
            office: OfficeId("/project".into()),
            office_path: "/project".into(),
            worker: id.clone(),
            agent: Agent::Codex,
            kind: EventKind::Seen {
                name: "Fix retries without changing clients".into(),
                git_branch: None,
            },
        });
        let profiles = BTreeMap::from([(
            "worker".into(),
            CharacterProfile {
                name: "Avery".into(),
                ..Default::default()
            },
        )]);
        let status = ControlStatus {
            tasks: BTreeMap::from([(
                "worker".into(),
                TaskAccess {
                    native: true,
                    description: "Observed in the original app".into(),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        let context = InspectorContext {
            world: &world,
            worker_id: &id,
            profiles: &profiles,
            status: &status,
            now: 2,
            scroll: 0,
        };
        let mut terminal = Terminal::new(TestBackend::new(90, 30)).unwrap();
        let mut hits = Vec::new();
        let area = Rect::new(50, 0, 40, 30);
        terminal
            .draw(|frame| hits = draw(frame, area, &context))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("Avery"));
        assert!(text.contains("Fix retries without changing clients"));
        assert!(hits.iter().any(|hit| matches!(
            hit.action,
            Action::Control(super::super::control::Command::OpenNative { .. })
        )));
        assert!(!hits.iter().any(|hit| matches!(
            hit.action,
            Action::Controls(_) | Action::Control(super::super::control::Command::Interrupt { .. })
        )));
        assert!(hits.iter().all(|hit| hit.area.x >= area.x
            && hit.area.right() <= area.right()
            && hit.area.bottom() <= area.bottom()));
        terminal
            .draw(|frame| hits = draw(frame, Rect::new(0, 0, 20, 8), &context))
            .unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn short_inspector_keeps_current_request_action_after_a_two_line_title() {
        let id = WorkerId("worker".into());
        let mut world = World::new();
        world.apply(Event {
            at: 1,
            office: OfficeId("/p".into()),
            office_path: "/p".into(),
            worker: id.clone(),
            agent: Agent::Codex,
            kind: EventKind::Seen {
                name: "Review the implementation and deployment plan together".into(),
                git_branch: None,
            },
        });
        let status = ControlStatus {
            requests: vec![super::super::control::Request {
                id: "current-request".into(),
                worker: id.clone(),
                origin: "Project".into(),
                title: "Approval".into(),
                detail: "Inspect before deciding".into(),
                choices: vec![],
                questions: vec![],
            }],
            ..Default::default()
        };
        let profiles = BTreeMap::new();
        let context = InspectorContext {
            world: &world,
            worker_id: &id,
            profiles: &profiles,
            status: &status,
            now: 2,
            scroll: 100,
        };
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        let mut hits = vec![];
        terminal
            .draw(|frame| hits = draw(frame, Rect::new(0, 0, 40, 12), &context))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Source not checked"));
        assert!(text.contains("[ Review request ]"));
        assert!(hits
            .iter()
            .any(|hit| hit.action == Action::Review(id.clone())));
    }

    #[test]
    fn coverage_stays_visible_while_scrolling_and_history_keeps_reported_outcomes() {
        use theywork_core::{Activity, Beat, SourceCoverage};
        let id = WorkerId("worker".into());
        let mut world = World::new();
        let event = |at, kind| Event {
            at,
            office: OfficeId("/project".into()),
            office_path: "/project".into(),
            worker: id.clone(),
            agent: Agent::Codex,
            kind,
        };
        world.apply(event(
            1,
            EventKind::Seen {
                name: "Verify changes".into(),
                git_branch: None,
            },
        ));
        world.apply(event(2, EventKind::Tokens(1234)));
        world.apply(event(
            3,
            EventKind::Did(Beat {
                at: 3,
                activity: Activity::Typing {
                    detail: "Run tests".into(),
                },
                outcome: Some(Outcome::Exited(7)),
            }),
        ));
        world.apply(event(
            4,
            EventKind::Did(Beat {
                at: 4,
                activity: Activity::Editing {
                    detail: "retry.rs".into(),
                },
                outcome: Some(Outcome::Changed {
                    added: 8,
                    removed: 2,
                }),
            }),
        ));
        world.apply(event(
            5,
            EventKind::Coverage(SourceCoverage {
                available: true,
                incomplete: true,
                observed_at: 5,
                ..Default::default()
            }),
        ));
        let profiles = BTreeMap::new();
        let status = ControlStatus::default();
        let mut context = InspectorContext {
            world: &world,
            worker_id: &id,
            profiles: &profiles,
            status: &status,
            now: 6,
            scroll: 0,
        };
        let mut terminal = Terminal::new(TestBackend::new(40, 24)).unwrap();
        let mut observed = String::new();
        for scroll in [0, 5, 10, 20, 1000] {
            context.scroll = scroll;
            terminal
                .draw(|frame| {
                    draw(frame, Rect::new(0, 0, 40, 24), &context);
                })
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(text.contains("Partial local history"));
            observed.push_str(&text);
        }
        let observed = observed.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(observed.contains("Observation only"));
        assert!(observed.contains("1.2K tokens"));
        assert!(observed.contains("command exited with code 7"));
        assert!(observed.contains("8 lines added, 2 removed"));
    }
}
