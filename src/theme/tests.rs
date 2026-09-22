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
fn bad_color_string_is_an_error() {
    assert!(toml::from_str::<Theme>(r#"accent = "not a color""#).is_err());
}

#[test]
fn unknown_key_is_an_error() {
    assert!(toml::from_str::<Theme>(r#"acent = "red""#).is_err());
}

#[test]
fn halving_blends_toward_the_surface() {
    let theme = Theme {
        text: Color::Rgb(200, 200, 200),
        surface: Color::Rgb(100, 100, 100),
        ..Theme::default()
    };
    assert_eq!(theme.pane(false).text().fg, Some(Color::Rgb(150, 150, 150)));
}

#[test]
fn a_terminal_owned_color_falls_back_to_the_faint_attribute() {
    let theme = Theme {
        text: Color::White,
        surface: Color::Reset,
        ..Theme::default()
    };
    let faded = theme.pane(false).text();

    assert_eq!(faded.fg, Some(Color::White));
    assert!(faded.add_modifier.contains(Modifier::DIM));
}
