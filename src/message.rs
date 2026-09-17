use librespot::core::Error;

use crate::spotify::library::TracksPage;
use crate::spotify::model::{LibraryItem, Source, Track};
use crate::spotify::player::PlayerUpdate;

#[derive(Debug)]
pub enum Message {
    Playlists {
        generation: u64,
        result: Result<Vec<LibraryItem>, Error>,
    },
    Tracks {
        generation: u64,
        source: Source,
        result: Result<TracksPage, Error>,
    },
    MoreTracks {
        generation: u64,
        source: Source,
        result: Result<Vec<Track>, Error>,
    },
    Player(PlayerUpdate),
    Tick,
}
