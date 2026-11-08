use std::io::ErrorKind;
use std::time::Duration;

use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use thiserror::Error;

use crate::logo::LOGO;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

pub const REDIRECT_URI: &str = "http://127.0.0.1:8898/login";
const LISTEN_ADDR: &str = "127.0.0.1:8898";
const SCOPES: &[&str] = &["streaming"];
const AUTH_URL: &str = "https://accounts.spotify.com/authorize";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(2);

type Client = BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

pub struct AuthRequest {
    pub url: String,
    verifier: PkceCodeVerifier,
    state: CsrfToken,
    client_id: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Callback {
    Code(String),
    Denied,
    NotLogin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    SignedIn,
    Cancelled,
    Failed,
}

#[derive(Debug, Error)]
pub enum SignInError {
    #[error("port 8898 is busy")]
    PortBusy,

    #[error("sign-in was cancelled")]
    Cancelled,

    #[error("{0}")]
    Exchange(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn client(client_id: &str) -> Client {
    BasicClient::new(ClientId::new(client_id.to_string()))
        .set_auth_uri(AuthUrl::new(AUTH_URL.to_string()).expect("valid auth url"))
        .set_token_uri(TokenUrl::new(TOKEN_URL.to_string()).expect("valid token url"))
        .set_redirect_uri(RedirectUrl::new(REDIRECT_URI.to_string()).expect("valid redirect uri"))
}

pub fn authorize(client_id: &str) -> AuthRequest {
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, state) = client(client_id)
        .authorize_url(CsrfToken::new_random)
        .add_scopes(SCOPES.iter().map(|s| Scope::new((*s).to_string())))
        .set_pkce_challenge(challenge)
        .add_extra_param("show_dialog", "true")
        .url();

    AuthRequest {
        url: url.to_string(),
        verifier,
        state,
        client_id: client_id.to_string(),
    }
}

pub fn parse_callback(request_line: &str, expected_state: &str) -> Callback {
    let Some(target) = request_line.split_whitespace().nth(1) else {
        return Callback::NotLogin;
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != "/login" {
        return Callback::NotLogin;
    }

    let mut code = None;
    let mut error = false;
    let mut state_matches = false;
    for (key, value) in form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "error" => error = true,
            "code" => code = Some(value.into_owned()),
            "state" => state_matches = same(value.as_bytes(), expected_state.as_bytes()),
            _ => {}
        }
    }
    if !state_matches {
        return Callback::NotLogin;
    }
    if error {
        return Callback::Denied;
    }
    code.map_or(Callback::NotLogin, Callback::Code)
}

fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |diff, (x, y)| diff | (x ^ y)) == 0
}

