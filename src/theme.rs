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
mod tests {
    use super::*;

    #[test]
    fn empty_table_is_the_default_theme() {
        let theme: Theme = toml::from_str("").unwrap();
        assert_eq!(theme, Theme::default());
    }

    #[test]
    fn partial_table_overrides_only_the_given_slot() {
        let theme: Theme = toml::from_str(r##"accent = "#ff0000""##).unwrap();
        assert_eq!(theme.accent, Color::Rgb(255, 0, 0));
        assert_eq!(theme.text, Theme::default().text);
        assert_eq!(theme.error, Theme::default().error);
    }

    #[test]
    fn named_and_indexed_colors_parse() {
        let theme: Theme = toml::from_str(r#"text = "bright black""#).unwrap();
        assert_eq!(theme.text, Color::DarkGray);
        let theme: Theme = toml::from_str(r#"text = "208""#).unwrap();
        assert_eq!(theme.text, Color::Indexed(208));
    }

    #[test]
    fn bad_color_string_is_an_error() {
        assert!(toml::from_str::<Theme>(r#"accent = "not a color""#).is_err());
    }

    #[test]
    fn unknown_key_is_an_error() {
        assert!(toml::from_str::<Theme>(r#"acent = "red""#).is_err());
    }

    #[test]
    fn retired_token_names_are_an_error() {
        assert!(toml::from_str::<Theme>(r#"bg = "black""#).is_err());
        assert!(toml::from_str::<Theme>(r#"dim = "black""#).is_err());
        assert!(toml::from_str::<Theme>(r#"selected_bg = "black""#).is_err());
    }

    #[test]
    fn defaults_are_white_on_black_with_a_green_accent() {
        assert_eq!(Theme::default(), DEFAULT);
        assert_eq!(DEFAULT.background, Color::Rgb(0x00, 0x00, 0x00));
        assert_eq!(DEFAULT.text, Color::Rgb(0xff, 0xff, 0xff));
        assert_eq!(DEFAULT.accent, Color::Rgb(0x1d, 0xb9, 0x54));
    }

    #[test]
    fn a_focused_pane_renders_at_full_strength() {
        let theme = Theme::default();
        let pane = theme.pane(true);

        assert_eq!(pane.border().fg, Some(theme.accent));
        assert_eq!(pane.title().fg, Some(theme.text));
        assert_eq!(pane.text().fg, Some(theme.text));
        assert_eq!(pane.playing().fg, Some(theme.accent));
    }

    #[test]
    fn an_unfocused_pane_renders_halfway_to_the_background() {
        let theme = Theme::default();
        let pane = theme.pane(false);

        assert_eq!(pane.border().fg, Some(Color::Rgb(0x0e, 0x5c, 0x2a)));
        assert_eq!(pane.title().fg, Some(Color::Rgb(0x7f, 0x7f, 0x7f)));
        assert_eq!(pane.text().fg, Some(Color::Rgb(0x7f, 0x7f, 0x7f)));
        assert_eq!(pane.playing().fg, Some(Color::Rgb(0x0e, 0x5c, 0x2a)));
    }

    #[test]
    fn halving_blends_toward_whatever_background_is_set() {
        let theme = Theme {
            text: Color::Rgb(200, 200, 200),
            background: Color::Rgb(100, 100, 100),
            ..Theme::default()
        };
        assert_eq!(theme.pane(false).text().fg, Some(Color::Rgb(150, 150, 150)));
    }

    #[test]
    fn a_terminal_owned_color_falls_back_to_the_faint_attribute() {
        let theme = Theme {
            text: Color::White,
            background: Color::Reset,
            ..Theme::default()
        };
        let faded = theme.pane(false).text();

        assert_eq!(faded.fg, Some(Color::White));
        assert!(faded.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn a_focused_selection_puts_the_background_on_the_accent() {
        let theme = Theme::default();
        let selected = theme.pane(true).selected();

        assert_eq!(selected.bg, Some(theme.accent));
        assert_eq!(selected.fg, Some(theme.background));
    }

    #[test]
    fn an_unfocused_selection_dims_the_bar_but_keeps_its_text_legible() {
        let theme = Theme::default();
        let selected = theme.pane(false).selected();

        assert_eq!(selected.bg, Some(Color::Rgb(0x0e, 0x5c, 0x2a)));
        assert_eq!(selected.fg, Some(theme.text));
    }
}
