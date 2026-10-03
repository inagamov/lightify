use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::ui::{fmt_time, gradient};

const VOLUME_CELLS: u16 = 10;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let playback = &app.playback;
    let theme = &app.theme;

    let max = u32::from(u16::MAX);
    let percent = (u32::from(playback.volume) * 100 + max / 2) / max;
    let mut volume = vec![Span::styled(" vol ", theme.dim())];
    volume.extend(gradient::meter(
        theme,
        f64::from(percent) / 100.0,
        VOLUME_CELLS,
    ));
    volume.push(Span::styled(format!(" {percent:>3}% "), theme.dim()));

    let title = match &playback.track {
        Some(track) => {
            let icon = if playback.is_playing() { "▶" } else { "⏸" };
            Line::from(vec![
                Span::styled(format!("{icon} "), Style::new().fg(theme.accent)),
                Span::styled(
                    track.name.clone(),
                    Style::new().fg(theme.text).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" · "),
                Span::raw(track.artists.join(", ")),
            ])
        }
        None => Line::from("nothing playing"),
    };

    let block = theme
        .pane(false)
        .block(title)
        .title_style(theme.dim())
        .title_top(Line::from(volume).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    gradient::border(frame.buffer_mut(), theme, area, playback.is_playing());

    let (position, duration) = match &playback.track {
        Some(track) => (
            playback.current_position_ms().min(track.duration_ms),
            track.duration_ms,
        ),
        None => (0, 0),
    };
    let ratio = if duration == 0 {
        0.0
    } else {
        f64::from(position) / f64::from(duration)
    };

    let label = Line::from(format!(
        "  {} / {} ",
        fmt_time(position),
        fmt_time(duration)
    ));
    let label_width = u16::try_from(label.width()).unwrap_or(u16::MAX);
    let [bar_area, label_area] =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(label_width)]).areas(inner);
    let bar_area = Rect {
        x: bar_area.x + 1,
        width: bar_area.width.saturating_sub(1),
        ..bar_area
    };
    gradient::stepped_bar(frame.buffer_mut(), theme, bar_area, ratio);
    frame.render_widget(Paragraph::new(label).style(theme.dim()), label_area);
}
