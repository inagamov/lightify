use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::app::{App, Status};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let paragraph = match &app.status {
        Some(Status::Error(text)) => Paragraph::new(text.as_str()).style(app.theme.error()),
        Some(Status::Info(text)) => Paragraph::new(text.as_str()),
        None => Paragraph::new(""),
    };
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    use super::*;
    use crate::app::Status;
    use crate::theme::Theme;

    #[test]
    fn error_status_uses_error_color() {
        let theme = Theme {
            error: Color::Rgb(9, 9, 9),
            ..Theme::default()
        };
        let mut app = App::new().with_theme(theme.clone());
        app.status = Some(Status::Error("bad".to_string()));

        let mut terminal = Terminal::new(TestBackend::new(10, 1)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, frame.area()))
            .unwrap();

        let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
        assert_eq!(cell.fg, theme.error);
    }
}
