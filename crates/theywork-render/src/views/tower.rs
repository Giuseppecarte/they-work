//! Graphics-first side-cut tower. Provider relationships choose rooms; art
//! never changes the roster, lifecycle, message history or attention state.
use super::{safe_display, ACCENT, BACKGROUND, INK, MUTED, WARNING};
mod compact;
use crate::{
    canvas::Canvas,
    design::{office_design_for, profile_for, CharacterProfile, OfficeDesign},
    interaction::{Action, HitRegion},
    living_office::{MeetingGroup, PixelRect, SceneCue, SceneLayout, SceneOptions, Studio},
};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Paragraph, Widget},
    Frame,
};
use std::collections::{BTreeMap, BTreeSet};
use theywork_core::{
    Millis, Office, RelationshipKind, WaitReason, Worker, WorkerId, WorkerLifecycle, World,
};

pub(crate) struct Context<'a> {
    /// The host reserves its navigation bar and inspector before choosing this area.
    pub area: Rect,
    pub world: &'a World,
    pub offices: &'a [&'a Office],
    pub selected_floor: usize,
    pub selected_worker: Option<&'a WorkerId>,
    pub team: Option<&'a WorkerId>,
    pub now: Millis,
    pub tower: bool,
    pub motion: bool,
    /// Hide optional aliases, never the selected identity or a recorded request.
    pub name_plates: bool,
    pub light: bool,
    pub palette: usize,
    pub wardrobe: &'a BTreeMap<String, usize>,
    pub profiles: &'a BTreeMap<String, CharacterProfile>,
    pub designs: &'a BTreeMap<String, OfficeDesign>,
}

#[derive(Debug, Default)]
pub(crate) struct TowerLayout {
    /// Available positions on the selected floor, stable across a partial last page.
    pub capacity: usize,
    pub visible_floors: usize,
    /// Names the displayed scope rather than deriving a page from all project workers.
    pub navigation_hint: String,
    /// Back-to-front order. The host resolves the last matching presented hit.
    pub hits: Vec<HitRegion>,
}

struct PaintedFloor<'a> {
    index: usize,
    office: &'a Office,
    area: Rect,
    selector: Option<Rect>,
    groups: Vec<MeetingGroup>,
    independent: usize,
    team: Option<WorkerId>,
    scenes: Vec<(SceneLayout, &'static str)>,
}

/// Observation health has precedence over the last recorded activity, just as
/// in the inspector. Unavailable workers must not increase a live work count.
#[derive(Default)]
pub(super) struct FloorCounts {
    pub total: usize,
    pub working: usize,
    pub ready: usize,
    pub attention: usize,
    pub waiting: usize,
    pub unavailable: usize,
    pub stale: usize,
}

impl FloorCounts {
    pub fn new(office: &Office, now: Millis) -> Self {
        let mut result = Self::default();
        for worker in &office.workers {
            result.total += 1;
            match crate::presentation::state_label(worker, now) {
                "Source unavailable" => result.unavailable += 1,
                "Last known state" => result.stale += 1,
                "Working" => result.working += 1,
                "Ready" => result.ready += 1,
                "Approval needed" | "Question for you" | "Needs a follow-up" | "Error reported" => {
                    result.attention += 1
                }
                _ => result.waiting += 1,
            }
        }
        result
    }
    pub fn compact(&self) -> String {
        let mut parts = vec![person_count(self.total)];
        if self.unavailable > 0 {
            parts.push(format!("{} offline", self.unavailable));
        }
        if self.stale > 0 {
            parts.push(format!("{} stale", self.stale));
        }
        if self.attention > 0 {
            parts.push(format!("! {}", self.attention));
        }
        if self.working > 0 {
            parts.push(format!("{} working", self.working));
        }
        if self.waiting > 0 {
            parts.push(format!("{} waiting", self.waiting));
        }
        if self.ready == self.total && self.total > 0 {
            parts.push("ready".into());
        }
        parts.join(" · ")
    }
}

fn observed_attention(worker: &Worker, now: Millis) -> bool {
    crate::presentation::needs_attention(worker, now)
}

fn state_marker(worker: &Worker, now: Millis) -> &'static str {
    match crate::presentation::state_label(worker, now) {
        "Source unavailable" => "-",
        "Last known state" => "~",
        "Approval needed" | "Question for you" => "!",
        "Needs a follow-up" => "?",
        "Error reported" => "×",
        "Working" => "▸",
        "Ready" => "·",
        _ => "…",
    }
}

