use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, List};

use crate::app::{App, Focus};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_style = if app.focus == Focus::Sidebar {
        Style::new().yellow()
    } else {
        Style::new()
    };

    let list = List::new(app.playlists.iter().map(|p| p.name.as_str()))
        .block(
            Block::bordered()
                .title("Playlists")
                .border_style(border_style),
        )
        .highlight_style(Style::new().reversed());

    frame.render_stateful_widget(list, area, &mut app.sidebar);
}
