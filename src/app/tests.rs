use super::*;
use crate::spotify::library::TracksPage;
use crate::spotify::model::LibraryItem;
use crate::spotify::player::PlayerCommand;

fn playlist(source: &str, name: &str) -> LibraryItem {
    LibraryItem {
        source: Source::Playlist(source.to_string()),
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
}

#[test]
fn refresh_while_lost_reconnects() {
    let mut app = App::new();
    update(&mut app, player(PlayerUpdate::ConnectionLost(None)));
    let effects = update(&mut app, Input::Action(Action::Refresh));
    assert_eq!(effects, vec![Effect::Player(PlayerCommand::Reconnect)]);
    assert_eq!(app.connection, ConnectionStatus::Reconnecting);
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
            source: Source::Playlist("p0".into()),
            result: Err(error()),
        },
        Message::MoreTracks {
            generation,
            source: Source::Playlist("p0".into()),
            result: Err(error()),
        },
        Message::Playlists {
            generation,
            result: Ok(vec![]),
        },
        Message::Tracks {
            generation,
            source: Source::Playlist("p0".into()),
            result: Ok(page(vec![], 0)),
        },
        Message::MoreTracks {
            generation,
            source: Source::Playlist("p0".into()),
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
            source: Source::Playlist("p0".into()),
            result: Ok(page(tracks, n)),
        }),
    );
    app
}

#[test]
fn normal_actions_are_ignored_while_searching() {
    let mut app = app_with_tracks(3);
    update(&mut app, Input::Action(Action::FocusSearch));

    for action in [
        Action::Select,
        Action::MoveDown,
        Action::GoTop,
        Action::FocusMain,
    ] {
        assert_eq!(update(&mut app, Input::Action(action)), Vec::new());
    }
    assert_eq!(app.focus, Focus::Search);
}
