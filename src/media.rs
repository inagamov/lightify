use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::message::Message;
use crate::spotify::player::PlayerUpdate;

/// A media key, Now Playing button press, or progress bar drag from the OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaEvent {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    SetPosition { position_ms: u32 },
}

/// lightify's entry in the OS media controls (Now Playing on macOS, MPRIS on Linux).
pub struct MediaSession {
    controls: MediaControls,
    // Seeked doesn't say whether we're playing, but the widget needs it with the position
    playing: bool,
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
            .attach(move |control_event| {
                tracing::debug!(?control_event, "media event");
                let event = match control_event {
                    MediaControlEvent::Play => MediaEvent::Play,
                    MediaControlEvent::Pause => MediaEvent::Pause,
                    MediaControlEvent::Toggle => MediaEvent::Toggle,
                    MediaControlEvent::Next => MediaEvent::Next,
                    MediaControlEvent::Previous => MediaEvent::Previous,
                    MediaControlEvent::SetPosition(MediaPosition(position)) => {
                        MediaEvent::SetPosition {
                            position_ms: u32::try_from(position.as_millis()).unwrap_or(u32::MAX),
                        }
                    }
                    _ => return,
                };
                // the receiver is gone only while lightify is quitting
                let _ = tx.send(Message::Media(event));
            })
            .inspect_err(|error| tracing::warn!(?error, "could not attach media keys"))
            .ok()?;

        #[cfg(target_os = "macos")]
        macos::disable_unhandled_commands();

        Some(Self {
            controls,
            playing: false,
        })
    }

    /// Mirrors a player update in the OS widget, like `Playback::apply` does for the TUI.
    pub fn apply(&mut self, update: &PlayerUpdate) {
        let result = match update {
            PlayerUpdate::TrackChanged {
                name,
                artists,
                album,
                duration_ms,
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
                    duration: Some(Duration::from_millis((*duration_ms).into())),
                })
            }
            // macOS doesn't tick the progress bar itself; it counts on from the last
            // position we send, so every state change carries one
            PlayerUpdate::Playing { position_ms } => {
                self.playing = true;
                self.set_playback(Some(*position_ms))
            }
            PlayerUpdate::Paused { position_ms } => {
                self.playing = false;
                self.set_playback(Some(*position_ms))
            }
            PlayerUpdate::Seeked { position_ms } => self.set_playback(Some(*position_ms)),
            // Stopped also arrives between tracks and right after Disconnected, so it
            // keeps the track on screen instead of clearing it
            PlayerUpdate::Stopped | PlayerUpdate::Disconnected => {
                self.playing = false;
                self.set_playback(None)
            }
            PlayerUpdate::ConnectionLost(_) => {
                self.clear();
                return;
            }
            PlayerUpdate::Volume(_) | PlayerUpdate::Reconnected => return,
        };
        if let Err(error) = result {
            tracing::warn!(?error, "could not update media controls");
        }
    }

    /// Sends the current play state, with the position in ms when it is known.
    fn set_playback(&mut self, position_ms: Option<u32>) -> Result<(), souvlaki::Error> {
        let progress = position_ms.map(|ms| MediaPosition(Duration::from_millis(ms.into())));
        let playback = if self.playing {
            MediaPlayback::Playing { progress }
        } else {
            MediaPlayback::Paused { progress }
        };
        self.controls.set_playback(playback)?;
        #[cfg(target_os = "macos")]
        macos::set_playback_rate(self.playing);
        Ok(())
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

/// Now Playing details souvlaki leaves out, set the way Chromium does.
#[cfg(target_os = "macos")]
mod macos {
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSMutableDictionary, NSNumber, NSString};
    use objc2_media_player::{
        MPNowPlayingInfoCenter, MPNowPlayingInfoPropertyPlaybackRate, MPRemoteCommandCenter,
    };

    /// Turns off the commands lightify doesn't handle. They start out enabled with no
    /// handler, and then Control Center dims next/previous and freezes the progress bar.
    pub fn disable_unhandled_commands() {
        // SAFETY: plain property access on the shared command center
        unsafe {
            let center = MPRemoteCommandCenter::sharedCommandCenter();
            center.stopCommand().setEnabled(false);
            center.skipForwardCommand().setEnabled(false);
            center.skipBackwardCommand().setEnabled(false);
            center.seekForwardCommand().setEnabled(false);
            center.seekBackwardCommand().setEnabled(false);
            center.changePlaybackRateCommand().setEnabled(false);
            center.changeRepeatModeCommand().setEnabled(false);
            center.changeShuffleModeCommand().setEnabled(false);
            center.ratingCommand().setEnabled(false);
            center.likeCommand().setEnabled(false);
            center.dislikeCommand().setEnabled(false);
            center.bookmarkCommand().setEnabled(false);
            center.enableLanguageOptionCommand().setEnabled(false);
            center.disableLanguageOptionCommand().setEnabled(false);
        }
    }

    /// Adds the playback rate to the current Now Playing info: 1 while playing, 0 while
    /// paused. macOS needs it to move the progress bar and to let it be dragged.
    pub fn set_playback_rate(playing: bool) {
        // SAFETY: the dictionary holds only NSString keys and Foundation values
        unsafe {
            let center = MPNowPlayingInfoCenter::defaultCenter();
            let info = NSMutableDictionary::<NSString, AnyObject>::new();
            if let Some(current) = center.nowPlayingInfo() {
                info.addEntriesFromDictionary(&current);
            }
            let rate = NSNumber::new_f64(if playing { 1.0 } else { 0.0 });
            info.insert(MPNowPlayingInfoPropertyPlaybackRate, rate.as_ref());
            center.setNowPlayingInfo(Some(&info));
        }
    }
}
