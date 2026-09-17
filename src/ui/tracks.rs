use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Cell, Row, Table};

use crate::app::{App, Focus};
use crate::ui::fmt_time;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let pane = app.theme.pane(app.focus == Focus::Main);

    let title = match app.showing_playlist() {
        Some(playlist) => playlist.name.clone(),
        None => String::from("Tracks"),
    };

    let playing = app.playing_uri();
    let rows = app.tracks.iter().enumerate().map(|(i, track)| {
        let row = Row::new([
            Cell::from((i + 1).to_string()),
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

    let header = Row::new(["#", "TITLE", "ARTIST", "ALBUM", "TIME"]).style(app.theme.dim());

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
        let cell = terminal.backend().buffer().cell((1, FIRST_ROW + 1)).unwrap();
        assert_eq!(cell.bg, theme.accent);
        assert_eq!(cell.fg, theme.background);
    }
}
