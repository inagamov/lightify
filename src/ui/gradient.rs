use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::theme::Theme;

const STEPS: u16 = 10;
const GLOW_FADE: f32 = 1.4; // ~70%

fn lerp(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (from, to) else {
        return if t < 0.5 { from } else { to };
    };
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    Color::Rgb(mix(r1, r2), mix(g1, g2), mix(b1, b2))
}

fn sweep(area: Rect, x: u16, y: u16) -> f32 {
    let span = f32::from(area.width) + 2.0 * f32::from(area.height);
    (f32::from(x - area.x) + 2.0 * f32::from(y - area.y)) / span
}

fn is_border(symbol: &str) -> bool {
    matches!(symbol, "─" | "│" | "╭" | "╮" | "╰" | "╯")
}

pub fn pane(buf: &mut Buffer, theme: &Theme, area: Rect, focused: bool) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    border(buf, theme, area, focused);

    for x in area.left()..area.right() {
        let Some(cell) = buf.cell_mut((x, area.top())) else {
            continue;
        };
        if is_border(cell.symbol()) {
            continue;
        }
        if focused {
            let t = f32::from(x - area.x) * 3.0 / f32::from(area.width);
            cell.set_fg(lerp(theme.accent, theme.accent_deep, t));
            cell.modifier.insert(Modifier::BOLD);
        } else {
            cell.set_fg(theme.muted);
        }
    }
}

pub fn border(buf: &mut Buffer, theme: &Theme, area: Rect, lit: bool) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let Some(cell) = buf.cell_mut((x, y)) else {
                continue;
            };
            if !is_border(cell.symbol()) {
                continue;
            }
            let color = if lit {
                lerp(theme.accent, theme.accent_deep, sweep(area, x, y))
            } else {
                theme.idle
            };
            cell.set_fg(color);
        }
    }
}

pub fn selected_row(
    buf: &mut Buffer,
    theme: &Theme,
    rows: Rect,
    index: usize,
    offset: usize,
    focused: bool,
) {
    let Some(y) = index
        .checked_sub(offset)
        .and_then(|visible| u16::try_from(visible).ok())
        .and_then(|visible| rows.y.checked_add(visible))
        .filter(|&y| y < rows.bottom())
    else {
        return;
    };

    for i in 0..rows.width {
        let Some(cell) = buf.cell_mut((rows.x + i, y)) else {
            continue;
        };
        if !focused {
            cell.set_bg(theme.shade);
            continue;
        }
        let t = f32::from(i) / f32::from(rows.width.max(1));
        cell.set_bg(lerp(theme.glow, theme.shade, t * GLOW_FADE));
        if i == 0 {
            cell.set_symbol("▌").set_fg(theme.accent);
        }
    }
}

pub fn stepped_bar(buf: &mut Buffer, theme: &Theme, area: Rect, ratio: f64) {
    let width = area.width;
    if width == 0 {
        return;
    }
    let filled = (ratio.clamp(0.0, 1.0) * f64::from(width)).round() as u16;
    for i in 0..width {
        let Some(cell) = buf.cell_mut((area.x + i, area.y)) else {
            continue;
        };
        let step = i * STEPS / width;
        let t = f32::from(step) / f32::from(STEPS - 1);
        let color = if i < filled {
            lerp(theme.accent, theme.accent_deep, t)
        } else {
            lerp(theme.shade, theme.idle, t)
        };
        cell.set_symbol("■").set_fg(color);
    }
}

pub fn meter(theme: &Theme, ratio: f64, width: u16) -> Vec<Span<'static>> {
    let filled = (ratio.clamp(0.0, 1.0) * f64::from(width)).round() as u16;
    (0..width)
        .map(|i| {
            let t = f32::from(i) / f32::from(width.saturating_sub(1).max(1));
            let color = if i < filled {
                lerp(theme.accent, theme.accent_deep, t)
            } else {
                theme.idle
            };
            Span::styled("■", Style::new().fg(color))
        })
        .collect()
}
