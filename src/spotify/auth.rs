use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use librespot::core::authentication::Credentials;
use librespot::core::cache::Cache;
use librespot::core::{Session, SessionConfig};
use librespot_oauth::{OAuthClientBuilder, OAuthToken};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const PLAYBACK_REDIRECT_URI: &str = "http://127.0.0.1:8898/login";
const PLAYBACK_SCOPES: &[&str] = &["streaming"];

const WEB_REDIRECT_URI: &str = "http://127.0.0.1:5588/login";
const WEB_SCOPES: &[&str] = &[
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-library-read",
    "user-read-private",
    "user-read-email",
];

const EXPIRY_MARGIN_SECS: u64 = 60;
const WEB_TOKEN_FILE: &str = "web_token.json";

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("could not open credential cache: {0}")]
    Cache(#[source] librespot::core::Error),

    #[error("browser login failed: {0}")]
    OAuth(#[from] librespot_oauth::OAuthError),

    #[error("could not read or write the web api token file: {0}")]
    TokenStore(#[from] std::io::Error),

    #[error("web api token file is corrupt: {0}")]
    TokenFormat(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

impl WebToken {
    pub fn from_oauth(token: &OAuthToken) -> Self {
        let now = Instant::now();
        let remaining = token.expires_at.saturating_duration_since(now);
        WebToken {
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
            expires_at: unix_now().saturating_add(remaining.as_secs()),
        }
    }

    pub fn is_expiring_at(&self, now_unix: u64) -> bool {
        now_unix.saturating_add(EXPIRY_MARGIN_SECS) >= self.expires_at
    }

    pub fn is_expiring(&self) -> bool {
        self.is_expiring_at(unix_now())
    }
}

pub struct PlaybackLogin {
    pub session: Session,
    pub credentials: Credentials,
}

pub async fn playback_login(cache_dir: &Path) -> Result<PlaybackLogin, AuthError> {
    let cache =
        Cache::new(Some(cache_dir.to_path_buf()), None, None, None).map_err(AuthError::Cache)?;
    let config = SessionConfig::default();

    let credentials = match cache.credentials() {
        Some(credentials) => credentials,
        None => {
            let token =
                browser_login(&config.client_id, PLAYBACK_REDIRECT_URI, PLAYBACK_SCOPES).await?;
            Credentials::with_access_token(token.access_token)
        }
    };

    let session = Session::new(config, Some(cache));
    Ok(PlaybackLogin {
        session,
        credentials,
    })
}

pub async fn web_login(cache_dir: &Path, client_id: &str) -> Result<WebToken, AuthError> {
    let cached = match load_web_token(cache_dir)? {
        Some(token) if !token.is_expiring() => Some(token),
        Some(token) => match refresh_web_token(client_id, &token).await {
            Ok(new_token) => Some(new_token),
            Err(error) => {
                tracing::warn!("web token refresh failed: {error}");
                None
            }
        },
        None => None,
    };

    let token = match cached {
        Some(token) => token,
        None => {
            let response = browser_login(client_id, WEB_REDIRECT_URI, WEB_SCOPES).await?;
            WebToken::from_oauth(&response)
        }
    };

    save_web_token(cache_dir, &token)?;
    Ok(token)
}

pub async fn refresh_web_token(client_id: &str, token: &WebToken) -> Result<WebToken, AuthError> {
    let client =
        OAuthClientBuilder::new(client_id, WEB_REDIRECT_URI, WEB_SCOPES.to_vec()).build()?;
    let response = client.refresh_token_async(&token.refresh_token).await?;

    let mut new_token = WebToken::from_oauth(&response);
    if new_token.refresh_token.is_empty() {
        new_token.refresh_token = token.refresh_token.clone();
    }

    Ok(new_token)
}

pub fn save_web_token(cache_dir: &Path, token: &WebToken) -> Result<(), AuthError> {
    std::fs::create_dir_all(cache_dir)?;

    let path = cache_dir.join(WEB_TOKEN_FILE);
    let json = serde_json::to_string_pretty(token)?;
    std::fs::write(&path, json)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

fn load_web_token(cache_dir: &Path) -> Result<Option<WebToken>, AuthError> {
    let token = match std::fs::read_to_string(cache_dir.join(WEB_TOKEN_FILE)) {
        Ok(token) => token,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => {
            return Err(AuthError::TokenStore(error));
        }
    };

    let parsed_token: WebToken = serde_json::from_str(&token)?;
    Ok(Some(parsed_token))
}

async fn browser_login(
    client_id: &str,
    redirect_uri: &str,
    scopes: &[&str],
) -> Result<OAuthToken, AuthError> {
    let client = OAuthClientBuilder::new(client_id, redirect_uri, scopes.to_vec())
        .open_in_browser()
        .build()?;

    let token = client.get_access_token_async().await?;

    Ok(token)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before 1970")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(expires_at: u64) -> WebToken {
        WebToken {
            access_token: "a".into(),
            refresh_token: "r".into(),
            expires_at,
        }
    }

    #[test]
    fn fresh_token_is_not_expiring() {
        assert!(!token(1000).is_expiring_at(1000 - EXPIRY_MARGIN_SECS - 1));
    }

    #[test]
    fn token_inside_margin_is_expiring() {
        assert!(token(1000).is_expiring_at(1000 - EXPIRY_MARGIN_SECS));
        assert!(token(1000).is_expiring_at(1000));
        assert!(token(1000).is_expiring_at(5000));
    }

    #[test]
    fn token_round_trips_through_json() {
        let original = token(42);
        let json = serde_json::to_string(&original).unwrap();
        let back: WebToken = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }
}
