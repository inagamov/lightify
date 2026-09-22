use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::Text;
use ratatui::widgets::{Cell, Row, Table};

use crate::app::{App, Focus};
use crate::ui::fmt_time;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Main;
    let pane = app.theme.pane(focused);

    let selected = if focused {
        app.track_list.selected()
    } else {
        None
    };

    let title = match app.showing_playlist() {
        Some(playlist) => playlist.name.clone(),
        None => String::from("Tracks"),
    };

    let playing = app.playing_uri();
    let rows = app.tracks.iter().enumerate().map(|(i, track)| {
        let number = match selected {
            Some(cursor) if cursor != i => {
                Cell::from(Text::from(cursor.abs_diff(i).to_string()).right_aligned())
                    .style(app.theme.dim())
            }
            _ => Cell::from(Text::from((i + 1).to_string()).right_aligned()),
        };
        let row = Row::new([
            number,
            Cell::from(track.name.clone()),
            Cell::from(track.artist_names()),
            Cell::from(track.album.clone()),
            Cell::from(fmt_time(track.duration_ms)),
        ]);

        if playing == Some(track.uri.as_str()) {
            row.style(pane.playing())
        } else {
            row
        }
    });

    let widths = [
        Constraint::Length(4),      // #
        Constraint::Min(20),        // TITLE
        Constraint::Percentage(25), // ARTIST
        Constraint::Percentage(25), // ALBUM
        Constraint::Length(5),      // TIME
    ];

    let header = Row::new([
        Cell::from(Text::from("#").right_aligned()),
        Cell::from("TITLE"),
        Cell::from("ARTIST"),
        Cell::from("ALBUM"),
        Cell::from("TIME"),
    ])
    .style(app.theme.dim());

    let table = Table::new(rows, widths)
        .header(header)
        .style(pane.text())
        .block(pane.block(title))
        .row_highlight_style(pane.selected());

    frame.render_stateful_widget(table, area, &mut app.track_list);
}

#[cfg(test)]
mod tests;