pub(crate) fn draw(
    frame: &mut Frame,
    canvas: &mut Canvas,
    studio: &mut Studio,
    ctx: Context<'_>,
) -> Option<TowerLayout> {
    ctx.offices.get(ctx.selected_floor)?;
    let area = ctx.area.intersection(frame.area());
    if area.width == 0 || area.height == 0 {
        return None;
    }
    if !canvas.has_image_density() || area.width < 36 || area.height < 10 {
        return Some(compact::draw(frame, &ctx, area));
    }
    canvas.resize_for_cells(area.width as usize, area.height as usize);
    let (cw, ch) = canvas.pixels_per_cell();
    if cw == 0 || ch == 0 || canvas.width() < 280 {
        return Some(compact::draw(frame, &ctx, area));
    }
    canvas.fill(if ctx.light {
        super::LIGHT_BACKGROUND
    } else {
        BACKGROUND
    });
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
    let building_width = if ctx.tower {
        area.width.min((1120 / cw).max(1) as u16)
    } else {
        area.width
    };
    let building_x = area.x + (area.width - building_width) / 2;
    let building_height = if ctx.tower {
        area.height.min((visible * 11) as u16)
    } else {
        area.height.min((512 / ch).max(1) as u16)
    };
    let building_y = if ctx.tower {
        area.y + (area.height - building_height) / 2
    } else {
        area.y
    };
    if ctx.tower && (building_height < area.height || building_width < area.width) {
        exterior(
            canvas,
            PixelRect {
                x: usize::from(building_x - area.x) * cw,
                y: usize::from(building_y - area.y) * ch,
                width: usize::from(building_width) * cw,
                height: usize::from(building_height) * ch,
            },
            ch,
            ctx.light,
        );
    }
    let cues = observed_cues(ctx.world, ctx.now);
    let mut painted = Vec::new();
    for index in start..start + visible {
        let office = ctx.offices[index];
        let slot = index - start;
        // Partition in whole terminal rows. No fractional rescaling, seams or
        // off-screen half-floors appear when the window changes dimensions.
        let top = building_y + (usize::from(building_height) * slot / visible) as u16;
        let bottom = building_y + (usize::from(building_height) * (slot + 1) / visible) as u16;
        let floor_area = Rect::new(building_x, top, building_width, bottom - top);
        let focused = index == ctx.selected_floor;
        let selected = ctx
            .selected_worker
            .filter(|id| focused && office.workers.iter().any(|worker| &worker.id == *id));
        let groups = meetings(ctx.world, office);
        let grouped: BTreeSet<_> = groups
            .iter()
            .flat_map(|group| {
                std::iter::once(group.parent.clone()).chain(group.members.iter().cloned())
            })
            .collect();
        let independent = office
            .workers
            .iter()
            .filter(|worker| !grouped.contains(&worker.id))
            .count();
        // Keyboard selection remains authoritative after changing families or
        // resizing. The remembered team is a default, never a hidden selection.
        let chosen = selected
            .and_then(|id| {
                groups
                    .iter()
                    .position(|group| &group.parent == id || group.members.contains(id))
            })
            .or_else(|| {
                focused
                    .then_some(ctx.team)
                    .flatten()
                    .and_then(|id| groups.iter().position(|group| &group.parent == id))
            });
        let selected_desks = selected.is_some_and(|id| !grouped.contains(id));
        let group = if selected_desks {
            None
        } else {
            chosen
                .and_then(|i| groups.get(i))
                .or_else(|| groups.first())
        };
        let team = group.map(|group| group.parent.clone());
        let selector = (focused && !groups.is_empty()).then_some(Rect::new(
            building_x,
            top + 1,
            building_width,
            1,
        ));
        let chrome = 1 + u16::from(selector.is_some());
        let room = PixelRect {
            x: usize::from(building_x - area.x) * cw,
            y: usize::from(top + chrome - area.y) * ch,
            width: usize::from(building_width) * cw,
            height: usize::from(floor_area.height.saturating_sub(chrome)) * ch,
        };
        let design = office_design_for(&office.id.0, ctx.designs);
        let workers = office.workers.iter().collect::<Vec<_>>();
        let decorations =
            crate::living_office::simulation::plan(&workers, ctx.now, ctx.motion, ctx.profiles);
        let options = |meeting, worker| SceneOptions {
            now: ctx.now,
            motion: ctx.motion,
            light: ctx.light,
            palette: ctx.palette,
            selected_worker: worker,
            page: 0,
            meeting,
            wardrobe: Some(ctx.wardrobe),
            cues: Some(&cues),
            show_elevator: true,
            overview: ctx.tower,
            design: Some(&design),
            profiles: Some(ctx.profiles),
            decorations: Some(&decorations),
        };
        // Every active delegation family is excluded, including ones not in the
        // focused meeting. Changing a room never fabricates independent desks.
        let desks = Office {
            id: office.id.clone(),
            path: office.path.clone(),
            name: office.name.clone(),
            workers: office
                .workers
                .iter()
                .filter(|worker| !grouped.contains(&worker.id))
                .cloned()
                .collect(),
        };
        let split = (!ctx.tower && group.is_some() && independent > 0)
            .then(|| readable_split_width(room, cw))
            .flatten();
        let mut scenes = Vec::new();
        if let Some(left) = split {
            scenes.push((
                studio.paint_region(
                    canvas,
                    PixelRect {
                        width: left,
                        ..room
                    },
                    &desks,
                    &options(None, selected),
                ),
                "DESKS",
            ));
            let mut meeting_options = options(group, selected);
            meeting_options.show_elevator = false;
            scenes.push((
                studio.paint_region(
                    canvas,
                    PixelRect {
                        x: room.x + left,
                        width: room.width - left,
                        ..room
                    },
                    office,
                    &meeting_options,
                ),
                "TEAM",
            ));
        } else if let Some(group) = group {
            scenes.push((
                studio.paint_region(canvas, room, office, &options(Some(group), selected)),
                "TEAM",
            ));
        } else {
            scenes.push((
                studio.paint_region(canvas, room, &desks, &options(None, selected)),
                "DESKS",
            ));
        }
        if !scenes.iter().all(|(scene, _)| scene.graphics) {
            return Some(compact::draw(frame, &ctx, area));
        }
        painted.push(PaintedFloor {
            index,
            office,
            area: floor_area,
            selector,
            groups,
            independent,
            team,
            scenes,
        });
    }
    canvas.render(frame.buffer_mut(), area);
    if building_y >= area.y + 3 {
        let roof_label = Rect::new(area.x + 2, building_y - 2, area.width.saturating_sub(4), 1);
        native_cells(frame, roof_label);
        Paragraph::new(format!(
            "SOFTWARE TOWER  ·  {} {}",
            ctx.offices.len(),
            if ctx.offices.len() == 1 {
                "floor"
            } else {
                "floors"
            }
        ))
        .style(Style::default().fg(INK).add_modifier(Modifier::BOLD))
        .render(roof_label, frame.buffer_mut());
    }
    let mut result = TowerLayout {
        visible_floors: visible,
        navigation_hint: format!("Floor {}/{}", ctx.selected_floor + 1, ctx.offices.len()),
        ..TowerLayout::default()
    };
    for floor in painted {
        let focused = floor.index == ctx.selected_floor;
        result.hits.push(HitRegion::new(
            floor.area,
            Action::SelectFloor(floor.office.id.clone()),
        ));
        draw_header(
            frame,
            &floor,
            ctx.offices.len(),
            focused,
            ctx.now,
            &mut result.hits,
        );
        if let Some(selector) = floor.selector {
            draw_teams(frame, selector, &floor, ctx.profiles, &mut result.hits);
        }
        for (scene, kind) in &floor.scenes {
            if focused {
                result.capacity += scene.capacity;
                if !ctx.tower
                    && (scene
                        .seats
                        .iter()
                        .any(|seat| Some(&seat.worker_id) == ctx.selected_worker)
                        || floor.scenes.len() == 1)
                {
                    let scope = if *kind == "TEAM" {
                        format!(
                            "Team {}/{}",
                            floor
                                .groups
                                .iter()
                                .position(|group| Some(&group.parent) == floor.team.as_ref())
                                .unwrap_or(0)
                                + 1,
                            floor.groups.len()
                        )
                    } else {
                        "Desks".into()
                    };
                    result.navigation_hint.push_str(&format!(
                        " · {} · People {}/{}{}",
                        scope,
                        scene.seats.len(),
                        scene.total_workers,
                        if scene.page_count > 1 {
                            format!(" · page {}/{}", scene.page + 1, scene.page_count)
                        } else {
                            String::new()
                        }
                    ));
                }
            }
            let elevator = native_rect(scene.elevator, area, cw, ch);
            push_hit(
                &mut result.hits,
                elevator,
                Action::EnterFloor(floor.office.id.clone()),
            );
            draw_scene_text(
                frame,
                scene,
                kind,
                &floor,
                area,
                (cw, ch),
                &ctx,
                &mut result.hits,
            );
        }
    }
    if !ctx.tower && area.height.saturating_sub(building_height) >= 4 {
        let summary = Rect::new(
            area.x,
            building_y + building_height,
            area.width,
            area.height - building_height,
        );
        result.hits.extend(compact::latest(frame, &ctx, summary));
    }
    result.capacity = result.capacity.max(1);
    Some(result)
}

