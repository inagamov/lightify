mod action;
mod app;
mod config;
mod message;
mod spotify;
mod ui;

use std::time::Duration;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use action::Action;
use app::{ApiRequest, App, Effect, Input, update};
use message::Message;
use spotify::api::SpotifyApi;

use crate::spotify::player::PlayerCommand;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cache_dir = config::cache_dir();
    std::fs::create_dir_all(&cache_dir)?;

    let log_file = tracing_appender::rolling::never(&cache_dir, "lightify.log");
    let (writer, _guard) = tracing_appender::non_blocking(log_file);
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .init();
    tracing::info!("lightify starting");

    let cfg = config::load()?;

    let playback = spotify::auth::playback_login(&cache_dir).await?;
    let web_token = spotify::auth::web_login(&cache_dir, &cfg.client_id).await?;

    let api = SpotifyApi::new(cfg.client_id.clone(), cache_dir, web_token);

    let (tx, rx) = mpsc::unbounded_channel::<Message>();

    let player =
        spotify::player::start(playback.session.clone(), playback.credentials, tx.clone()).await?;

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, api, player, tx, rx).await;
    ratatui::restore();
    playback.session.shutdown();
    result
}

async fn run(
    terminal: &mut DefaultTerminal,
    api: SpotifyApi,
    player: UnboundedSender<PlayerCommand>,
    tx: UnboundedSender<Message>,
    mut rx: UnboundedReceiver<Message>,
) -> anyhow::Result<()> {
    let mut app = App::new();

    let mut events = EventStream::new();

    let mut pending = update(&mut app, Input::Action(Action::Refresh));
    let mut tick = tokio::time::interval(Duration::from_millis(250));

    loop {
        for effect in pending.drain(..) {
            match effect {
                Effect::Quit => {
                    let _ = player.send(PlayerCommand::Shutdown);
                    return Ok(());
                }
                Effect::Api(request) => spawn_api(request, api.clone(), tx.clone()),
                Effect::Player(command) => {
                    if player.send(command).is_err() {
                        tracing::error!("player task is gone");
                        app.status = Some("player stopped".to_string());
                    }
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

fn spawn_api(request: ApiRequest, api: SpotifyApi, tx: UnboundedSender<Message>) {
    tracing::info!(?request, "api request");

    tokio::spawn(async move {
        let message = match request {
            ApiRequest::Playlists => Message::Playlists(api.my_playlists().await),
            ApiRequest::PlaylistTracks { id } => {
                let result = api.playlist_tracks(&id).await;
                Message::Tracks {
                    playlist_id: id,
                    result,
                }
            }
            ApiRequest::MoreTracks {
                playlist_id,
                next_url,
            } => {
                let result = api.next_tracks(&next_url).await;
                Message::MoreTracks {
                    playlist_id,
                    result,
                }
            }
        };

        let _ = tx.send(message);
    });
}
