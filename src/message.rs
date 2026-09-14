use librespot::core::Error;

use crate::spotify::library::TracksPage;
use crate::spotify::model::{Playlist, Track};
use crate::spotify::player::PlayerUpdate;

#[derive(Debug)]
pub enum Message {
    Playlists(Result<Vec<Playlist>, Error>),
    Tracks {
        playlist_id: String,
        result: Result<TracksPage, Error>,
    },
    MoreTracks {
        playlist_id: String,
        result: Result<Vec<Track>, Error>,
    },
    Player(PlayerUpdate),
    Tick,
}
