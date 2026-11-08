use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, ConnectionStatus, Focus, Mode};
use crate::ui::gradient;

pub struct Binding {
    pub keys: &'static str,
    pub description: &'static str,
}

const fn bind(keys: &'static str, description: &'static str) -> Binding {
    Binding { keys, description }
}

pub const KEYBINDINGS: &[(&str, &[Binding])] = &[
    (
        "Navigation",
        &[
            bind("j / k", "move down / up"),
            bind("h / l", "focus playlists / tracks"),
            bind("ctrl+d / ctrl+u", "half page down / up"),
            bind("gg / G", "jump to top / bottom"),
            bind("<count>gg", "jump to row"),
            bind("zt / zz / zb", "row to top / middle / bottom"),
            bind("enter", "open playlist / play track"),
        ],
    ),
    (
        "Playback",
        &[
            bind("space", "play / pause"),
            bind("n / N", "next / previous track"),
            bind("> / <", "seek forward / back"),
            bind("+ / -", "volume up / down"),
        ],
    ),
    (
        "Search",
        &[
            bind("s", "focus search"),
            bind("enter", "search"),
            bind("esc", "cancel"),
        ],
    ),
    (
        "General",
        &[
            bind("R", "refresh / reconnect"),
            bind("?", "toggle help"),
            bind("q", "quit"),
        ],
    ),
];

pub fn hints(app: &App) -> Vec<Binding> {
    let focus = app.focus;
    match app.mode() {
        Mode::Help => vec![bind("esc", "close")],
        Mode::Insert => vec![bind("enter", "search"), bind("esc", "cancel")],
        Mode::Normal => {
            let mut hints = Vec::new();
            if app.connection == ConnectionStatus::Lost {
                hints.push(bind("R", "reconnect"));
            }
            match focus {
                Focus::Sidebar if !app.playlists.is_empty() => {
                    hints.push(bind("j/k", "move"));
                    hints.push(bind("enter", "open"));
                    hints.push(bind("l", "tracks"));
                }
                Focus::Main if !app.tracks.is_empty() => {
                    hints.push(bind("j/k", "move"));
                    hints.push(bind("enter", "play"));
                    hints.push(bind("h", "playlists"));
                }
                _ => {}
            }
            if app.playback.track.is_some() {
                let toggle = if app.playback.is_playing() {
                    "pause"
                } else {
                    "play"
                };
                hints.push(bind("space", toggle));
                hints.push(bind("n/N", "next/prev"));
            }
            hints.push(bind("s", "search"));
            hints.push(bind("?", "help"));
            hints.push(bind("q", "quit"));
            hints
        }
    }
}

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let pane = app.theme.pane(true);
    let block = pane.block("Keybindings");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    gradient::border(frame.buffer_mut(), &app.theme, area, true);

    let key_width = KEYBINDINGS
        .iter()
        .flat_map(|(_, bindings)| bindings.iter())
        .map(|b| b.keys.len())
        .max()
        .unwrap_or(0);
    let groups: Vec<Vec<Line>> = KEYBINDINGS
        .iter()
        .map(|(group, bindings)| group_lines(app, group, bindings, key_width))
        .collect();
    let column_width = groups.iter().flatten().map(Line::width).max().unwrap_or(0);

    let fits_two = usize::from(inner.width) >= 2 * column_width + COLUMN_GAP;
    let columns: Vec<Vec<Line>> = if fits_two {
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for (i, group) in groups.into_iter().enumerate() {
            stack(if i % 2 == 0 { &mut left } else { &mut right }, group);
        }
        vec![left, right]
    } else {
        let mut single = Vec::new();
        for group in groups {
            stack(&mut single, group);
        }
        vec![single]
    };

    let column_count = columns.len();
    let width = column_count * column_width + (column_count - 1) * COLUMN_GAP;
    let height = columns.iter().map(Vec::len).max().unwrap_or(0);
    let content = inner.centered(
        Constraint::Length(u16::try_from(width).unwrap_or(u16::MAX)),
        Constraint::Length(u16::try_from(height).unwrap_or(u16::MAX)),
    );

    let column_areas = Layout::horizontal(vec![
        Constraint::Length(
            u16::try_from(column_width).unwrap_or(u16::MAX)
        );
        column_count
    ])
    .spacing(u16::try_from(COLUMN_GAP).unwrap_or(u16::MAX))
    .split(content);
    for (lines, column_area) in columns.into_iter().zip(column_areas.iter()) {
        frame.render_widget(Paragraph::new(lines), *column_area);
    }
}

const COLUMN_GAP: usize = 4;

fn group_lines(
    app: &App,
    group: &'static str,
    bindings: &'static [Binding],
    key_width: usize,
) -> Vec<Line<'static>> {
    let key_style = Style::new()
        .fg(app.theme.accent)
        .add_modifier(Modifier::BOLD);
    let heading_style = Style::new().fg(app.theme.text).add_modifier(Modifier::BOLD);

    let mut lines = vec![Line::styled(group, heading_style)];
    for binding in bindings {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<key_width$}  ", binding.keys), key_style),
            Span::styled(binding.description, app.theme.dim()),
        ]));
    }
    lines
}

fn stack(column: &mut Vec<Line<'static>>, group: Vec<Line<'static>>) {
    if !column.is_empty() {
        column.push(Line::default());
    }
    column.extend(group);
}

pub fn hint_spans(app: &App, max_width: usize) -> Vec<Span<'static>> {
    let key_style = Style::new().fg(app.theme.accent);
    let mut spans = Vec::new();
    let mut width = 0;
    for (i, hint) in hints(app).into_iter().enumerate() {
        let separator = if i == 0 { "" } else { "  " };
        let hint_width = separator.len() + hint.keys.len() + 1 + hint.description.len();
        if width + hint_width > max_width {
            break;
        }
        width += hint_width;
        spans.push(Span::raw(separator));
        spans.push(Span::styled(hint.keys, key_style));
        spans.push(Span::styled(
            format!(" {}", hint.description),
            app.theme.dim(),
        ));
    }
    spans
}
