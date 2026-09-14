use std::path::Path;

use librespot::core::authentication::Credentials;
use librespot::core::cache::Cache;
use librespot::core::{Session, SessionConfig};
use librespot_oauth::{OAuthClientBuilder, OAuthToken};
use thiserror::Error;

const REDIRECT_URI: &str = "http://127.0.0.1:8898/login";
const SCOPES: &[&str] = &["streaming"];

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("could not open credential cache: {0}")]
    Cache(#[source] librespot::core::Error),

    #[error("browser login failed: {0}")]
    OAuth(#[from] librespot_oauth::OAuthError),
}

pub struct Login {
    pub session: Session,
    pub credentials: Credentials,
}

pub async fn login(cache_dir: &Path) -> Result<Login, AuthError> {
    let cache =
        Cache::new(Some(cache_dir.to_path_buf()), None, None, None).map_err(AuthError::Cache)?;
    let config = SessionConfig::default();

    let credentials = match cache.credentials() {
        Some(credentials) => credentials,
        None => {
            let token = browser_login(&config.client_id, REDIRECT_URI, SCOPES).await?;
            Credentials::with_access_token(token.access_token)
        }
    };

    let session = Session::new(config, Some(cache));
    Ok(Login {
        session,
        credentials,
    })
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
