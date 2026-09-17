use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::List;

use crate::app::{App, Focus};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let pane = app.theme.pane(app.focus == Focus::Sidebar);
    let list = List::new(app.playlists.iter().map(|p| p.name.as_str()))
        .style(pane.text())
        .block(pane.block("Playlists"))
        .highlight_style(pane.selected());

    frame.render_stateful_widget(list, area, &mut app.sidebar);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    use super::*;
    use crate::theme::Theme;

    fn render(app: &mut App) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(12, 4)).unwrap();
        terminal
            .draw(|frame| draw(frame, app, frame.area()))
            .unwrap();
        terminal
    }

    #[test]
    fn the_border_halves_toward_the_background_when_unfocused() {
        let theme = Theme {
            accent: Color::Rgb(100, 200, 40),
            background: Color::Rgb(0, 0, 0),
            ..Theme::default()
        };

        let mut app = App::new().with_theme(theme.clone());
        app.focus = Focus::Sidebar;
        let terminal = render(&mut app);
        let corner = terminal.backend().buffer().cell((0, 0)).unwrap();
        assert_eq!(corner.fg, theme.accent);
        assert_eq!(corner.symbol(), "\u{256d}");

        app.focus = Focus::Main;
        let terminal = render(&mut app);
        let corner = terminal.backend().buffer().cell((0, 0)).unwrap();
        assert_eq!(corner.fg, Color::Rgb(50, 100, 20));
    }
}
