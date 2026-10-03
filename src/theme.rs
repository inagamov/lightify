use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub background: Color,
    pub text: Color,
    pub accent: Color,
    pub accent_deep: Color,
    pub highlight: Color,
    pub glow: Color,
    pub shade: Color,
    pub idle: Color,
    pub muted: Color,
    pub surface: Color,
    pub error: Color,
}

const DEFAULT: Theme = Theme {
    background: Color::Reset,
    text: Color::Reset,
    accent: Color::Rgb(0x1e, 0xd7, 0x60),
    accent_deep: Color::Rgb(0x0a, 0x6e, 0x33),
    highlight: Color::Rgb(0xff, 0xff, 0xff),
    glow: Color::Rgb(0x17, 0x4d, 0x2c),
    shade: Color::Rgb(0x18, 0x18, 0x18),
    idle: Color::Rgb(0x2a, 0x2a, 0x2a),
    muted: Color::Rgb(0xb3, 0xb3, 0xb3),
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
        Style::new().fg(self.muted)
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
        let mut title = title.into();
        title.spans.insert(0, Span::raw(" "));
        title.spans.push(Span::raw(" "));
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

    pub fn highlight(&self) -> Style {
        Style::new().fg(self.theme.highlight)
    }

    fn at_strength(&self, color: Color) -> Style {
        if self.focused {
            Style::new().fg(color)
        } else {
            self.theme.faded(color)
        }
    }
}
