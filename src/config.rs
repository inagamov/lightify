use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub client_id: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error(
        "No config file at {0}\n\n\
         Create it with your Spotify app's client id:\n\n\
         \tclient_id = \"...\"\n\n\
         Register an app at https://developer.spotify.com/dashboard with redirect URI\n\
         http://127.0.0.1:5588/login"
    )]
    Missing(PathBuf),
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

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

pub fn config_dir() -> PathBuf {
    app_dir("XDG_CONFIG_HOME", ".config", dirs::config_dir)
}

pub fn cache_dir() -> PathBuf {
    app_dir("XDG_CACHE_HOME", ".cache", dirs::cache_dir)
}

pub fn load() -> Result<Config, ConfigError> {
    load_from(config_dir().join("config.toml"))
}

fn load_from(path: PathBuf) -> Result<Config, ConfigError> {
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ConfigError::Missing(path));
        }
        Err(error) => {
            return Err(ConfigError::Read {
                path,
                source: error,
            });
        }
    };

    toml::from_str(&text).map_err(|error| ConfigError::Parse {
        path,
        source: error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirs_end_with_app_name() {
        assert!(config_dir().ends_with("lightify"));
        assert!(cache_dir().ends_with("lightify"));
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_dirs_live_under_dot_directories() {
        if std::env::var_os("XDG_CONFIG_HOME").is_none() {
            assert!(config_dir().to_string_lossy().contains("/.config/"));
        }
        if std::env::var_os("XDG_CACHE_HOME").is_none() {
            assert!(cache_dir().to_string_lossy().contains("/.cache/"));
        }
    }

    #[test]
    fn missing_file_is_a_helpful_error() {
        let err = load_from(PathBuf::from("/definitely/not/here/config.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::Missing(_)));
        assert!(err.to_string().contains("developer.spotify.com"));
    }

    #[test]
    fn parses_client_id() {
        let dir = std::env::temp_dir().join(format!("lightify-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "client_id = \"abc123\"\n").unwrap();
        let config = load_from(path).unwrap();
        assert_eq!(config.client_id, "abc123");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bad_toml_names_the_file() {
        let dir = std::env::temp_dir().join(format!("lightify-test-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "client_id = \n").unwrap();
        let err = load_from(path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        assert!(err.to_string().contains("config.toml"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
