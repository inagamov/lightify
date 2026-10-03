use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Block;

use crate::app::App;

pub mod gradient;
pub mod nowplaying;
pub mod search;
pub mod sidebar;
pub mod status;
pub mod tracks;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [search_area, body, nowplaying_area, status_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    frame.render_widget(Block::new().style(app.theme.base()), frame.area());

    let [sidebar_area, tracks_area] =
        Layout::horizontal([Constraint::Length(24), Constraint::Min(20)]).areas(body);

    search::draw(frame, app, search_area);
    sidebar::draw(frame, app, sidebar_area);
    tracks::draw(frame, app, tracks_area);
    nowplaying::draw(frame, app, nowplaying_area);
    status::draw(frame, app, status_area);
}

pub(crate) fn fmt_time(ms: u32) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}
