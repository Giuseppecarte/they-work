//! Graphics-first side-cut tower. Provider relationships choose rooms; art
//! never changes the roster, lifecycle, message history or attention state.
use super::{safe_display, ACCENT, BACKGROUND, INK, MUTED, WARNING};
use crate::{
    canvas::Canvas,
    living_office::{MeetingGroup, PixelRect, SceneCue, SceneOptions, Studio},
};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Paragraph, Widget},
    Frame,
};
use std::collections::{BTreeMap, BTreeSet};
use theywork_core::{
    Millis, Office, RelationshipKind, WaitReason, Worker, WorkerId, WorkerLifecycle, WorkerStatus,
    World,
};

pub(crate) struct Context<'a> {
    pub world: &'a World,
    pub offices: &'a [&'a Office],
    pub selected_floor: usize,
    pub selected_worker: Option<&'a WorkerId>,
    pub now: Millis,
    pub tower: bool,
    pub motion: bool,
    pub light: bool,
    pub palette: usize,
    pub wardrobe: &'a BTreeMap<String, usize>,
}

pub(crate) fn draw(
    frame: &mut Frame,
    canvas: &mut Canvas,
    studio: &mut Studio,
    ctx: Context<'_>,
) -> Option<usize> {
    let office = *ctx.offices.get(ctx.selected_floor)?;
    let area = super::below_tab_bar(frame.area());
    if !canvas.has_image_density() || area.width < 50 || area.height < 16 {
        return None;
    }
    let body = Rect::new(area.x, area.y + 1, area.width, area.height - 4);
    canvas.resize_for_cells(body.width as usize, body.height as usize);
    let (cw, ch) = canvas.pixels_per_cell();
    let context_height = if ctx.tower && ctx.offices.len() > 1 {
        ch * 3
    } else {
        0
    };
    let room = PixelRect {
        x: 0,
        y: context_height,
        width: canvas.width(),
        height: canvas.height().saturating_sub(context_height * 2),
    };
    if room.width < 280 || room.height < 156 {
        return None;
    }
    canvas.fill(Color::Rgb(28, 35, 48));
    let group = meeting(ctx.world, office, ctx.selected_worker);
    let mut cues = BTreeMap::new();
    for worker in &office.workers {
        if worker.wait_reason == Some(WaitReason::Child) {
            cues.insert(worker.id.0.clone(), SceneCue::WaitingForTeam);
        }
    }
    for event in ctx
        .world
        .collaboration()
        .filter(|event| event.at <= ctx.now && ctx.now.saturating_sub(event.at) < 6_000)
    {
        let cue = match event.kind {
            theywork_core::CollaborationKind::Delegated => Some(SceneCue::Delegating),
            theywork_core::CollaborationKind::Message => Some(SceneCue::Message),
            theywork_core::CollaborationKind::Result => Some(SceneCue::Delivering),
            _ => None,
        };
        if let Some(cue) = cue {
            cues.insert(event.actor.0.clone(), cue);
        }
    }
    let mut layouts = Vec::new();
    let options = |meeting, selected| SceneOptions {
        now: ctx.now,
        motion: ctx.motion,
        light: ctx.light,
        palette: ctx.palette,
        selected_worker: selected,
        page: 0,
        meeting,
        wardrobe: Some(ctx.wardrobe),
        cues: Some(&cues),
        show_elevator: true,
    };
    let mut desks = office.clone();
    if let Some(group) = &group {
        desks
            .workers
            .retain(|worker| worker.id != group.parent && !group.members.contains(&worker.id));
    }
    let split_width = if group.is_some() && !desks.workers.is_empty() {
        readable_split_width(room, cw)
    } else {
        None
    };
    let split = split_width.is_some();
    if let Some(left_width) = split_width {
        let left = PixelRect {
            width: left_width,
            ..room
        };
        let right = PixelRect {
            x: left_width,
            width: room.width - left_width,
            ..room
        };
        layouts.push((
            studio.paint_region(canvas, left, &desks, &options(None, ctx.selected_worker)),
            "DESKS",
        ));
        let mut meeting_options = options(group.as_ref(), ctx.selected_worker);
        meeting_options.show_elevator = false;
        layouts.push((
            studio.paint_region(canvas, right, office, &meeting_options),
            "MEETING",
        ));
    } else {
        // On compact displays the selected team's room fills the floor. Its
        // colleagues remain reachable with arrows, at their ordinary desks.
        let independent = ctx
            .selected_worker
            .is_some_and(|id| desks.workers.iter().any(|worker| &worker.id == id));
        if group.is_some() && independent && !desks.workers.is_empty() {
            layouts.push((
                studio.paint_region(canvas, room, &desks, &options(None, ctx.selected_worker)),
                "DESKS",
            ));
        } else {
            layouts.push((
                studio.paint_region(
                    canvas,
                    room,
                    office,
                    &options(group.as_ref(), ctx.selected_worker),
                ),
                if group.is_some() { "MEETING" } else { "FLOOR" },
            ));
        }
    }
    if !layouts.iter().any(|(layout, _)| layout.graphics) {
        return None;
    }
    let shaft = layouts
        .first()
        .map_or(8 * cw, |(layout, _)| layout.scale * 78);
    if context_height > 0 {
        for start in [0, room.y + room.height] {
            floor_band(canvas, start, context_height, shaft);
        }
    }
    canvas.render(frame.buffer_mut(), body);
    let attention = office
        .workers
        .iter()
        .filter(|worker| worker.status_at(ctx.now).needs_attention())
        .count();
    let heading = format!(
        "  F{:02} / {:02}  {}   ·   {} workers   ·   {} attention",
        ctx.selected_floor + 1,
        ctx.offices.len(),
        safe_display(&office.name),
        office.workers.len(),
        attention
    );
    Paragraph::new(heading)
        .style(Style::default().fg(INK).bg(BACKGROUND))
        .render(Rect::new(area.x, area.y, area.width, 1), frame.buffer_mut());
    let mut capacity = 0;
    for (layout, kind) in &layouts {
        if !layout.graphics {
            continue;
        }
        capacity += layout.seats.len();
        let mut sign = native_rect(layout.sign, body, cw, ch);
        sign.y += sign.height.saturating_sub(1) / 2;
        sign.height = 1;
        native_cells(frame, sign);
        let label = if split {
            format!(
                "{} · {}  {}/{}",
                safe_display(&office.name),
                kind,
                layout.page + 1,
                layout.page_count
            )
        } else {
            format!(
                "{}   {}  {}/{}",
                safe_display(&office.name),
                kind,
                layout.page + 1,
                layout.page_count
            )
        };
        Paragraph::new(label)
            .style(Style::default().fg(INK).add_modifier(Modifier::BOLD))
            .render(sign, frame.buffer_mut());
        for seat in &layout.seats {
            let Some(worker) = office
                .workers
                .iter()
                .find(|worker| worker.id == seat.worker_id)
            else {
                continue;
            };
            let mut plate = native_rect(seat.nameplate, body, cw, ch);
            plate.height = plate.height.min(2);
            native_cells(frame, plate);
            let selected = ctx.selected_worker == Some(&worker.id);
            let color = if worker.status_at(ctx.now).needs_attention() {
                WARNING
            } else if selected {
                ACCENT
            } else {
                INK
            };
            let marker = if worker.status_at(ctx.now).needs_attention() {
                "!"
            } else if selected {
                "›"
            } else {
                " "
            };
            let name = super::short_path(
                &safe_display(&worker.name),
                plate.width.saturating_sub(1) as usize,
            );
            Paragraph::new(format!("{marker}{name}\n{}", state(worker, ctx.now)))
                .style(Style::default().fg(color))
                .render(plate, frame.buffer_mut());
        }
    }
    if context_height > 0 {
        for (start, adjacent) in [
            (0, ctx.selected_floor.checked_sub(1)),
            (room.y + room.height, Some(ctx.selected_floor + 1)),
        ] {
            let label = adjacent
                .and_then(|index| ctx.offices.get(index).map(|office| (index, *office)))
                .map(|(index, office)| {
                    format!(
                        "F{:02}  {}  ·  {} workers",
                        index + 1,
                        safe_display(&office.name),
                        office.workers.len()
                    )
                })
                .unwrap_or_else(|| {
                    if start == 0 {
                        "THEY-WORK / SOFTWARE TOWER".into()
                    } else {
                        "LOBBY / ↑ ↓ ELEVATOR".into()
                    }
                });
            let band_x = (shaft / cw + 2).min(body.width.saturating_sub(16) as usize) as u16;
            let band = Rect::new(
                body.x + band_x,
                body.y + (start / ch) as u16 + 1,
                body.width - band_x - 1,
                1,
            );
            native_cells(frame, band);
            Paragraph::new(label)
                .style(Style::default().fg(MUTED))
                .render(band, frame.buffer_mut());
        }
    }
    let selected = ctx
        .selected_worker
        .and_then(|id| office.workers.iter().find(|worker| &worker.id == id));
    let detail = selected
        .map(|worker| {
            format!(
                "  {}  ·  {}  ·  {}",
                safe_display(&worker.name),
                state(worker, ctx.now),
                safe_display(worker.activity.detail().unwrap_or(""))
            )
        })
        .unwrap_or_else(|| "  Empty floor · n creates a task".into());
    Paragraph::new(detail)
        .style(Style::default().fg(INK).bg(BACKGROUND))
        .render(
            Rect::new(area.x, body.bottom(), area.width, 1),
            frame.buffer_mut(),
        );
    let relation = group
        .as_ref()
        .map(|group| {
            format!(
                "  Lead: {} · {} participants · g tree · b deliveries",
                ctx.world
                    .worker(&group.parent)
                    .map(|worker| safe_display(&worker.name))
                    .unwrap_or_else(|| "unavailable".into()),
                group.members.len() + 1
            )
        })
        .unwrap_or_else(|| {
            "  b attention / deliveries / changes · g team · m controls · n new task".into()
        });
    Paragraph::new(relation)
        .style(Style::default().fg(MUTED).bg(BACKGROUND))
        .render(
            Rect::new(area.x, body.bottom() + 1, area.width, 1),
            frame.buffer_mut(),
        );
    let footer = if ctx.tower {
        "↑/↓ elevator · Enter visit · / find · b notebook · n new · ? help"
    } else {
        "arrows worker · Enter inspect · m controls · Tab elevator · Esc tower · ? help"
    };
    Paragraph::new(format!("  {footer}"))
        .style(Style::default().fg(ACCENT).bg(BACKGROUND))
        .render(
            Rect::new(area.x, area.bottom() - 1, area.width, 1),
            frame.buffer_mut(),
        );
    Some(capacity.max(1))
}

