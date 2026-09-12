mod action;
mod app;
mod config;
mod message;
mod spotify;
mod ui;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::{self, UnboundedSender};

use action::Action;
use app::{ApiRequest, App, Effect, Input, update};
use message::Message;
use spotify::api::SpotifyApi;

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

    let login = spotify::auth::login(&cache_dir, &cfg.client_id).await?;
    spotify::auth::connect(&login.session, login.credentials).await?;

    let api = SpotifyApi::new(cfg.client_id.clone(), cache_dir, login.web_token);

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, api).await;
    ratatui::restore();
    login.session.shutdown();
    result
}

async fn run(terminal: &mut DefaultTerminal, api: SpotifyApi) -> anyhow::Result<()> {
    let mut app = App::new();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    let mut events = EventStream::new();

    let mut pending = update(&mut app, Input::Action(Action::Refresh));

    loop {
        for effect in pending.drain(..) {
            match effect {
                Effect::Quit => return Ok(()),
                Effect::Api(request) => spawn_api(request, api.clone(), tx.clone()),
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
