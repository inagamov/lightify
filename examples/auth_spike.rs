use librespot::core::authentication::Credentials;
use librespot::core::cache::Cache;
use librespot::core::{Session, SessionConfig};
use librespot_oauth::OAuthClientBuilder;

const REDIRECT_URI: &str = "http://127.0.0.1:5588/login";
const SCOPES: &[&str] = &[
    "streaming",
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-library-read",
    "user-read-private",
    "user-read-email",
];
const MY_CLIENT_ID: &str = "1f24a7947bf0410e990ee4031a1cb2a8";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cache_dir = dirs::home_dir()
        .expect("no home directory")
        .join(".cache")
        .join("lightify");
    // librespot treats this as a directory and writes credentials.json inside it.
    let cache = Cache::new(Some(cache_dir.clone()), None, None, None)?;

    let config = SessionConfig {
        client_id: MY_CLIENT_ID.to_string(),
        ..Default::default()
    };

    // One browser login. Its token is used for both librespot and the Web API.
    let client = OAuthClientBuilder::new(&config.client_id, REDIRECT_URI, SCOPES.to_vec())
        .open_in_browser()
        .build()?;
    let token = client.get_access_token_async().await?;
    println!("token expires at {:?}", token.expires_at);

    let credentials = match cache.credentials() {
        Some(saved) => {
            println!("using cached streaming credentials");
            saved
        }
        None => Credentials::with_access_token(token.access_token.clone()),
    };

    let session = Session::new(config, Some(cache));
    session.connect(credentials, true).await?;
    println!("connected as {}", session.username());

    let http = reqwest::Client::new();
    let body: serde_json::Value = http
        .get("https://api.spotify.com/v1/me/playlists?limit=50")
        .bearer_auth(&token.access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    for item in body["items"].as_array().unwrap_or(&vec![]) {
        println!("{} ({} tracks)", item["name"], item["items"]["total"]);
    }

    session.shutdown();
    Ok(())
}
