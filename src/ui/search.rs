use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Focus};
use crate::ui::gradient;

const PROMPT: &str = "› ";

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Search;
    let pane = app.theme.pane(focused);

    let line = Line::from(vec![
        Span::styled(
            PROMPT,
            Style::new()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(app.search.as_str()),
    ]);
    let width = u16::try_from(line.width()).unwrap_or(u16::MAX);

    let bar = Paragraph::new(line)
        .style(pane.text())
        .block(pane.block("Search"));
    frame.render_widget(bar, area);
    gradient::border(frame.buffer_mut(), &app.theme, area, focused);

    if focused {
        let x = area
            .x
            .saturating_add(1)
            .saturating_add(width)
            .min(area.right().saturating_sub(2));
        frame.set_cursor_position((x, area.y + 1));
    }
}
