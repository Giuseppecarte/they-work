//! Native counterpart of the physical tower. The same real identities and
//! family boundaries remain actionable when image transport is unavailable.
use super::*;

/// Identity yields space to status and source observation age, never the reverse.
/// Both native rosters use this row; the real task title stays on the next line.
fn worker_header(
    worker: &Worker,
    ctx: &Context<'_>,
    selected: bool,
    alias: bool,
    width: u16,
) -> String {
    let compact = width < 48;
    let state = crate::presentation::state_label(worker, ctx.now);
    let state = if compact {
        match state {
            "Source unavailable" => "Unavailable",
            "Last known state" => "Last known",
            "Approval needed" => "Approval",
            "Question for you" => "Question",
            "Needs a follow-up" => "Follow-up",
            "Error reported" => "Error",
            "Automatic review" => "Auto review",
            "Waiting for the team" => "Team wait",
            "Waiting for a process" => "Process wait",
            other => other,
        }
    } else {
        state
    };
    let observed = crate::presentation::observation_age(&worker.coverage, ctx.now);
    let age = if compact && observed == "age unknown" {
        "unknown"
    } else if compact && observed.len() > 7 {
        observed.strip_suffix(" ago").unwrap_or(&observed)
    } else {
        &observed
    };
    let age_width = if compact { 7 } else { 11 };
    let identity_width = usize::from(width).saturating_sub(5 + state.len() + age_width);
    let identity = if alias {
        format!(
            "{} · {}",
            character_name(worker, ctx.profiles),
            worker.agent.label()
        )
    } else {
        worker.agent.label().into()
    };
    let identity = super::super::short_path(&identity, identity_width);
    let padding = " ".repeat(
        identity_width.saturating_sub(ratatui::text::Line::from(identity.as_str()).width()),
    );
    let header = format!(
        "{}{} {identity}{padding} {state} {age:>age_width$}",
        if selected { ">" } else { " " },
        state_marker(worker, ctx.now),
    );
    super::super::short_path(&header, width.into())
}

/// A tall office uses surplus space for actual work, rather than extending its
/// physical floor indefinitely. This is a project roster, not invented desks.
pub(super) fn latest(frame: &mut Frame, ctx: &Context<'_>, area: Rect) -> Vec<HitRegion> {
    let office = ctx.offices[ctx.selected_floor];
    native_cells(frame, area);
    Paragraph::new("")
        .style(Style::default().fg(INK).bg(BACKGROUND))
        .render(area, frame.buffer_mut());
    Paragraph::new("  Latest on this floor")
        .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .render(Rect::new(area.x, area.y, area.width, 1), frame.buffer_mut());
    let capacity = usize::from(area.height.saturating_sub(2) / 2).max(1);
    let selected = ctx
        .selected_worker
        .and_then(|id| office.workers.iter().position(|worker| &worker.id == id));
    let page = selected.unwrap_or(0) / capacity;
    let start = page * capacity;
    let mut hits = Vec::new();
    for (row, worker) in office.workers.iter().skip(start).take(capacity).enumerate() {
        let y = area.y + 1 + row as u16 * 2;
        let record = Rect::new(area.x + 1, y, area.width.saturating_sub(2), 2);
        let focused = ctx.selected_worker == Some(&worker.id);
        let warning = observed_attention(worker, ctx.now);
        let label = worker_header(worker, ctx, focused, true, record.width);
        let latest = worker
            .activity
            .detail()
            .filter(|text| !text.is_empty())
            .map(|text| format!(" · Latest: {}", safe_display(text)))
            .unwrap_or_default();
        let task = format!("  {}{}", safe_display(&worker.name), latest);
        let text = format!(
            "{}\n{}",
            super::super::short_path(&label, record.width.into()),
            super::super::short_path(&task, record.width.into())
        );
        Paragraph::new(text)
            .style(
                Style::default()
                    .fg(if warning {
                        WARNING
                    } else if focused {
                        ACCENT
                    } else {
                        INK
                    })
                    .bg(if focused {
                        super::super::PANEL_HIGHLIGHT
                    } else {
                        BACKGROUND
                    }),
            )
            .render(record, frame.buffer_mut());
        push_hit(&mut hits, record, Action::Inspect(worker.id.clone()));
    }
    let count = office.workers.len().saturating_sub(start).min(capacity);
    let pages = office.workers.len().div_ceil(capacity).max(1);
    let text = format!(
        "  {count}/{} people shown · List {}/{} · Select a person to inspect",
        office.workers.len(),
        page + 1,
        pages
    );
    Paragraph::new(super::super::short_path(&text, area.width.into()))
        .style(Style::default().fg(MUTED))
        .render(
            Rect::new(area.x, area.bottom() - 1, area.width, 1),
            frame.buffer_mut(),
        );
    hits
}

