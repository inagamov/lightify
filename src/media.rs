use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::message::Message;
use crate::spotify::player::PlayerUpdate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaEvent {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    SetPosition { position_ms: u32 },
    SeekBy { offset_ms: i32 },
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayState {
    Stopped,
    Paused,
    Playing,
}

pub struct MediaSession {
    controls: MediaControls,
    state: PlayState,
}

impl MediaSession {
    pub fn start(tx: UnboundedSender<Message>) -> Option<Self> {
        if cfg!(target_os = "windows") {
            tracing::info!("media controls are not supported on Windows");
            return None;
        }

        let controls = on_main_thread(move || Self::register(tx))?;
        Some(Self {
            controls,
            state: PlayState::Stopped,
        })
    }

    fn register(tx: UnboundedSender<Message>) -> Option<MediaControls> {
        let dbus_name = format!("lightify.instance{}", std::process::id());
        let config = PlatformConfig {
            dbus_name: &dbus_name,
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
                    MediaControlEvent::Pause | MediaControlEvent::Stop => MediaEvent::Pause,
                    MediaControlEvent::Toggle => MediaEvent::Toggle,
                    MediaControlEvent::Next => MediaEvent::Next,
                    MediaControlEvent::Previous => MediaEvent::Previous,
                    MediaControlEvent::SetPosition(MediaPosition(position)) => {
                        MediaEvent::SetPosition {
                            position_ms: u32::try_from(position.as_millis()).unwrap_or(u32::MAX),
                        }
                    }
                    MediaControlEvent::SeekBy(direction, offset) => {
                        let offset_ms = i32::try_from(offset.as_millis()).unwrap_or(i32::MAX);
                        MediaEvent::SeekBy {
                            offset_ms: match direction {
                                SeekDirection::Forward => offset_ms,
                                SeekDirection::Backward => -offset_ms,
                            },
                        }
                    }
                    MediaControlEvent::Quit => MediaEvent::Quit,
                    other => {
                        tracing::info!(?other, "media event not supported");
                        return;
                    }
                };
                let _ = tx.send(Message::Media(event));
            })
            .inspect_err(|error| tracing::warn!(?error, "could not attach media keys"))
            .ok()?;

        #[cfg(target_os = "macos")]
        macos::disable_unhandled_commands();

        Some(controls)
    }

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
                    cover_url: cover_url.as_deref(),
                    duration: Some(Duration::from_millis((*duration_ms).into())),
                })
            }
            PlayerUpdate::Playing { position_ms } => {
                self.set_playback(PlayState::Playing, Some(*position_ms))
            }
            PlayerUpdate::Paused { position_ms } => {
                self.set_playback(PlayState::Paused, Some(*position_ms))
            }
            PlayerUpdate::Seeked { .. } if self.state == PlayState::Stopped => return,
            PlayerUpdate::Seeked { position_ms } => {
                self.set_playback(self.state, Some(*position_ms))
            }
            PlayerUpdate::Stopped | PlayerUpdate::Disconnected => {
                self.set_playback(PlayState::Paused, None)
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

    fn set_playback(
        &mut self,
        state: PlayState,
        position_ms: Option<u32>,
    ) -> Result<(), souvlaki::Error> {
        self.state = state;
        let progress = position_ms.map(|ms| MediaPosition(Duration::from_millis(ms.into())));
        let playback = match state {
            PlayState::Stopped => MediaPlayback::Stopped,
            PlayState::Paused => MediaPlayback::Paused { progress },
            PlayState::Playing => MediaPlayback::Playing { progress },
        };
        self.controls.set_playback(playback)?;
        #[cfg(target_os = "macos")]
        macos::set_playback_rate(state == PlayState::Playing);
        Ok(())
    }

    fn clear(&mut self) {
        let result = self
            .set_playback(PlayState::Stopped, None)
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

#[cfg(target_os = "macos")]
use macos::on_main_thread;

#[cfg(not(target_os = "macos"))]
fn on_main_thread<R: Send>(work: impl FnOnce() -> R + Send) -> R {
    work()
}

#[cfg(target_os = "macos")]
mod macos {
    use dispatch2::DispatchQueue;
    use objc2::MainThreadMarker;
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSMutableDictionary, NSNumber, NSString};
    use objc2_media_player::{
        MPNowPlayingInfoCenter, MPNowPlayingInfoPropertyPlaybackRate, MPRemoteCommandCenter,
    };

    pub fn on_main_thread<R: Send>(work: impl FnOnce() -> R + Send) -> R {
        if MainThreadMarker::new().is_some() {
            return work();
        }
        let mut result = None;
        DispatchQueue::main().exec_sync(|| result = Some(work()));
        result.expect("exec_sync runs the closure before returning")
    }

    pub fn disable_unhandled_commands() {
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

    pub fn set_playback_rate(playing: bool) {
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
