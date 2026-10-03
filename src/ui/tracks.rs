use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Text;
use ratatui::widgets::{Cell, HighlightSpacing, Row, Table};

use crate::app::{App, Focus};
use crate::spotify::model::Source;
use crate::ui::{fmt_time, gradient};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Main;
    let pane = app.theme.pane(focused);

    let selected = if focused {
        app.track_list.selected()
    } else {
        None
    };

    let title = match &app.tracks_for {
        Some(Source::Search(query)) => format!("Results: {query}"),
        _ => match app.showing_playlist() {
            Some(playlist) => playlist.name.clone(),
            None => String::from("Tracks"),
        },
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
        } else if selected == Some(i) {
            row.style(pane.highlight())
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

    let block = pane.block(title);
    let [_header_area, rows_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(block.inner(area));

    let table = Table::new(rows, widths)
        .header(header)
        .style(pane.text())
        .block(block)
        .highlight_symbol(" ")
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(table, area, &mut app.track_list);
    gradient::border(frame.buffer_mut(), &app.theme, area, focused);

    if let Some(index) = app.track_list.selected() {
        let offset = app.track_list.offset();
        gradient::selected_row(
            frame.buffer_mut(),
            &app.theme,
            rows_area,
            index,
            offset,
            focused,
        );
    }
}
