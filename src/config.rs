use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

use crate::theme::Theme;

fn app_dir(xdg_var: &str, unix_dot_dir: &str, windows_dir: fn() -> Option<PathBuf>) -> PathBuf {
    let base = if cfg!(windows) {
        windows_dir().expect("No data directory")
    } else {
        std::env::var_os(xdg_var)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .expect("No home directory")
                    .join(unix_dot_dir)
            })
    };
    base.join("lightify")
}

pub fn cache_dir() -> PathBuf {
    app_dir("XDG_CACHE_HOME", ".cache", dirs::cache_dir)
}

pub fn config_dir() -> PathBuf {
    app_dir("XDG_CONFIG_HOME", ".config", dirs::config_dir)
}

#[derive(Debug, Default, PartialEq, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub theme: Theme,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&config_dir().join("config.toml"))
    }

    fn load_from(path: &Path) -> anyhow::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lightify-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn missing_config_file_gives_defaults() {
        let path = scratch_file("does-not-exist.toml");
        assert_eq!(Config::load_from(&path).unwrap(), Config::default());
    }

    #[test]
    fn theme_table_is_applied() {
        let path = scratch_file("theme.toml");
        std::fs::write(&path, "[theme]\naccent = \"blue\"\n").unwrap();
        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.theme.accent, ratatui::style::Color::Blue);
        assert_eq!(config.theme.text, Theme::default().text);
    }

    #[test]
    fn unknown_top_level_key_is_ignored() {
        let path = scratch_file("future.toml");
        std::fs::write(&path, "future_key = 1\n").unwrap();
        assert_eq!(Config::load_from(&path).unwrap(), Config::default());
    }

    #[test]
    fn parse_error_names_the_file() {
        let path = scratch_file("broken.toml");
        std::fs::write(&path, "[theme]\naccent = \"nope\"\n").unwrap();
        let message = format!("{:#}", Config::load_from(&path).unwrap_err());
        assert!(message.contains("broken.toml"), "{message}");
        assert!(message.contains("accent"), "{message}");
    }
}