pub fn response(page: Option<Page>) -> String {
    let Some(page) = page else {
        return "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_string();
    };
    let body = html(page);
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html(page: Page) -> String {
    let (mark, mark_class, headline, line) = match page {
        Page::SignedIn => (
            "✓",
            "ok",
            "You're signed in",
            "Head back to your terminal. You can close this tab.",
        ),
        Page::Cancelled => (
            "×",
            "bad",
            "Sign-in was cancelled",
            "Head back to your terminal and press enter to try again.",
        ),
        Page::Failed => (
            "×",
            "bad",
            "Something went wrong",
            "Head back to your terminal for details.",
        ),
    };

    let logo: String = LOGO
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let class = if i < 3 { "g" } else { "gd" };
            format!("<span class=\"{class}\">{}</span>\n", escape(row))
        })
        .collect();

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>lightify</title>
<style>
  html, body {{ margin: 0; height: 100%; background: #000; }}
  body {{ display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 16px;
         font-family: "SF Mono", Menlo, Consolas, monospace; }}
  pre {{ margin: 0; font-size: 13px; line-height: 1.25; }}
  .g {{ color: #1ed760; }} .gd {{ color: #0a6e33; }}
  .headline {{ color: #fff; font-size: 17px; font-weight: 600; }}
  .ok {{ color: #1ed760; }} .bad {{ color: #e5534b; }}
  .line {{ color: #6a6a6a; font-size: 13px; }}
</style>
</head>
<body>
<pre>{logo}</pre>
<div class="headline"><span class="{mark_class}">{mark}</span> {headline}</div>
<div class="line">{line}</div>
</body>
</html>
"#
    )
}

pub async fn listen() -> Result<TcpListener, SignInError> {
    TcpListener::bind(LISTEN_ADDR).await.map_err(|error| {
        if error.kind() == ErrorKind::AddrInUse {
            SignInError::PortBusy
        } else {
            SignInError::Io(error)
        }
    })
}

pub async fn sign_in(listener: TcpListener, request: AuthRequest) -> Result<String, SignInError> {
    let AuthRequest {
        verifier,
        state,
        client_id,
        ..
    } = request;

    loop {
        let (mut stream, _) = listener.accept().await?;
        let line = match tokio::time::timeout(READ_TIMEOUT, read_request_line(&mut stream)).await {
            Ok(Ok(line)) => line,
            Ok(Err(error)) => {
                tracing::warn!("could not read sign-in request: {error}");
                continue;
            }
            Err(_) => {
                tracing::debug!("dropped an idle browser connection");
                continue;
            }
        };

        match parse_callback(&line, state.secret()) {
            Callback::NotLogin => respond(&mut stream, None).await,
            Callback::Denied => {
                respond(&mut stream, Some(Page::Cancelled)).await;
                return Err(SignInError::Cancelled);
            }
            Callback::Code(code) => {
                let result = exchange(client_id, code, verifier).await;
                let page = if result.is_ok() {
                    Page::SignedIn
                } else {
                    Page::Failed
                };
                respond(&mut stream, Some(page)).await;
                return result;
            }
        }
    }
}

async fn read_request_line(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).await?;
    Ok(line)
}

async fn respond(stream: &mut TcpStream, page: Option<Page>) {
    let result = async {
        stream.write_all(response(page).as_bytes()).await?;
        stream.shutdown().await
    }
    .await;
    if let Err(error) = result {
        tracing::warn!("could not answer the browser: {error}");
    }
}

async fn exchange(
    client_id: String,
    code: String,
    verifier: PkceCodeVerifier,
) -> Result<String, SignInError> {
    let task = tokio::task::spawn_blocking(move || {
        let http = oauth2::reqwest::blocking::Client::new();
        client(&client_id)
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(verifier)
            .request(&http)
            .map(|token| token.access_token().secret().to_string())
            .map_err(|error| SignInError::Exchange(error.to_string()))
    });

    match tokio::time::timeout(EXCHANGE_TIMEOUT, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(join)) => Err(SignInError::Exchange(join.to_string())),
        Err(_) => Err(SignInError::Exchange(
            "Spotify took too long to answer".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_idle_connection_does_not_block_the_redirect() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (_, verifier) = PkceCodeChallenge::new_random_sha256();
        let request = AuthRequest {
            url: String::new(),
            verifier,
            state: CsrfToken::new("x".into()),
            client_id: "id".into(),
        };
        let sign_in = tokio::spawn(sign_in(listener, request));

        let _idle = TcpStream::connect(addr).await.unwrap();
        let mut real = TcpStream::connect(addr).await.unwrap();
        real.write_all(b"GET /login?error=access_denied&state=x HTTP/1.1\r\n\r\n")
            .await
            .unwrap();

        let result = tokio::time::timeout(Duration::from_secs(10), sign_in)
            .await
            .expect("sign-in stalled on the idle connection")
            .unwrap();
        assert!(matches!(result, Err(SignInError::Cancelled)));
    }

    #[test]
    fn code_is_read_from_login_redirect() {
        assert_eq!(
            parse_callback("GET /login?code=abc&state=x HTTP/1.1", "x"),
            Callback::Code("abc".into())
        );
    }

    #[test]
    fn error_param_means_denied() {
        assert_eq!(
            parse_callback("GET /login?error=access_denied&state=x HTTP/1.1", "x"),
            Callback::Denied
        );
    }

    #[test]
    fn wrong_or_missing_state_is_ignored() {
        for line in [
            "GET /login?code=abc&state=y HTTP/1.1",
            "GET /login?code=abc HTTP/1.1",
            "GET /login?error=access_denied HTTP/1.1",
        ] {
            assert_eq!(parse_callback(line, "x"), Callback::NotLogin, "{line}");
        }
    }

    #[test]
    fn non_login_paths_are_ignored() {
        assert_eq!(
            parse_callback("GET /favicon.ico HTTP/1.1", "x"),
            Callback::NotLogin
        );
        assert!(response(None).starts_with("HTTP/1.1 404"));
    }

    #[test]
    fn authorize_url_forces_the_dialog() {
        let url = authorize("id").url;
        assert!(url.contains("show_dialog=true"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A8898%2Flogin"));
    }
}
