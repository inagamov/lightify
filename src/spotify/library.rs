use librespot::{
    core::{Session, SpotifyUri},
    protocol::playlist4_external::{Item, MetaItem, SelectedListContent},
};
use protobuf::Message;

use crate::spotify::model::Playlist;

const ROOTLIST_PAGE: usize = 120;

#[derive(Clone)]
pub struct Library {
    session: Session,
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
}
