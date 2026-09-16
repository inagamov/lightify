use ratatui::style::{Color, Style};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub accent: Color,
    pub title: Color,
    pub selected_fg: Color,
    pub selected_bg: Color,
    pub playing: Color,
    pub error: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::Reset,
            dim: Color::DarkGray,
            accent: Color::Green,
            title: Color::Reset,
            selected_fg: Color::Black,
            selected_bg: Color::Green,
            playing: Color::Green,
            error: Color::Red,
        }
    }
}

impl Theme {
    pub fn base(&self) -> Style {
        Style::new().fg(self.fg).bg(self.bg)
    }

    pub fn border(&self, focused: bool) -> Style {
        let color = if focused { self.accent } else { self.dim };
        Style::new().fg(color)
    }

    pub fn title(&self) -> Style {
        Style::new().fg(self.title)
    }

    pub fn selected(&self) -> Style {
        Style::new().fg(self.selected_fg).bg(self.selected_bg)
    }

    pub fn dim(&self) -> Style {
        Style::new().fg(self.dim)
    }

    pub fn playing(&self) -> Style {
        Style::new().fg(self.playing)
    }

    pub fn error(&self) -> Style {
        Style::new().fg(self.error)
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
        assert_eq!(theme.dim, Theme::default().dim);
        assert_eq!(theme.error, Theme::default().error);
    }

    #[test]
    fn named_and_indexed_colors_parse() {
        let theme: Theme = toml::from_str(r#"dim = "bright black""#).unwrap();
        assert_eq!(theme.dim, Color::DarkGray);
        let theme: Theme = toml::from_str(r#"dim = "208""#).unwrap();
        assert_eq!(theme.dim, Color::Indexed(208));
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
    fn border_uses_accent_when_focused_and_dim_otherwise() {
        let theme = Theme::default();
        assert_eq!(theme.border(true).fg, Some(theme.accent));
        assert_eq!(theme.border(false).fg, Some(theme.dim));
    }
}
