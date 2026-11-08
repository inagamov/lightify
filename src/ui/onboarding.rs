use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::logo::LOGO;
use crate::onboarding::{Failure, Note, Onboarding, STEPS, Screen};
use crate::theme::Theme;
use crate::ui::gradient;

const CARD_WIDTH: u16 = 50;
const PAD_LEFT: usize = 3;
const PAD_RIGHT: usize = 1;
const TAGLINE: &str = "Spotify in your terminal, played right here.";
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(frame: &mut Frame, onboarding: &Onboarding, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::new().style(theme.base()), area);

    let card_width = CARD_WIDTH.min(area.width);
    let text_width = usize::from(card_width).saturating_sub(2 + PAD_LEFT + PAD_RIGHT);
    let body = body(onboarding, theme, text_width);
    let card_height = to_u16(body.len() + 2);

    let logo_height = to_u16(LOGO.len());
    let with_logo = logo_height + 1 + 1 + 1 + card_height + 1 + 1;
    let show_logo = with_logo <= area.height;
    let height = if show_logo {
        with_logo
    } else {
        card_height + 1 + 1
    };

    let group = area.centered(
        Constraint::Length(card_width),
        Constraint::Length(height.min(area.height)),
    );
    let mut constraints = Vec::new();
    if show_logo {
        constraints.extend([
            Constraint::Length(logo_height),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ]);
    }
    constraints.extend([
        Constraint::Length(card_height),
        Constraint::Length(1),
        Constraint::Length(1),
    ]);
    let rows = Layout::vertical(constraints).split(group);

    let mut next = rows.iter().copied();
    if show_logo {
        let logo_area = next.next().unwrap_or_default();
        draw_logo(frame, theme, logo_area);
        next.next();
        let tagline_area = next.next().unwrap_or_default();
        let tagline = Line::styled(
            TAGLINE,
            Style::new()
                .fg(theme.highlight)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(Paragraph::new(tagline).centered(), tagline_area);
        next.next();
    }
    let card_area = next.next().unwrap_or_default();
    next.next();
    let hints_area = next.next().unwrap_or_default();

    draw_card(frame, onboarding, theme, card_area, body);
    frame.render_widget(
        Paragraph::new(Line::from(hint_spans(onboarding, theme))).centered(),
        hints_area,
    );
}

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