/// Preserve the scene's height-selected character scale in both rooms.
/// A fixed width threshold would halve tall rooms into tiny people and blank
/// walls. Use the actual cell-aligned halves, including their unequal remainder.
fn readable_split_width(room: PixelRect, cell_width: usize) -> Option<usize> {
    if cell_width == 0 {
        return None;
    }
    let scale = (room.height / 150).clamp(1, 4);
    // 108 authored pixels of circulation plus two 92-pixel seats is 292.
    // Keep a small margin so each meeting page can show its lead and one child.
    let minimum = 304 * scale;
    let left = (room.width / cell_width / 2) * cell_width;
    (left >= minimum && room.width - left >= minimum).then_some(left)
}

fn native_cells(frame: &mut Frame, area: Rect) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            frame.buffer_mut()[(x, y)].set_symbol(" ").set_skip(false);
        }
    }
}

fn native_rect(rect: PixelRect, area: Rect, cw: usize, ch: usize) -> Rect {
    let x = (rect.x / cw).min(area.width as usize) as u16;
    let y = (rect.y / ch).min(area.height as usize) as u16;
    Rect::new(
        area.x + x,
        area.y + y,
        (rect.width / cw)
            .max(1)
            .min(area.width.saturating_sub(x) as usize) as u16,
        (rect.height / ch)
            .max(1)
            .min(area.height.saturating_sub(y) as usize) as u16,
    )
}

