use futures::{StreamExt, TryStreamExt, stream};
use librespot::{
    core::{Session, SpotifyId, SpotifyUri},
    metadata::{self, Metadata},
    protocol::playlist4_external::{Item, MetaItem, SelectedListContent},
};
use protobuf::Message;

use crate::spotify::model::{Playlist, Track};

const ROOTLIST_PAGE: usize = 120;
pub const PAGE_SIZE: usize = 50;
const CONCURRENCY: usize = 8;

#[derive(Clone)]
pub struct Library {
    session: Session,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TracksPage {
    pub uris: Vec<String>,
    pub tracks: Vec<Track>,
}

impl Library {
    pub fn new(session: Session) -> Self {
        Self { session }
    }

    pub async fn my_playlists(&self) -> Result<Vec<Playlist>, librespot::core::Error> {
        let mut playlists = Vec::new();
        let mut from = 0;
        loop {
            let bytes = self
                .session
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

    pub async fn playlist_tracks(&self, id: &str) -> Result<Vec<String>, librespot::core::Error> {
        let uri = SpotifyUri::Playlist {
            user: None,
            id: SpotifyId::from_base62(id)?,
        };
        let playlist = metadata::Playlist::get(&self.session, &uri).await?;
        playlist
            .tracks()
            .filter(|uri| matches!(uri, SpotifyUri::Track { .. }))
            .map(SpotifyUri::to_uri)
            .collect()
    }

    pub async fn track_details(
        &self,
        uris: Vec<String>,
    ) -> Result<Vec<Track>, librespot::core::Error> {
        stream::iter(uris)
            .map(|uri| async move {
                let track =
                    metadata::Track::get(&self.session, &SpotifyUri::from_uri(&uri)?).await?;
                Ok::<Track, librespot::core::Error>(to_track(&uri, track))
            })
            .buffered(CONCURRENCY)
            .try_collect()
            .await
    }

    pub async fn first_page(&self, id: &str) -> Result<TracksPage, librespot::core::Error> {
        let uris = self.playlist_tracks(id).await?;
        let first = uris.iter().take(PAGE_SIZE).cloned().collect::<Vec<_>>();
        let tracks = self.track_details(first).await?;
        Ok(TracksPage { uris, tracks })
    }
}

fn to_playlists(items: &[Item], meta_items: &[MetaItem]) -> Vec<Playlist> {
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
            Some(Playlist {
                id: id.to_base62().ok()?,
                name: meta.attributes.name().to_string(),
                track_count: usize::try_from(meta.length()).unwrap_or(0),
            })
        })
        .collect()
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
    use super::*;
    use librespot::protocol::playlist4_external::{Item, ListAttributes, MetaItem};

    fn entry(uri: &str, name: &str, length: i32) -> (Item, MetaItem) {
        let mut item = Item::new();
        item.set_uri(uri.to_string());
        let mut attributes = ListAttributes::new();
        attributes.set_name(name.to_string());
        let mut meta = MetaItem::new();
        meta.attributes = Some(attributes).into();
        meta.set_length(length);
        (item, meta)
    }

    #[test]
    fn playlist_entries_convert_and_folder_markers_are_skipped() {
        let (a, a_meta) = entry("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M", "Hits", 50);
        let (f, f_meta) = entry("spotify:start-group:abc:Folder", "Folder", 0);
        let playlists = to_playlists(&[a, f], &[a_meta, f_meta]);
        assert_eq!(
            playlists,
            vec![Playlist {
                id: "37i9dQZF1DXcBWIGoYBM5M".into(),
                name: "Hits".into(),
                track_count: 50,
            }]
        );
    }

    #[test]
    fn track_conversion_keeps_uri_and_joins_artists() {
        use librespot::metadata::Metadata;
        use librespot::protocol::metadata::{Album, Artist, Track as TrackMessage};

        let mut msg = TrackMessage::new();
        msg.set_gid(vec![1; 16]);
        msg.set_name("Dani California".into());
        msg.set_duration(282_160);
        let mut album = Album::new();
        album.set_name("Stadium Arcadium".into());
        msg.album = Some(album).into();
        let mut artist = Artist::new();
        artist.set_name("Red Hot Chili Peppers".into());
        msg.artist.push(artist);

        let uri = SpotifyUri::from_uri("spotify:track:6ZmzpDDsIJzKHFzxb5cOMj").unwrap();
        let track = librespot::metadata::Track::parse(&msg, &uri).unwrap();
        let converted = to_track("spotify:track:6ZmzpDDsIJzKHFzxb5cOMj", track);
        assert_eq!(converted.uri, "spotify:track:6ZmzpDDsIJzKHFzxb5cOMj");
        assert_eq!(converted.duration_ms, 282_160);
        assert_eq!(converted.artist_names(), "Red Hot Chili Peppers");
        assert_eq!(converted.album, "Stadium Arcadium");
    }
}
