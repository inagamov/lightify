use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, List};

use crate::app::{App, Focus};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;

    let title = match app.showing_playlist() {
        Some(playlist) => playlist.name.clone(),
        None => String::from("Tracks"),
    };

    let rows = app
        .tracks
        .iter()
        .map(|track| format!("{} {} {}", track.name, track.artist_names(), track.album));

    let list = List::new(rows)
        .block(
            Block::bordered()
                .title(title)
                .title_style(theme.title())
                .border_style(theme.border(app.focus == Focus::Sidebar)),
        )
        .highlight_style(theme.selected());

    frame.render_stateful_widget(list, area, &mut app.track_list);
}
