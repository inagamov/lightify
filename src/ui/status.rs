use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::app::{App, Status};
use crate::ui::help;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let status = app.status.as_ref().map(|status| {
        let style = match status {
            Status::Error(_) => app.theme.error(),
            Status::Info(_) => Style::new(),
        };
        Line::styled(status.text().to_string(), style)
    });
    let status_width = status.as_ref().map_or(0, |line| line.width() + 2);

    let [hints_area, status_area] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(u16::try_from(status_width).unwrap_or(u16::MAX)),
    ])
    .areas(area.inner(Margin::new(1, 0)));

    let hints = help::hint_spans(app, usize::from(hints_area.width));
    frame.render_widget(Paragraph::new(Line::from(hints)), hints_area);

    if let Some(status) = status {
        frame.render_widget(Paragraph::new(status.right_aligned()), status_area);
    }
}
