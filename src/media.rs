use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use tokio::sync::mpsc::UnboundedSender;

use crate::message::Message;
use crate::spotify::player::PlayerUpdate;

/// A media key or Now Playing button press from the OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKey {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
}

/// lightify's entry in the OS media controls (Now Playing on macOS, MPRIS on Linux).
pub struct MediaSession {
    controls: MediaControls,
}

impl MediaSession {
    /// Registers with the OS and forwards media keys to `tx`. Returns `None` where
    /// media controls are unavailable; lightify works without them.
    pub fn start(tx: UnboundedSender<Message>) -> Option<Self> {
        // Windows needs a window handle, which a terminal app doesn't have
        if cfg!(target_os = "windows") {
            return None;
        }

        let config = PlatformConfig {
            dbus_name: "lightify",
            display_name: "lightify",
            hwnd: None,
        };
        let mut controls = MediaControls::new(config)
            .inspect_err(|error| tracing::warn!(?error, "media controls unavailable"))
            .ok()?;
        controls
            .attach(move |event| {
                let key = match event {
                    MediaControlEvent::Play => MediaKey::Play,
                    MediaControlEvent::Pause => MediaKey::Pause,
                    MediaControlEvent::Toggle => MediaKey::Toggle,
                    MediaControlEvent::Next => MediaKey::Next,
                    MediaControlEvent::Previous => MediaKey::Previous,
                    _ => return,
                };
                // the receiver is gone only while lightify is quitting
                let _ = tx.send(Message::MediaKey(key));
            })
            .inspect_err(|error| tracing::warn!(?error, "could not attach media keys"))
            .ok()?;

        Some(Self { controls })
    }

    /// Mirrors a player update in the OS widget, like `Playback::apply` does for the TUI.
    pub fn apply(&mut self, update: &PlayerUpdate) {
        let result = match update {
            PlayerUpdate::TrackChanged {
                name,
                artists,
                album,
                cover_url,
                ..
            } => {
                let artist = artists.join(", ");
                self.controls.set_metadata(MediaMetadata {
                    title: Some(name.as_str()),
                    artist: Some(artist.as_str()),
                    album: Some(album.as_str()),
                    // macOS downloads it in the background, so it shows up a moment later
                    cover_url: cover_url.as_deref(),
                    ..Default::default()
                })
            }
            PlayerUpdate::Playing { .. } => self
                .controls
                .set_playback(MediaPlayback::Playing { progress: None }),
            // Stopped also arrives between tracks and right after Disconnected, so it
            // keeps the track on screen instead of clearing it
            PlayerUpdate::Paused { .. } | PlayerUpdate::Stopped | PlayerUpdate::Disconnected => {
                self.controls
                    .set_playback(MediaPlayback::Paused { progress: None })
            }
            PlayerUpdate::ConnectionLost(_) => {
                self.clear();
                return;
            }
            PlayerUpdate::Seeked { .. } | PlayerUpdate::Volume(_) | PlayerUpdate::Reconnected => {
                return;
            }
        };
        if let Err(error) = result {
            tracing::warn!(?error, "could not update media controls");
        }
    }

    fn clear(&mut self) {
        let result = self
            .controls
            .set_playback(MediaPlayback::Stopped)
            .and_then(|()| self.controls.set_metadata(MediaMetadata::default()));
        if let Err(error) = result {
            tracing::warn!(?error, "could not clear media controls");
        }
    }
}

impl Drop for MediaSession {
    fn drop(&mut self) {
        self.clear();
    }
}
