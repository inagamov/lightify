mod action;
mod app;
mod config;
mod message;
mod spotify;
mod theme;
mod ui;

use std::time::Duration;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::MissedTickBehavior;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;

use crate::app::Status;
use crate::spotify::library::Library;
use crate::spotify::player::PlayerCommand;
use crate::theme::Theme;
use action::Action;
use app::{App, Effect, Input, LibraryRequest, update};
use message::Message;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cache_dir = config::cache_dir();
    std::fs::create_dir_all(&cache_dir)?;

    let log_file = tracing_appender::rolling::never(&cache_dir, "lightify.log");
    let (writer, _guard) = tracing_appender::non_blocking(log_file);
    let filter = Targets::new()
        .with_default(Level::INFO)
        .with_target("librespot_core::session", Level::TRACE)
        .with_target("symphonia_bundle_mp3", Level::ERROR);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false),
        )
        .with(filter)
        .init();
    tracing::info!("lightify starting");

    let config = config::Config::load()?;

    let login = spotify::auth::login(&cache_dir).await?;

    let (tx, rx) = mpsc::unbounded_channel::<Message>();

    let player =
        spotify::player::start(login.session.clone(), login.credentials, tx.clone()).await?;

    let mut terminal = ratatui::init();
    let library = Library::new(login.session.clone());
    let result = run(
        &mut terminal,
        library,
        player.commands.clone(),
        tx,
        rx,
        config.theme,
    )
    .await;
    ratatui::restore();

    player.shutdown().await;
    login.session.shutdown();
    result
}

async fn run(
    terminal: &mut DefaultTerminal,
    library: Library,
    player: UnboundedSender<PlayerCommand>,
    tx: UnboundedSender<Message>,
    mut rx: UnboundedReceiver<Message>,
    theme: Theme,
) -> anyhow::Result<()> {
    let mut app = App::new().with_theme(theme);

    let mut events = EventStream::new();

    let mut pending = update(&mut app, Input::Action(Action::Refresh));
    let mut tick = tokio::time::interval(Duration::from_millis(250));

    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        for effect in pending.drain(..) {
            match effect {
                Effect::Quit => return Ok(()),
                Effect::Api(request) => {
                    spawn_api(request, app.library_generation, library.clone(), tx.clone())
                }
                Effect::Player(command) => {
                    send_player_command(&mut app, &player, command);
                }
            }
        }

        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        let input = tokio::select! {
            maybe_event = events.next() => match maybe_event {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    match key_to_action(key) {
                        Some(action) => Input::Action(action),
                        None => continue,
                    }
                }
                Some(Ok(_)) => continue,
                Some(Err(error)) => return Err(error.into()),
                None => return Ok(()),
            },
            Some(message) = rx.recv() => Input::Message(message),
            _ = tick.tick(), if app.playback.is_playing() => Input::Message(Message::Tick),
        };

        pending = update(&mut app, input);
    }
}

fn send_player_command(
    app: &mut App,
    player: &UnboundedSender<PlayerCommand>,
    command: PlayerCommand,
) {
    if player.send(command).is_err() {
        tracing::error!("player task is gone");
        app.connection = app::ConnectionStatus::Lost;
        app.status = Some(Status::Error("player stopped".to_string()));
    }
}

fn key_to_action(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        // TODO: switch to 'gg' (needs multi-key keymap)
        KeyCode::Char('g') => Some(Action::GoTop),
        KeyCode::Char('G') => Some(Action::GoBottom),
        KeyCode::Char('h') => Some(Action::FocusSidebar),
        KeyCode::Char('l') => Some(Action::FocusMain),
        KeyCode::Enter => Some(Action::Select),
        KeyCode::Char(' ') => Some(Action::PlayPause),
        KeyCode::Char('n') => Some(Action::Next),
        KeyCode::Char('N') => Some(Action::Prev),
        KeyCode::Char('>') => Some(Action::SeekForward),
        KeyCode::Char('<') => Some(Action::SeekBackward),
        // TODO: move to a group (because VolumeUp needs shift and VolumeDown does not)
        KeyCode::Char('+') => Some(Action::VolumeUp),
        KeyCode::Char('-') => Some(Action::VolumeDown),
        KeyCode::Char('R') => Some(Action::Refresh),
        KeyCode::Char('q') => Some(Action::Quit),
        _ => None,
    }
}

fn spawn_api(
    request: LibraryRequest,
    generation: u64,
    library: Library,
    tx: UnboundedSender<Message>,
) {
    tracing::info!(?request, "api request");

    tokio::spawn(async move {
        let message = match request {
            LibraryRequest::Playlists => Message::Playlists {
                generation,
                result: library.my_playlists().await,
            },
            LibraryRequest::PlaylistTracks { id } => {
                let result = library.first_page(&id).await;
                Message::Tracks {
                    generation,
                    playlist_id: id,
                    result,
                }
            }
            LibraryRequest::MoreTracks { playlist_id, uris } => {
                let result = library.track_details(uris).await;
                Message::MoreTracks {
                    generation,
                    playlist_id,
                    result,
                }
            }
        };

        let _ = tx.send(message);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use app::ConnectionStatus;

    #[test]
    fn failed_reconnect_send_leaves_refresh_retryable() {
        let (player, receiver) = mpsc::unbounded_channel();
        drop(receiver);
        let mut app = App::new();
        app.connection = ConnectionStatus::Lost;

        for _ in 0..2 {
            let effects = update(&mut app, Input::Action(Action::Refresh));
            assert_eq!(effects, vec![Effect::Player(PlayerCommand::Reconnect)]);
            assert_eq!(app.connection, ConnectionStatus::Reconnecting);

            send_player_command(&mut app, &player, PlayerCommand::Reconnect);

            assert_eq!(app.connection, ConnectionStatus::Lost);
            assert_eq!(app.status_text(), Some("player stopped"));
        }
    }
}
