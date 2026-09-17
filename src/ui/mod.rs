use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Block;

use crate::app::App;

pub mod nowplaying;
pub mod sidebar;
pub mod status;
pub mod tracks;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [body, nowplaying_area, status_area] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    frame.render_widget(Block::new().style(app.theme.base()), frame.area());

    let [sidebar_area, tracks_area] =
        Layout::horizontal([Constraint::Length(24), Constraint::Min(20)]).areas(body);

    sidebar::draw(frame, app, sidebar_area);
    tracks::draw(frame, app, tracks_area);
    nowplaying::draw(frame, app, nowplaying_area);
    status::draw(frame, app, status_area);
}

pub(crate) fn fmt_time(ms: u32) -> String {
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
