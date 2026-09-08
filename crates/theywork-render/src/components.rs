//! Shared native-text controls. Their geometry is also their click target.
use crate::{
    interaction::{Action, HitRegion},
    views,
};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Paragraph, Widget},
    Frame,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Quiet,
}

/// The same semantic colors are used by host-owned native forms.
pub struct NativePalette {
    pub ink: ratatui::style::Color,
    pub muted: ratatui::style::Color,
    pub accent: ratatui::style::Color,
    pub background: ratatui::style::Color,
    pub warning: ratatui::style::Color,
}
pub fn palette() -> NativePalette {
    NativePalette {
        ink: views::INK,
        muted: views::MUTED,
        accent: views::ACCENT,
        background: views::PANEL,
        warning: views::WARNING,
    }
}
pub fn theme_buffer(buffer: &mut ratatui::buffer::Buffer, light: bool, monochrome: bool) {
    views::remap_buffer_theme(
        buffer,
        if light {
            views::UiTheme::Light
        } else {
            views::UiTheme::Dark
        },
    );
    if monochrome {
        for cell in &mut buffer.content {
            cell.set_fg(ratatui::style::Color::Reset);
            cell.set_bg(ratatui::style::Color::Reset);
        }
    }
}

pub fn button(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    action: Action,
    kind: ButtonKind,
) -> Option<HitRegion> {
    let area = area.intersection(frame.area());
    let label = views::safe_display(label);
    let width = ratatui::text::Line::from(label.as_str())
        .width()
        .saturating_add(4)
        .min(u16::MAX as usize) as u16;
    if area.height == 0 || area.width < width {
        return None;
    }
    let rect = Rect::new(area.x, area.y, width, 1);
    let style = match kind {
        ButtonKind::Primary => Style::default()
            .fg(views::ACCENT)
            .bg(views::PANEL_HIGHLIGHT)
            .add_modifier(Modifier::BOLD),
        ButtonKind::Secondary => Style::default().fg(views::INK).bg(views::PANEL),
        ButtonKind::Quiet => Style::default().fg(views::MUTED).bg(views::PANEL),
    };
    views::paint_opaque(frame, rect, style);
    Paragraph::new(format!("[ {label} ]"))
        .style(style)
        .render(rect, frame.buffer_mut());
    Some(HitRegion::new(rect, action))
}

pub fn heading(frame: &mut Frame, area: Rect, title: &str) {
    Paragraph::new(views::safe_display(title))
        .style(Style::default().fg(views::INK).add_modifier(Modifier::BOLD))
        .render(area.intersection(frame.area()), frame.buffer_mut());
}

pub fn notice(frame: &mut Frame, area: Rect, text: &str, warning: bool) {
    let style = Style::default()
        .fg(if warning {
            views::WARNING
        } else {
            views::MUTED
        })
        .bg(views::PANEL);
    views::paint_opaque(frame, area, style);
    Paragraph::new(views::safe_multiline(text))
        .style(style)
        .render(area, frame.buffer_mut());
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;
    fn luminance(c: Color) -> f64 {
        let Color::Rgb(r, g, b) = c else {
            panic!("built-in palette must be RGB")
        };
        let linear = |x: u8| {
            let x = x as f64 / 255.;
            if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    }
    #[test]
    fn built_in_text_and_control_outlines_meet_contrast_targets() {
        for light in [false, true] {
            let remap = |c| if light { views::light_color(c) } else { c };
            for bg in [views::BACKGROUND, views::PANEL, views::PANEL_HIGHLIGHT] {
                for fg in [
                    views::INK,
                    views::MUTED,
                    views::ACCENT,
                    views::WARNING,
                    views::GOOD,
                    views::HOT,
                ] {
                    let a = luminance(remap(bg));
                    let b = luminance(remap(fg));
                    let ratio = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                    assert!(ratio >= 4.5, "light={light}, {fg:?} on {bg:?}: {ratio}");
                }
            }
        }
    }
}
