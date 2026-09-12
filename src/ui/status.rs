use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let text = app.status.as_deref().unwrap_or("");
    frame.render_widget(Paragraph::new(text), area);
}
