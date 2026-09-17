use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, List, ListItem};

use crate::app::{App, Focus};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;

    let title = match app.showing_playlist() {
        Some(playlist) => playlist.name.clone(),
        None => String::from("Tracks"),
    };

    let playing = app.playing_uri();
    let rows = app.tracks.iter().map(|track| {
        let text = format!("{} {} {}", track.name, track.artist_names(), track.album);
        let item = ListItem::new(text);
        if playing == Some(track.uri.as_str()) {
            item.style(app.theme.playing())
        } else {
            item
        }
    });

    let list = List::new(rows)
        .block(
            Block::bordered()
                .title(title)
                .title_style(theme.title())
                .border_style(theme.border(app.focus == Focus::Main)),
        )
        .highlight_style(theme.selected());

    frame.render_stateful_widget(list, area, &mut app.track_list);
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
    fn playing_row_uses_playing_color_unless_selected() {
        let theme = Theme {
            playing: Color::Rgb(1, 1, 1),
            selected_bg: Color::Rgb(2, 2, 2),
            ..Theme::default()
        };
        let mut app = App::new().with_theme(theme.clone());
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

        assert_eq!(buffer.cell((1, 1)).unwrap().bg, theme.selected_bg);
        assert_eq!(buffer.cell((1, 2)).unwrap().fg, theme.playing);

        app.track_list.select(Some(1));
        terminal
            .draw(|frame| draw(frame, &mut app, frame.area()))
            .unwrap();
        let cell = terminal.backend().buffer().cell((1, 2)).unwrap();
        assert_eq!(cell.bg, theme.selected_bg);
        assert_eq!(cell.fg, theme.selected_fg);
    }
}
