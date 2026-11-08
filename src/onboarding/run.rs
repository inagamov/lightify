use std::io::Write;
use std::time::{Duration, Instant};

use anyhow::Context;
use base64::Engine;
use crossterm::event::{
    Event as TermEvent, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use futures::StreamExt;
use librespot::core::SessionConfig;
use librespot::core::authentication::Credentials;
use librespot::core::cache::Cache;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::message::Message;
use crate::onboarding::{Effect, Event, Key, Onboarding, update};
use crate::spotify::oauth;
use crate::spotify::player::{self, PlayerError, PlayerHandle};
use crate::spotify::session::SessionHandle;
use crate::theme::Theme;
use crate::ui;

const TICK: Duration = Duration::from_millis(100);

pub struct Ready {
    pub session: SessionHandle,
    pub player: PlayerHandle,
}

enum Internal {
    SignIn { attempt: u64, event: Event },
    Copied,
    Player(Result<PlayerHandle, PlayerError>),
}

pub fn key(key: KeyEvent) -> Option<Key> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Key::Quit),
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Char('c') => Some(Key::Copy),
        KeyCode::Char('o') => Some(Key::Open),
        KeyCode::Esc => Some(Key::Back),
        KeyCode::Char('q') => Some(Key::Quit),
        _ => None,
    }
}

pub async fn run(
    terminal: &mut DefaultTerminal,
    theme: &Theme,
    cache: Cache,
    tx: UnboundedSender<Message>,
) -> anyhow::Result<Option<Ready>> {
    let config = SessionConfig::default();
    let client_id = config.client_id.clone();
    let session = SessionHandle::new(config, cache.clone());

    let (mut onboarding, mut pending) = Onboarding::new(cache.credentials());
    let (events_tx, mut events) = mpsc::unbounded_channel::<Internal>();
    let mut keys = EventStream::new();
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut sign_in: Option<JoinHandle<()>> = None;
    let mut player: Option<PlayerHandle> = None;
    let mut attempts = 0usize;
    let mut sign_in_attempt = 0u64;

    loop {
        for effect in pending.drain(..) {
            match effect {
                Effect::StartSignIn => {
                    stop(&mut sign_in).await;
                    sign_in_attempt += 1;
                    sign_in = Some(spawn_sign_in(
                        client_id.clone(),
                        sign_in_attempt,
                        events_tx.clone(),
                    ));
                }
                Effect::CancelSignIn => stop(&mut sign_in).await,
                Effect::OpenBrowser(url) => {
                    open::that_in_background(url);
                }
                Effect::CopyLink(url) => {
                    if copy(&url) {
                        let _ = events_tx.send(Internal::Copied);
                    }
                }
                Effect::StartPlayer(credentials) => {
                    if attempts > 0 {
                        session.reconnect();
                    }
                    attempts += 1;
                    spawn_player(session.clone(), credentials, tx.clone(), events_tx.clone());
                }
                Effect::Finish => {
                    let player = player
                        .take()
                        .context("onboarding finished without a player")?;
                    return Ok(Some(Ready { session, player }));
                }
                Effect::Quit => {
                    stop(&mut sign_in).await;
                    if let Some(player) = player.take() {
                        player.shutdown().await;
                    }
                    session.shutdown();
                    return Ok(None);
                }
            }
        }

        terminal.draw(|frame| ui::onboarding::draw(frame, &onboarding, theme))?;

        let event = tokio::select! {
            maybe_event = keys.next() => match maybe_event {
                Some(Ok(TermEvent::Key(pressed))) => match key(pressed) {
                    Some(key) => Event::Key(key),
                    None => continue,
                },
                Some(Ok(_)) => continue,
                Some(Err(error)) => return Err(error.into()),
                None => Event::Key(Key::Quit),
            },
            Some(internal) = events.recv() => match internal {
                Internal::SignIn { attempt, .. } if attempt != sign_in_attempt => continue,
                Internal::SignIn { event, .. } => event,
                Internal::Copied => Event::LinkCopied,
                Internal::Player(Ok(handle)) => {
                    player = Some(handle);
                    Event::PlayerStarted(Ok(()))
                }
                Internal::Player(Err(error)) => {
                    tracing::error!("could not start the player: {error}");
                    Event::PlayerStarted(Err(error.failure()))
                }
            },
            _ = tick.tick() => Event::Tick(Instant::now()),
        };

        pending = update(&mut onboarding, event);
    }
}

/// aborts sign-in task and waits until it's gone, so its listener has released the port
async fn stop(task: &mut Option<JoinHandle<()>>) {
    if let Some(task) = task.take() {
        task.abort();
        let _ = task.await;
    }
}

fn spawn_sign_in(
    client_id: String,
    attempt: u64,
    events: UnboundedSender<Internal>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = async {
            let listener = oauth::listen().await?;
            let request = oauth::authorize(&client_id);
            let _ = events.send(Internal::SignIn {
                attempt,
                event: Event::Listening {
                    url: request.url.clone(),
                },
            });
            oauth::sign_in(listener, request).await
        }
        .await;
        if let Err(error) = &result {
            tracing::warn!("sign-in failed: {error}");
        }
        let _ = events.send(Internal::SignIn {
            attempt,
            event: Event::SignInFinished(result.map(Credentials::with_access_token)),
        });
    })
}

fn spawn_player(
    session: SessionHandle,
    credentials: Credentials,
    tx: UnboundedSender<Message>,
    events: UnboundedSender<Internal>,
) {
    tokio::spawn(async move {
        let result = tokio::spawn(player::start(session, credentials, tx))
            .await
            .unwrap_or_else(|join| Err(PlayerError::Crashed(join.to_string())));
        let _ = events.send(Internal::Player(result));
    });
}

/// copies through pbcopy on macOS, otherwise asks the terminal to via OSC 52
/// returns whether the copy went out
/// a terminal that ignores OSC 52 can't be detected
fn copy(text: &str) -> bool {
    if cfg!(target_os = "macos") {
        match copy_with_pbcopy(text) {
            Ok(()) => return true,
            Err(error) => tracing::warn!("pbcopy failed, trying the terminal: {error}"),
        }
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut stdout = std::io::stdout();
    let result = write!(stdout, "\x1b]52;c;{encoded}\x07").and_then(|()| stdout.flush());
    if let Err(error) = &result {
        tracing::warn!("could not copy the link: {error}");
    }
    result.is_ok()
}

fn copy_with_pbcopy(text: &str) -> std::io::Result<()> {
    use std::process::{Command, Stdio};

    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "pbcopy exited with {status}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_quits() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key(ctrl_c), Some(Key::Quit));
        assert_eq!(key(KeyEvent::from(KeyCode::Char('c'))), Some(Key::Copy));
    }
}
