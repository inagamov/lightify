use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use thiserror::Error;
use tokio::sync::Mutex;

use super::auth::{AuthError, WebToken, refresh_web_token, save_web_token};

const API: &str = "https://api.spotify.com/v1";

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("web api auth failed: {0}")]
    Auth(#[from] AuthError),

    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("spotify session error: {0}")]
    Session(#[from] librespot::core::Error),
}

#[derive(Clone)]
pub struct SpotifyApi {
    http: reqwest::Client,
    client_id: String,
    cache_dir: PathBuf,
    token: Arc<Mutex<WebToken>>,
}

#[allow(dead_code)]
impl SpotifyApi {
    pub fn new(client_id: String, cache_dir: PathBuf, token: WebToken) -> Self {
        Self {
            http: reqwest::Client::new(),
            client_id,
            cache_dir,
            token: Arc::new(Mutex::new(token)),
        }
    }

    async fn access_token(&self) -> Result<String, ApiError> {
        let mut guard = self.token.lock().await;

        if guard.is_expiring() {
            let new_token = refresh_web_token(&self.client_id, &guard).await?;
            save_web_token(&self.cache_dir, &new_token)?;
            *guard = new_token;
        }

        Ok(guard.access_token.clone())
    }

    async fn get<T: DeserializeOwned>(&self, url: &str) -> Result<T, ApiError> {
        let mut response = self.send(url).await?;

        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            let seconds = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|text| text.parse::<u64>().ok())
                .unwrap_or(1);

            tokio::time::sleep(Duration::from_secs(seconds)).await;
            response = self.send(url).await?;
        }

        let value = response.error_for_status()?.json::<T>().await?;
        Ok(value)
    }

    async fn send(&self, url: &str) -> Result<reqwest::Response, ApiError> {
        let token = self.access_token().await?;
        let response = self.http.get(url).bearer_auth(token).send().await?;
        Ok(response)
    }
}