fn draw_logo(frame: &mut Frame, theme: &Theme, area: Rect) {
    let width = LOGO.iter().map(|row| row.len()).max().unwrap_or(0);
    let area = area.centered_horizontally(Constraint::Length(to_u16(width)));
    let lines: Vec<Line> = LOGO
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let color = if i < 3 {
                theme.accent
            } else {
                theme.accent_deep
            };
            Line::styled(*row, Style::new().fg(color))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_card(
    frame: &mut Frame,
    onboarding: &Onboarding,
    theme: &Theme,
    area: Rect,
    body: Vec<Line<'static>>,
) {
    let mut block = theme.pane(true).block("Connect Spotify");
    if let Some(filled) = onboarding.progress() {
        let mut spans = vec![Span::raw(" ")];
        for step in 0..STEPS {
            if step > 0 {
                spans.push(Span::raw(" "));
            }
            if step < filled {
                spans.push(Span::styled("●", Style::new().fg(theme.accent)));
            } else {
                spans.push(Span::styled("○", theme.dim()));
            }
        }
        spans.push(Span::raw(" "));
        block = block.title_top(Line::from(spans).right_aligned());
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);
    gradient::border(frame.buffer_mut(), theme, area, true);

    let padded: Vec<Line> = body
        .into_iter()
        .map(|line| {
            let mut spans = vec![Span::raw(" ".repeat(PAD_LEFT))];
            spans.extend(line.spans);
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(padded), inner);
}

fn body(onboarding: &Onboarding, theme: &Theme, width: usize) -> Vec<Line<'static>> {
    let accent = Style::new().fg(theme.accent);
    let dim = theme.dim();
    let spinner = Span::styled(SPINNER[onboarding.spinner % SPINNER.len()], accent);

    let mut lines = vec![Line::default()];
    match &onboarding.screen {
        Screen::Welcome { note } => {
            if *note == Some(Note::SavedLoginRejected) {
                for row in wrap("Your saved login stopped working. Sign in again.", width) {
                    lines.push(Line::styled(row, dim));
                }
                lines.push(Line::default());
            }
            for (n, text) in [
                ("1", "Your browser opens Spotify's sign-in page"),
                ("2", "You approve lightify there"),
                ("3", "Come back here and start listening"),
            ] {
                lines.push(Line::from(vec![
                    Span::styled(n, accent),
                    Span::raw(format!("  {}", truncate(text, width.saturating_sub(3)))),
                ]));
            }
            lines.push(Line::default());
            lines.push(Line::styled("Playback needs Spotify Premium.", dim));
        }
        Screen::Waiting { url } => {
            lines.push(Line::from(vec![
                spinner,
                Span::raw(" Waiting for you to approve in the browser"),
            ]));
            lines.push(Line::default());
            lines.push(Line::styled(
                "Browser didn't open? Copy the link with c,",
                dim,
            ));
            lines.push(Line::styled("or open it yourself:", dim));
            if let Some(url) = url {
                lines.push(Line::styled(truncate(url, width), dim));
            }
        }
        Screen::SignedIn { .. } => {
            lines.push(Line::from(vec![
                Span::styled("✓", accent),
                Span::raw(" "),
                Span::styled("Signed in.", Style::new().add_modifier(Modifier::BOLD)),
                Span::raw(" Loading your playlists…"),
            ]));
        }
        Screen::Connecting => {
            lines.push(Line::from(vec![spinner, Span::raw(" Connecting…")]));
        }
        Screen::Failed { failure } => {
            let (message, detail) = match failure {
                Failure::Cancelled => ("Sign-in was cancelled.", None),
                Failure::PortBusy => ("Port 8898 is busy. Is another lightify running?", None),
                Failure::NoPremium => (
                    "This account doesn't have Spotify Premium, which playback needs.",
                    None,
                ),
                Failure::Network(error) => ("Couldn't reach Spotify.", Some(error.as_str())),
            };
            for (i, row) in wrap(message, width.saturating_sub(2))
                .into_iter()
                .enumerate()
            {
                let prefix = if i == 0 { "× " } else { "  " };
                lines.push(Line::styled(format!("{prefix}{row}"), theme.error()));
            }
            if let Some(detail) = detail {
                lines.push(Line::styled(truncate(detail, width), dim));
            }
            lines.push(Line::default());
            lines.push(Line::styled("Press enter to try again.", dim));
        }
    }
    lines.push(Line::default());
    lines
}

fn hint_spans(onboarding: &Onboarding, theme: &Theme) -> Vec<Span<'static>> {
    let hints: &[(&str, &str)] = match onboarding.screen {
        Screen::Welcome { .. } => &[("enter", "sign in"), ("q", "quit")],
        Screen::Waiting { .. } => &[("c", "copy link"), ("o", "open again"), ("esc", "back")],
        Screen::Failed { .. } => &[("enter", "try again"), ("q", "quit")],
        Screen::Connecting | Screen::SignedIn { .. } => &[("q", "quit")],
    };
    let key_style = Style::new().fg(theme.accent);
    let mut spans = Vec::new();
    for (i, (key, description)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(*key, key_style));
        spans.push(Span::styled(format!(" {description}"), theme.dim()));
    }
    spans
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(width - 1).collect();
    out.push('…');
    out
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut row = String::new();
    for word in text.split_whitespace() {
        let needed = if row.is_empty() {
            word.chars().count()
        } else {
            row.chars().count() + 1 + word.chars().count()
        };
        if needed > width && !row.is_empty() {
            rows.push(std::mem::take(&mut row));
        }
        if !row.is_empty() {
            row.push(' ');
        }
        row.push_str(word);
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn render(width: u16, height: u16, onboarding: &Onboarding) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw(frame, onboarding, &Theme::default()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn welcome_shows_logo_card_and_dots() {
        let (onboarding, _) = Onboarding::new(None);
        let screen = render(80, 24, &onboarding);
        assert!(screen.contains("|___/"));
        assert!(screen.contains("Connect Spotify"));
        assert!(screen.contains("● ○"));
        assert!(screen.contains("enter sign in"));
    }

    #[test]
    fn short_terminal_drops_the_logo() {
        let (onboarding, _) = Onboarding::new(None);
        let screen = render(60, 14, &onboarding);
        assert!(!screen.contains("|___/"));
        assert!(screen.contains("Connect Spotify"));
    }

    #[test]
    fn tiny_terminal_draws_without_panic() {
        let (onboarding, _) = Onboarding::new(None);
        render(30, 8, &onboarding);
    }
}