fn floor_band(canvas: &mut Canvas, y: usize, height: usize, shaft: usize) {
    for py in y..(y + height).min(canvas.height()) {
        for x in 0..canvas.width() {
            let color = if py < y + 3 || py + 4 >= y + height {
                Color::Rgb(64, 73, 87)
            } else if x < shaft {
                Color::Rgb(43, 55, 72)
            } else {
                Color::Rgb(30, 39, 53)
            };
            canvas.set(x, py, color);
        }
    }
}

fn state(worker: &Worker, now: Millis) -> &'static str {
    match worker.wait_reason {
        Some(WaitReason::HumanApproval) => "approval needed",
        Some(WaitReason::HumanInput) => "answer needed",
        Some(WaitReason::AutomaticReview) => "automatic review",
        Some(WaitReason::Child) => "waiting for team",
        Some(WaitReason::Process) => "waiting for process",
        _ => match worker.status_at(now) {
            WorkerStatus::Blocked => "attention · inspect reason",
            WorkerStatus::Failed => "error",
            WorkerStatus::Running => "working",
            WorkerStatus::Idle => "idle",
        },
    }
}

fn meeting(world: &World, office: &Office, selected: Option<&WorkerId>) -> Option<MeetingGroup> {
    meeting_for(world, office, selected).or_else(|| {
        let parents: BTreeSet<_> = world
            .relationships()
            .filter(|relation| relation.kind == RelationshipKind::Delegation)
            .map(|relation| &relation.parent)
            .collect();
        parents
            .into_iter()
            .filter(|id| office.workers.iter().any(|worker| worker.id == **id))
            .find_map(|id| meeting_for(world, office, Some(id)))
    })
}

