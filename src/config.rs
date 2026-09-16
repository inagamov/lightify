use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirs_end_with_app_name() {
        assert!(cache_dir().ends_with("lightify"));
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_dirs_live_under_dot_directories() {
        if std::env::var_os("XDG_CACHE_HOME").is_none() {
            assert!(cache_dir().to_string_lossy().contains("/.cache/"));
        }
    }
}
