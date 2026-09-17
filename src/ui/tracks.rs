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
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;

    use super::*;
    use crate::app::NowPlaying;
    use crate::spotify::model::Track;
    use crate::theme::Theme;

    const FIRST_ROW: u16 = 2;

    fn track(uri: &str) -> Track {
        Track {
            uri: uri.to_string(),
            name: uri.to_string(),
            duration_ms: 1000,
            artists: vec![],
            album: String::new(),
        }
    }

    #[test]
    fn playing_row_uses_accent_unless_selected() {
        let theme = Theme {
            accent: Color::Rgb(180, 90, 0),
            text: Color::Rgb(9, 9, 9),
            ..Theme::default()
        };
        let mut app = App::new().with_theme(theme.clone());
        app.focus = Focus::Main;
        app.tracks = vec![track("a"), track("b")];
        app.track_list.select(Some(0));
        app.playback.track = Some(NowPlaying {
            uri: "b".to_string(),
            name: "b".to_string(),
            artists: vec![],
            album: String::new(),
            duration_ms: 1000,
        });

        let mut terminal = Terminal::new(TestBackend::new(20, 5)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer.cell((1, FIRST_ROW)).unwrap().bg, theme.accent);
        assert_eq!(buffer.cell((1, FIRST_ROW + 1)).unwrap().fg, theme.accent);

        app.track_list.select(Some(1));
        terminal
            .draw(|frame| draw(frame, &mut app, frame.area()))
            .unwrap();
        let cell = terminal
            .backend()
            .buffer()
            .cell((1, FIRST_ROW + 1))
            .unwrap();
        assert_eq!(cell.bg, theme.accent);
        assert_eq!(cell.fg, theme.background);
    }

    fn number_column(buffer: &Buffer, row: u16) -> String {
        (1..=4)
            .map(|x| buffer.cell((x, row)).unwrap().symbol())
            .collect::<String>()
            .trim()
            .to_string()
    }

    #[test]
    fn number_column_is_relative_to_the_selection() {
        let mut app = App::new();
        app.focus = Focus::Main;
        app.tracks = (0..5).map(|i| track(&i.to_string())).collect();
        app.track_list.select(Some(2));

        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();

        let numbers: Vec<String> = (0..5)
            .map(|i| number_column(buffer, FIRST_ROW + i))
            .collect();

        assert_eq!(numbers, ["2", "1", "3", "1", "2"]);
    }

    #[test]
    fn number_column_is_absolute_when_the_pane_is_not_focused() {
        let mut app = App::new();
        app.focus = Focus::Sidebar;
        app.tracks = (0..5).map(|i| track(&i.to_string())).collect();
        app.track_list.select(Some(2));

        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();

        let numbers: Vec<String> = (0..5)
            .map(|i| number_column(buffer, FIRST_ROW + i))
            .collect();

        assert_eq!(numbers, ["1", "2", "3", "4", "5"]);
    }

    #[test]
    fn relative_numbers_are_dimmed_but_the_cursor_number_is_not() {
        let theme = Theme {
            text: Color::Rgb(200, 200, 200),
            background: Color::Rgb(0, 0, 0),
            ..Theme::default()
        };
        let mut app = App::new().with_theme(theme.clone());
        app.focus = Focus::Main;
        app.tracks = (0..3).map(|i| track(&i.to_string())).collect();
        app.track_list.select(Some(1));

        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();

        const DIGIT: u16 = 4;
        let dim = theme.dim().fg.unwrap();

        assert_eq!(buffer.cell((DIGIT, FIRST_ROW)).unwrap().fg, dim);
        assert_eq!(buffer.cell((DIGIT, FIRST_ROW + 2)).unwrap().fg, dim);
        assert_eq!(
            buffer.cell((DIGIT, FIRST_ROW + 1)).unwrap().fg,
            theme.background
        );
    }
}
