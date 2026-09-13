use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, List};

use crate::app::{App, Focus};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_style = if app.focus == Focus::Main {
        Style::new().yellow()
    } else {
        Style::new()
    };

    let title = match app.showing_playlist() {
        Some(playlist) => playlist.name.clone(),
        None => String::from("Tracks"),
    };

    let rows = app
        .tracks
        .iter()
        .map(|track| format!("{} {} {}", track.name, track.artist_names(), track.album));

    let list = List::new(rows)
        .block(Block::bordered().title(title).border_style(border_style))
        .highlight_style(Style::new().reversed());

    frame.render_stateful_widget(list, area, &mut app.track_list);
}
