use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

use super::*;
use crate::spotify::model::Track;

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

fn number_column(buffer: &Buffer, row: u16) -> String {
    (1..=4)
        .map(|x| buffer.cell((x, row)).unwrap().symbol())
        .collect::<String>()
        .trim()
        .to_string()
}

/// Renders five tracks with the third one selected and returns the `#` column.
fn numbers(focus: Focus) -> Vec<String> {
    let mut app = App::new();
    app.focus = focus;
    app.tracks = (0..5).map(|i| track(&i.to_string())).collect();
    app.track_list.select(Some(2));

    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();

    (0..5)
        .map(|i| number_column(buffer, FIRST_ROW + i))
        .collect()
}

#[test]
fn number_column_is_relative_to_the_selection() {
    assert_eq!(numbers(Focus::Main), ["2", "1", "3", "1", "2"]);
}

#[test]
fn number_column_is_absolute_when_the_pane_is_not_focused() {
    assert_eq!(numbers(Focus::Sidebar), ["1", "2", "3", "4", "5"]);
}
