use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{HighlightSpacing, List};

use crate::app::{App, Focus};
use crate::ui::gradient;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Sidebar;
    let pane = app.theme.pane(focused);
    let block = pane.block("Playlists");
    let rows = block.inner(area);

    let list = List::new(app.playlists.iter().map(|p| p.name.as_str()))
        .style(pane.text())
        .block(block)
        .highlight_symbol(" ")
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(list, area, &mut app.sidebar);
    gradient::border(frame.buffer_mut(), &app.theme, area, focused);

    if let Some(index) = app.sidebar.selected() {
        let offset = app.sidebar.offset();
        gradient::selected_row(frame.buffer_mut(), &app.theme, rows, index, offset, focused);
    }
}
