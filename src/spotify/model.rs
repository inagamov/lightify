use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub tracks: TrackCount,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrackCount {
    pub total: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaylistItem {
    pub track: Option<Track>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub uri: String,
    pub name: String,
    pub duration_ms: u32,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub album: Option<Album>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artist {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Album {
    pub name: String,
}

impl Track {
    pub fn artist_names(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn album_name(&self) -> &str {
        match &self.album {
            Some(album) => album.name.as_str(),
            None => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYLISTS: &str = include_str!("../../tests/fixtures/playlists.json");
    const TRACKS: &str = include_str!("../../tests/fixtures/playlist_tracks.json");

    #[test]
    fn parses_playlist_page() {
        let page: Page<Playlist> = serde_json::from_str(PLAYLISTS).unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].name, "Discover Weekly");
        assert_eq!(page.items[0].tracks.total, 30);
        assert_eq!(page.items[1].id, "1a2b3c");
        assert!(page.next.is_some());
    }

    #[test]
    fn parses_playlist_tracks_including_null_and_episode() {
        let page: Page<PlaylistItem> = serde_json::from_str(TRACKS).unwrap();
        assert_eq!(page.items.len(), 4);
        assert!(page.items[1].track.is_none());
        assert!(page.next.is_none());

        let first = page.items[0].track.as_ref().unwrap();
        assert_eq!(first.name, "Dani California");
        assert_eq!(first.duration_ms, 282160);
        assert_eq!(first.artist_names(), "Red Hot Chili Peppers");
        assert_eq!(first.album_name(), "Stadium Arcadium");

        let third = page.items[2].track.as_ref().unwrap();
        assert_eq!(third.artist_names(), "Red Hot Chili Peppers, Someone Else");

        let episode = page.items[3].track.as_ref().unwrap();
        assert_eq!(episode.artist_names(), "");
        assert_eq!(episode.album_name(), "");
    }
}
