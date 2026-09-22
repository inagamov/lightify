use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;

use crate::app::{App, Status};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let Some(status) = &app.status else { return };

    let style = match status {
        Status::Error(_) => app.theme.error(),
        Status::Info(_) => Style::new(),
    };

    frame.render_widget(Paragraph::new(status.text()).style(style), area);
}