fn meeting_for(
    world: &World,
    office: &Office,
    selected: Option<&WorkerId>,
) -> Option<MeetingGroup> {
    let selected = selected?;
    let mut root = selected.clone();
    let present: BTreeSet<_> = office
        .workers
        .iter()
        .map(|worker| worker.id.clone())
        .collect();
    let mut visited = BTreeSet::new();
    while visited.insert(root.clone()) {
        if let Some(parent) = world.relationships().find(|relation| {
            relation.kind == RelationshipKind::Delegation
                && relation.child == root
                && present.contains(&relation.parent)
        }) {
            root = parent.parent.clone();
        } else {
            break;
        }
    }
    let mut members = Vec::new();
    let mut queue = vec![root.clone()];
    let mut seen = BTreeSet::from([root.clone()]);
    while let Some(parent) = queue.pop() {
        for relation in world.relationships().filter(|relation| {
            relation.kind == RelationshipKind::Delegation && relation.parent == parent
        }) {
            if present.contains(&relation.child) && seen.insert(relation.child.clone()) {
                members.push(relation.child.clone());
                queue.push(relation.child.clone());
            }
        }
    }
    if !members.iter().any(|id| {
        world.worker(id).is_some_and(|worker| {
            worker.turn_in_flight || worker.lifecycle == WorkerLifecycle::Active
        })
    }) {
        return None;
    }
    Some(MeetingGroup {
        parent: root,
        members,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn room_splits_preserve_character_scale_including_cell_alignment() {
        let mut office = Office::new(theywork_core::OfficeId("/p".into()), "/p".into());
        for id in ["lead", "child-a", "child-b"] {
            office.workers.push(Worker::new(
                WorkerId(id.into()),
                office.id.clone(),
                theywork_core::Agent::Codex,
                id.into(),
                0,
            ));
        }
        let group = MeetingGroup {
            parent: office.workers[0].id.clone(),
            members: office
                .workers
                .iter()
                .skip(1)
                .map(|worker| worker.id.clone())
                .collect(),
        };
        let options = SceneOptions {
            motion: false,
            meeting: Some(&group),
            selected_worker: Some(&office.workers[2].id),
            ..SceneOptions::default()
        };
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((1, 1)));
        for (width, height, cell_width, expected) in [
            (640, 304, 8, None),
            (1056, 592, 8, None),
            (1536, 848, 8, None),
            (1120, 304, 8, None),
            (1216, 304, 8, Some(608)),
            (1680, 592, 8, None),
            (1824, 592, 8, Some(912)),
            (1824, 592, 32, None),
            (1856, 592, 32, Some(928)),
            (2240, 848, 8, None),
            (2432, 848, 8, Some(1216)),
            (640, 196, 8, Some(320)),
        ] {
            let room = PixelRect {
                width,
                height,
                ..PixelRect::default()
            };
            let split = readable_split_width(room, cell_width);
            assert_eq!(split, expected, "{width}x{height}, cell width {cell_width}");
            canvas.resize(width, height);
            let full = studio.paint(&mut canvas, &office, &options);
            if let Some(left) = split {
                for half in [left, width - left] {
                    canvas.resize(half, height);
                    let divided = studio.paint(&mut canvas, &office, &options);
                    assert!(divided.graphics);
                    assert_eq!(
                        divided.scale, full.scale,
                        "{width}x{height} narrowed to {half}"
                    );
                    assert!(divided.seats.len() >= 2);
                    assert_eq!(divided.seats[0].worker_id, group.parent);
                    assert!(divided
                        .seats
                        .iter()
                        .any(|seat| seat.worker_id == office.workers[2].id));
                }
            } else {
                assert_eq!(full.scale, (height / 150).clamp(1, 4));
            }
        }
    }

    #[test]
    fn source_membership_alone_does_not_invent_a_meeting() {
        let world = World::new();
        let office = Office::new(theywork_core::OfficeId("/p".into()), "/p".into());
        assert!(meeting(&world, &office, Some(&WorkerId("x".into()))).is_none());
    }
}
