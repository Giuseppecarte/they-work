//! A continuous side-cut office drawn in physical pixels. Native text, input,
//! provider facts and image transport stay with the host. The same deterministic
//! composition can therefore be inspected without pretending a PTY is a GPU.

pub mod art;
pub mod simulation;

use std::collections::{BTreeMap, HashMap, VecDeque};

use theywork_core::{Activity, Millis, Office, Worker, WorkerId, WorkerStatus};

use crate::{canvas::Canvas, sprite::Sprite};
use art::{darken, lighten, rgb, Character, Pose, Raster};

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
            Self::WaitingForTeam => Pose::Read,
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
}

#[derive(Debug, Clone, Default)]
pub struct SceneLayout {
    pub seats: Vec<SeatLayout>,
    pub sign: PixelRect,
    pub elevator: PixelRect,
    pub page: usize,
    pub page_count: usize,
    pub total_workers: usize,
    pub active_gags: usize,
    pub scale: usize,
    /// False means the host should draw its compact native presentation.
    pub graphics: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FrameKey {
    character: Character,
    pose: Pose,
    phase: u8,
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
        surface.set_image_cell_size(Some((1, 1)));
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
        }
        self.region_canvas = Some(surface);
        layout
    }

    pub fn character_frame(&mut self, character: Character, pose: Pose, phase: u8) -> Sprite {
        let key = FrameKey {
            character,
            pose,
            phase: phase % 8,
        };
        if let Some(sprite) = self.frames.get(&key) {
            return sprite.clone();
        }
        if self.frames.len() >= 512 {
            if let Some(old) = self.insertion_order.pop_front() {
                self.frames.remove(&old);
            }
        }
        let sprite = art::character(character, pose, key.phase);
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
        if !canvas.has_image_density() || canvas.width() < 280 || canvas.height() < 156 {
            return SceneLayout {
                total_workers: office.workers.len(),
                ..SceneLayout::default()
            };
        }
        // Height sets the character's readable size. Narrow windows paginate
        // seats instead of reducing the entire cast to tiny figures.
        let scale = (canvas.height() / 150)
            .clamp(1, 4)
            .min((canvas.width() / 280).max(1));
        let width = canvas.width() / scale;
        let height = canvas.height() / scale;
        let capacity = ((width.saturating_sub(108)) / 92).clamp(1, 8);
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
        let key = RoomKey {
            width,
            height,
            palette: options.palette % 4,
            light: options.light,
            seats: visible.len(),
            meeting,
            elevator: options.show_elevator,
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
        let floor = height as i32 - 42;
        let seat_width = 92;
        let region_width = width as i32 - 104;
        let first_x = 88 + (region_width - visible.len() as i32 * seat_width).max(0) / 2;
        let decorations = simulation::decorations(&all, options.now, options.motion);
        let mut layout = SceneLayout {
            sign: PixelRect {
                x: 96,
                y: 18,
                width: width.saturating_sub(125),
                height: 24,
            }
            .scaled(scale),
            elevator: PixelRect {
                x: 12,
                y: (floor - 102).max(0) as usize,
                width: 58,
                height: 103,
            }
            .scaled(scale),
            page,
            page_count,
            total_workers: total,
            scale,
            graphics: true,
            ..SceneLayout::default()
        };
        // Back chairs go behind bodies. Desks, keyboards and monitors go in front.
        let mut foreground = Raster::new(width, height);
        let mut aisle_actors = Vec::new();
        for (slot, worker) in visible.iter().enumerate() {
            let seat_x = first_x + slot as i32 * seat_width;
            let selected = options.selected_worker == Some(&worker.id);
            let preset = options
                .wardrobe
                .and_then(|map| map.get(&worker.id.0))
                .copied();
            let character = art::identity(&worker.id.0, preset);
            let decoration = decorations.iter().find(|(id, _)| id == &worker.id);
            let needs_attention = matches!(
                worker.status_at(options.now),
                WorkerStatus::Blocked | WorkerStatus::Failed
            );
            let decoration = decoration.filter(|_| !needs_attention && !meeting);
            let cue = options
                .cues
                .and_then(|cues| cues.get(&worker.id.0))
                .copied()
                .filter(|_| {
                    observed_pose(worker, options.now) != Pose::Waiting
                        && worker.status_at(options.now) != WorkerStatus::Failed
                });
            let (pose, offset, aisle) = if let Some(cue) = cue {
                (cue.pose(), 0, false)
            } else {
                match decoration {
                    Some((_, decoration)) => {
                        layout.active_gags += 1;
                        (decoration.pose, decoration.x_offset, true)
                    }
                    None => (observed_pose(worker, options.now), 0, false),
                }
            };
            let phase = if options.motion {
                ((options.now.max(0) as u64 / 125 + art::stable_hash(&worker.id.0) % 8) % 8) as u8
            } else {
                0
            };
            let sprite = self.character_frame(character, pose, phase);
            let px = (seat_x + 18 + offset).max(0) as usize;
            let py = (floor - 63 + if aisle { 19 } else { 0 }).max(0) as usize;
            // The sprite owns its cast shadow. It is deliberately not a motion/status color.
            let mut shadow = Raster::new(48, 8);
            shadow.ellipse(6, 0, 39, 6, rgb(42, 48, 49));
            let shadow = shadow.sprite();
            canvas.blit_scaled(
                &shadow,
                px * scale,
                (py + 60) * scale,
                48 * scale,
                8 * scale,
            );
            if !aisle {
                canvas.blit_scaled(
                    &sprite,
                    px * scale,
                    py * scale,
                    art::WIDTH * scale,
                    art::HEIGHT * scale,
                );
            } else {
                aisle_actors.push((sprite, px, py));
            }
            if meeting {
                meeting_front(&mut foreground, seat_x, floor, selected);
            } else {
                desk_front(&mut foreground, seat_x, floor, selected, character.costume);
            }
            let plate = PixelRect {
                x: seat_x.max(0) as usize,
                y: (floor + 25).max(0) as usize,
                width: 86,
                height: 15,
            };
            layout.seats.push(SeatLayout {
                worker_id: worker.id.clone(),
                bounds: PixelRect {
                    x: px,
                    y: py,
                    width: 48,
                    height: 64,
                }
                .scaled(scale),
                nameplate: plate.scaled(scale),
                costume: art::COSTUMES[character.costume as usize],
            });
            // Actors in the clear aisle are composed later, in front of furniture.
        }
        let foreground = foreground.sprite();
        canvas.blit_scaled(&foreground, 0, 0, width * scale, height * scale);
        for (sprite, x, y) in aisle_actors {
            canvas.blit_scaled(&sprite, x * scale, y * scale, 48 * scale, 64 * scale);
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

fn observed_pose(worker: &Worker, now: Millis) -> Pose {
    if worker.status_at(now) == WorkerStatus::Failed {
        return Pose::Error;
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
        Activity::Reading { .. } => Pose::Read,
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
    let mut art = Raster::new(key.width, key.height);
    let w = key.width as i32;
    let h = key.height as i32;
    let floor = h - 42;
    let wall = if key.light {
        [
            rgb(222, 208, 185),
            rgb(199, 217, 215),
            rgb(216, 222, 189),
            rgb(226, 205, 212),
        ][key.palette]
    } else {
        [
            rgb(81, 83, 91),
            rgb(53, 78, 99),
            rgb(65, 87, 77),
            rgb(87, 69, 91),
        ][key.palette]
    };
    let trim = if key.light {
        rgb(132, 126, 118)
    } else {
        rgb(37, 42, 52)
    };
    art.rect(0, 0, w, h, rgb(40, 45, 55));
    art.rect(79, 12, w - 83, floor - 10, wall);
    art.rect(80, 14, w - 86, 3, lighten(wall, 24));
    art.rect(79, floor, w - 80, 42, rgb(104, 86, 73));
    for y in (floor..h).step_by(13) {
        art.rect(80, y, w - 80, 1, rgb(137, 115, 91));
        let stagger = if ((y - floor) / 13) % 2 == 0 { 0 } else { 43 };
        for x in (80 + stagger..w).step_by(86) {
            art.rect(x, y, 1, 13, rgb(81, 72, 67));
            art.rect(x + 10, y + 5, 22, 1, rgb(115, 94, 77));
            art.rect(x + 41, y + 8, 13, 1, rgb(94, 79, 70));
        }
    }
    // Wall panels, skirting, and a continuous structural slab tie floors together.
    for x in (84..w).step_by(112) {
        art.rect(x, 48, 1, (floor - 48).max(0), darken(wall, 7));
    }
    art.rect(79, floor - 6, w - 79, 6, trim);
    art.rect(80, floor - 6, w - 80, 1, lighten(trim, 21));
    art.rect(79, floor, w - 79, 3, rgb(60, 60, 59));
    art.rect(0, 0, w, 10, rgb(30, 35, 44));
    art.rect(0, 10, w, 3, rgb(114, 105, 99));
    art.rect(0, h - 3, w, 3, rgb(27, 33, 41));
    // Fixed light direction, restrained windows, deep reveals and city silhouettes.
    let window_y = 53;
    let window_h = (floor - 124).clamp(38, 74);
    let count = ((w - 110) / 150).max(1);
    for n in 0..count {
        let x = 100 + n * 150;
        window(&mut art, x, window_y, 106, window_h, key.light);
        art.poly(
            &[
                (x + 2, window_y + window_h + 3),
                (x + 102, window_y + window_h + 3),
                (x + 143, floor - 7),
                (x + 53, floor - 7),
            ],
            lighten(wall, 6),
        );
    }
    // Physically mounted project fascia, native title fitted by the host.
    art.rect(92, 20, w - 118, 26, darken(wall, 29));
    art.rect(94, 18, w - 118, 26, trim);
    art.rect(95, 19, w - 120, 1, lighten(trim, 30));
    for x in [98, w - 30] {
        art.rect(x, 22, 2, 2, rgb(160, 157, 142));
    }
    // A stable shaft is architectural, not a claim that a conversation moved.
    // Adjacent project rooms share the tower shaft and have ordinary doors.
    art.rect(0, 12, 78, h - 15, rgb(43, 49, 62));
    art.rect(3, 12, 4, h - 15, rgb(74, 81, 94));
    art.rect(73, 12, 5, h - 15, rgb(24, 31, 43));
    let door_y = (floor - 92).max(50);
    art.rect(12, door_y - 10, 56, floor - door_y + 11, rgb(25, 32, 43));
    if key.elevator {
        art.rect(14, door_y, 52, floor - door_y, rgb(103, 117, 128));
        art.rect(15, door_y + 1, 23, floor - door_y - 1, rgb(133, 145, 151));
        art.rect(41, door_y + 1, 23, floor - door_y - 1, rgb(113, 128, 140));
        art.rect(18, door_y + 3, 3, floor - door_y - 5, rgb(155, 165, 166));
        art.rect(38, door_y, 3, floor - door_y, rgb(52, 65, 80));
        art.rect(24, door_y - 8, 29, 6, rgb(26, 35, 46));
        art.rect(29, door_y - 6, 5, 2, rgb(144, 206, 183));
        art.rect(61, door_y + 25, 5, 13, rgb(46, 59, 72));
        art.rect(62, door_y + 28, 2, 2, rgb(220, 197, 144));
    } else {
        art.rect(14, door_y, 52, floor - door_y, rgb(114, 82, 59));
        art.rect(16, door_y + 2, 48, floor - door_y - 3, rgb(151, 113, 79));
        art.rect(19, door_y + 6, 40, 37, rgb(95, 120, 126));
        art.rect(21, door_y + 8, 36, 32, rgb(145, 166, 164));
        art.rect(23, door_y + 9, 4, 29, rgb(177, 191, 176));
        art.rect(
            19,
            door_y + 51,
            40,
            (floor - door_y - 59).max(3),
            rgb(129, 91, 62),
        );
        art.rect(
            20,
            door_y + 52,
            2,
            (floor - door_y - 61).max(1),
            rgb(166, 126, 86),
        );
        art.rect(56, door_y + 45, 5, 3, rgb(221, 185, 111));
    }
    art.rect(11, floor, 58, 3, rgb(151, 151, 143));
    // Lobby notice board and fire-safe visual gaps around the doorway.
    art.rect(17, 22, 45, 23, rgb(34, 39, 51));
    art.rect(19, 24, 41, 19, rgb(112, 128, 127));
    art.rect(23, 27, 17, 2, rgb(206, 216, 196));
    art.rect(23, 32, 30, 2, rgb(181, 193, 178));
    art.rect(23, 37, 22, 2, rgb(157, 177, 167));
    let first_x = 88 + ((w - 104 - key.seats as i32 * 92).max(0)) / 2;
    for slot in 0..key.seats {
        let x = first_x + slot as i32 * 92;
        art.ellipse(x + 11, floor - 5, 68, 10, rgb(58, 57, 54));
        art.rect(x + 23, floor - 37, 35, 27, rgb(31, 42, 56));
        art.rect(x + 25, floor - 36, 29, 21, rgb(72, 98, 113));
        art.rect(x + 27, floor - 34, 25, 3, rgb(96, 124, 135));
        art.rect(x + 38, floor - 12, 5, 10, rgb(60, 69, 76));
        art.rect(x + 25, floor - 3, 29, 3, rgb(38, 45, 54));
        art.rect(x + 23, floor, 5, 3, rgb(29, 34, 43));
        art.rect(x + 52, floor, 5, 3, rgb(29, 34, 43));
    }
    if key.seats == 0 {
        plant(&mut art, (w + 70) / 2, floor, 2);
    }
    plant(&mut art, w - 23, floor, 1);
    art.sprite()
}

fn window(art: &mut Raster, x: i32, y: i32, w: i32, h: i32, light: bool) {
    art.rect(x - 3, y - 3, w + 6, h + 7, rgb(40, 48, 58));
    art.rect(x - 2, y - 2, w + 3, h + 3, rgb(107, 126, 132));
    art.rect(
        x,
        y,
        w,
        h,
        if light {
            rgb(151, 193, 204)
        } else {
            rgb(93, 131, 156)
        },
    );
    art.rect(
        x,
        y,
        w,
        h / 2,
        if light {
            rgb(177, 211, 215)
        } else {
            rgb(118, 156, 174)
        },
    );
    for n in 0..8 {
        let bw = 9 + n % 3 * 3;
        let bh = 7 + (n * 7) % 19;
        let bx = x + n * 14;
        art.rect(bx, y + h - bh, bw, bh, rgb(80, 111, 130));
        art.rect(bx + 2, y + h - bh + 3, 2, 2, rgb(160, 179, 167));
        art.rect(bx + 6, y + h - bh + 7, 2, 2, rgb(159, 176, 164));
    }
    art.poly(
        &[
            (x + 6, y + 1),
            (x + 20, y + 1),
            (x + 8, y + h - 1),
            (x + 1, y + h - 1),
        ],
        rgb(161, 193, 197),
    );
    art.rect(x + w / 2, y, 3, h, rgb(66, 83, 97));
    art.rect(x, y + h / 2, w, 2, rgb(73, 90, 100));
    art.rect(x - 4, y + h + 2, w + 8, 4, rgb(149, 154, 144));
    art.rect(x - 4, y + h + 6, w + 8, 2, rgb(50, 57, 65));
}

fn desk_front(art: &mut Raster, x: i32, floor: i32, selected: bool, costume: u8) {
    let y = floor - 25;
    art.rect(x + 3, y + 5, 5, 23, rgb(52, 56, 62));
    art.rect(x + 72, y + 5, 5, 23, rgb(43, 48, 56));
    art.rect(x + 4, y + 6, 2, 19, rgb(119, 126, 128));
    art.rect(x + 72, y + 6, 2, 19, rgb(93, 106, 114));
    art.rect(x, y, 82, 6, rgb(102, 71, 51));
    art.rect(x, y, 82, 2, rgb(217, 174, 118));
    art.rect(x + 1, y + 2, 80, 2, rgb(169, 120, 79));
    art.rect(x + 4, y + 6, 25, 15, rgb(124, 88, 66));
    art.rect(x + 6, y + 8, 21, 5, rgb(146, 102, 72));
    art.rect(x + 15, y + 10, 5, 1, rgb(211, 177, 124));
    art.rect(x + 15, y + 17, 5, 1, rgb(199, 166, 120));
    art.rect(x + 60, y - 4, 20, 3, rgb(36, 43, 55));
    art.rect(x + 67, y - 14, 4, 11, rgb(55, 66, 79));
    art.rect(x + 59, y - 29, 23, 18, rgb(30, 39, 54));
    art.rect(x + 61, y - 27, 19, 13, rgb(54, 90, 106));
    art.rect(x + 62, y - 26, 17, 2, rgb(91, 155, 162));
    for (offset, length) in [(0, 10), (3, 15), (6, 8)] {
        art.rect(x + 63, y - 22 + offset, length, 1, rgb(143, 185, 171));
    }
    art.rect(x + 24, y - 3, 25, 3, rgb(35, 45, 56));
    for key in 0..7 {
        art.rect(x + 25 + key * 3, y - 3, 2, 1, rgb(135, 152, 154));
    }
    art.rect(x + 6, y - 7, 6, 7, rgb(223, 213, 180));
    art.rect(x + 6, y - 7, 6, 2, rgb(82, 58, 43));
    if costume.is_multiple_of(3) {
        plant(art, x + 17, y, 0);
    } else if costume % 3 == 1 {
        art.rect(x + 13, y - 4, 8, 3, rgb(139, 94, 115));
        art.rect(x + 14, y - 7, 7, 3, rgb(197, 178, 128));
    } else {
        art.rect(x + 15, y - 7, 6, 6, rgb(213, 160, 81));
        art.rect(x + 16, y - 6, 4, 2, rgb(239, 215, 152));
    }
    // Native status lives on the nameplate; selection has a separate cool border.
    art.rect(x, floor + 24, 85, 16, rgb(30, 38, 49));
    if selected {
        art.rect(x, floor + 24, 85, 1, rgb(160, 207, 200));
        art.rect(x, floor + 24, 2, 16, rgb(160, 207, 200));
    }
}

fn meeting_front(art: &mut Raster, x: i32, floor: i32, selected: bool) {
    let y = floor - 26;
    art.rect(x - 2, y + 4, 96, 9, rgb(103, 79, 65));
    art.rect(x - 2, y, 96, 5, rgb(186, 145, 98));
    art.rect(x - 2, y, 96, 1, rgb(234, 199, 137));
    art.rect(x + 11, y + 13, 5, 16, rgb(64, 62, 60));
    art.rect(x + 29, y - 2, 26, 2, rgb(218, 206, 174));
    art.rect(x + 31, y - 4, 23, 2, rgb(246, 232, 191));
    art.rect(x + 67, y - 6, 5, 6, rgb(119, 163, 163));
    art.rect(x + 67, y - 6, 5, 1, rgb(223, 228, 205));
    art.rect(x, floor + 24, 85, 16, rgb(30, 38, 49));
    if selected {
        art.rect(x, floor + 24, 85, 1, rgb(160, 207, 200));
        art.rect(x, floor + 24, 2, 16, rgb(160, 207, 200));
    }
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
        assert_eq!(layout.scale, 3);
        assert_eq!(layout.seats.len(), 2);
        assert_eq!(layout.page_count, 2);
        for seat in layout.seats {
            assert!(seat.bounds.x + seat.bounds.width <= canvas.width());
            assert!(seat.bounds.y + seat.bounds.height <= canvas.height());
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
        let office = office(3);
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
                selected_worker: Some(&office.workers[2].id),
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
            vec![&office.workers[0].id, &office.workers[2].id]
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
}
