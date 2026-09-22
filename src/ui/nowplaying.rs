use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
};

use crate::app::App;
use crate::ui::fmt_time;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let [title_area, gauge_area, volume_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    let playback = &app.playback;
    let Some(track) = &playback.track else {
        frame.render_widget(Paragraph::new("nothing playing"), title_area);
        return;
    };

    let icon = if playback.is_playing() { "▶" } else { "⏸" };

    let title = Line::from(vec![
        Span::raw(format!("{icon} {}  ", track.name)),
        Span::styled(track.artists.join(", "), app.theme.dim()),
    ]);
    frame.render_widget(Paragraph::new(title), title_area);

    let position = playback.current_position_ms().min(track.duration_ms);
    let ratio = if track.duration_ms == 0 {
        0.0
    } else {
        f64::from(position) / f64::from(track.duration_ms)
    };

    let label = format!("{} / {}", fmt_time(position), fmt_time(track.duration_ms));
    let gauge = Gauge::default()
        .ratio(ratio)
        .label(label)
        .style(app.theme.dim())
        .gauge_style(app.theme.accent);
    frame.render_widget(gauge, gauge_area);

    let max = u32::from(u16::MAX);
    let percent = (u32::from(playback.volume) * 100 + max / 2) / max;
    frame.render_widget(Paragraph::new(format!("vol {percent}%")), volume_area);
}
