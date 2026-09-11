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
        ButtonKind::Primary => views::selection_style(),
        ButtonKind::Secondary => Style::default().fg(views::SECONDARY).bg(views::PANEL),
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
    fn display_color(c: Color, light: bool, palette256: bool) -> Color {
        let c = if light { views::light_color(c) } else { c };
        if !palette256 {
            return c;
        }
        let mut buffer = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 1, 1));
        buffer[(0, 0)].set_fg(c);
        crate::canvas::Canvas::quantize_colors(&mut buffer);
        buffer[(0, 0)].fg
    }
    fn luminance(c: Color) -> f64 {
        let (r, g, b) = match c {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::Indexed(index @ 16..=231) => {
                let cube = [0, 95, 135, 175, 215, 255];
                let n = usize::from(index - 16);
                (cube[n / 36], cube[n / 6 % 6], cube[n % 6])
            }
            Color::Indexed(index @ 232..=255) => {
                let gray = 8 + (index - 232) * 10;
                (gray, gray, gray)
            }
            _ => panic!("built-in palette must use RGB or fixed xterm colors"),
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
        for (light, palette256) in [(false, false), (true, false), (false, true), (true, true)] {
            let remap = |c| display_color(c, light, palette256);
            for bg in [
                views::BACKGROUND,
                views::PANEL,
                views::PANEL_HIGHLIGHT,
                views::ATTENTION_PANEL,
            ] {
                for fg in [
                    views::INK,
                    views::MUTED,
                    views::ACCENT,
                    views::SECONDARY,
                    views::WARNING,
                    views::GOOD,
                    views::HOT,
                ] {
                    let a = luminance(remap(bg));
                    let b = luminance(remap(fg));
                    let ratio = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                    assert!(
                        ratio >= 4.5,
                        "light={light}, palette256={palette256}, {fg:?} on {bg:?}: {ratio}"
                    );
                }
            }
        }
    }

    #[test]
    fn filled_controls_have_readable_text_and_distinct_selection_boundaries() {
        for (light, palette256) in [(false, false), (true, false), (false, true), (true, true)] {
            let remap = |c| display_color(c, light, palette256);
            let selection = views::selection_style();
            let fill = luminance(remap(selection.bg.unwrap()));
            let ink = luminance(remap(selection.fg.unwrap()));
            let text_ratio = (ink.max(fill) + 0.05) / (ink.min(fill) + 0.05);
            assert!(
                text_ratio >= 4.5,
                "light={light}, palette256={palette256}: {text_ratio}"
            );
            for background in [views::BACKGROUND, views::PANEL, views::PANEL_HIGHLIGHT] {
                let surface = luminance(remap(background));
                let ratio = (fill.max(surface) + 0.05) / (fill.min(surface) + 0.05);
                assert!(
                    ratio >= 3.0,
                    "light={light}, palette256={palette256}, {background:?}: {ratio}"
                );
            }
        }
    }

    #[test]
    fn filled_controls_keep_labels_targets_and_emphasis_without_color() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut terminal = Terminal::new(TestBackend::new(24, 3)).unwrap();
        let mut hit = None;
        terminal
            .draw(|frame| {
                hit = button(
                    frame,
                    Rect::new(2, 1, 20, 1),
                    "Now",
                    Action::WorkTab(crate::work_brief::WorkTab::Now),
                    ButtonKind::Primary,
                );
                theme_buffer(frame.buffer_mut(), false, true);
            })
            .unwrap();
        let hit = hit.unwrap();
        assert_eq!(hit.area, Rect::new(2, 1, 7, 1));
        assert_eq!(hit.action, Action::WorkTab(crate::work_brief::WorkTab::Now));
        let buffer = terminal.backend().buffer();
        let label: String = (2..9).map(|x| buffer[(x, 1)].symbol()).collect();
        assert_eq!(label, "[ Now ]");
        for x in 2..9 {
            let cell = &buffer[(x, 1)];
            assert_eq!(cell.fg, Color::Reset);
            assert_eq!(cell.bg, Color::Reset);
            assert!(cell.modifier.contains(Modifier::BOLD));
        }
    }
}
