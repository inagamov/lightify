use futures::{StreamExt, TryStreamExt, stream};
use librespot::{
    core::{SpotifyId, SpotifyUri},
    metadata::{self, Metadata},
    protocol::{
        context::Context,
        playlist4_external::{Item, MetaItem, SelectedListContent},
    },
};
use protobuf::Message;

use crate::spotify::model::{LibraryItem, Source, Track};
use crate::spotify::session::SessionHandle;

const ROOTLIST_PAGE: usize = 120;
pub const PAGE_SIZE: usize = 50;
const CONCURRENCY: usize = 8;

#[derive(Clone)]
pub struct Library {
    session: SessionHandle,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TracksPage {
    pub uris: Vec<String>,
    pub tracks: Vec<Track>,
}

impl Library {
    pub fn new(session: SessionHandle) -> Self {
        Self { session }
    }

    pub async fn my_playlists(&self) -> Result<Vec<LibraryItem>, librespot::core::Error> {
        let session = self.session.get();
        let mut playlists = vec![LibraryItem {
            source: Source::LikedSongs,
            name: "Liked Songs".into(),
            track_count: self.liked_tracks().await?.len(),
        }];
        let mut from = 0;
        loop {
            let bytes = session
                .spclient()
                .get_rootlist(from, Some(ROOTLIST_PAGE))
                .await?;
            let list = SelectedListContent::parse_from_bytes(&bytes)?;
            let contents = &list.contents;
            playlists.extend(to_playlists(&contents.items, &contents.meta_items));

            from += contents.items.len();
            if contents.items.is_empty() || from >= usize::try_from(list.length()).unwrap_or(0) {
                break;
            }
        }
        Ok(playlists)
    }

    pub async fn track_uris(
        &self,
        source: &Source,
    ) -> Result<Vec<String>, librespot::core::Error> {
        let session = self.session.get();
        match source {
            Source::LikedSongs => self.liked_tracks().await,
            Source::Playlist(id) => {
                let uri = SpotifyUri::Playlist {
                    user: None,
                    id: SpotifyId::from_base62(id)?,
                };

                let playlist = metadata::Playlist::get(&session, &uri).await?;
                playlist
                    .tracks()
                    .filter(|uri| matches!(uri, SpotifyUri::Track { .. }))
                    .map(SpotifyUri::to_uri)
                    .collect()
            }
            Source::Search(query) => self.context_uris(&search_uri(query)).await,
        }
    }

    async fn liked_tracks(&self) -> Result<Vec<String>, librespot::core::Error> {
        let session = self.session.get();
        let uri = format!("spotify:user:{}:collection", session.username());
        self.context_uris(&uri).await
    }

    async fn context_uris(&self, uri: &str) -> Result<Vec<String>, librespot::core::Error> {
        let session = self.session.get();
        let context = session.spclient().get_context(uri).await?;
        Ok(to_uris(&context))
    }

    pub async fn track_details(
        &self,
        uris: Vec<String>,
    ) -> Result<Vec<Track>, librespot::core::Error> {
        let session = self.session.get();
        stream::iter(uris)
            .map(|uri| {
                let session = session.clone();
                async move {
                    let track =
                        metadata::Track::get(&session, &SpotifyUri::from_uri(&uri)?).await?;
                    Ok::<Track, librespot::core::Error>(to_track(&uri, track))
                }
            })
            .buffered(CONCURRENCY)
            .try_collect()
            .await
    }

    pub async fn first_page(&self, source: &Source) -> Result<TracksPage, librespot::core::Error> {
        let uris = self.track_uris(source).await?;
        let first = uris.iter().take(PAGE_SIZE).cloned().collect::<Vec<_>>();
        let tracks = self.track_details(first).await?;
        Ok(TracksPage { uris, tracks })
    }
}

fn to_playlists(items: &[Item], meta_items: &[MetaItem]) -> Vec<LibraryItem> {
    if items.len() != meta_items.len() {
        tracing::warn!(
            "rootlist has {} items but {} meta items",
            items.len(),
            meta_items.len()
        );
    }
    items
        .iter()
        .zip(meta_items)
        .filter_map(|(item, meta)| {
            let SpotifyUri::Playlist { id, .. } = SpotifyUri::from_uri(item.uri()).ok()? else {
                return None;
            };
            Some(LibraryItem {
                source: Source::Playlist(id.to_base62().ok()?),
                name: meta.attributes.name().to_string(),
                track_count: usize::try_from(meta.length()).unwrap_or(0),
            })
        })
        .collect()
}

fn to_uris(context: &Context) -> Vec<String> {
    context
        .pages
        .iter()
        .flat_map(|page| &page.tracks)
        .filter(|track| {
            matches!(
                SpotifyUri::from_uri(track.uri()),
                Ok(SpotifyUri::Track { .. })
            )
        })
        .map(|track| track.uri().to_string())
        .collect()
}

fn search_uri(query: &str) -> String {
    let encoded: String = form_urlencoded::byte_serialize(query.as_bytes()).collect();
    format!("spotify:search:{encoded}")
}

fn to_track(uri: &str, track: metadata::Track) -> Track {
    Track {
        uri: uri.to_string(),
        name: track.name,
        duration_ms: u32::try_from(track.duration).unwrap_or(0),
        artists: track
            .artists
            .0
            .into_iter()
            .map(|artist| artist.name)
            .collect(),
        album: track.album.name,
    }
}

#[cfg(test)]
mod tests {
    use super::search_uri;

    #[test]
    fn search_uri_encodes_the_query() {
        assert_eq!(search_uri("never gonna"), "spotify:search:never+gonna");
        assert_eq!(
            search_uri("c++ & ac/dc"),
            "spotify:search:c%2B%2B+%26+ac%2Fdc"
        );
        assert_eq!(search_uri("café"), "spotify:search:caf%C3%A9");
    }
}