/// Exterior space belongs to the building, not an infinitely stretched room.
/// This static backdrop is deliberately quiet; only source-observed room cues
/// and the shared actor simulation carry movement.
fn exterior(canvas: &mut Canvas, building: PixelRect, cell_height: usize, light: bool) {
    let top = building.y;
    let height = building.height;
    let sky = if light {
        Color::Rgb(205, 220, 224)
    } else {
        Color::Rgb(25, 34, 49)
    };
    let distant = if light {
        Color::Rgb(179, 197, 202)
    } else {
        Color::Rgb(34, 46, 63)
    };
    let ground = if light {
        Color::Rgb(207, 210, 199)
    } else {
        Color::Rgb(34, 44, 49)
    };
    let edge = if light {
        Color::Rgb(129, 153, 155)
    } else {
        Color::Rgb(67, 85, 95)
    };
    canvas.fill(sky);
    let base = top.saturating_sub(cell_height);
    let bottom = top + height;
    for x in 0..canvas.width() {
        let block = x / 64;
        let roof = base.saturating_sub(12 + (block * 37 % 73));
        if x % 64 < 54 {
            for y in roof..base {
                canvas.set(x, y, distant);
            }
        }
        if x >= building.x && x < building.x + building.width {
            for y in top.saturating_sub(4)..top {
                canvas.set(x, y, edge);
            }
        }
        for y in bottom..canvas.height() {
            canvas.set(x, y, ground);
        }
        if x >= building.x && x < building.x + building.width {
            for y in bottom..(bottom + 5).min(canvas.height()) {
                canvas.set(x, y, edge);
            }
        }
    }
}

fn floor_capacity(height: u16) -> usize {
    (usize::from(height) / 10).clamp(1, 5)
}

fn window_start(selected: usize, total: usize, visible: usize) -> usize {
    selected
        .saturating_sub(visible / 2)
        .min(total.saturating_sub(visible))
}

fn observed_cues(world: &World, now: Millis) -> BTreeMap<String, SceneCue> {
    let mut cues = BTreeMap::new();
    for worker in world.offices().flat_map(|office| &office.workers) {
        if worker.wait_reason == Some(WaitReason::Child) {
            cues.insert(worker.id.0.clone(), SceneCue::WaitingForTeam);
        }
    }
    for event in world
        .collaboration()
        .filter(|event| event.at <= now && now.saturating_sub(event.at) < 6_000)
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
    cues
}

fn draw_header(
    frame: &mut Frame,
    floor: &PaintedFloor<'_>,
    total: usize,
    focused: bool,
    now: Millis,
    hits: &mut Vec<HitRegion>,
) {
    let header = Rect::new(floor.area.x, floor.area.y, floor.area.width, 1);
    native_cells(frame, header);
    let style = Style::default()
        .fg(if focused { ACCENT } else { INK })
        .bg(if focused {
            super::PANEL_HIGHLIGHT
        } else {
            super::PANEL
        })
        .add_modifier(Modifier::BOLD);
    Paragraph::new("")
        .style(style)
        .render(header, frame.buffer_mut());
    let button_width = header.width.min(9);
    let button = Rect::new(header.right() - button_width, header.y, button_width, 1);
    Paragraph::new(" [Visit] ")
        .style(style)
        .render(button, frame.buffer_mut());
    push_hit(hits, button, Action::EnterFloor(floor.office.id.clone()));
    let counts = FloorCounts::new(floor.office, now);
    let count_text = counts.compact();
    let available = header.width.saturating_sub(button.width + 1);
    let prefix = format!(
        "{} F{:02}/{:02} ",
        if focused { "▸" } else { " " },
        floor.index + 1,
        total
    );
    let count_width = count_text.chars().count().min(usize::from(available / 2)) as u16;
    let title_width = available.saturating_sub(count_width + 2);
    let title = format!("{}{}", prefix, safe_display(&floor.office.name));
    Paragraph::new(super::short_path(&title, title_width.into()))
        .style(style)
        .render(
            Rect::new(header.x, header.y, title_width, 1),
            frame.buffer_mut(),
        );
    Paragraph::new(super::short_path(&count_text, count_width.into()))
        .style(style.fg(if counts.attention > 0 { WARNING } else { MUTED }))
        .render(
            Rect::new(header.x + available - count_width, header.y, count_width, 1),
            frame.buffer_mut(),
        );
}

