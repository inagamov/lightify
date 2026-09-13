use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::{Gauge, Paragraph},
};

use crate::app::App;

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
    let title = format!("{icon} {}  {}", track.name, track.artists.join(", "));
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
        .gauge_style(Style::new().green());
    frame.render_widget(gauge, gauge_area);

    let percent = u32::from(playback.volume) * 100 / u32::from(u16::MAX);
    frame.render_widget(Paragraph::new(format!("vol {percent}%")), volume_area);
}

fn fmt_time(ms: u32) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_time_pads_seconds() {
        assert_eq!(fmt_time(0), "0:00");
        assert_eq!(fmt_time(65_000), "1:05");
        assert_eq!(fmt_time(3_600_000), "60:00");
    }
}