pub(super) fn draw(frame: &mut Frame, ctx: &Context<'_>, area: Rect) -> TowerLayout {
    let visible = if ctx.tower {
        floor_capacity(area.height).min(ctx.offices.len())
    } else {
        1
    };
    let start = if ctx.tower {
        window_start(ctx.selected_floor, ctx.offices.len(), visible)
    } else {
        ctx.selected_floor
    };
    let mut result = TowerLayout {
        visible_floors: visible,
        navigation_hint: format!("Floor {}/{}", ctx.selected_floor + 1, ctx.offices.len()),
        ..TowerLayout::default()
    };
    for index in start..start + visible {
        let slot = index - start;
        let top = area.y + (usize::from(area.height) * slot / visible) as u16;
        let bottom = area.y + (usize::from(area.height) * (slot + 1) / visible) as u16;
        let floor_area = Rect::new(area.x, top, area.width, bottom - top);
        let office = ctx.offices[index];
        let focused = index == ctx.selected_floor;
        let selected = ctx
            .selected_worker
            .filter(|id| focused && office.workers.iter().any(|worker| &worker.id == *id));
        let groups = meetings(ctx.world, office);
        let grouped = groups
            .iter()
            .flat_map(|group| std::iter::once(&group.parent).chain(&group.members))
            .collect::<BTreeSet<_>>();
        let independent = office
            .workers
            .iter()
            .filter(|worker| !grouped.contains(&worker.id))
            .count();
        let selected_desks = selected.is_some_and(|id| !grouped.contains(id));
        let group = if selected_desks {
            None
        } else {
            selected
                .and_then(|id| {
                    groups
                        .iter()
                        .find(|group| &group.parent == id || group.members.contains(id))
                })
                .or_else(|| {
                    ctx.team
                        .filter(|_| focused)
                        .and_then(|id| groups.iter().find(|group| &group.parent == id))
                })
                .or_else(|| groups.first())
        };
        let team = group.map(|group| group.parent.clone());
        let all = if let Some(group) = group {
            std::iter::once(&group.parent)
                .chain(&group.members)
                .filter_map(|id| office.workers.iter().find(|worker| &worker.id == id))
                .collect::<Vec<_>>()
        } else {
            office
                .workers
                .iter()
                .filter(|worker| !grouped.contains(&worker.id))
                .collect::<Vec<_>>()
        };
        let selector = (focused && !groups.is_empty() && floor_area.height >= 6)
            .then_some(Rect::new(area.x, top + 1, area.width, 1));
        let content_y = top + 1 + u16::from(selector.is_some());
        let available = bottom.saturating_sub(content_y + 1);
        let row_height = if available >= 3 { 3 } else { 2 };
        let capacity = usize::from(available / row_height).max(1);
        let (people, page, pages) = page_people(&all, capacity, selected, group.is_some());
        let total = all.len();
        if focused {
            result.capacity = capacity;
            if !ctx.tower {
                let scope = group.map_or_else(
                    || "Desks".into(),
                    |group| {
                        format!(
                            "Team {}/{}",
                            groups
                                .iter()
                                .position(|item| item.parent == group.parent)
                                .unwrap_or(0)
                                + 1,
                            groups.len()
                        )
                    },
                );
                result
                    .navigation_hint
                    .push_str(&format!(" · {scope} · People {}/{total}", people.len()));
                if pages > 1 {
                    result
                        .navigation_hint
                        .push_str(&format!(" · page {}/{}", page + 1, pages));
                }
            }
        }
        let floor = PaintedFloor {
            index,
            office,
            area: floor_area,
            selector,
            groups,
            independent,
            team,
            scenes: Vec::new(),
        };
        result.hits.push(HitRegion::new(
            floor_area,
            Action::SelectFloor(office.id.clone()),
        ));
        draw_header(
            frame,
            &floor,
            ctx.offices.len(),
            focused,
            ctx.now,
            &mut result.hits,
        );
        if let Some(selector) = selector {
            draw_teams(frame, selector, &floor, ctx.profiles, &mut result.hits);
        }
        for (row, worker) in people.iter().enumerate() {
            let y = content_y + row as u16 * row_height;
            let height = row_height.min(bottom.saturating_sub(y + 1));
            if height == 0 {
                break;
            }
            let rect = Rect::new(area.x + 1, y, area.width.saturating_sub(2), height);
            let selected = selected == Some(&worker.id);
            let warning = observed_attention(worker, ctx.now);
            let style = Style::default()
                .fg(if warning {
                    WARNING
                } else if selected {
                    ACCENT
                } else {
                    INK
                })
                .bg(if selected {
                    super::super::PANEL_HIGHLIGHT
                } else {
                    BACKGROUND
                });
            let alias = ctx.name_plates || selected || crate::presentation::human_request(worker);
            let header = worker_header(worker, ctx, selected, alias, rect.width);
            let mut lines = vec![header];
            if height > 1 {
                lines.push(super::super::short_path(
                    &format!("  {}", safe_display(&worker.name)),
                    rect.width.into(),
                ));
            }
            if height > 2 {
                lines.push(super::super::short_path(
                    &format!(
                        "  {}",
                        crate::presentation::coverage_label(&worker.coverage, ctx.now)
                    ),
                    rect.width.into(),
                ));
            }
            Paragraph::new(lines.join("\n"))
                .style(style)
                .render(rect, frame.buffer_mut());
            push_hit(&mut result.hits, rect, Action::Inspect(worker.id.clone()));
        }
        if available > 0 && people.is_empty() {
            let empty = Rect::new(area.x + 1, content_y, area.width.saturating_sub(2), 1);
            Paragraph::new("No conversations on this floor. Use New task.")
                .style(Style::default().fg(MUTED))
                .render(empty, frame.buffer_mut());
        }
        if floor_area.height > 2 {
            let footer = Rect::new(area.x + 1, bottom - 1, area.width.saturating_sub(2), 1);
            let text = format!(
                "People {}/{} · page {}/{}{}",
                people.len(),
                total,
                page + 1,
                pages,
                if ctx.tower {
                    " · Enter visit"
                } else {
                    " · arrows people"
                }
            );
            Paragraph::new(super::super::short_path(&text, footer.width.into()))
                .style(Style::default().fg(MUTED).bg(BACKGROUND))
                .render(footer, frame.buffer_mut());
        }
    }
    result.capacity = result.capacity.max(1);
    result
}

