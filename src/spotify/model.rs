#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    LikedSongs,
    Playlist(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryItem {
    pub source: Source,
    pub name: String,
    pub track_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub uri: String,
    pub name: String,
    pub duration_ms: u32,
    pub artists: Vec<String>,
    pub album: String,
}

impl Track {
    pub fn artist_names(&self) -> String {
        self.artists.join(", ")
    }
}
