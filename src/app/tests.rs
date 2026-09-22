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
    assert_eq!(app.status_text(), Some("connection dropped, reconnecting"));
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
                source: Source::Playlist("p0".into()),
                result: Err(error()),
            },
            Message::MoreTracks {
                generation: 0,
                source: Source::Playlist("p0".into()),
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
            source: Source::Playlist("p0".into()),
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
            source: Source::Playlist("p0".into()),
            result: Ok(page(tracks, 51)),
        }),
    );

    assert_eq!(update(&mut app, Input::Action(Action::MoveDown)), vec![]);

    assert_eq!(
        update(&mut app, Input::Action(Action::GoBottom)),
        vec![Effect::Api(LibraryRequest::MoreTracks {
            source: Source::Playlist("p0".into()),
            uris: vec!["spotify:track:t50".into()],
        })]
    );

    assert_eq!(update(&mut app, Input::Action(Action::MoveUp)), vec![]);

    update(
        &mut app,
        Input::Message(Message::MoreTracks {
            generation: 0,
            source: Source::Playlist("p0".into()),
            result: Ok(vec![track("t50")]),
        }),
    );
    assert_eq!(app.tracks.len(), 51);
    assert_eq!(update(&mut app, Input::Action(Action::GoBottom)), vec![]);
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
fn motions_on_empty_list_do_nothing() {
    let mut app = App::new();
    update(&mut app, Input::Action(Action::MoveDown));
    update(&mut app, Input::Action(Action::GoBottom));
    assert_eq!(app.sidebar.selected(), Some(0));
}

#[test]
fn count_prefix_moves_down_that_many_rows() {
    let mut app = app_with_playlists(9);

    update(&mut app, Input::Action(Action::Digit(3)));
    assert_eq!(app.sidebar.selected(), Some(0));

    update(&mut app, Input::Action(Action::MoveDown));
    assert_eq!(app.sidebar.selected(), Some(3));
}

#[test]
fn count_prefix_sends_go_bottom_to_that_row() {
    let mut app = app_with_playlists(9);

    update(&mut app, Input::Action(Action::Digit(4)));
    update(&mut app, Input::Action(Action::GoBottom));
    assert_eq!(app.sidebar.selected(), Some(3));

    update(&mut app, Input::Action(Action::GoBottom));
    assert_eq!(app.sidebar.selected(), Some(8));
}

#[test]
fn an_unrelated_action_clears_the_pending_count() {
    let mut app = app_with_playlists(9);

    update(&mut app, Input::Action(Action::Digit(5)));
    update(&mut app, Input::Action(Action::PlayPause));
    update(&mut app, Input::Action(Action::MoveDown));

    assert_eq!(app.sidebar.selected(), Some(1));
}

#[test]
fn a_leading_zero_does_not_start_a_count() {
    let mut app = app_with_playlists(9);

    update(&mut app, Input::Action(Action::Digit(0)));
    assert_eq!(app.pending_count, None);

    update(&mut app, Input::Action(Action::Digit(1)));
    update(&mut app, Input::Action(Action::Digit(0)));
    assert_eq!(app.pending_count, Some(10));
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
            source: Source::Playlist("p0".into()),
            result: Ok(page(vec![], 3)),
        }),
    );
    assert_eq!(
        app.tracks_for.as_ref(),
        Some(&Source::Playlist("p1".into()))
    );
    assert!(app.track_uris.is_empty());
}