fn draw_teams(
    frame: &mut Frame,
    area: Rect,
    floor: &PaintedFloor<'_>,
    profiles: &BTreeMap<String, CharacterProfile>,
    hits: &mut Vec<HitRegion>,
) {
    native_cells(frame, area);
    Paragraph::new("")
        .style(Style::default().bg(BACKGROUND))
        .render(area, frame.buffer_mut());
    let mut choices = Vec::new();
    if floor.independent > 0 {
        choices.push((
            format!("Desks {}", floor.independent),
            Action::SelectDesks(
                floor.office.id.clone(),
                floor
                    .office
                    .workers
                    .iter()
                    .find(|worker| {
                        !floor.groups.iter().any(|group| {
                            group.parent == worker.id || group.members.contains(&worker.id)
                        })
                    })
                    .expect("independent count was checked")
                    .id
                    .clone(),
            ),
            floor.team.is_none(),
        ));
    }
    choices.extend(floor.groups.iter().map(|group| {
        let name = profile_for(&group.parent.0, profiles).name;
        (
            format!("Team {} · {}", safe_display(&name), group.members.len() + 1),
            Action::SelectTeam(group.parent.clone()),
            floor.team.as_ref() == Some(&group.parent),
        )
    }));
    let current = choices
        .iter()
        .position(|(_, _, active)| *active)
        .unwrap_or(0);
    let slot_width = 23u16.min(area.width);
    let capacity = usize::from(area.width / slot_width).max(1);
    let start = window_start(current, choices.len(), capacity.min(choices.len()));
    let more = choices.len() > capacity;
    let shown = capacity.saturating_sub(usize::from(more)).max(1);
    let start = start.max(current.saturating_sub(shown - 1));
    for (slot, (label, action, active)) in choices.iter().skip(start).take(shown).enumerate() {
        let chip = Rect::new(area.x + slot as u16 * slot_width, area.y, slot_width, 1);
        let label = format!("{} {}", if *active { "●" } else { "○" }, label);
        Paragraph::new(super::short_path(
            &label,
            chip.width.saturating_sub(1).into(),
        ))
        .style(
            Style::default()
                .fg(if *active { ACCENT } else { MUTED })
                .bg(BACKGROUND),
        )
        .render(chip, frame.buffer_mut());
        push_hit(hits, chip, action.clone());
    }
    if more {
        let next = (start + shown) % choices.len();
        let x = area.x + shown as u16 * slot_width;
        if x < area.right() {
            let chip = Rect::new(x, area.y, area.right() - x, 1);
            let label = format!("+{} rooms ›", choices.len() - shown);
            Paragraph::new(super::short_path(&label, chip.width.into()))
                .style(Style::default().fg(ACCENT).bg(BACKGROUND))
                .render(chip, frame.buffer_mut());
            push_hit(hits, chip, choices[next].1.clone());
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_scene_text(
    frame: &mut Frame,
    scene: &SceneLayout,
    kind: &str,
    floor: &PaintedFloor<'_>,
    body: Rect,
    density: (usize, usize),
    ctx: &Context<'_>,
    hits: &mut Vec<HitRegion>,
) {
    let (cw, ch) = density;
    let focused = floor.index == ctx.selected_floor;
    let mut sign = native_rect(scene.sign, body, cw, ch);
    sign.y += sign.height.saturating_sub(1) / 2;
    sign.height = sign.height.min(1);
    native_cells(frame, sign);
    let selected = ctx
        .selected_worker
        .filter(|_| focused)
        .and_then(|id| floor.office.workers.iter().find(|worker| &worker.id == id));
    let label = if let Some(worker) =
        selected.filter(|worker| scene.seats.iter().any(|seat| seat.worker_id == worker.id))
    {
        format!("{} · {}", kind, safe_display(&worker.name))
    } else if kind == "TEAM" {
        format!(
            "Team {} · {}",
            floor
                .team
                .as_ref()
                .map(|id| profile_for(&id.0, ctx.profiles).name)
                .unwrap_or_default(),
            person_count(scene.total_workers)
        )
    } else if scene.total_workers == 0 {
        "An empty office · create a task to bring it to life".into()
    } else {
        format!("{} · {}", kind, person_count(scene.total_workers))
    };
    let page = if scene.page_count > 1 {
        format!("  People page {}/{}", scene.page + 1, scene.page_count)
    } else {
        String::new()
    };
    let text = format!(
        "{}{}",
        super::short_path(
            &label,
            usize::from(sign.width).saturating_sub(page.chars().count())
        ),
        page
    );
    Paragraph::new(text)
        .style(Style::default().fg(INK).add_modifier(Modifier::BOLD))
        .render(sign, frame.buffer_mut());
    for seat in &scene.seats {
        let Some(worker) = floor
            .office
            .workers
            .iter()
            .find(|worker| worker.id == seat.worker_id)
        else {
            continue;
        };
        let mut plate = native_plate_rect(seat.nameplate, body, cw, ch);
        plate.height = plate.height.min(if ctx.tower { 1 } else { 2 });
        native_cells(frame, plate);
        let selected = focused && ctx.selected_worker == Some(&worker.id);
        let warning = observed_attention(worker, ctx.now);
        let marker = format!(
            "{}{}",
            if selected { ">" } else { " " },
            state_marker(worker, ctx.now)
        );
        let show_name = ctx.name_plates || selected || crate::presentation::human_request(worker);
        let name = if show_name {
            character_name(worker, ctx.profiles)
        } else {
            String::new()
        };
        let name = super::short_path(&name, usize::from(plate.width.saturating_sub(2)));
        let text = if plate.height > 1 {
            format!(
                "{marker}{name}\n{}",
                super::short_path(
                    crate::presentation::state_label(worker, ctx.now),
                    usize::from(plate.width.saturating_sub(1))
                )
            )
        } else {
            format!("{marker}{name}")
        };
        Paragraph::new(text)
            .style(Style::default().fg(if warning {
                WARNING
            } else if selected {
                ACCENT
            } else {
                INK
            }))
            .render(plate, frame.buffer_mut());
        push_hit(
            hits,
            native_rect(seat.bounds, body, cw, ch),
            Action::Inspect(worker.id.clone()),
        );
        for target in [
            seat.workstation.selection,
            seat.workstation.chair,
            seat.workstation.computer,
            seat.workstation.keyboard,
        ] {
            push_hit(
                hits,
                native_rect(target, body, cw, ch),
                Action::Inspect(worker.id.clone()),
            );
        }
        push_hit(hits, plate, Action::Inspect(worker.id.clone()));
    }
}

fn character_name(worker: &Worker, profiles: &BTreeMap<String, CharacterProfile>) -> String {
    safe_display(&profile_for(&worker.id.0, profiles).name)
}

fn person_count(count: usize) -> String {
    format!("{count} {}", if count == 1 { "person" } else { "people" })
}

fn push_hit(hits: &mut Vec<HitRegion>, area: Rect, action: Action) {
    if area.width > 0 && area.height > 0 {
        hits.push(HitRegion::new(area, action));
    }
}

/// Preserve the scene's height-selected character scale in both rooms.
/// A fixed width threshold would halve tall rooms into tiny people and blank
/// walls. Use the actual cell-aligned halves, including their unequal remainder.
fn readable_split_width(room: PixelRect, cell_width: usize) -> Option<usize> {
    if cell_width == 0 {
        return None;
    }
    let scale = crate::living_office::detail_scale(room.width, room.height);
    // Studio requires 280 authored pixels. This also fits circulation and at
    // least two 76-pixel seats, so the parent and a child remain together.
    let minimum = 280 * scale;
    let left = (room.width / cell_width / 2) * cell_width;
    (left >= minimum && room.width - left >= minimum).then_some(left)
}

fn native_cells(frame: &mut Frame, area: Rect) {
    // A sampled art background does not participate in native theme mapping.
    // Use a semantic panel so light-theme text never lands on a dark pixel row.
    super::paint_opaque(frame, area, Style::default().fg(INK).bg(super::PANEL));
}

/// Text starts on the next complete terminal row. Actor hitboxes expand to
/// cover touched cells, but expanding a label upward would erase its body.
fn native_plate_rect(rect: PixelRect, area: Rect, cw: usize, ch: usize) -> Rect {
    if cw == 0 || ch == 0 || rect.width == 0 || rect.height == 0 {
        return Rect::new(area.x, area.y, 0, 0);
    }
    let x = rect.x.div_ceil(cw).min(area.width as usize) as u16;
    let y = rect.y.div_ceil(ch).min(area.height as usize) as u16;
    Rect::new(
        area.x + x,
        area.y + y,
        (rect.width / cw).min(usize::from(area.width - x)) as u16,
        (rect.height / ch).min(usize::from(area.height - y)) as u16,
    )
}

fn native_rect(rect: PixelRect, area: Rect, cw: usize, ch: usize) -> Rect {
    if cw == 0 || ch == 0 || rect.width == 0 || rect.height == 0 {
        return Rect::new(area.x, area.y, 0, 0);
    }
    let x = (rect.x / cw).min(area.width as usize) as u16;
    let y = (rect.y / ch).min(area.height as usize) as u16;
    let right = rect
        .x
        .saturating_add(rect.width)
        .div_ceil(cw)
        .min(area.width as usize) as u16;
    let bottom = rect
        .y
        .saturating_add(rect.height)
        .div_ceil(ch)
        .min(area.height as usize) as u16;
    Rect::new(
        area.x + x,
        area.y + y,
        right.saturating_sub(x),
        bottom.saturating_sub(y),
    )
}

fn meetings(world: &World, office: &Office) -> Vec<MeetingGroup> {
    let mut groups = BTreeMap::new();
    for worker in &office.workers {
        if let Some(group) = meeting_for(world, office, Some(&worker.id)) {
            groups.entry(group.parent.clone()).or_insert(group);
        }
    }
    groups.into_values().collect()
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
    loop {
        if !visited.insert(root.clone()) {
            return None;
        }
        let parents = world
            .relationships()
            .filter(|relation| {
                relation.kind == RelationshipKind::Delegation && relation.child == root
            })
            .map(|relation| &relation.parent)
            .collect::<BTreeSet<_>>();
        if parents.len() > 1 {
            return None;
        }
        match parents.first() {
            Some(parent) if present.contains(*parent) => root = (*parent).clone(),
            Some(_) => return None,
            None => break,
        }
    }
    let mut members = Vec::new();
    let mut queue = vec![root.clone()];
    let mut seen = BTreeSet::from([root.clone()]);
    while let Some(parent) = queue.pop() {
        for relation in world.relationships().filter(|relation| {
            relation.kind == RelationshipKind::Delegation && relation.parent == parent
        }) {
            let parents = world
                .relationships()
                .filter(|candidate| {
                    candidate.kind == RelationshipKind::Delegation
                        && candidate.child == relation.child
                })
                .map(|candidate| &candidate.parent)
                .collect::<BTreeSet<_>>();
            if parents.len() == 1
                && present.contains(&relation.child)
                && seen.insert(relation.child.clone())
            {
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
    fn light_native_plates_replace_sampled_art_backgrounds() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut terminal = Terminal::new(TestBackend::new(12, 3)).unwrap();
        terminal
            .draw(|frame| {
                for (row, foreground) in [INK, ACCENT, WARNING].into_iter().enumerate() {
                    let area = Rect::new(0, row as u16, 12, 1);
                    for x in 0..12 {
                        frame.buffer_mut()[(x, row as u16)]
                            .set_bg(Color::Rgb(34, 48, 56))
                            .set_skip(true);
                    }
                    native_cells(frame, area);
                    Paragraph::new("Label")
                        .style(Style::default().fg(foreground))
                        .render(area, frame.buffer_mut());
                }
                crate::views::remap_buffer_theme(frame.buffer_mut(), crate::views::UiTheme::Light);
            })
            .unwrap();
        for (row, foreground) in [INK, ACCENT, WARNING].into_iter().enumerate() {
            let cell = &terminal.backend().buffer()[(0, row as u16)];
            assert_eq!(cell.bg, crate::views::light_color(crate::views::PANEL));
            assert_eq!(cell.fg, crate::views::light_color(foreground));
            assert!(!cell.skip);
        }
    }

    #[test]
    fn source_health_excludes_old_activity_from_live_counts() {
        let mut office = Office::new(theywork_core::OfficeId("p".into()), "p".into());
        let now = theywork_core::BLOCKED_AFTER_MS + 2;
        for (id, available, observed) in [
            ("live", true, now),
            ("offline", false, now),
            ("old", true, 1),
        ] {
            let mut worker = Worker::new(
                WorkerId(id.into()),
                office.id.clone(),
                theywork_core::Agent::Codex,
                id.into(),
                now,
            );
            worker.turn_in_flight = true;
            worker.coverage.available = available;
            worker.coverage.observed_at = observed;
            office.workers.push(worker);
        }
        let counts = FloorCounts::new(&office, now);
        assert_eq!(
            (
                counts.working,
                counts.attention,
                counts.unavailable,
                counts.stale
            ),
            (1, 0, 1, 1)
        );
        assert!(counts.compact().contains("1 offline · 1 stale"));
        assert_eq!(state_marker(&office.workers[1], now), "-");
        assert_eq!(state_marker(&office.workers[2], now), "~");
    }

    #[test]
    fn names_off_keeps_selection_and_requests_distinct_without_color() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut world = World::new();
        let mut profiles = BTreeMap::new();
        for (id, alias) in [("a", "Leader"), ("b", "Peer"), ("c", "Ask")] {
            worker(&mut world, 0, id);
            let mut profile = profile_for(id, &BTreeMap::new());
            profile.name = alias.into();
            profiles.insert(id.into(), profile);
        }
        apply(
            &mut world,
            0,
            "c",
            theywork_core::EventKind::Wait(Some(WaitReason::HumanInput)),
        );
        let offices = world.offices().collect::<Vec<_>>();
        for image in [false, true] {
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            let mut canvas = Canvas::new(0, 0);
            canvas.set_color_depth(crate::canvas::ColorDepth::None);
            if image {
                canvas.set_image_cell_size(Some((8, 16)));
            }
            let mut studio = Studio::new();
            terminal
                .draw(|frame| {
                    draw(
                        frame,
                        &mut canvas,
                        &mut studio,
                        Context {
                            area: Rect::new(0, 0, 80, 24),
                            world: &world,
                            offices: &offices,
                            selected_floor: 0,
                            selected_worker: Some(&WorkerId("a".into())),
                            team: None,
                            now: 1_000,
                            tower: true,
                            motion: false,
                            name_plates: false,
                            light: false,
                            palette: 0,
                            wardrobe: &BTreeMap::new(),
                            profiles: &profiles,
                            designs: &BTreeMap::new(),
                        },
                    );
                })
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(
                text.contains(">·Leader") || text.contains(">· Leader"),
                "selected alias must remain visible: {text}"
            );
            assert!(
                text.contains("!Ask") || text.contains("! Ask"),
                "the requester must remain named"
            );
            assert!(!text.contains("Peer"), "optional peer alias must be hidden");
        }
    }
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
            (1536, 848, 8, Some(768)),
            (1120, 304, 8, Some(560)),
            (1120, 304, 32, None),
            (1152, 304, 32, Some(576)),
            (1216, 304, 8, Some(608)),
            (1680, 592, 8, Some(840)),
            (1824, 592, 8, Some(912)),
            (2240, 592, 8, Some(1120)),
            (2240, 592, 32, Some(1120)),
            (2240, 848, 8, Some(1120)),
            (2800, 848, 8, Some(1400)),
            (2800, 848, 32, Some(1376)),
            (2816, 848, 32, Some(1408)),
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
                assert_eq!(
                    full.scale,
                    crate::living_office::detail_scale(width, height)
                );
            }
        }
    }

    fn apply(world: &mut World, floor: usize, id: &str, kind: theywork_core::EventKind) {
        let path = format!("/projects/{floor:02}-project");
        world.apply(theywork_core::Event {
            at: 1_000,
            office: theywork_core::OfficeId(path.clone()),
            office_path: path,
            worker: WorkerId(id.into()),
            agent: theywork_core::Agent::Codex,
            kind,
        });
    }
    fn worker(world: &mut World, floor: usize, id: &str) {
        apply(
            world,
            floor,
            id,
            theywork_core::EventKind::Seen {
                name: format!("Implement a long shared task title for {id}"),
                git_branch: None,
            },
        );
    }
    fn relation(world: &mut World, parent: &str, child: &str, kind: RelationshipKind) {
        apply(
            world,
            0,
            child,
            theywork_core::EventKind::Relationship(theywork_core::Relationship {
                parent: WorkerId(parent.into()),
                child: WorkerId(child.into()),
                kind,
                evidence: theywork_core::Evidence::Demo,
                at: 1_000,
                correlation_id: None,
            }),
        );
    }
    fn active(world: &mut World, id: &str) {
        apply(
            world,
            0,
            id,
            theywork_core::EventKind::Lifecycle(WorkerLifecycle::Active),
        );
        apply(
            world,
            0,
            id,
            theywork_core::EventKind::Turn { in_flight: true },
        );
    }
    pub(super) fn family_world() -> World {
        let mut world = World::new();
        for id in ["parent-a", "child-a", "parent-b", "child-b", "independent"] {
            worker(&mut world, 0, id);
        }
        relation(
            &mut world,
            "parent-a",
            "child-a",
            RelationshipKind::Delegation,
        );
        relation(
            &mut world,
            "parent-b",
            "child-b",
            RelationshipKind::Delegation,
        );
        active(&mut world, "child-a");
        active(&mut world, "child-b");
        world
    }

    #[test]
    fn navigation_names_the_selected_family_instead_of_paging_the_entire_roster() {
        let world = family_world();
        for width in [80, 192] {
            let layout = render(
                &world,
                (width, 36),
                Rect::new(0, 2, width, 32),
                0,
                Some(&WorkerId("parent-b".into())),
                None,
                false,
            );
            assert!(layout.navigation_hint.contains("Floor 1/1"));
            assert!(
                layout.navigation_hint.contains("Team 2/2"),
                "{}",
                layout.navigation_hint
            );
            assert!(
                layout.navigation_hint.contains("People 2/2"),
                "{}",
                layout.navigation_hint
            );
            assert!(
                !layout.navigation_hint.contains("page"),
                "both members are visible; other families are not extra pages"
            );
        }
    }

    fn render(
        world: &World,
        size: (u16, u16),
        area: Rect,
        selected_floor: usize,
        selected: Option<&WorkerId>,
        team: Option<&WorkerId>,
        tower: bool,
    ) -> TowerLayout {
        use ratatui::{backend::TestBackend, Terminal};
        let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        let mut studio = Studio::new();
        let offices = world.offices().collect::<Vec<_>>();
        let mut result = None;
        terminal
            .draw(|frame| {
                result = draw(
                    frame,
                    &mut canvas,
                    &mut studio,
                    Context {
                        area,
                        world,
                        offices: &offices,
                        selected_floor,
                        selected_worker: selected,
                        team,
                        now: 1_000,
                        tower,
                        motion: false,
                        name_plates: true,
                        light: false,
                        palette: 0,
                        wardrobe: &BTreeMap::new(),
                        profiles: &BTreeMap::new(),
                        designs: &BTreeMap::new(),
                    },
                );
                if let Some(layout) = &result {
                    for hit in &layout.hits {
                        if matches!(hit.action, Action::SelectFloor(_)) {
                            for x in hit.area.x..hit.area.right() {
                                assert!(
                                    !frame.buffer_mut()[(x, hit.area.y)].skip,
                                    "floor names and Visit must survive the physical image mask"
                                );
                            }
                        }
                    }
                }
            })
            .unwrap();
        result.expect("a physical image floor must render at the documented size")
    }

    #[test]
    fn stacked_floor_targets_cover_contiguous_complete_floors_after_resize() {
        let mut world = World::new();
        for floor in 0..20 {
            worker(&mut world, floor, &format!("worker-{floor}"));
        }
        for (width, height, count) in [(80, 24, 2), (120, 36, 3), (192, 58, 5)] {
            let area = Rect::new(0, 2, width, height - 4);
            for selected in [0, 10, 19] {
                let result = render(&world, (width, height), area, selected, None, None, true);
                assert_eq!(result.visible_floors, count);
                let floors = result
                    .hits
                    .iter()
                    .filter(|hit| matches!(hit.action, Action::SelectFloor(_)))
                    .collect::<Vec<_>>();
                assert_eq!(floors.len(), count);
                if width == 192 {
                    assert!(
                        floors
                            .iter()
                            .all(|hit| hit.area.x == 26 && hit.area.width == 140),
                        "the wide-window tower has centered, translated click targets"
                    );
                }
                assert_eq!(floors.first().unwrap().area.y, area.y);
                assert_eq!(floors.last().unwrap().area.bottom(), area.bottom());
                for pair in floors.windows(2) {
                    assert_eq!(pair[0].area.bottom(), pair[1].area.y);
                }
                assert!(floors.iter().any(|hit| hit.action
                    == Action::SelectFloor(theywork_core::OfficeId(format!(
                        "/projects/{selected:02}-project"
                    )))));
                assert!(result
                    .hits
                    .iter()
                    .all(|hit| hit.area.intersection(area) == hit.area));
            }
        }
    }

    #[test]
    fn side_inspector_offset_never_moves_hits_outside_the_scene() {
        let mut world = World::new();
        for floor in 0..3 {
            worker(&mut world, floor, &format!("worker-{floor}"));
        }
        let area = Rect::new(3, 2, 80, 32);
        let result = render(&world, (125, 36), area, 1, None, None, true);
        for hit in result.hits {
            assert_eq!(hit.area.intersection(area), hit.area);
            assert!(!hit.contains(90, 20), "inspector is not a floor hit");
            assert!(!hit.contains(5, 0), "toolbar is not a floor hit");
        }
    }

    #[test]
    fn active_families_have_independent_selectors_and_do_not_leak_into_desks() {
        let world = family_world();
        let area = Rect::new(0, 2, 120, 32);
        let child_a = WorkerId("child-a".into());
        let result = render(&world, (120, 36), area, 0, Some(&child_a), None, true);
        assert!(result
            .hits
            .iter()
            .any(|hit| hit.action == Action::SelectTeam(WorkerId("parent-a".into()))));
        assert!(result
            .hits
            .iter()
            .any(|hit| hit.action == Action::SelectTeam(WorkerId("parent-b".into()))));
        assert!(result
            .hits
            .iter()
            .any(|hit| hit.action == Action::Inspect(child_a.clone())));
        assert!(!result
            .hits
            .iter()
            .any(|hit| hit.action == Action::Inspect(WorkerId("child-b".into()))));
        let independent = WorkerId("independent".into());
        assert!(result.hits.iter().any(|hit| hit.action
            == Action::SelectDesks(
                theywork_core::OfficeId("/projects/00-project".into()),
                independent.clone()
            )));
        let result = render(&world, (120, 36), area, 0, Some(&independent), None, true);
        let people = result
            .hits
            .iter()
            .filter_map(|hit| match &hit.action {
                Action::Inspect(id) => Some(id),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(people, BTreeSet::from([&independent]));
    }

    #[test]
    fn keyboard_selection_overrides_a_previously_chosen_team() {
        let world = family_world();
        let area = Rect::new(0, 2, 120, 32);
        let child_b = WorkerId("child-b".into());
        let previous = WorkerId("parent-a".into());
        let result = render(
            &world,
            (120, 36),
            area,
            0,
            Some(&child_b),
            Some(&previous),
            true,
        );
        assert!(result
            .hits
            .iter()
            .any(|hit| hit.action == Action::Inspect(child_b.clone())));
        assert!(!result
            .hits
            .iter()
            .any(|hit| hit.action == Action::Inspect(WorkerId("child-a".into()))));
        let independent = WorkerId("independent".into());
        let result = render(
            &world,
            (120, 36),
            area,
            0,
            Some(&independent),
            Some(&previous),
            true,
        );
        assert!(result
            .hits
            .iter()
            .any(|hit| hit.action == Action::Inspect(independent.clone())));
        assert!(!result
            .hits
            .iter()
            .any(|hit| hit.action == Action::Inspect(child_b.clone())));
    }

    #[test]
    fn selected_last_worker_stays_reachable_on_a_crowded_floor() {
        let mut world = World::new();
        for index in 0..20 {
            worker(&mut world, 0, &format!("worker-{index:02}"));
        }
        let selected = WorkerId("worker-19".into());
        for (width, height) in [(80, 24), (120, 36), (192, 58)] {
            let result = render(
                &world,
                (width, height),
                Rect::new(0, 2, width, height - 4),
                0,
                Some(&selected),
                None,
                true,
            );
            assert!(result
                .hits
                .iter()
                .any(|hit| hit.action == Action::Inspect(selected.clone())));
            let first = render(
                &world,
                (width, height),
                Rect::new(0, 2, width, height - 4),
                0,
                Some(&WorkerId("worker-00".into())),
                None,
                true,
            );
            assert_eq!(
                result.capacity, first.capacity,
                "a partial last page must not shrink the PageUp step"
            );
        }
    }

    #[test]
    fn explicit_character_names_do_not_change_with_neighbors() {
        let mut world = World::new();
        for id in ["b", "a", "c"] {
            worker(&mut world, 0, id);
        }
        let mut profiles: BTreeMap<String, CharacterProfile> = ["a", "b", "c"]
            .into_iter()
            .map(|id| {
                (
                    id.into(),
                    CharacterProfile {
                        name: "Sam".into(),
                        ..CharacterProfile::default()
                    },
                )
            })
            .collect();
        for (id, name) in [("a", "Sam"), ("b", "Sam"), ("c", "Sam")] {
            assert_eq!(
                character_name(world.worker(&WorkerId(id.into())).unwrap(), &profiles),
                name
            );
        }
        worker(&mut world, 0, "00-earlier");
        profiles.insert(
            "00-earlier".into(),
            CharacterProfile {
                name: "Sam".into(),
                ..CharacterProfile::default()
            },
        );
        assert_eq!(
            character_name(world.worker(&WorkerId("b".into())).unwrap(), &profiles),
            "Sam"
        );
    }

    #[test]
    fn incomplete_and_cyclic_relations_cannot_fabricate_a_team() {
        for kind in [RelationshipKind::SessionMembership, RelationshipKind::Fork] {
            let mut world = World::new();
            worker(&mut world, 0, "parent");
            worker(&mut world, 0, "child");
            active(&mut world, "child");
            relation(&mut world, "parent", "child", kind);
            assert!(meetings(&world, world.offices().next().unwrap()).is_empty());
        }
        let mut world = World::new();
        worker(&mut world, 0, "a");
        worker(&mut world, 0, "b");
        active(&mut world, "b");
        relation(&mut world, "a", "b", RelationshipKind::Delegation);
        relation(&mut world, "b", "a", RelationshipKind::Delegation);
        // World rejects the cycle-forming edge and retains the valid A → B link.
        let groups = meetings(&world, world.offices().next().unwrap());
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].parent, WorkerId("a".into()));
        assert_eq!(groups[0].members, vec![WorkerId("b".into())]);
        let mut world = World::new();
        worker(&mut world, 0, "child");
        active(&mut world, "child");
        relation(
            &mut world,
            "unavailable-parent",
            "child",
            RelationshipKind::Delegation,
        );
        assert!(meetings(&world, world.offices().next().unwrap()).is_empty());
        let mut world = World::new();
        for id in ["a", "b", "child"] {
            worker(&mut world, 0, id);
        }
        active(&mut world, "child");
        relation(&mut world, "a", "child", RelationshipKind::Delegation);
        relation(&mut world, "b", "child", RelationshipKind::Delegation);
        assert!(meetings(&world, world.offices().next().unwrap()).is_empty());
    }

    #[test]
    fn tall_office_uses_surplus_space_for_real_work_and_clipped_hits() {
        let mut world = World::new();
        for index in 0..20 {
            worker(&mut world, 0, &format!("worker-{index:02}"));
        }
        let selected = WorkerId("worker-19".into());
        let area = Rect::new(3, 2, 151, 54);
        let result = render(&world, (196, 58), area, 0, Some(&selected), None, false);
        let floor = result
            .hits
            .iter()
            .find(|hit| matches!(hit.action, Action::SelectFloor(_)))
            .unwrap();
        assert_eq!(floor.area.height, 32);
        assert!(
            result
                .hits
                .iter()
                .any(|hit| hit.area.y >= floor.area.bottom()
                    && hit.action == Action::Inspect(selected.clone())),
            "the native work list follows the actual selected person"
        );
        assert!(result
            .hits
            .iter()
            .all(|hit| hit.area.intersection(area) == hit.area));
    }

    #[test]
    fn native_labels_round_downward_without_erasing_actor_pixels() {
        let area = Rect::new(26, 2, 140, 54);
        for (height, rows) in [(16, 1), (32, 2)] {
            for y in 96..112 {
                let pixel = PixelRect {
                    x: 96,
                    y,
                    width: 72,
                    height,
                };
                let label = native_plate_rect(pixel, area, 8, 16);
                assert!(usize::from(label.y - area.y) * 16 >= pixel.y);
                assert_eq!(label.height, rows);
                assert_eq!(label.intersection(area), label);
            }
        }
        let pixel = PixelRect {
            x: 96,
            y: 110,
            width: 72,
            height: 16,
        };
        assert_eq!(
            native_plate_rect(pixel, area, 8, 16),
            Rect::new(38, 9, 9, 1)
        );
        assert_eq!(
            native_rect(pixel, area, 8, 16).height,
            2,
            "hitbox rounding remains expansive"
        );
    }

    #[test]
    fn zero_or_offscreen_geometry_never_becomes_a_phantom_click() {
        let area = Rect::new(10, 5, 80, 20);
        for rect in [
            PixelRect::default(),
            PixelRect {
                x: 1000,
                y: 1000,
                width: 20,
                height: 20,
            },
        ] {
            let hit = native_rect(rect, area, 8, 16);
            assert!(hit.width == 0 || hit.height == 0);
        }
        let pixel = PixelRect {
            x: 15,
            y: 31,
            width: 2,
            height: 2,
        };
        assert_eq!(native_rect(pixel, area, 8, 16), Rect::new(11, 6, 2, 2));
    }
}
