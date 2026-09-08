//! Appearance and input preferences, with compatibility settings kept explicit.
use super::{
    office::Projection, paint_opaque, UiTheme, ACCENT, INK, MUTED, PANEL, PANEL_HIGHLIGHT,
};
use crate::{
    canvas::{ColorDepth, PixelEncoding},
    interaction::{Action, HitRegion},
};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Paragraph, Widget},
    Frame,
};

pub(crate) struct SettingsDrawContext {
    pub(crate) projection: Projection,
    pub(crate) theme: UiTheme,
    pub(crate) color_depth: ColorDepth,
    pub(crate) color_locked: bool,
    pub(crate) encoding: PixelEncoding,
    pub(crate) encoding_locked: bool,
    pub(crate) motion: bool,
    pub(crate) mouse: bool,
    pub(crate) name_plates: bool,
    pub(crate) cursor: usize,
    pub(crate) advanced: bool,
    pub(crate) room_palette: usize,
}

pub(crate) fn draw(frame: &mut Frame, c: SettingsDrawContext) -> Vec<HitRegion> {
    let full = frame.area();
    let width = full.width.min(68);
    let height = full.height.saturating_sub(3).min(19);
    let area = Rect::new(full.x + (full.width - width) / 2, full.y + 2, width, height);
    paint_opaque(frame, area, Style::default().fg(INK).bg(PANEL));
    if width < 16 || height < 5 {
        return Vec::new();
    }
    let inner = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
    crate::components::heading(
        frame,
        Rect::new(inner.x, inner.y, inner.width, 1),
        if c.advanced {
            "Advanced · compatibility"
        } else {
            "Settings · appearance & input"
        },
    );
    let rows: Vec<(&str, String)> = if c.advanced {
        vec![
            ("Camera", c.projection.label().into()),
            (
                "Text graphics",
                format!(
                    "{}{}",
                    c.encoding.label(),
                    if c.encoding_locked {
                        " · set externally"
                    } else {
                        ""
                    }
                ),
            ),
            ("Legacy palette", format!("{} / 4", c.room_palette + 1)),
            ("Back to Settings", "Enter".into()),
        ]
    } else {
        vec![
            (
                "Theme",
                if c.theme == UiTheme::Light {
                    "Light"
                } else {
                    "Dark"
                }
                .into(),
            ),
            (
                "Color",
                format!(
                    "{}{}",
                    color_depth_label(c.color_depth),
                    if c.color_locked {
                        " · set externally"
                    } else {
                        ""
                    }
                ),
            ),
            ("Motion", if c.motion { "Full" } else { "Reduced" }.into()),
            (
                "Nameplates",
                if c.name_plates {
                    "All workers"
                } else {
                    "Selection & requests"
                }
                .into(),
            ),
            (
                "Mouse",
                if c.mouse {
                    "Click to inspect"
                } else {
                    "Terminal selection"
                }
                .into(),
            ),
            ("Advanced", "Cameras & text graphics".into()),
        ]
    };
    let available = inner.height.saturating_sub(3).max(1) as usize;
    let first = c.cursor.saturating_sub(available - 1);
    let mut hits = Vec::new();
    for (i, (label, value)) in rows.iter().enumerate().skip(first).take(available) {
        let row = Rect::new(inner.x, inner.y + 1 + (i - first) as u16, inner.width, 1);
        let selected = i == c.cursor;
        let style = Style::default()
            .fg(if selected { ACCENT } else { INK })
            .bg(if selected { PANEL_HIGHLIGHT } else { PANEL });
        let label_width = (inner.width / 2).min(20) as usize;
        let label = super::short_path(label, label_width.saturating_sub(2));
        paint_opaque(frame, row, style);
        Paragraph::new(format!(
            "{} {:label_width$}{}",
            if selected { ">" } else { " " },
            label,
            value
        ))
        .style(if selected {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        })
        .render(row, frame.buffer_mut());
        hits.push(HitRegion::new(row, Action::Setting(i)));
    }
    let message = if c.advanced {
        "Older cameras: limited artwork and controls."
    } else {
        "Changes apply now. Decor has Apply / Cancel."
    };
    Paragraph::new(super::wrap_text(message, inner.width).join("\n"))
        .style(Style::default().fg(MUTED))
        .render(
            Rect::new(inner.x, inner.bottom().saturating_sub(2), inner.width, 2),
            frame.buffer_mut(),
        );
    hits
}

pub(crate) fn color_depth_label(depth: ColorDepth) -> &'static str {
    match depth {
        ColorDepth::TrueColor => "truecolor",
        ColorDepth::Palette256 => "256",
        ColorDepth::None => "none",
    }
}
