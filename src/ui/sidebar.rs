use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::List;

use crate::app::{App, Focus};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let pane = app.theme.pane(app.focus == Focus::Sidebar);
    let list = List::new(app.playlists.iter().map(|p| p.name.as_str()))
        .style(pane.text())
        .block(pane.block("Playlists"))
        .highlight_style(pane.selected());

    frame.render_stateful_widget(list, area, &mut app.sidebar);
}
