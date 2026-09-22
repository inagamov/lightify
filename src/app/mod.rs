use std::time::Instant;

use ratatui::widgets::{ListState, TableState};

use crate::action::Action;
use crate::message::Message;
use crate::spotify::library::PAGE_SIZE;
use crate::spotify::model::{LibraryItem, Source, Track};
use crate::spotify::player::{DEFAULT_VOLUME, PlayerCommand, PlayerUpdate};
use crate::theme::Theme;

trait Selectable {
    fn selected(&self) -> Option<usize>;
    fn select(&mut self, index: Option<usize>);

    fn move_by(&mut self, len: usize, delta: isize) {
        if len == 0 {
            return;
        }
        let current = self.selected().unwrap_or(0);
        self.select(Some(current.saturating_add_signed(delta).min(len - 1)));
    }

    fn select_row(&mut self, len: usize, row: usize) {
        if len == 0 {
            return;
        }
        self.select(Some(row.saturating_sub(1).min(len - 1)));
    }
}

impl Selectable for ListState {
    fn selected(&self) -> Option<usize> {
        ListState::selected(self)
    }
    fn select(&mut self, index: Option<usize>) {
        ListState::select(self, index);
    }
}

impl Selectable for TableState {
    fn selected(&self) -> Option<usize> {
        TableState::selected(self)
    }
    fn select(&mut self, index: Option<usize>) {
        TableState::select(self, index);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Reconnecting,
    Lost,
}

#[derive(Debug)]
pub enum Input {
    Action(Action),
    Message(Message),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryRequest {
    Playlists,
    PlaylistTracks { source: Source },
    MoreTracks { source: Source, uris: Vec<String> },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    Api(LibraryRequest),
    Quit,
    Player(PlayerCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playback {
    pub track: Option<NowPlaying>,
    position_ms: u32,
    position_at: Option<Instant>,
    pub volume: u16,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            track: None,
            position_ms: 0,
            position_at: None,
            volume: DEFAULT_VOLUME,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlaying {
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: u32,
    pub uri: String,
}

impl Playback {
    pub fn current_position_ms(&self) -> u32 {
        match self.position_at {
            Some(at) => {
                let elapsed = u32::try_from(at.elapsed().as_millis()).unwrap_or(u32::MAX);
                self.position_ms.saturating_add(elapsed)
            }
            None => self.position_ms,
        }
    }

    pub fn apply(&mut self, update: PlayerUpdate) {
        match update {
            PlayerUpdate::TrackChanged {
                name,
                artists,
                album,
                duration_ms,
                uri,
            } => {
                self.track = Some(NowPlaying {
                    name,
                    artists,
                    album,
                    duration_ms,
                    uri,
                });
            }
            PlayerUpdate::Playing { position_ms } => {
                self.position_ms = position_ms;
                self.position_at = Some(Instant::now());
            }
            PlayerUpdate::Paused { position_ms } => {
                self.position_ms = position_ms;
                self.position_at = None;
            }
            PlayerUpdate::Seeked { position_ms } => {
                self.position_ms = position_ms;
                if self.position_at.is_some() {
                    self.position_at = Some(Instant::now());
                }
            }
            PlayerUpdate::Volume(volume) => self.volume = volume,
            PlayerUpdate::Stopped | PlayerUpdate::Disconnected => {
                self.track = None;
                self.position_ms = 0;
                self.position_at = None;
            }
            PlayerUpdate::Reconnected | PlayerUpdate::ConnectionLost(_) => {}
        }
    }

    pub fn is_playing(&self) -> bool {
        self.position_at.is_some()
    }
}

pub struct App {
    pub focus: Focus,
    pub pending_count: Option<usize>,
    pub connection: ConnectionStatus,
    pub library_generation: u64,
    pub playlists: Vec<LibraryItem>,
    pub sidebar: ListState,
    pub tracks: Vec<Track>,
    pub track_list: TableState,
    pub tracks_for: Option<Source>,
    pub track_uris: Vec<String>,
    pub loading_more: bool,
    pub status: Option<Status>,
    pub playback: Playback,
    pub theme: Theme,
}

impl App {
    pub fn new() -> Self {
        Self {
            focus: Focus::Sidebar,
            pending_count: None,
            connection: ConnectionStatus::Connected,
            library_generation: 0,
            playlists: Vec::new(),
            sidebar: ListState::default().with_selected(Some(0)),
            tracks: Vec::new(),
            track_list: TableState::default(),
            tracks_for: None,
            track_uris: Vec::new(),
            loading_more: false,
            status: None,
            playback: Playback::default(),
            theme: Theme::default(),
        }
    }

    pub fn push_digit(&mut self, digit: u8) {
        if digit == 0 && self.pending_count.is_none() {
            return;
        }

        let current = self.pending_count.unwrap_or(0);
        self.pending_count = Some(
            current
                .saturating_mul(10)
                .saturating_add(usize::from(digit)),
        );
    }

    pub fn selected_playlist(&self) -> Option<&LibraryItem> {
        self.sidebar.selected().and_then(|i| self.playlists.get(i))
    }

    pub fn is_showing(&self, source: &Source) -> bool {
        self.tracks_for.as_ref() == Some(source)
    }

    pub fn showing_playlist(&self) -> Option<&LibraryItem> {
        let source = self.tracks_for.as_ref()?;
        self.playlists.iter().find(|p| &p.source == source)
    }

    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    #[cfg(test)]
    pub fn status_text(&self) -> Option<&str> {
        self.status.as_ref().map(Status::text)
    }

    pub fn playing_uri(&self) -> Option<&str> {
        self.playback.track.as_ref().map(|t| t.uri.as_str())
    }
}

pub fn update(app: &mut App, input: Input) -> Vec<Effect> {
    match input {
        Input::Action(action) => update_action(app, action),
        Input::Message(message) => update_message(app, message),
    }
}

pub fn update_action(app: &mut App, action: Action) -> Vec<Effect> {
    let count = match action {
        Action::Digit(d) => {
            app.push_digit(d);
            return Vec::new();
        }
        _ => app.pending_count.take(),
    };

    match action {
        Action::MoveDown => {
            let (state, len) = focused_list(app);
            state.move_by(len, row_delta(count));
            load_more_if_near_end(app)
        }
        Action::MoveUp => {
            let (state, len) = focused_list(app);
            state.move_by(len, -row_delta(count));
            load_more_if_near_end(app)
        }
        Action::GoTop => {
            let (state, len) = focused_list(app);
            state.move_by(len, isize::MIN);
            load_more_if_near_end(app)
        }
        Action::GoBottom => {
            let (state, len) = focused_list(app);
            match count {
                Some(row) => state.select_row(len, row),
                None => state.move_by(len, isize::MAX),
            }
            load_more_if_near_end(app)
        }
        Action::FocusSidebar => {
            app.focus = Focus::Sidebar;
            Vec::new()
        }
        Action::FocusMain => {
            app.focus = Focus::Main;
            Vec::new()
        }
        Action::Select => match app.focus {
            Focus::Sidebar => select_playlist(app),
            Focus::Main => play_selected(app),
        },
        Action::PlayPause => vec![Effect::Player(PlayerCommand::PlayPause)],
        Action::Next => vec![Effect::Player(PlayerCommand::Next)],
        Action::Prev => vec![Effect::Player(PlayerCommand::Prev)],
        Action::SeekForward => {
            let target = app
                .playback
                .current_position_ms()
                .saturating_add(SEEK_STEP_MS);
            vec![Effect::Player(PlayerCommand::Seek(target))]
        }
        Action::SeekBackward => {
            let target = app
                .playback
                .current_position_ms()
                .saturating_sub(SEEK_STEP_MS);
            vec![Effect::Player(PlayerCommand::Seek(target))]
        }
        Action::VolumeUp => {
            let target = app.playback.volume.saturating_add(VOLUME_STEP);
            vec![Effect::Player(PlayerCommand::SetVolume(target))]
        }
        Action::VolumeDown => {
            let target = app.playback.volume.saturating_sub(VOLUME_STEP);
            vec![Effect::Player(PlayerCommand::SetVolume(target))]
        }
        Action::Refresh => match app.connection {
            ConnectionStatus::Connected => vec![Effect::Api(LibraryRequest::Playlists)],
            ConnectionStatus::Reconnecting => Vec::new(),
            ConnectionStatus::Lost => {
                app.connection = ConnectionStatus::Reconnecting;
                app.status = Some(Status::Info("reconnecting".to_string()));
                vec![Effect::Player(PlayerCommand::Reconnect)]
            }
        },
        Action::Quit => vec![Effect::Quit],
        Action::Digit(_) => unreachable!("digits return early above"),
    }
}

pub fn update_message(app: &mut App, message: Message) -> Vec<Effect> {
    let generation = match &message {
        Message::Playlists { generation, .. }
        | Message::Tracks { generation, .. }
        | Message::MoreTracks { generation, .. } => Some(*generation),
        _ => None,
    };
    if generation.is_some_and(|generation| generation != app.library_generation) {
        tracing::debug!("ignoring library response from an earlier connection");
        return Vec::new();
    }
    match message {
        Message::Playlists {
            result: Ok(playlists),
            ..
        } => {
            app.playlists = playlists;
            app.sidebar.select(Some(0));

            match app.playlists.first().map(|p| p.source.clone()) {
                Some(source) => request_tracks(app, source),
                None => Vec::new(),
            }
        }
        Message::Playlists {
            result: Err(error), ..
        } => {
            report_error(app, &error);
            Vec::new()
        }
        Message::Tracks { source, result, .. } => {
            if !app.is_showing(&source) {
                return Vec::new();
            }
            match result {
                Ok(page) => {
                    app.tracks = page.tracks;
                    app.track_uris = page.uris;
                    app.track_list.select(Some(0));
                }
                Err(error) => report_error(app, &error),
            }
            Vec::new()
        }
        Message::MoreTracks { source, result, .. } => {
            if !app.is_showing(&source) {
                return Vec::new();
            }
            app.loading_more = false;
            match result {
                Ok(tracks) => app.tracks.extend(tracks),
                Err(error) => report_error(app, &error),
            }
            Vec::new()
        }
        Message::Player(update) => {
            match update {
                PlayerUpdate::Disconnected => {
                    app.library_generation += 1;
                    app.loading_more = false;
                    app.connection = ConnectionStatus::Reconnecting;
                    app.status = Some(Status::Error(
                        "connection dropped, reconnecting".to_string(),
                    ));
                }
                PlayerUpdate::Reconnected => {
                    app.connection = ConnectionStatus::Connected;
                    app.status = Some(Status::Error(
                        "reconnected, press enter on a track to play".to_string(),
                    ));
                }
                PlayerUpdate::ConnectionLost(ref error) => {
                    app.connection = ConnectionStatus::Lost;
                    app.status =
                        Some(Status::Error(error.clone().unwrap_or_else(|| {
                            "connection lost, press R to reconnect".to_string()
                        })));
                }
                _ => {}
            }
            app.playback.apply(update);
            Vec::new()
        }
        Message::Tick => Vec::new(),
    }
}

fn report_error(app: &mut App, error: &librespot::core::Error) {
    if library_available(app) {
        app.status = Some(Status::Error(error.to_string()));
    } else {
        tracing::warn!("library error while not connected: {error}");
    }
}

fn library_available(app: &App) -> bool {
    app.connection == ConnectionStatus::Connected
}

fn select_playlist(app: &mut App) -> Vec<Effect> {
    if !library_available(app) {
        return Vec::new();
    }
    let Some(id) = app.selected_playlist().map(|p| p.source.clone()) else {
        return Vec::new();
    };
    app.focus = Focus::Main;
    request_tracks(app, id)
}

fn play_selected(app: &App) -> Vec<Effect> {
    let Some(start_index) = app.track_list.selected() else {
        return Vec::new();
    };
    let uris = app.track_uris.clone();
    vec![Effect::Player(PlayerCommand::Load { uris, start_index })]
}

fn request_tracks(app: &mut App, source: Source) -> Vec<Effect> {
    app.tracks.clear();
    app.track_list.select(None);
    app.track_uris.clear();
    app.tracks_for = Some(source.clone());
    app.loading_more = false;

    vec![Effect::Api(LibraryRequest::PlaylistTracks { source })]
}

fn focused_list(app: &mut App) -> (&mut dyn Selectable, usize) {
    match app.focus {
        Focus::Sidebar => (&mut app.sidebar, app.playlists.len()),
        Focus::Main => (&mut app.track_list, app.tracks.len()),
    }
}

fn row_delta(count: Option<usize>) -> isize {
    isize::try_from(count.unwrap_or(1)).unwrap_or(isize::MAX)
}

const LOAD_MORE_MARGIN: usize = 10;
const SEEK_STEP_MS: u32 = 10_000;
const VOLUME_STEP: u16 = 4096;

fn load_more_if_near_end(app: &mut App) -> Vec<Effect> {
    if app.focus != Focus::Main || app.loading_more || !library_available(app) {
        return Vec::new();
    }
    let Some(selected) = app.track_list.selected() else {
        return Vec::new();
    };
    if selected + LOAD_MORE_MARGIN < app.tracks.len() {
        return Vec::new();
    }
    let Some(source) = app.tracks_for.clone() else {
        return Vec::new();
    };
    let uris: Vec<String> = app.track_uris[app.tracks.len()..]
        .iter()
        .take(PAGE_SIZE)
        .cloned()
        .collect();
    if uris.is_empty() {
        return Vec::new();
    }

    app.loading_more = true;
    vec![Effect::Api(LibraryRequest::MoreTracks { source, uris })]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Info(String),
    Error(String),
}

impl Status {
    pub fn text(&self) -> &str {
        match self {
            Self::Info(text) | Self::Error(text) => text,
        }
    }
}

#[cfg(test)]
mod tests;
