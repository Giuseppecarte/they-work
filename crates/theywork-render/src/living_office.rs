//! A continuous side-cut office drawn in physical pixels. Native text, input,
//! provider facts and image transport stay with the host. The same deterministic
//! composition can therefore be inspected without pretending a PTY is a GPU.

pub mod art;
pub mod overview;
mod rooms;
mod workstation;
pub use workstation::WorkstationLayout;
pub mod simulation;

use std::collections::{BTreeMap, HashMap, VecDeque};

use theywork_core::{Activity, Millis, Office, Worker, WorkerId, WorkerStatus};

use crate::design::{CharacterProfile, OfficeDesign, OfficePreset};
use crate::{canvas::Canvas, sprite::Sprite};
use art::{rgb, Character, Facing, Pose, Raster};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PixelRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}
impl PixelRect {
    fn scaled(self, scale: usize) -> Self {
        Self {
            x: self.x * scale,
            y: self.y * scale,
            width: self.width * scale,
            height: self.height * scale,
        }
    }
}

/// Only explicit, source-confirmed group membership belongs here. The scene
/// does not infer delegation from names, costumes or shared project paths.
#[derive(Debug, Clone)]
pub struct MeetingGroup {
    pub parent: WorkerId,
    pub members: Vec<WorkerId>,
}

