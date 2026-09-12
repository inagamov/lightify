use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::app::App;

mod sidebar;
mod status;
mod tracks;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [body, status_area] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());

    let [sidebar_area, tracks_area] =
        Layout::horizontal([Constraint::Length(24), Constraint::Min(20)]).areas(body);

    sidebar::draw(frame, app, sidebar_area);
    tracks::draw(frame, app, tracks_area);
    status::draw(frame, app, status_area);
}
