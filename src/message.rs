use crate::spotify::api::{ApiError, TracksPage};
use crate::spotify::model::Playlist;
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
        result: Result<TracksPage, ApiError>,
    },
    Player(PlayerUpdate),
}