/// A recent, source-observed interaction selected by the host. Decorative
/// simulation cannot create one of these or invent the content of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneCue {
    Delegating,
    Message,
    Delivering,
    WaitingForTeam,
}
impl SceneCue {
    fn pose(self) -> Pose {
        match self {
            Self::Delegating => Pose::FolderOut,
            Self::Message => Pose::Message,
            Self::Delivering => Pose::FolderIn,
            Self::WaitingForTeam => Pose::ScreenRead,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SceneOptions<'a> {
    pub now: Millis,
    pub motion: bool,
    pub light: bool,
    pub palette: usize,
    pub selected_worker: Option<&'a WorkerId>,
    pub page: usize,
    pub meeting: Option<&'a MeetingGroup>,
    pub wardrobe: Option<&'a BTreeMap<String, usize>>,
    pub cues: Option<&'a BTreeMap<String, SceneCue>>,
    /// False for an adjoining room: draw an ordinary doorway, not another shaft.
    pub show_elevator: bool,
    pub overview: bool,
    pub design: Option<&'a OfficeDesign>,
    pub profiles: Option<&'a BTreeMap<String, CharacterProfile>>,
    pub decorations: Option<&'a BTreeMap<String, simulation::Decoration>>,
}
impl Default for SceneOptions<'_> {
    fn default() -> Self {
        Self {
            now: 0,
            motion: true,
            light: false,
            palette: 0,
            selected_worker: None,
            page: 0,
            meeting: None,
            wardrobe: None,
            cues: None,
            show_elevator: true,
            overview: false,
            design: None,
            profiles: None,
            decorations: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SeatLayout {
    pub worker_id: WorkerId,
    /// Full decorative actor hitbox, including its movement in the aisle.
    pub bounds: PixelRect,
    /// Stable native name/status destination, never animated with the actor.
    pub nameplate: PixelRect,
    pub costume: &'static str,
    /// Stable equipment and home position, independent of decorative travel.
    pub workstation: WorkstationLayout,
}

#[derive(Debug, Clone, Default)]
pub struct SceneLayout {
    pub seats: Vec<SeatLayout>,
    pub sign: PixelRect,
    pub elevator: PixelRect,
    pub page: usize,
    pub page_count: usize,
    /// Physical seat capacity, independent of how full the last page is.
    pub capacity: usize,
    pub total_workers: usize,
    pub active_gags: usize,
    pub scale: usize,
    /// False means the host should draw its compact native presentation.
    pub graphics: bool,
    pub anchors: Vec<SceneAnchor>,
}

#[derive(Debug, Clone, Copy)]
pub struct SceneAnchor {
    pub destination: simulation::Destination,
    pub x: usize,
    pub y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FrameKey {
    character: Character,
    pose: Pose,
    phase: u8,
    overview: bool,
    facing: Facing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RoomKey {
    width: usize,
    height: usize,
    palette: usize,
    light: bool,
    seats: usize,
    meeting: bool,
    elevator: bool,
    overview: bool,
    preset: OfficePreset,
    zones: [u8; 4],
    floor: usize,
    shaft: usize,
    slot: usize,
    title: usize,
    plate: usize,
    first_x: usize,
}

/// Detail art is authored at 48×64. Extra terminal space adds usable room,
/// never larger heads or higher windows. Shared with the host's split decision.
pub fn detail_scale(width: usize, height: usize) -> usize {
    (height / 140).clamp(1, 2).min((width / 280).max(1))
}

impl RoomKey {
    fn plate_y(self) -> usize {
        self.floor + 3
    }

    fn rest_floor(self) -> usize {
        // A lower aisle is available only when a complete actor can stand below
        // the native labels. Compact floors keep their route beside the desks.
        if !self.overview && self.height >= self.floor + self.plate + 64 + 16 {
            (self.floor + self.plate + 64 + 12).min(self.height - 7)
        } else {
            self.floor
        }
    }
}

/// Piecewise route through a side corridor. Vertical travel is outside every
/// nameplate; crossing the lower aisle keeps the entire actor below the labels.
fn route_position(
    origin: (usize, usize),
    target: (usize, usize),
    corridor_x: usize,
    progress: u16,
) -> (usize, usize) {
    let points = if origin.1 == target.1 {
        [origin, origin, target, target]
    } else {
        [
            origin,
            (corridor_x, origin.1),
            (corridor_x, target.1),
            target,
        ]
    };
    let distance = |a: (usize, usize), b: (usize, usize)| a.0.abs_diff(b.0) + a.1.abs_diff(b.1);
    let lengths = [
        distance(points[0], points[1]),
        distance(points[1], points[2]),
        distance(points[2], points[3]),
    ];
    let total: usize = lengths.iter().sum();
    let mut travel = total * usize::from(progress.min(1000)) / 1000;
    for i in 0..3 {
        if travel <= lengths[i] && lengths[i] > 0 {
            let mix = |a: usize, b: usize| {
                ((a as i64 * (lengths[i] - travel) as i64 + b as i64 * travel as i64)
                    / lengths[i] as i64) as usize
            };
            return (
                mix(points[i].0, points[i + 1].0),
                mix(points[i].1, points[i + 1].1),
            );
        }
        travel = travel.saturating_sub(lengths[i]);
    }
    target
}

/// Bounded reusable art. There is no animation history or queued output frame.
/// The host owns presentation pacing and may freely drop intermediate draws.
pub struct Studio {
    frames: HashMap<FrameKey, Sprite>,
    insertion_order: VecDeque<FrameKey>,
    rooms: VecDeque<(RoomKey, Sprite)>,
    region_canvas: Option<Canvas>,
}
impl Default for Studio {
    fn default() -> Self {
        Self::new()
    }
}
impl Studio {
    pub fn new() -> Self {
        Self {
            frames: HashMap::new(),
            insertion_order: VecDeque::new(),
            rooms: VecDeque::new(),
            region_canvas: None,
        }
    }

    /// Compose a floor into a physical subrectangle of a shared tower canvas.
    /// All returned hitboxes/nameplates are translated into that shared canvas.
    /// Call `Canvas::render` only once, after composing every visible floor.
    pub fn paint_region(
        &mut self,
        canvas: &mut Canvas,
        region: PixelRect,
        office: &Office,
        options: &SceneOptions<'_>,
    ) -> SceneLayout {
        if !canvas.has_image_density() {
            return SceneLayout::default();
        }
        let width = region.width.min(canvas.width().saturating_sub(region.x));
        let height = region.height.min(canvas.height().saturating_sub(region.y));
        let mut surface = self
            .region_canvas
            .take()
            .unwrap_or_else(|| Canvas::with_color_depth(0, 0, canvas.color_depth()));
        surface.set_color_depth(canvas.color_depth());
        surface.set_image_cell_size(Some(canvas.pixels_per_cell()));
        surface.resize(width, height);
        let mut layout = self.paint(&mut surface, office, options);
        if layout.graphics {
            canvas.blit_canvas(&surface, region.x, region.y);
        }
        let translate = |rect: &mut PixelRect| {
            rect.x += region.x;
            rect.y += region.y;
        };
        translate(&mut layout.sign);
        translate(&mut layout.elevator);
        for seat in &mut layout.seats {
            translate(&mut seat.bounds);
            translate(&mut seat.nameplate);
            seat.workstation.translate(region.x, region.y);
        }
        for anchor in &mut layout.anchors {
            anchor.x += region.x;
            anchor.y += region.y;
        }
        self.region_canvas = Some(surface);
        layout
    }

    pub fn character_frame(&mut self, character: Character, pose: Pose, phase: u8) -> Sprite {
        self.frame(character, pose, phase, false, Facing::Right)
    }

    fn frame(
        &mut self,
        character: Character,
        pose: Pose,
        phase: u8,
        overview: bool,
        facing: Facing,
    ) -> Sprite {
        let key = FrameKey {
            character,
            pose,
            phase: phase % 8,
            overview,
            facing,
        };
        if let Some(sprite) = self.frames.get(&key) {
            return sprite.clone();
        }
        if self.frames.len() >= 512 {
            if let Some(old) = self.insertion_order.pop_front() {
                self.frames.remove(&old);
            }
        }
        let sprite = if overview {
            overview::character_facing(character, pose, key.phase, facing)
        } else {
            art::character_facing(character, pose, key.phase, facing)
        };
        self.frames.insert(key, sprite.clone());
        self.insertion_order.push_back(key);
        sprite
    }

    /// Paint into a canvas already sized with `resize_for_cells`. The negotiated
    /// image-cell size is required: fallback sextants are not physical pixels.
    /// Returned rectangles use physical canvas coordinates for native labels.
    pub fn paint(
        &mut self,
        canvas: &mut Canvas,
        office: &Office,
        options: &SceneOptions<'_>,
    ) -> SceneLayout {
        if !canvas.has_image_density()
            || canvas.width() < if options.overview { 160 } else { 280 }
            || canvas.height() < if options.overview { 96 } else { 156 }
        {
            return SceneLayout {
                total_workers: office.workers.len(),
                ..SceneLayout::default()
            };
        }
        // Overview and inspection have separate authored grids and geometry.
        let scale = if options.overview {
            (canvas.height() / 64)
                .clamp(1, 2)
                .min((canvas.width() / 160).max(1))
        } else {
            detail_scale(canvas.width(), canvas.height())
        };
        let width = canvas.width() / scale;
        let height = canvas.height() / scale;
        let (cast_w, cast_h, shaft, slot) = if options.overview {
            (24, 32, 36, 44)
        } else {
            (48, 64, 52, 84)
        };
        let native_height = canvas.pixels_per_cell().1.max(12);
        let title = native_height.div_ceil(scale);
        let plate = (native_height * if options.overview { 1 } else { 2 }).div_ceil(scale);
        let floor = height
            .saturating_sub(plate + 4)
            .min(if options.overview { 76 } else { 112 });
        let rest = if width >= 430 { 48 } else { 0 };
        let capacity = ((width.saturating_sub(shaft + 16 + rest)) / slot).clamp(1, 8);
        let all = ordered_workers(office, options.meeting);
        let total = all.len();
        let meeting = options.meeting.is_some();
        let has_parent = options
            .meeting
            .is_some_and(|group| all.first().is_some_and(|worker| worker.id == group.parent));
        let pinned = has_parent && all.len() > 1 && capacity > 1;
        let requested = options
            .selected_worker
            .and_then(|id| all.iter().position(|worker| &worker.id == id))
            .filter(|&index| !pinned || index > 0)
            .map_or(options.page, |index| {
                (index - usize::from(pinned)) / (capacity - usize::from(pinned))
            });
        let (page, page_count, visible) = page_workers(&all, capacity, requested, has_parent);
        let first_x =
            shaft + 8 + (width.saturating_sub(shaft + 16 + rest + visible.len() * slot)) / 2;
        let design = options.design.cloned().unwrap_or_default();
        let key = RoomKey {
            width,
            height,
            palette: options.palette % 4,
            light: options.light,
            seats: visible.len(),
            meeting,
            elevator: options.show_elevator,
            overview: options.overview,
            preset: design.preset,
            zones: [design.entrance, design.desks, design.meeting, design.rest],
            floor,
            shaft,
            slot,
            title,
            plate,
            first_x,
        };
        let background = if let Some((_, background)) =
            self.rooms.iter().find(|(previous, _)| *previous == key)
        {
            background.clone()
        } else {
            let background = room_art(key);
            if self.rooms.len() >= 8 {
                self.rooms.pop_front();
            }
            self.rooms.push_back((key, background.clone()));
            background
        };
        canvas.fill(rgb(26, 32, 43));
        canvas.blit_scaled(&background, 0, 0, width * scale, height * scale);
        let empty_profiles = BTreeMap::new();
        let generated = if options.decorations.is_none() {
            simulation::plan(
                &all,
                options.now,
                options.motion,
                options.profiles.unwrap_or(&empty_profiles),
            )
        } else {
            BTreeMap::new()
        };
        let decorations = options.decorations.unwrap_or(&generated);
        let destinations = [
            simulation::Destination::Coffee,
            simulation::Destination::Reading,
            simulation::Destination::Plant,
            simulation::Destination::PaperTray,
            simulation::Destination::StretchSpot,
        ];
        let anchors = destinations
            .into_iter()
            .map(|destination| SceneAnchor {
                destination,
                x: match destination {
                    simulation::Destination::Coffee => {
                        width.saturating_sub(if options.overview { 28 } else { 44 } + cast_w / 2)
                    }
                    simulation::Destination::Plant => width
                        .saturating_sub(if key.zones[3] % 3 == 2 { 34 } else { 11 } + cast_w / 2),
                    simulation::Destination::Reading | simulation::Destination::PaperTray => {
                        shaft + if options.overview { 27 } else { 38 } + cast_w / 2
                    }
                    simulation::Destination::StretchSpot => width / 2,
                },
                y: key.rest_floor(),
            })
            .collect::<Vec<_>>();
        let mut layout = SceneLayout {
            sign: PixelRect {
                x: shaft + 7,
                y: 4,
                width: width.saturating_sub(shaft + 14),
                height: title,
            }
            .scaled(scale),
            elevator: PixelRect {
                x: 6,
                y: floor.saturating_sub(if options.overview { 37 } else { 65 }),
                width: shaft - 12,
                height: if options.overview { 38 } else { 66 },
            }
            .scaled(scale),
            page,
            page_count,
            capacity,
            total_workers: total,
            scale,
            graphics: true,
            anchors: anchors
                .iter()
                .map(|a| SceneAnchor {
                    destination: a.destination,
                    x: a.x * scale,
                    y: a.y * scale,
                })
                .collect(),
            ..SceneLayout::default()
        };
        // Back chairs go behind bodies. Desks, keyboards and monitors go in front.
        let mut foreground = Raster::new(width, height);
        let mut aisle_actors = Vec::new();
        let mut tabletop_gestures = Vec::new();
        for (slot, worker) in visible.iter().enumerate() {
            let seat_x = first_x + slot * key.slot;
            let selected = options.selected_worker == Some(&worker.id);
            let preset = options
                .wardrobe
                .and_then(|map| map.get(&worker.id.0))
                .copied();
            let character = art::identity(&worker.id.0, preset);
            let decoration = decorations.get(&worker.id.0);
            let needs_attention = matches!(
                worker.status_at(options.now),
                WorkerStatus::Blocked | WorkerStatus::Failed
            );
            let unavailable = observation_unavailable(worker, options.now);
            let decoration =
                decoration.filter(|_| !needs_attention && options.motion && !unavailable);
            let cue = options
                .cues
                .and_then(|cues| cues.get(&worker.id.0))
                .copied()
                .filter(|_| {
                    !unavailable
                        && observed_pose(worker, options.now) != Pose::Waiting
                        && worker.status_at(options.now) != WorkerStatus::Failed
                });
            let (mut pose, _offset, aisle) = if let Some(cue) = cue {
                (cue.pose(), 0, false)
            } else {
                match decoration {
                    Some(decoration) => {
                        layout.active_gags += 1;
                        (decoration.pose, decoration.x_offset, true)
                    }
                    None => (observed_pose(worker, options.now), 0, false),
                }
            };
            if pose == Pose::Rest && !aisle {
                pose = Pose::SeatedRest;
            }
            let phase = if options.motion && !unavailable {
                ((options.now.max(0) as u64 / 125 + art::stable_hash(&worker.id.0) % 8) % 8) as u8
            } else {
                0
            };
            let station = workstation::geometry(key, seat_x);
            let origin_x = station.actor_home.x;
            let origin_y = station.actor_home.y;
            let (px, py, facing) = if aisle {
                let d = decoration.expect("aisle requires a decoration");
                let anchor = anchors
                    .iter()
                    .find(|anchor| anchor.destination == d.destination)
                    .expect("known destination");
                let target_x = anchor
                    .x
                    .saturating_sub(cast_w / 2)
                    .clamp(shaft, width.saturating_sub(cast_w));
                let target_y = anchor.y.saturating_sub(cast_h - 1);
                // The shaft-side corridor is wider than the 48-pixel actor.
                // Native nameplates start to its right and remain unobstructed.
                let (x, y) =
                    route_position((origin_x, origin_y), (target_x, target_y), 2, d.progress);
                let (next_x, _) = route_position(
                    (origin_x, origin_y),
                    (target_x, target_y),
                    2,
                    d.progress.saturating_add(20).min(1000),
                );
                let right = if d.walking {
                    (if next_x == x {
                        target_x >= origin_x
                    } else {
                        next_x > x
                    }) ^ d.returning
                } else {
                    matches!(
                        d.destination,
                        simulation::Destination::Coffee
                            | simulation::Destination::Plant
                            | simulation::Destination::StretchSpot
                    )
                };
                (x, y, if right { Facing::Right } else { Facing::Left })
            } else {
                (
                    origin_x,
                    origin_y,
                    if matches!(
                        pose,
                        Pose::Rest | Pose::SeatedRest | Pose::Waiting | Pose::Error
                    ) {
                        Facing::Front
                    } else if meeting
                        && slot > visible.len() / 2
                        && !matches!(pose, Pose::Work | Pose::ScreenRead | Pose::Search)
                    {
                        Facing::Left
                    } else {
                        Facing::Right
                    },
                )
            };
            let sprite = self.frame(character, pose, phase, options.overview, facing);
            // The sprite owns its cast shadow. It is deliberately not a motion/status color.
            let mut shadow = Raster::new(cast_w, 4);
            shadow.ellipse(2, 0, cast_w as i32 - 4, 3, rgb(42, 48, 49));
            let shadow = shadow.sprite();
            canvas.blit_scaled(
                &shadow,
                px * scale,
                (py + cast_h - 3) * scale,
                cast_w * scale,
                4 * scale,
            );
            if !aisle {
                canvas.blit_scaled(
                    &sprite,
                    px * scale,
                    py * scale,
                    cast_w * scale,
                    cast_h * scale,
                );
                if let Some(gesture) = art::gesture_layer(&sprite, pose, facing, options.overview) {
                    tabletop_gestures.push((gesture, px, py));
                }
            } else {
                aisle_actors.push((sprite, px, py));
            }
            workstation::furniture(
                &mut foreground,
                key,
                seat_x,
                selected,
                workstation::screen(worker, options.now),
                phase,
            );
            let plate = PixelRect {
                x: seat_x,
                y: key.plate_y(),
                width: key.slot - 4,
                height: key.plate,
            };
            layout.seats.push(SeatLayout {
                worker_id: worker.id.clone(),
                bounds: PixelRect {
                    x: px,
                    y: py,
                    width: cast_w,
                    height: cast_h,
                }
                .scaled(scale),
                nameplate: plate.scaled(scale),
                costume: art::COSTUMES[character.costume as usize],
                workstation: station.scaled(scale),
            });
            // Actors in the clear aisle are composed later, in front of furniture.
        }
        let foreground = foreground.sprite();
        canvas.blit_scaled(&foreground, 0, 0, width * scale, height * scale);
        for (sprite, x, y) in tabletop_gestures {
            canvas.blit_scaled(
                &sprite,
                x * scale,
                y * scale,
                cast_w * scale,
                cast_h * scale,
            );
        }
        for (sprite, x, y) in aisle_actors {
            canvas.blit_scaled(
                &sprite,
                x * scale,
                y * scale,
                cast_w * scale,
                cast_h * scale,
            );
        }
        layout
    }
}

fn ordered_workers<'a>(office: &'a Office, meeting: Option<&MeetingGroup>) -> Vec<&'a Worker> {
    if let Some(group) = meeting {
        let mut ids = vec![&group.parent];
        for id in &group.members {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids.into_iter()
            .filter_map(|id| office.workers.iter().find(|worker| &worker.id == id))
            .collect()
    } else {
        office.workers.iter().collect()
    }
}

fn page_workers<'a>(
    all: &[&'a Worker],
    capacity: usize,
    requested: usize,
    meeting: bool,
) -> (usize, usize, Vec<&'a Worker>) {
    let pin_parent = meeting && all.len() > 1 && capacity > 1;
    let chunk = capacity - usize::from(pin_parent);
    let total = all.len() - usize::from(pin_parent);
    let pages = total.div_ceil(chunk).max(1);
    let page = requested.min(pages - 1);
    let mut visible = Vec::new();
    if pin_parent {
        visible.push(all[0]);
    }
    visible.extend(
        all.iter()
            .skip(usize::from(pin_parent) + page * chunk)
            .take(chunk)
            .copied(),
    );
    (page, pages, visible)
}

fn observation_unavailable(worker: &Worker, now: Millis) -> bool {
    worker.coverage.observed_at > 0
        && (!worker.coverage.available || worker.coverage.is_stale_at(now))
}

fn observed_pose(worker: &Worker, now: Millis) -> Pose {
    if observation_unavailable(worker, now) {
        return Pose::Rest;
    }
    if worker.status_at(now) == WorkerStatus::Failed {
        return Pose::Error;
    }
    // A Wait event can follow Editing without replacing the last activity.
    // Explicit human input/approval is sufficient evidence for the raised hand.
    if crate::presentation::human_request(worker) {
        return Pose::Waiting;
    }
    match &worker.activity {
        Activity::Waiting { .. }
            if matches!(
                worker.wait_reason,
                None | Some(
                    theywork_core::WaitReason::HumanApproval
                        | theywork_core::WaitReason::HumanInput
                        | theywork_core::WaitReason::Unknown
                )
            ) =>
        {
            Pose::Waiting
        }
        Activity::Reading { .. } => Pose::ScreenRead,
        Activity::Searching { .. } => Pose::Search,
        Activity::Typing { .. } | Activity::Editing { .. }
            if worker.status_at(now) == WorkerStatus::Running =>
        {
            Pose::Work
        }
        _ => Pose::Rest,
    }
}

fn room_art(key: RoomKey) -> Sprite {
    rooms::background(key)
}

fn plant(art: &mut Raster, x: i32, floor: i32, size: i32) {
    let scale = size + 1;
    art.ellipse(
        x - 8 * scale,
        floor - 3 * scale,
        16 * scale,
        5 * scale,
        rgb(49, 59, 54),
    );
    art.poly(
        &[
            (x - 4 * scale, floor - 7 * scale),
            (x + 4 * scale, floor - 7 * scale),
            (x + 3 * scale, floor),
            (x - 3 * scale, floor),
        ],
        rgb(161, 91, 71),
    );
    art.rect(
        x - 4 * scale,
        floor - 8 * scale,
        8 * scale,
        2 * scale,
        rgb(211, 139, 99),
    );
    art.rect(x, floor - 20 * scale, scale, 13 * scale, rgb(93, 130, 82));
    for (dx, dy, light) in [
        (-6, -17, true),
        (1, -20, false),
        (-6, -24, false),
        (1, -28, true),
    ] {
        art.ellipse(
            x + dx * scale,
            floor + dy * scale,
            7 * scale,
            5 * scale,
            if light {
                rgb(129, 164, 97)
            } else {
                rgb(70, 120, 89)
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use theywork_core::{Agent, OfficeId};
    fn office(count: usize) -> Office {
        let mut office = Office::new(OfficeId("studio".into()), "/studio".into());
        office.workers = (0..count)
            .map(|i| {
                Worker::new(
                    WorkerId(format!("person-{i}")),
                    office.id.clone(),
                    Agent::Codex,
                    format!("Task {i}"),
                    0,
                )
            })
            .collect();
        office
    }
    #[test]
    fn workstation_anchors_fit_three_at_eighty_columns_and_translate_with_the_scene() {
        let office = office(3);
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize(640, 304);
        let options = SceneOptions {
            motion: false,
            ..Default::default()
        };
        let layout = studio.paint(&mut canvas, &office, &options);
        assert_eq!(
            (layout.seats.len(), layout.capacity, layout.scale),
            (3, 3, 2)
        );
        let mut bigger = Canvas::new(0, 0);
        bigger.set_image_cell_size(Some((8, 16)));
        bigger.resize(800, 400);
        let moved = studio.paint_region(
            &mut bigger,
            PixelRect {
                x: 80,
                y: 48,
                width: 640,
                height: 304,
            },
            &office,
            &options,
        );
        for (original, moved) in layout.seats.iter().zip(&moved.seats) {
            let mut expected = original.workstation;
            expected.translate(80, 48);
            assert_eq!(expected, moved.workstation);
            for r in [
                original.workstation.computer,
                original.workstation.keyboard,
                original.workstation.chair,
                original.workstation.actor_home,
            ] {
                assert!(
                    r.x >= original.workstation.selection.x
                        && r.x + r.width
                            <= original.workstation.selection.x
                                + original.workstation.selection.width
                );
                assert!(r.y + r.height <= 304);
            }
        }
    }
    #[test]
    fn unavailable_observations_do_not_animate_past_work_errors_or_requests() {
        let mut office = office(1);
        let id = office.workers[0].id.0.clone();
        let cues = BTreeMap::from([(id, SceneCue::Delivering)]);
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize(640, 480);
        for activity in [
            Activity::Editing {
                detail: "file.rs".into(),
            },
            Activity::Error {
                detail: "old failure".into(),
            },
            Activity::Waiting {
                detail: "recorded question".into(),
            },
        ] {
            let worker = &mut office.workers[0];
            worker.activity = activity;
            worker.turn_in_flight = true;
            worker.wait_reason = Some(theywork_core::WaitReason::HumanInput);
            worker.coverage.observed_at = 1;
            worker.coverage.available = false;
            worker.coverage.available = true;
            assert_eq!(observed_pose(worker, 200_001), Pose::Rest);
            worker.coverage.available = false;
            assert_eq!(observed_pose(worker, 1000), Pose::Rest);
            assert_eq!(
                workstation::screen(worker, 1000),
                workstation::Screen::Unknown
            );
            let options = SceneOptions {
                now: 1000,
                motion: true,
                cues: Some(&cues),
                ..Default::default()
            };
            let frame = studio.paint(&mut canvas, &office, &options);
            assert_eq!(frame.active_gags, 0);
            let expected = canvas.pixel_frame();
            studio.paint(
                &mut canvas,
                &office,
                &SceneOptions {
                    now: 1125,
                    ..options
                },
            );
            assert_eq!(expected.rgba(), canvas.pixel_frame().rgba());
        }
    }
    #[test]
    fn frontal_and_three_quarter_poses_are_authored_at_both_grids() {
        for costume in 0..12 {
            let person = Character {
                costume,
                skin: 0,
                hair: 0,
            };
            for small in [false, true] {
                let make = |facing| {
                    if small {
                        overview::character_facing(person, Pose::Waiting, 0, facing)
                    } else {
                        art::character_facing(person, Pose::Waiting, 0, facing)
                    }
                };
                let front = make(Facing::Front);
                let right = make(Facing::Right);
                let left = make(Facing::Left);
                let pixels = |s: &Sprite| {
                    (0..s.height())
                        .flat_map(|y| (0..s.width()).map(move |x| s.pixel(x, y)))
                        .collect::<Vec<_>>()
                };
                assert_ne!(
                    pixels(&front),
                    pixels(&right),
                    "frontal pose differs for costume {costume}, overview {small}"
                );
                assert_ne!(pixels(&front), pixels(&left));
                let (hand_x, hand_y) = if small { (20, 5) } else { (41, 7) };
                assert!(
                    front.pixel(hand_x, hand_y).is_some(),
                    "raised human-help hand remains visible"
                );
            }
        }
    }

    #[test]
    fn image_geometry_and_fallback_are_explicit() {
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.resize_for_cells(120, 32);
        assert!(
            !studio
                .paint(&mut canvas, &office(3), &SceneOptions::default())
                .graphics
        );
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize_for_cells(120, 32);
        let layout = studio.paint(&mut canvas, &office(3), &SceneOptions::default());
        assert!(layout.graphics);
        assert_eq!((canvas.width(), canvas.height()), (960, 512));
        assert_eq!(layout.scale, 2);
        assert_eq!(layout.seats.len(), 3);
        assert_eq!(layout.page_count, 1);
        for seat in layout.seats {
            assert!(seat.bounds.x + seat.bounds.width <= canvas.width());
            assert!(seat.bounds.y + seat.bounds.height <= canvas.height());
        }
    }
    #[test]
    fn detail_geometry_preserves_scale_and_keeps_labels_next_to_people_when_resized() {
        for (width, height, expected) in
            [(528, 592, 1), (560, 280, 2), (960, 512, 2), (1536, 848, 2)]
        {
            assert_eq!(detail_scale(width, height), expected);
            let mut canvas = Canvas::new(0, 0);
            canvas.set_image_cell_size(Some((8, 16)));
            canvas.resize(width, height);
            let layout = Studio::new().paint(
                &mut canvas,
                &office(3),
                &SceneOptions {
                    motion: false,
                    ..SceneOptions::default()
                },
            );
            assert_eq!(layout.scale, expected);
            for seat in layout.seats {
                assert_eq!(seat.bounds.height, 64 * expected);
                assert_eq!(
                    seat.nameplate.y - (seat.bounds.y + seat.bounds.height),
                    2 * expected
                );
                assert!(seat.nameplate.y + seat.nameplate.height <= height);
            }
        }
    }

    #[test]
    fn lower_aisle_routes_never_cross_native_seat_labels() {
        let office = office(3);
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize(960, 832);
        let mut studio = Studio::new();
        for now in (8_000..24_000).step_by(125) {
            let layout = studio.paint(
                &mut canvas,
                &office,
                &SceneOptions {
                    now,
                    ..SceneOptions::default()
                },
            );
            for actor in &layout.seats {
                for label in &layout.seats {
                    let a = actor.bounds;
                    let b = label.nameplate;
                    assert!(
                        a.x + a.width <= b.x
                            || b.x + b.width <= a.x
                            || a.y + a.height <= b.y
                            || b.y + b.height <= a.y,
                        "actor {} crosses a native label at {now}",
                        actor.worker_id.0
                    );
                }
            }
        }
    }

    #[test]
    fn meeting_keeps_parent_and_paginates_every_member() {
        let office = office(20);
        let all = office.workers.iter().collect::<Vec<_>>();
        let mut children = std::collections::BTreeSet::new();
        for page in 0..5 {
            let (_, _, visible) = page_workers(&all, 5, page, true);
            assert_eq!(visible[0].id, all[0].id);
            for child in &visible[1..] {
                children.insert(child.id.clone());
            }
        }
        assert_eq!(children.len(), 19);
    }
    #[test]
    fn reduced_motion_is_identical_but_real_requests_are_not_frozen() {
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize_for_cells(100, 24);
        let mut office = office(3);
        let mut options = SceneOptions {
            motion: false,
            ..SceneOptions::default()
        };
        studio.paint(&mut canvas, &office, &options);
        let before = canvas.pixel_frame();
        options.now = 90_000;
        studio.paint(&mut canvas, &office, &options);
        assert_eq!(before.rgba(), canvas.pixel_frame().rgba());
        office.workers[0].activity = Activity::Waiting {
            detail: "Review requested".into(),
        };
        studio.paint(&mut canvas, &office, &options);
        assert_ne!(before.rgba(), canvas.pixel_frame().rgba());
    }
    #[test]
    fn artwork_cache_is_bounded() {
        let mut studio = Studio::new();
        for i in 0..700 {
            let look = Character {
                costume: (i % 12) as u8,
                skin: ((i / 12) % 6) as u8,
                hair: ((i / 72) % 6) as u8,
            };
            studio.character_frame(look, Pose::Work, (i / 432) as u8);
        }
        assert!(studio.frames.len() <= 512);
    }

    #[test]
    fn selecting_a_member_follows_its_page_but_parent_keeps_the_requested_page() {
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize_for_cells(80, 24);
        let office = office(20);
        let group = MeetingGroup {
            parent: office.workers[0].id.clone(),
            members: office
                .workers
                .iter()
                .skip(1)
                .map(|w| w.id.clone())
                .collect(),
        };
        let mut options = SceneOptions {
            meeting: Some(&group),
            selected_worker: Some(&office.workers[19].id),
            ..SceneOptions::default()
        };
        let last = studio.paint(&mut canvas, &office, &options);
        assert!(last
            .seats
            .iter()
            .any(|seat| seat.worker_id == office.workers[19].id));
        assert_eq!(last.seats[0].worker_id, office.workers[0].id);
        options.selected_worker = Some(&office.workers[0].id);
        options.page = 4;
        assert_eq!(studio.paint(&mut canvas, &office, &options).page, 4);
    }

    #[test]
    fn observed_human_request_has_priority_over_a_delivery_cue() {
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize_for_cells(80, 24);
        let mut office = office(1);
        office.workers[0].activity = Activity::Waiting {
            detail: "Please review".into(),
        };
        office.workers[0].wait_reason = Some(theywork_core::WaitReason::HumanApproval);
        let cues = BTreeMap::from([(office.workers[0].id.0.clone(), SceneCue::Delivering)]);
        studio.paint(
            &mut canvas,
            &office,
            &SceneOptions {
                motion: false,
                ..SceneOptions::default()
            },
        );
        let expected = canvas.pixel_frame();
        studio.paint(
            &mut canvas,
            &office,
            &SceneOptions {
                motion: false,
                cues: Some(&cues),
                ..SceneOptions::default()
            },
        );
        assert_eq!(canvas.pixel_frame().rgba(), expected.rgba());
    }

    #[test]
    fn explicit_human_wait_overrides_previous_editing_but_not_a_failure() {
        use theywork_core::WaitReason;
        let mut worker = office(1).workers.remove(0);
        worker.activity = Activity::Editing {
            detail: "src/account.rs".into(),
        };
        for reason in [WaitReason::HumanApproval, WaitReason::HumanInput] {
            worker.wait_reason = Some(reason);
            assert_eq!(observed_pose(&worker, 0), Pose::Waiting);
        }
        for reason in [
            WaitReason::AutomaticReview,
            WaitReason::Child,
            WaitReason::Process,
        ] {
            worker.wait_reason = Some(reason);
            assert_ne!(observed_pose(&worker, 0), Pose::Waiting);
        }
        worker.wait_reason = Some(WaitReason::HumanInput);
        worker.activity = Activity::Error {
            detail: "Process failed".into(),
        };
        assert_eq!(observed_pose(&worker, 0), Pose::Error);
    }

    #[test]
    fn automatic_waits_do_not_raise_a_hand() {
        let mut worker = office(1).workers.remove(0);
        worker.activity = Activity::Waiting {
            detail: "Waiting for a result".into(),
        };
        for reason in [
            theywork_core::WaitReason::AutomaticReview,
            theywork_core::WaitReason::Child,
            theywork_core::WaitReason::Process,
        ] {
            worker.wait_reason = Some(reason);
            assert_eq!(observed_pose(&worker, 0), Pose::Rest);
        }
        for reason in [
            theywork_core::WaitReason::HumanApproval,
            theywork_core::WaitReason::HumanInput,
            theywork_core::WaitReason::Unknown,
        ] {
            worker.wait_reason = Some(reason);
            assert_eq!(observed_pose(&worker, 0), Pose::Waiting);
        }
    }

    #[test]
    fn adjoining_room_door_keeps_seat_geometry_and_its_own_cached_background() {
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize_for_cells(80, 24);
        let office = office(3);
        let mut options = SceneOptions {
            motion: false,
            ..SceneOptions::default()
        };
        let primary = studio.paint(&mut canvas, &office, &options);
        let primary_pixels = canvas.pixel_frame();
        options.show_elevator = false;
        let adjoining = studio.paint(&mut canvas, &office, &options);
        assert_eq!(primary.seats[0].bounds, adjoining.seats[0].bounds);
        assert_ne!(primary_pixels.rgba(), canvas.pixel_frame().rgba());
        options.show_elevator = true;
        studio.paint(&mut canvas, &office, &options);
        assert_eq!(primary_pixels.rgba(), canvas.pixel_frame().rgba());
    }

    #[test]
    fn compact_graphics_use_double_size_and_follow_last_child_with_pinned_lead() {
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        // 80×24 terminal after native navigation and footer rows.
        canvas.resize_for_cells(80, 19);
        let office = office(4);
        let group = MeetingGroup {
            parent: office.workers[0].id.clone(),
            members: office
                .workers
                .iter()
                .skip(1)
                .map(|worker| worker.id.clone())
                .collect(),
        };
        let layout = studio.paint(
            &mut canvas,
            &office,
            &SceneOptions {
                motion: false,
                meeting: Some(&group),
                selected_worker: Some(&office.workers[3].id),
                ..SceneOptions::default()
            },
        );
        assert_eq!(layout.scale, 2);
        assert_eq!((layout.page, layout.page_count), (1, 2));
        assert_eq!(
            layout
                .seats
                .iter()
                .map(|seat| &seat.worker_id)
                .collect::<Vec<_>>(),
            vec![&office.workers[0].id, &office.workers[3].id]
        );
        assert!(layout.seats.iter().all(|seat| seat.bounds.height == 128));
    }

    #[test]
    fn contiguous_regions_keep_one_image_and_translated_hitboxes() {
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize_for_cells(100, 40);
        canvas.fill(rgb(1, 2, 3));
        let top = studio.paint_region(
            &mut canvas,
            PixelRect {
                x: 0,
                y: 0,
                width: 800,
                height: 196,
            },
            &office(3),
            &SceneOptions::default(),
        );
        let bottom = studio.paint_region(
            &mut canvas,
            PixelRect {
                x: 0,
                y: 196,
                width: 800,
                height: 196,
            },
            &office(3),
            &SceneOptions::default(),
        );
        assert!(top.graphics && bottom.graphics);
        assert_eq!(top.elevator.x, bottom.elevator.x);
        assert_eq!(top.seats[0].bounds.y + 196, bottom.seats[0].bounds.y);
        assert_eq!(canvas.pixel(0, 400), Some(rgb(1, 2, 3)));
    }

    #[test]
    fn overview_has_authored_double_size_people_and_native_label_space_at_128_pixels() {
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize(640, 128);
        let layout = studio.paint(
            &mut canvas,
            &office(3),
            &SceneOptions {
                overview: true,
                motion: false,
                ..SceneOptions::default()
            },
        );
        assert!(layout.graphics);
        assert_eq!(layout.scale, 2);
        assert_eq!(layout.sign.height, 16);
        for seat in &layout.seats {
            assert_eq!((seat.bounds.width, seat.bounds.height), (48, 64));
            assert!(seat.bounds.y >= layout.sign.y + layout.sign.height);
            assert!(seat.bounds.y + seat.bounds.height <= seat.nameplate.y);
            assert_eq!(seat.nameplate.height, 16);
            assert!(seat.nameplate.y + seat.nameplate.height <= 128);
        }
    }

    #[test]
    fn shared_office_plan_keeps_two_gags_across_rooms_and_returns_to_the_desk() {
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize(640, 304);
        let office = office(4);
        let workers = office.workers.iter().collect::<Vec<_>>();
        let profiles = BTreeMap::new();
        let planned = simulation::plan(&workers, 14_000, true, &profiles);
        assert_eq!(planned.len(), 2);
        let mut total = 0;
        for parity in 0..2 {
            let mut room = office.clone();
            room.workers = office
                .workers
                .iter()
                .enumerate()
                .filter(|(i, _)| i % 2 == parity)
                .map(|(_, w)| w.clone())
                .collect();
            total += studio
                .paint(
                    &mut canvas,
                    &room,
                    &SceneOptions {
                        now: 14_000,
                        decorations: Some(&planned),
                        ..SceneOptions::default()
                    },
                )
                .active_gags;
        }
        assert_eq!(total, 2);
        let before = studio.paint(
            &mut canvas,
            &office,
            &SceneOptions {
                now: 8_000,
                ..SceneOptions::default()
            },
        );
        let away = studio.paint(
            &mut canvas,
            &office,
            &SceneOptions {
                now: 14_000,
                ..SceneOptions::default()
            },
        );
        let returned = studio.paint(
            &mut canvas,
            &office,
            &SceneOptions {
                now: 23_999,
                ..SceneOptions::default()
            },
        );
        assert!(before
            .seats
            .iter()
            .zip(&away.seats)
            .any(|(a, b)| a.bounds != b.bounds));
        assert_eq!(
            before.seats.iter().map(|s| s.bounds).collect::<Vec<_>>(),
            returned.seats.iter().map(|s| s.bounds).collect::<Vec<_>>()
        );
    }

    #[test]
    fn all_presets_and_zones_change_visible_art_without_changing_worker_identity() {
        let mut studio = Studio::new();
        let mut canvas = Canvas::new(0, 0);
        canvas.set_image_cell_size(Some((8, 16)));
        canvas.resize(640, 304);
        let office = office(3);
        let mut hashes = std::collections::HashSet::new();
        for preset in [
            OfficePreset::Studio,
            OfficePreset::Workshop,
            OfficePreset::Laboratory,
        ] {
            let design = OfficeDesign {
                preset,
                ..OfficeDesign::default()
            };
            let layout = studio.paint(
                &mut canvas,
                &office,
                &SceneOptions {
                    motion: false,
                    design: Some(&design),
                    ..SceneOptions::default()
                },
            );
            assert_eq!(
                layout
                    .seats
                    .iter()
                    .map(|s| s.worker_id.clone())
                    .collect::<Vec<_>>(),
                office
                    .workers
                    .iter()
                    .map(|w| w.id.clone())
                    .collect::<Vec<_>>()
            );
            hashes.insert(canvas.pixel_frame().rgba().to_vec());
        }
        assert_eq!(hashes.len(), 3);
        for zone in 0..4 {
            let mut variants = std::collections::HashSet::new();
            for choice in 0..3 {
                let mut design = OfficeDesign::default();
                match zone {
                    0 => design.entrance = choice,
                    1 => design.desks = choice,
                    2 => design.meeting = choice,
                    _ => design.rest = choice,
                };
                let group = MeetingGroup {
                    parent: office.workers[0].id.clone(),
                    members: office
                        .workers
                        .iter()
                        .skip(1)
                        .map(|w| w.id.clone())
                        .collect(),
                };
                studio.paint(
                    &mut canvas,
                    &office,
                    &SceneOptions {
                        motion: false,
                        design: Some(&design),
                        meeting: (zone == 2).then_some(&group),
                        ..SceneOptions::default()
                    },
                );
                variants.insert(canvas.pixel_frame().rgba().to_vec());
            }
            assert_eq!(variants.len(), 3, "zone {zone}");
        }
    }
}
