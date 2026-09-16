use std::time::Instant;

use ratatui::widgets::ListState;

use crate::action::Action;
use crate::message::Message;
use crate::spotify::library::PAGE_SIZE;
use crate::spotify::model::{Playlist, Track};
use crate::spotify::player::{PlayerCommand, PlayerUpdate};
use crate::theme::Theme;

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
    PlaylistTracks {
        id: String,
    },
    MoreTracks {
        playlist_id: String,
        uris: Vec<String>,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    Api(LibraryRequest),
    Quit,
    Player(PlayerCommand),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Playback {
    pub track: Option<NowPlaying>,
    position_ms: u32,
    position_at: Option<Instant>,
    pub volume: u16,
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
    pub connection: ConnectionStatus,
    pub library_generation: u64,
    pub playlists: Vec<Playlist>,
    pub sidebar: ListState,
    pub tracks: Vec<Track>,
    pub track_list: ListState,
    pub tracks_for: Option<String>,
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
            connection: ConnectionStatus::Connected,
            library_generation: 0,
            playlists: Vec::new(),
            sidebar: ListState::default().with_selected(Some(0)),
            tracks: Vec::new(),
            track_list: ListState::default(),
            tracks_for: None,
            track_uris: Vec::new(),
            loading_more: false,
            status: None,
            playback: Playback::default(),
            theme: Theme::default(),
        }
    }

    pub fn selected_playlist(&self) -> Option<&Playlist> {
        self.sidebar.selected().and_then(|i| self.playlists.get(i))
    }

    pub fn is_showing(&self, playlist_id: &str) -> bool {
        self.tracks_for.as_deref() == Some(playlist_id)
    }

    pub fn showing_playlist(&self) -> Option<&Playlist> {
        let id = self.tracks_for.as_deref()?;
        self.playlists.iter().find(|p| p.id == id)
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
    match action {
        Action::MoveDown => {
            let (state, len) = focused_list(app);
            move_selection(state, len, 1);
            load_more_if_near_end(app)
        }
        Action::MoveUp => {
            let (state, len) = focused_list(app);
            move_selection(state, len, -1);
            load_more_if_near_end(app)
        }
        Action::GoTop => {
            let (state, len) = focused_list(app);
            move_selection(state, len, isize::MIN);
            load_more_if_near_end(app)
        }
        Action::GoBottom => {
            let (state, len) = focused_list(app);
            move_selection(state, len, isize::MAX);
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

            match app.playlists.first().map(|p| p.id.clone()) {
                Some(id) => request_tracks(app, id),
                None => Vec::new(),
            }
        }
        Message::Playlists {
            result: Err(error), ..
        } => {
            report_error(app, &error);
            Vec::new()
        }
        Message::Tracks {
            playlist_id,
            result,
            ..
        } => {
            if !app.is_showing(&playlist_id) {
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
        Message::MoreTracks {
            playlist_id,
            result,
            ..
        } => {
            if !app.is_showing(&playlist_id) {
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
    let Some(id) = app.selected_playlist().map(|p| p.id.clone()) else {
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

fn request_tracks(app: &mut App, id: String) -> Vec<Effect> {
    app.tracks.clear();
    app.track_list.select(None);
    app.track_uris.clear();
    app.tracks_for = Some(id.clone());
    app.loading_more = false;

    vec![Effect::Api(LibraryRequest::PlaylistTracks { id })]
}

fn focused_list(app: &mut App) -> (&mut ListState, usize) {
    match app.focus {
        Focus::Sidebar => (&mut app.sidebar, app.playlists.len()),
        Focus::Main => (&mut app.track_list, app.tracks.len()),
    }
}

fn move_selection(state: &mut ListState, len: usize, delta: isize) {
    if len == 0 {
        return;
    }
    let current = state.selected().unwrap_or(0);
    let target = current.saturating_add_signed(delta).min(len - 1);
    state.select(Some(target));
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
    let Some(playlist_id) = app.tracks_for.clone() else {
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
    vec![Effect::Api(LibraryRequest::MoreTracks {
        playlist_id,
        uris,
    })]
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
mod tests {
    use super::*;
    use crate::spotify::library::TracksPage;
    use crate::spotify::model::Playlist;
    use crate::spotify::player::PlayerCommand;

    fn playlist(id: &str, name: &str) -> Playlist {
        Playlist {
            id: id.to_string(),
            name: name.to_string(),
            track_count: 0,
        }
    }

    fn player(update: PlayerUpdate) -> Input {
        Input::Message(Message::Player(update))
    }

    #[test]
    fn disconnect_clears_playback_and_starts_reconnecting() {
        let mut app = App::new();
        update(
            &mut app,
            player(PlayerUpdate::TrackChanged {
                name: "t".into(),
                artists: vec![],
                album: "".into(),
                duration_ms: 1,
                uri: "spotify:track:t".into(),
            }),
        );
        update(&mut app, player(PlayerUpdate::Playing { position_ms: 0 }));
        update(&mut app, player(PlayerUpdate::Disconnected));
        assert_eq!(app.connection, ConnectionStatus::Reconnecting);
        assert_eq!(app.playback.track, None);
        assert!(!app.playback.is_playing());
        assert_eq!(app.status_text(), Some("connection dropped, reconnecting"));
    }

    #[test]
    fn reconnected_restores_connected() {
        let mut app = App::new();
        update(&mut app, player(PlayerUpdate::Disconnected));
        assert!(update(&mut app, player(PlayerUpdate::Reconnected)).is_empty());
        assert_eq!(app.connection, ConnectionStatus::Connected);
        assert!(!app.playback.is_playing());
        assert_eq!(app.playback.track, None);
        assert_eq!(
            app.status_text(),
            Some("reconnected, press enter on a track to play")
        );
    }

    #[test]
    fn connection_lost_is_shown() {
        let mut app = App::new();
        update(&mut app, player(PlayerUpdate::ConnectionLost(None)));
        assert_eq!(app.connection, ConnectionStatus::Lost);
        assert_eq!(
            app.status_text(),
            Some("connection lost, press R to reconnect")
        );
    }

    #[test]
    fn missing_credentials_shows_login_instructions() {
        let mut app = App::new();
        let error = crate::spotify::player::PlayerError::NoCredentials.to_string();
        update(&mut app, player(PlayerUpdate::ConnectionLost(Some(error))));
        assert_eq!(app.connection, ConnectionStatus::Lost);
        assert_eq!(
            app.status_text(),
            Some("no cached credentials; restart lightify to log in again")
        );
    }

    #[test]
    fn refresh_while_lost_reconnects() {
        let mut app = App::new();
        update(&mut app, player(PlayerUpdate::ConnectionLost(None)));
        let effects = update(&mut app, Input::Action(Action::Refresh));
        assert_eq!(effects, vec![Effect::Player(PlayerCommand::Reconnect)]);
        assert_eq!(app.connection, ConnectionStatus::Reconnecting);
        assert_eq!(app.status_text(), Some("reconnecting"));
    }

    #[test]
    fn refresh_while_reconnecting_does_nothing() {
        let mut app = App::new();
        update(&mut app, player(PlayerUpdate::Disconnected));
        assert_eq!(update(&mut app, Input::Action(Action::Refresh)), vec![]);
    }

    #[test]
    fn select_playlist_while_not_connected_does_nothing() {
        for connection_update in [
            PlayerUpdate::Disconnected,
            PlayerUpdate::ConnectionLost(None),
        ] {
            let mut app = app_with_tracks(2);
            update(&mut app, Input::Action(Action::FocusSidebar));
            update(&mut app, player(connection_update));
            assert_eq!(update(&mut app, Input::Action(Action::Select)), vec![]);
            assert_eq!(app.focus, Focus::Sidebar);
            assert_eq!(app.tracks.len(), 2);
        }
    }

    #[test]
    fn load_more_while_not_connected_does_nothing() {
        let mut app = app_with_playlists(1);
        update(&mut app, Input::Action(Action::Select));
        update(
            &mut app,
            Input::Message(Message::Tracks {
                generation: 0,
                playlist_id: "p0".into(),
                result: Ok(page(vec![track("t0")], PAGE_SIZE + 1)),
            }),
        );
        update(&mut app, player(PlayerUpdate::Disconnected));
        assert_eq!(update(&mut app, Input::Action(Action::GoBottom)), vec![]);
        assert!(!app.loading_more);
    }

    #[test]
    fn stale_library_responses_after_reconnect_are_ignored() {
        let mut app = app_with_tracks(1);
        let generation = app.library_generation;
        app.loading_more = true;
        update(&mut app, player(PlayerUpdate::Disconnected));
        assert!(!app.loading_more);
        update(&mut app, player(PlayerUpdate::Reconnected));
        let status = app.status.clone();

        app.loading_more = true;
        let error = || librespot::core::Error::unavailable("late failure");
        for message in [
            Message::Playlists {
                generation,
                result: Err(error()),
            },
            Message::Tracks {
                generation,
                playlist_id: "p0".into(),
                result: Err(error()),
            },
            Message::MoreTracks {
                generation,
                playlist_id: "p0".into(),
                result: Err(error()),
            },
            Message::Playlists {
                generation,
                result: Ok(vec![]),
            },
            Message::Tracks {
                generation,
                playlist_id: "p0".into(),
                result: Ok(page(vec![], 0)),
            },
            Message::MoreTracks {
                generation,
                playlist_id: "p0".into(),
                result: Ok(vec![track("stale")]),
            },
        ] {
            assert!(update(&mut app, Input::Message(message)).is_empty());
            assert_eq!(app.status, status);
            assert_eq!(app.playlists.len(), 1);
            assert_eq!(app.tracks.len(), 1);
            assert!(app.loading_more);
        }

        let generation = app.library_generation;
        update(
            &mut app,
            Input::Message(Message::Playlists {
                generation,
                result: Err(error()),
            }),
        );
        assert!(app.status_text().unwrap().contains("late failure"));
    }

    #[test]
    fn library_errors_do_not_hide_reconnect_instructions() {
        for connection_update in [
            PlayerUpdate::Disconnected,
            PlayerUpdate::ConnectionLost(None),
        ] {
            let mut app = app_with_tracks(1);
            update(&mut app, player(connection_update));
            let status = app.status.clone();
            let error = || librespot::core::Error::unavailable("boom");
            for message in [
                Message::Playlists {
                    generation: 0,
                    result: Err(error()),
                },
                Message::Tracks {
                    generation: 0,
                    playlist_id: "p0".into(),
                    result: Err(error()),
                },
                Message::MoreTracks {
                    generation: 0,
                    playlist_id: "p0".into(),
                    result: Err(error()),
                },
            ] {
                update(&mut app, Input::Message(message));
                assert_eq!(app.status, status);
            }
        }
    }

    fn app_with_playlists(n: usize) -> App {
        let mut app = App::new();
        let lists = (0..n)
            .map(|i| playlist(&format!("p{i}"), &format!("Playlist {i}")))
            .collect();
        update(
            &mut app,
            Input::Message(Message::Playlists {
                generation: 0,
                result: Ok(lists),
            }),
        );
        app
    }

    fn page(tracks: Vec<Track>, total: usize) -> TracksPage {
        let uris = (0..total).map(|i| format!("spotify:track:t{i}")).collect();
        TracksPage { uris, tracks }
    }

    fn track(name: &str) -> Track {
        Track {
            uri: format!("spotify:track:{name}"),
            name: name.to_string(),
            duration_ms: 0,
            artists: Vec::new(),
            album: String::new(),
        }
    }

    fn app_with_tracks(n: usize) -> App {
        let mut app = app_with_playlists(1);
        update(&mut app, Input::Action(Action::Select));
        let tracks = (0..n).map(|i| track(&format!("t{i}"))).collect();
        update(
            &mut app,
            Input::Message(Message::Tracks {
                generation: 0,
                playlist_id: "p0".into(),
                result: Ok(page(tracks, n)),
            }),
        );
        app
    }

    #[test]
    fn nearing_the_end_of_tracks_requests_the_next_page_once() {
        let mut app = app_with_playlists(1);
        update(&mut app, Input::Action(Action::Select));
        let tracks = (0..50).map(|i| track(&format!("t{i}"))).collect();
        update(
            &mut app,
            Input::Message(Message::Tracks {
                generation: 0,
                playlist_id: "p0".into(),
                result: Ok(page(tracks, 51)),
            }),
        );

        assert_eq!(update(&mut app, Input::Action(Action::MoveDown)), vec![]);

        assert_eq!(
            update(&mut app, Input::Action(Action::GoBottom)),
            vec![Effect::Api(LibraryRequest::MoreTracks {
                playlist_id: "p0".into(),
                uris: vec!["spotify:track:t50".into()],
            })]
        );

        assert_eq!(update(&mut app, Input::Action(Action::MoveUp)), vec![]);

        update(
            &mut app,
            Input::Message(Message::MoreTracks {
                generation: 0,
                playlist_id: "p0".into(),
                result: Ok(vec![track("t50")]),
            }),
        );
        assert_eq!(app.tracks.len(), 51);
        assert_eq!(update(&mut app, Input::Action(Action::GoBottom)), vec![]);
    }

    #[test]
    fn refresh_requests_playlists() {
        let mut app = App::new();
        let effects = update(&mut app, Input::Action(Action::Refresh));
        assert_eq!(effects, vec![Effect::Api(LibraryRequest::Playlists)]);
    }

    #[test]
    fn quit_produces_quit_effect() {
        let mut app = App::new();
        assert_eq!(
            update(&mut app, Input::Action(Action::Quit)),
            vec![Effect::Quit]
        );
    }

    #[test]
    fn playlists_loaded_selects_first_and_requests_its_tracks() {
        let mut app = App::new();
        let lists = vec![playlist("p0", "First"), playlist("p1", "Second")];
        let effects = update(
            &mut app,
            Input::Message(Message::Playlists {
                generation: 0,
                result: Ok(lists),
            }),
        );
        assert_eq!(app.playlists.len(), 2);
        assert_eq!(app.sidebar.selected(), Some(0));
        assert_eq!(
            effects,
            vec![Effect::Api(LibraryRequest::PlaylistTracks {
                id: "p0".into()
            })]
        );
    }

    #[test]
    fn move_down_in_sidebar_stops_at_last_item() {
        let mut app = app_with_playlists(2);
        update(&mut app, Input::Action(Action::MoveDown));
        assert_eq!(app.sidebar.selected(), Some(1));
        update(&mut app, Input::Action(Action::MoveDown));
        assert_eq!(app.sidebar.selected(), Some(1));
    }

    #[test]
    fn move_up_in_sidebar_stops_at_first_item() {
        let mut app = app_with_playlists(2);
        update(&mut app, Input::Action(Action::MoveUp));
        assert_eq!(app.sidebar.selected(), Some(0));
    }

    #[test]
    fn go_top_and_go_bottom() {
        let mut app = app_with_playlists(5);
        update(&mut app, Input::Action(Action::GoBottom));
        assert_eq!(app.sidebar.selected(), Some(4));
        update(&mut app, Input::Action(Action::GoTop));
        assert_eq!(app.sidebar.selected(), Some(0));
    }

    #[test]
    fn motions_on_empty_list_do_nothing() {
        let mut app = App::new();
        update(&mut app, Input::Action(Action::MoveDown));
        update(&mut app, Input::Action(Action::GoBottom));
        assert_eq!(app.sidebar.selected(), Some(0));
    }

    #[test]
    fn select_in_sidebar_requests_that_playlists_tracks_and_focuses_main() {
        let mut app = app_with_playlists(3);
        update(&mut app, Input::Action(Action::MoveDown));
        let effects = update(&mut app, Input::Action(Action::Select));
        assert_eq!(
            effects,
            vec![Effect::Api(LibraryRequest::PlaylistTracks {
                id: "p1".into()
            })]
        );
        assert_eq!(app.focus, Focus::Main);
        assert_eq!(app.tracks_for.as_deref(), Some("p1"));
    }

    #[test]
    fn tracks_loaded_replace_the_list_and_keep_all_uris() {
        let mut app = app_with_playlists(1);
        update(&mut app, Input::Action(Action::Select));
        update(
            &mut app,
            Input::Message(Message::Tracks {
                generation: 0,
                playlist_id: "p0".into(),
                result: Ok(page(vec![], 3)),
            }),
        );
        assert_eq!(app.track_uris.len(), 3);
        assert_eq!(app.track_list.selected(), Some(0));
    }

    #[test]
    fn tracks_loaded_for_a_stale_playlist_are_ignored() {
        let mut app = app_with_playlists(2);
        update(&mut app, Input::Action(Action::Select));
        update(&mut app, Input::Action(Action::FocusSidebar));
        update(&mut app, Input::Action(Action::MoveDown));
        update(&mut app, Input::Action(Action::Select));
        update(
            &mut app,
            Input::Message(Message::Tracks {
                generation: 0,
                playlist_id: "p0".into(),
                result: Ok(page(vec![], 3)),
            }),
        );
        assert_eq!(app.tracks_for.as_deref(), Some("p1"));
        assert!(app.track_uris.is_empty());
    }

    #[test]
    fn library_error_goes_to_status_line() {
        let mut app = App::new();
        let err = librespot::core::Error::unavailable("boom");
        update(
            &mut app,
            Input::Message(Message::Playlists {
                generation: 0,
                result: Err(err),
            }),
        );
        assert!(app.status_text().unwrap_or("").contains("boom"));
    }

    #[test]
    fn focus_switching() {
        let mut app = App::new();
        update(&mut app, Input::Action(Action::FocusMain));
        assert_eq!(app.focus, Focus::Main);
        update(&mut app, Input::Action(Action::FocusSidebar));
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn select_track_loads_whole_list_from_that_index() {
        let mut app = app_with_tracks(3);
        update(&mut app, Input::Action(Action::MoveDown));
        let effects = update(&mut app, Input::Action(Action::Select));
        assert_eq!(
            effects,
            vec![Effect::Player(PlayerCommand::Load {
                uris: vec![
                    "spotify:track:t0".into(),
                    "spotify:track:t1".into(),
                    "spotify:track:t2".into(),
                ],
                start_index: 1,
            })]
        );
    }

    #[test]
    fn play_pause_is_forwarded() {
        let mut app = App::new();
        let effects = update(&mut app, Input::Action(Action::PlayPause));
        assert_eq!(effects, vec![Effect::Player(PlayerCommand::PlayPause)]);
    }

    #[test]
    fn seek_forward_uses_current_position() {
        let mut app = App::new();
        update(
            &mut app,
            Input::Message(Message::Player(PlayerUpdate::TrackChanged {
                name: "t".into(),
                artists: vec![],
                album: "".into(),
                duration_ms: 100_000,
                uri: "spotify:track:t".into(),
            })),
        );
        update(
            &mut app,
            Input::Message(Message::Player(PlayerUpdate::Paused {
                position_ms: 30_000,
            })),
        );
        let effects = update(&mut app, Input::Action(Action::SeekForward));
        assert_eq!(effects, vec![Effect::Player(PlayerCommand::Seek(40_000))]);
    }

    #[test]
    fn playing_update_starts_the_clock() {
        let mut app = App::new();
        update(
            &mut app,
            Input::Message(Message::Player(PlayerUpdate::Playing {
                position_ms: 5_000,
            })),
        );
        assert!(app.playback.position_at.is_some());
        assert!(app.playback.current_position_ms() >= 5_000);
    }

    #[test]
    fn paused_update_freezes_position() {
        let mut app = App::new();
        update(
            &mut app,
            Input::Message(Message::Player(PlayerUpdate::Paused { position_ms: 5_000 })),
        );
        assert_eq!(app.playback.position_at, None);
        assert_eq!(app.playback.current_position_ms(), 5_000);
    }

    #[test]
    fn tick_produces_no_effects() {
        let mut app = App::new();
        assert!(update(&mut app, Input::Message(Message::Tick)).is_empty());
    }

    #[test]
    fn library_errors_are_error_status() {
        let mut app = App::new();
        report_error(&mut app, &librespot::core::Error::unavailable("boom"));
        assert!(matches!(app.status, Some(Status::Error(_))));
        assert!(app.status_text().unwrap_or("").contains("boom"));
    }

    #[test]
    fn reconnecting_is_info_status() {
        let mut app = App::new();
        app.connection = ConnectionStatus::Lost;
        update(&mut app, Input::Action(Action::Refresh));
        assert_eq!(app.status, Some(Status::Info("reconnecting".to_string())));
    }

    #[test]
    fn playing_uri_follows_track_changed() {
        let mut app = App::new();
        assert_eq!(app.playing_uri(), None);
        update(
            &mut app,
            Input::Message(Message::Player(PlayerUpdate::TrackChanged {
                uri: "spotify:track:abc".to_string(),
                name: "Song".to_string(),
                artists: vec![],
                album: String::new(),
                duration_ms: 1000,
            })),
        );
        assert_eq!(app.playing_uri(), Some("spotify:track:abc"));
    }
}