fn page_people<'a>(
    all: &[&'a Worker],
    capacity: usize,
    selected: Option<&WorkerId>,
    meeting: bool,
) -> (Vec<&'a Worker>, usize, usize) {
    let pinned = meeting && all.len() > 1 && capacity > 1;
    let offset = usize::from(pinned);
    let chunk = capacity - offset;
    let page = selected
        .and_then(|id| all.iter().position(|worker| &worker.id == id))
        .map_or(0, |index| index.saturating_sub(offset) / chunk);
    let pages = all.len().saturating_sub(offset).div_ceil(chunk).max(1);
    let mut visible = Vec::new();
    if pinned {
        visible.push(all[0]);
    }
    visible.extend(all.iter().skip(offset + page * chunk).take(chunk).copied());
    (visible, page, pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn native_fallback_has_real_clicks_and_keeps_the_selected_family_visible() {
        let world = super::super::tests::family_world();
        let offices = world.offices().collect::<Vec<_>>();
        let selected = WorkerId("child-b".into());
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut canvas = Canvas::new(0, 0);
        let mut studio = Studio::new();
        let mut result = None;
        terminal
            .draw(|frame| {
                result = super::super::draw(
                    frame,
                    &mut canvas,
                    &mut studio,
                    Context {
                        area: Rect::new(0, 2, 80, 20),
                        world: &world,
                        offices: &offices,
                        selected_floor: 0,
                        selected_worker: Some(&selected),
                        team: None,
                        now: 1_000,
                        tower: true,
                        motion: false,
                        name_plates: true,
                        light: false,
                        palette: 0,
                        wardrobe: &BTreeMap::new(),
                        profiles: &BTreeMap::new(),
                        designs: &BTreeMap::new(),
                    },
                );
            })
            .unwrap();
        let result = result.unwrap();
        assert!(result
            .hits
            .iter()
            .any(|hit| hit.action == Action::Inspect(selected.clone())));
        assert!(!result
            .hits
            .iter()
            .any(|hit| hit.action == Action::Inspect(WorkerId("child-a".into()))));
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("page 1/1"));
        assert!(text.contains("Source not checked"));
        assert!(text.contains("Implement a long shared task title"));
    }

    #[test]
    fn both_native_rosters_reserve_state_and_source_age_with_long_aliases() {
        use theywork_core::{Agent, Event, EventKind, OfficeId, SourceCoverage};

        let selected = WorkerId("long-alias".into());
        for observed_at in [0, 1_000] {
            let mut world = World::new();
            for kind in [
                EventKind::Seen {
                    name: "Implement real task title with a long path and many details".into(),
                    git_branch: None,
                },
                EventKind::Coverage(SourceCoverage {
                    observed_at,
                    available: true,
                    ..SourceCoverage::default()
                }),
                EventKind::Wait(Some(WaitReason::HumanApproval)),
            ] {
                world.apply(Event {
                    at: 61_000,
                    office: OfficeId("/project".into()),
                    office_path: "/project".into(),
                    worker: selected.clone(),
                    agent: Agent::Codex,
                    kind,
                });
            }
            let offices = world.offices().collect::<Vec<_>>();
            let profiles = BTreeMap::from([(
                selected.0.clone(),
                CharacterProfile {
                    name: "Avery with a very long decorative name 界界界界界界".into(),
                    ..CharacterProfile::default()
                },
            )]);
            for width in [32, 80, 192] {
                for latest_mode in [false, true] {
                    let area = Rect::new(0, 0, width, 12);
                    let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
                    let mut hits = Vec::new();
                    terminal
                        .draw(|frame| {
                            let ctx = Context {
                                area,
                                world: &world,
                                offices: &offices,
                                selected_floor: 0,
                                selected_worker: Some(&selected),
                                team: None,
                                now: 61_000,
                                tower: false,
                                motion: false,
                                name_plates: true,
                                light: false,
                                palette: 0,
                                wardrobe: &BTreeMap::new(),
                                profiles: &profiles,
                                designs: &BTreeMap::new(),
                            };
                            hits = if latest_mode {
                                latest(frame, &ctx, area)
                            } else {
                                draw(frame, &ctx, area).hits
                            };
                        })
                        .unwrap();
                    let hit = hits
                        .iter()
                        .find(|hit| hit.action == Action::Inspect(selected.clone()))
                        .unwrap();
                    assert_eq!(hit.area.height, if latest_mode { 2 } else { 3 });
                    let line = |y| {
                        (hit.area.x..hit.area.right())
                            .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                            .collect::<String>()
                    };
                    let header = line(hit.area.y);
                    assert!(header.starts_with(">! Avery"), "{header}");
                    assert!(
                        header.contains(if width == 32 {
                            "Approval"
                        } else {
                            "Approval needed"
                        }),
                        "{header}"
                    );
                    assert!(
                        header.ends_with(if observed_at == 0 {
                            "unknown"
                        } else {
                            "1m ago"
                        }),
                        "{header}"
                    );
                    assert!(!header.contains("0s ago"), "{header}");
                    assert!(line(hit.area.y + 1).contains("Implement real task"));
                    assert!(hit.area.right() <= width && hit.area.bottom() <= 12);
                }
            }
        }
    }

    #[test]
    fn native_pagination_pins_the_parent_without_duplicating_people() {
        let office = theywork_core::OfficeId("p".into());
        let workers = (0..20)
            .map(|index| {
                Worker::new(
                    WorkerId(format!("w{index}")),
                    office.clone(),
                    theywork_core::Agent::Codex,
                    format!("Task {index}"),
                    0,
                )
            })
            .collect::<Vec<_>>();
        let all = workers.iter().collect::<Vec<_>>();
        let (page, index, pages) = page_people(&all, 3, Some(&WorkerId("w19".into())), true);
        assert_eq!(index, 9);
        assert_eq!(pages, 10);
        assert_eq!(page[0].id, WorkerId("w0".into()));
        assert_eq!(page[1].id, WorkerId("w19".into()));
        assert_eq!(page.len(), 2);
    }
}
