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
