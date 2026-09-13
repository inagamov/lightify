use ratatui::widgets::ListState;

use crate::action::Action;
use crate::message::Message;
use crate::spotify::model::{Playlist, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Main,
}

#[derive(Debug)]
pub enum Input {
    Action(Action),
    Message(Message),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiRequest {
    Playlists,
    PlaylistTracks {
        id: String,
    },
    MoreTracks {
        playlist_id: String,
        next_url: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    Api(ApiRequest),
    Quit,
}

pub struct App {
    pub focus: Focus,

    pub playlists: Vec<Playlist>,
    pub sidebar: ListState,

    pub tracks: Vec<Track>,
    pub track_list: ListState,
    pub tracks_for: Option<String>,
    pub tracks_next: Option<String>,
    pub loading_more: bool,

    pub status: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            focus: Focus::Sidebar,
            playlists: Vec::new(),

            sidebar: ListState::default().with_selected(Some(0)),
            tracks: Vec::new(),
            track_list: ListState::default(),
            tracks_for: None,
            tracks_next: None,
            loading_more: false,

            status: None,
        }
    }

    pub fn selected_playlist(&self) -> Option<&Playlist> {
        self.sidebar.selected().and_then(|i| self.playlists.get(i))
    }

    pub fn selected_track(&self) -> Option<&Track> {
        self.track_list.selected().and_then(|i| self.tracks.get(i))
    }

    pub fn is_showing(&self, playlist_id: &str) -> bool {
        self.tracks_for.as_deref() == Some(playlist_id)
    }

    pub fn showing_playlist(&self) -> Option<&Playlist> {
        let id = self.tracks_for.as_deref()?;
        self.playlists.iter().find(|p| p.id == id)
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
            Focus::Main => Vec::new(),
        },

        Action::Refresh => vec![Effect::Api(ApiRequest::Playlists)],
        Action::Quit => vec![Effect::Quit],
    }
}

pub fn update_message(app: &mut App, message: Message) -> Vec<Effect> {
    match message {
        Message::Playlists(Ok(playlists)) => {
            app.playlists = playlists;
            app.sidebar.select(Some(0));

            match app.playlists.first().map(|p| p.id.clone()) {
                Some(id) => request_tracks(app, id),
                None => Vec::new(),
            }
        }
        Message::Playlists(Err(error)) => {
            app.status = Some(error.to_string());
            Vec::new()
        }
        Message::Tracks {
            playlist_id,
            result,
        } => {
            if !app.is_showing(&playlist_id) {
                return Vec::new();
            }
            match result {
                Ok(page) => {
                    app.tracks = page.tracks;
                    app.tracks_next = page.next;
                    app.track_list.select(Some(0));
                }
                Err(error) => app.status = Some(error.to_string()),
            }
            Vec::new()
        }
        Message::MoreTracks {
            playlist_id,
            result,
        } => {
            if !app.is_showing(&playlist_id) {
                return Vec::new();
            }
            app.loading_more = false;
            match result {
                Ok(page) => {
                    app.tracks.extend(page.tracks);
                    app.tracks_next = page.next;
                }
                Err(error) => app.status = Some(error.to_string()),
            }
            Vec::new()
        }
        Message::Player(_) => Vec::new(),
    }
}

fn select_playlist(app: &mut App) -> Vec<Effect> {
    let Some(id) = app.selected_playlist().map(|p| p.id.clone()) else {
        return Vec::new();
    };
    app.focus = Focus::Main;
    request_tracks(app, id)
}

