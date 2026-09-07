//! Renderer customization and its live preview.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use theywork_core::{Millis, Office, Worker};

use crate::canvas::{Canvas, ColorDepth, PixelEncoding};
use crate::sprite::{SpriteSet, WorkerLook};

use super::office::Projection;
use super::{
    draw_panel, fill_office_background, has_area, inset, paint_opaque, render_worker_with_look,
    short_path, PixelRect, UiTheme, ACCENT, INK, MUTED, PANEL, PANEL_HIGHLIGHT,
};

pub(crate) struct SettingsDrawContext<'a> {
    pub(crate) projection: Projection,
    pub(crate) theme: UiTheme,
    pub(crate) color_depth: ColorDepth,
    pub(crate) color_locked: bool,
    pub(crate) encoding: PixelEncoding,
    pub(crate) encoding_locked: bool,
    pub(crate) motion: bool,
    pub(crate) name_plates: bool,
    pub(crate) cursor: usize,
    pub(crate) worker: Option<(&'a Worker, WorkerLook)>,
    pub(crate) office: Option<&'a Office>,
    pub(crate) now: Millis,
    pub(crate) canvas: &'a mut Canvas,
    pub(crate) sprites: &'a SpriteSet,
}

pub(crate) fn draw(frame: &mut Frame, context: SettingsDrawContext<'_>) {
    let SettingsDrawContext {
        projection,
        theme,
        color_depth,
        color_locked,
        encoding,
        encoding_locked,
        motion,
        name_plates,
        cursor,
        worker,
        office,
        now,
        canvas,
        sprites,
    } = context;
    let area = frame.area();
    if area.width < 20 || area.height < 8 {
        super::draw_tiny(frame, "settings need a little more terminal space");
        return;
    }

    let popup_width = area.width.saturating_sub(2).clamp(20, 76);
    let popup_height = area.height.saturating_sub(2).clamp(8, 24);
    let popup = Rect::new(
        area.x
            .saturating_add(area.width.saturating_sub(popup_width) / 2),
        area.y
            .saturating_add(area.height.saturating_sub(popup_height) / 2),
        popup_width,
        popup_height,
    );
    let popup_style = Style::default().fg(INK).bg(PANEL);
    paint_opaque(frame, popup, popup_style);
    Block::default()
        .title(" CUSTOMIZE OFFICE ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT))
        .style(popup_style)
        .render(popup, frame.buffer_mut());

    let inner = inset(popup, 1);
    if !has_area(inner) {
        return;
    }
    let left_width = if inner.width >= 58 { 30 } else { inner.width };
    let left = Rect::new(inner.x, inner.y, left_width, inner.height);
    let right = Rect::new(
        inner.x.saturating_add(left_width),
        inner.y,
        inner.width.saturating_sub(left_width),
        inner.height,
    );
    let content_height = left.height.saturating_sub(3);
    let options_inner = Rect::new(left.x, left.y, left.width, content_height);
    let hint = if left.width < 28 {
        "↑↓ select · ←→ edit\ns close · c sources"
    } else {
        "↑↓ select · ←→ / Enter edit\ns / Esc close · c sources"
    };
    Paragraph::new(hint)
        .style(Style::default().fg(MUTED).bg(PANEL))
        .render(
            Rect::new(left.x, left.y + content_height + 1, left.width, 2),
            frame.buffer_mut(),
        );
    let preview_inner = draw_panel(frame, right, "LIVE PREVIEW", false);
    let colour = if color_locked {
        format!("{} (env)", color_depth_label(color_depth))
    } else {
        color_depth_label(color_depth).to_string()
    };
    let pixels = if encoding_locked {
        format!("{} (env)", encoding.label())
    } else {
        encoding.label().to_string()
    };
    let options = [
        ("camera", projection.label()),
        (
            "theme",
            if theme == UiTheme::Light {
                "paper"
            } else {
                "noir"
            },
        ),
        ("colour", colour.as_str()),
        ("motion", if motion { "on" } else { "off" }),
        ("names", if name_plates { "on" } else { "off" }),
        ("pixels", pixels.as_str()),
        (
            "room",
            office.map_or("no project", |office| sprites.office_palette_label(office)),
        ),
    ];
    if has_area(options_inner) {
        let visible = usize::from(options_inner.height);
        let first = cursor.saturating_sub(visible.saturating_sub(1));
        for (index, (label, value)) in options.iter().enumerate().skip(first) {
            let row = options_inner.y.saturating_add((index - first) as u16);
            if row >= options_inner.y.saturating_add(options_inner.height) {
                break;
            }
            let selected = index == cursor;
            let style = if selected {
                Style::default()
                    .fg(INK)
                    .bg(PANEL_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(MUTED).bg(PANEL)
            };
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{} {:<7}", if selected { ">" } else { " " }, label),
                    style,
                ),
                Span::styled((*value).to_string(), style.fg(ACCENT)),
            ]))
            .style(style)
            .render(
                Rect::new(options_inner.x, row, options_inner.width, 1),
                frame.buffer_mut(),
            );
        }
    }
    if has_area(preview_inner) {
        let label_height = preview_inner.height.min(2);
        let label_area = Rect::new(
            preview_inner.x,
            preview_inner.y,
            preview_inner.width,
            label_height,
        );
        let worker_label = worker.map_or_else(
            || "no worker selected".to_string(),
            |(worker, _)| short_path(&worker.name, 24),
        );
        Paragraph::new(format!("{}  •  {}", projection.label(), worker_label))
            .style(Style::default().fg(MUTED).bg(PANEL))
            .render(label_area, frame.buffer_mut());
        let preview = Rect::new(
            preview_inner.x,
            preview_inner.y.saturating_add(label_height),
            preview_inner.width,
            preview_inner.height.saturating_sub(label_height),
        );
        if has_area(preview) {
            canvas.resize_for_cells(preview.width as usize, preview.height as usize);
            let floor_start = fill_office_background(canvas, sprites);
            if let Some((worker, look)) = worker {
                let full = sprites.worker_frame(worker, look, now);
                let available_width = canvas
                    .scale_image_sprite_width(full.width())
                    .min(canvas.width().saturating_sub(2))
                    .max(1);
                let available_height = canvas
                    .scale_image_sprite_height(full.height())
                    .min(floor_start.saturating_sub(1).max(1))
                    .max(1);
                let pixel_width = super::sprite_pixel_width(canvas);
                let sprite = sprites.worker_frame_fitting(
                    worker,
                    look,
                    now,
                    available_width / pixel_width,
                    available_height,
                );
                let scale = (available_width / sprite.width() / pixel_width)
                    .min(available_height / sprite.height())
                    .max(1);
                let width = (sprite.width() * scale * pixel_width).min(available_width);
                let height = (sprite.height() * scale).min(available_height);
                let worker_x = canvas.width().saturating_sub(width) / 2;
                let worker_y = floor_start.saturating_sub(height);
                render_worker_with_look(
                    canvas,
                    sprites,
                    worker,
                    &look,
                    now,
                    PixelRect {
                        x: worker_x,
                        y: worker_y,
                        width,
                        height,
                    },
                );
            }
            let desk_width = canvas
                .scale_image_sprite_width(sprites.desk.width())
                .min(canvas.width())
                .max(1);
            let desk_height = canvas
                .scale_image_sprite_height(sprites.desk.height())
                .min(canvas.height())
                .max(1);
            canvas.blit_scaled(
                &sprites.desk,
                canvas.width().saturating_sub(desk_width) / 2,
                floor_start.saturating_sub(desk_height / 2),
                desk_width,
                desk_height,
            );
            if let Some(office) = office {
                super::office::apply_room_palette(canvas, sprites.office_palette_index(office));
            }
            canvas.render(frame.buffer_mut(), preview);
        }
    }
}

pub(crate) fn color_depth_label(depth: ColorDepth) -> &'static str {
    match depth {
        ColorDepth::TrueColor => "truecolor",
        ColorDepth::Palette256 => "256",
        ColorDepth::None => "none",
    }
}
