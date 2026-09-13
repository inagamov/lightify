use crate::spotify::api::ApiError;
use crate::spotify::library::TracksPage;
use crate::spotify::model::{Playlist, Track};
use crate::spotify::player::PlayerUpdate;

#[derive(Debug)]
pub enum Message {
    Playlists(Result<Vec<Playlist>, ApiError>),
    Tracks {
        playlist_id: String,
        result: Result<TracksPage, ApiError>,
    },
    MoreTracks {
        playlist_id: String,
        result: Result<Vec<Track>, ApiError>,
    },
    Player(PlayerUpdate),
    Tick,
}