fn request_tracks(app: &mut App, id: String) -> Vec<Effect> {
    app.tracks.clear();
    app.track_list.select(None);
    app.tracks_next = None;
    app.tracks_for = Some(id.clone());
    app.loading_more = false;

    vec![Effect::Api(ApiRequest::PlaylistTracks { id })]
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

fn load_more_if_near_end(app: &mut App) -> Vec<Effect> {
    if app.focus != Focus::Main || app.loading_more {
        return Vec::new();
    }
    let Some(selected) = app.track_list.selected() else {
        return Vec::new();
    };
    if selected + LOAD_MORE_MARGIN < app.tracks.len() {
        return Vec::new();
    }
    let (Some(playlist_id), Some(next_url)) = (app.tracks_for.clone(), app.tracks_next.clone())
    else {
        return Vec::new();
    };

    app.loading_more = true;
    vec![Effect::Api(ApiRequest::MoreTracks {
        playlist_id,
        next_url,
    })]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spotify::api::{ApiError, TracksPage};
    use crate::spotify::auth::AuthError;
    use crate::spotify::model::{Playlist, TrackCount};

    fn playlist(id: &str, name: &str) -> Playlist {
        Playlist {
            id: id.to_string(),
            name: name.to_string(),
            tracks: TrackCount { total: 0 },
        }
    }

    fn app_with_playlists(n: usize) -> App {
        let mut app = App::new();
        let lists = (0..n)
            .map(|i| playlist(&format!("p{i}"), &format!("Playlist {i}")))
            .collect();
        update(&mut app, Input::Message(Message::Playlists(Ok(lists))));
        app
    }

    fn page(tracks: Vec<Track>, next: Option<&str>) -> TracksPage {
        TracksPage {
            tracks,
            next: next.map(str::to_string),
        }
    }

    fn track(name: &str) -> Track {
        Track {
            uri: format!("spotify:track:{name}"),
            name: name.to_string(),
            duration_ms: 0,
            artists: Vec::new(),
            album: None,
        }
    }

    #[test]
    fn nearing_the_end_of_tracks_requests_the_next_page_once() {
        let mut app = app_with_playlists(1);
        update(&mut app, Input::Action(Action::Select));
        let tracks = (0..50).map(|i| track(&format!("t{i}"))).collect();
        update(
            &mut app,
            Input::Message(Message::Tracks {
                playlist_id: "p0".into(),
                result: Ok(page(tracks, Some("https://page2"))),
            }),
        );

        assert_eq!(update(&mut app, Input::Action(Action::MoveDown)), vec![]);

        assert_eq!(
            update(&mut app, Input::Action(Action::GoBottom)),
            vec![Effect::Api(ApiRequest::MoreTracks {
                playlist_id: "p0".into(),
                next_url: "https://page2".into(),
            })]
        );

        assert_eq!(update(&mut app, Input::Action(Action::MoveUp)), vec![]);

        update(
            &mut app,
            Input::Message(Message::MoreTracks {
                playlist_id: "p0".into(),
                result: Ok(page(vec![track("t50")], None)),
            }),
        );
        assert_eq!(app.tracks.len(), 51);
        assert_eq!(update(&mut app, Input::Action(Action::GoBottom)), vec![]);
    }

    #[test]
    fn refresh_requests_playlists() {
        let mut app = App::new();
        let effects = update(&mut app, Input::Action(Action::Refresh));
        assert_eq!(effects, vec![Effect::Api(ApiRequest::Playlists)]);
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
        let effects = update(&mut app, Input::Message(Message::Playlists(Ok(lists))));
        assert_eq!(app.playlists.len(), 2);
        assert_eq!(app.sidebar.selected(), Some(0));
        assert_eq!(
            effects,
            vec![Effect::Api(ApiRequest::PlaylistTracks { id: "p0".into() })]
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
            vec![Effect::Api(ApiRequest::PlaylistTracks { id: "p1".into() })]
        );
        assert_eq!(app.focus, Focus::Main);
        assert_eq!(app.tracks_for.as_deref(), Some("p1"));
    }

    #[test]
    fn tracks_loaded_replace_the_list_and_keep_the_next_url() {
        let mut app = app_with_playlists(1);
        update(&mut app, Input::Action(Action::Select));
        update(
            &mut app,
            Input::Message(Message::Tracks {
                playlist_id: "p0".into(),
                result: Ok(page(vec![], Some("https://next"))),
            }),
        );
        assert_eq!(app.tracks_next.as_deref(), Some("https://next"));
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
                playlist_id: "p0".into(),
                result: Ok(page(vec![], Some("https://stale"))),
            }),
        );
        assert_eq!(app.tracks_for.as_deref(), Some("p1"));
        assert_eq!(app.tracks_next, None);
    }

    #[test]
    fn api_error_goes_to_status_line() {
        let mut app = App::new();
        let err = ApiError::Auth(AuthError::TokenStore(std::io::Error::other("boom")));
        update(&mut app, Input::Message(Message::Playlists(Err(err))));
        assert!(app.status.as_deref().unwrap_or("").contains("boom"));
    }

    #[test]
    fn focus_switching() {
        let mut app = App::new();
        update(&mut app, Input::Action(Action::FocusMain));
        assert_eq!(app.focus, Focus::Main);
        update(&mut app, Input::Action(Action::FocusSidebar));
        assert_eq!(app.focus, Focus::Sidebar);
    }
}
