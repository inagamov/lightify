use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub background: Color,
    pub text: Color,
    pub accent: Color,
    pub surface: Color,
    pub error: Color,
}

const DEFAULT: Theme = Theme {
    background: Color::Reset,
    text: Color::Reset,
    accent: Color::Rgb(0x1d, 0xb9, 0x54),
    surface: Color::Rgb(0x00, 0x00, 0x00),
    error: Color::Red,
};

impl Default for Theme {
    fn default() -> Self {
        DEFAULT
    }
}

impl Theme {
    pub fn base(&self) -> Style {
        Style::new().fg(self.text).bg(self.background)
    }

    pub fn pane(&self, focused: bool) -> Pane<'_> {
        Pane {
            theme: self,
            focused,
        }
    }

    pub fn dim(&self) -> Style {
        self.faded(self.text)
    }

    pub fn error(&self) -> Style {
        Style::new().fg(self.error)
    }

    fn half(&self, color: Color) -> Option<Color> {
        let (Color::Rgb(r, g, b), Color::Rgb(br, bg, bb)) = (color, self.surface) else {
            return None;
        };
        let mid = |a: u8, b: u8| ((u16::from(a) + u16::from(b)) / 2) as u8;
        Some(Color::Rgb(mid(r, br), mid(g, bg), mid(b, bb)))
    }

    fn faded(&self, color: Color) -> Style {
        match self.half(color) {
            Some(half) => Style::new().fg(half),
            None => Style::new().fg(color).add_modifier(Modifier::DIM),
        }
    }
}

pub struct Pane<'a> {
    theme: &'a Theme,
    focused: bool,
}

impl Pane<'_> {
    pub fn block(&self, title: impl Into<Line<'static>>) -> Block<'static> {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(title)
            .title_style(self.title())
            .border_style(self.border())
    }

    pub fn text(&self) -> Style {
        self.at_strength(self.theme.text)
    }

    pub fn border(&self) -> Style {
        self.at_strength(self.theme.accent)
    }

    pub fn title(&self) -> Style {
        self.at_strength(self.theme.text)
    }

    pub fn playing(&self) -> Style {
        self.at_strength(self.theme.accent)
    }

    pub fn selected(&self) -> Style {
        if self.focused {
            return Style::new().fg(self.theme.surface).bg(self.theme.accent);
        }
        let bar = self
            .theme
            .half(self.theme.accent)
            .unwrap_or(self.theme.accent);
        Style::new().fg(self.theme.text).bg(bar)
    }

    fn at_strength(&self, color: Color) -> Style {
        if self.focused {
            Style::new().fg(color)
        } else {
            self.theme.faded(color)
        }
    }
}

#[cfg(test)]
mod tests;
