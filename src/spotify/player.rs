use librespot::connect::{ConnectConfig, LoadRequest, LoadRequestOptions, PlayingTrack, Spirc};
use librespot::core::Session;
use librespot::core::authentication::Credentials;
use librespot::metadata::audio::UniqueFields;
use librespot::playback::config::{AudioFormat, PlayerConfig};
use librespot::playback::mixer::MixerConfig;
use librespot::playback::player::{Player, PlayerEvent};
use librespot::playback::{audio_backend, mixer};
use thiserror::Error;
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::message::Message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerCommand {
    Load {
        uris: Vec<String>,
        start_index: usize,
    },
    PlayPause,
    Next,
    Prev,
    Seek(u32),
    SetVolume(u16),
    Shutdown,
}

#[derive(Debug, Error)]
pub enum PlayerError {
    #[error("no audio backend available")]
    NoBackend,

    #[error("could not create mixer: {0}")]
    Mixer(#[source] librespot::core::Error),

    #[error("could not start Spotify Connect device: {0}")]
    Spirc(#[source] librespot::core::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerUpdate {
    TrackChanged {
        name: String,
        artists: Vec<String>,
        album: String,
        duration_ms: u32,
    },
    Playing {
        position_ms: u32,
    },
    Paused {
        position_ms: u32,
    },
    Seeked {
        position_ms: u32,
    },
    Volume(u16),
    Stopped,
}

pub async fn start(
    session: Session,
    credentials: Credentials,
    tx: UnboundedSender<Message>,
) -> Result<UnboundedSender<PlayerCommand>, PlayerError> {
    let backend = audio_backend::find(None).ok_or(PlayerError::NoBackend)?;

    let mixer_builder = mixer::find(None).ok_or(PlayerError::NoBackend)?;
    let mixer = mixer_builder(MixerConfig::default()).map_err(PlayerError::Mixer)?;

    let player = Player::new(
        PlayerConfig::default(),
        session.clone(),
        mixer.get_soft_volume(),
        move || backend(None, AudioFormat::default()),
    );

    let mut events = player.get_player_event_channel();

    let connect_config = ConnectConfig {
        name: "lightify".to_string(),
        ..ConnectConfig::default()
    };

    let (spirc, spirc_task) = Spirc::new(connect_config, session, credentials, player, mixer)
        .await
        .map_err(PlayerError::Spirc)?;
    tokio::spawn(spirc_task);

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<PlayerCommand>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(command) = cmd_rx.recv() => {
                    let is_shutdown = command == PlayerCommand::Shutdown;
                    if let Err(error) = handle(&spirc, command) {
                        tracing::error!("player command failed: {error}");
                    }
                    if is_shutdown {
                        break;
                    }
                }
                Some(event) = events.recv() => {
                    if let Some(update) = translate(event) {
                        let _ = tx.send(Message::Player(update));
                    }
                }
                else => break,
            }
        }
    });

    Ok(cmd_tx)
}

fn handle(spirc: &Spirc, command: PlayerCommand) -> Result<(), librespot::core::Error> {
    match command {
        PlayerCommand::Load { uris, start_index } => {
            let options = LoadRequestOptions {
                start_playing: true,
                playing_track: Some(PlayingTrack::Index(start_index as u32)),
                ..Default::default()
            };
            spirc.load(LoadRequest::from_tracks(uris, options))
        }
        PlayerCommand::PlayPause => spirc.play_pause(),
        PlayerCommand::Next => spirc.next(),
        PlayerCommand::Prev => spirc.prev(),
        PlayerCommand::Seek(position_ms) => spirc.set_position_ms(position_ms),
        PlayerCommand::SetVolume(volume) => spirc.set_volume(volume),
        PlayerCommand::Shutdown => spirc.shutdown(),
    }
}

fn translate(event: PlayerEvent) -> Option<PlayerUpdate> {
    match event {
        PlayerEvent::VolumeChanged { volume } => Some(PlayerUpdate::Volume(volume)),
        PlayerEvent::Playing { position_ms, .. } => Some(PlayerUpdate::Playing { position_ms }),
        PlayerEvent::Paused { position_ms, .. } => Some(PlayerUpdate::Paused { position_ms }),

        PlayerEvent::Seeked { position_ms, .. }
        | PlayerEvent::PositionCorrection { position_ms, .. } => {
            Some(PlayerUpdate::Seeked { position_ms })
        }

        PlayerEvent::Stopped { .. } | PlayerEvent::EndOfTrack { .. } => Some(PlayerUpdate::Stopped),

        PlayerEvent::TrackChanged { audio_item } => {
            let item = *audio_item;
            let (artists, album) = match item.unique_fields {
                UniqueFields::Track { artists, album, .. } => (
                    artists.iter().map(|artist| artist.name.clone()).collect(),
                    album,
                ),
                _ => (Vec::new(), String::new()),
            };
            Some(PlayerUpdate::TrackChanged {
                name: item.name,
                artists,
                album,
                duration_ms: item.duration_ms,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use librespot::playback::player::PlayerEvent;

    #[test]
    fn volume_event_translates() {
        let update = translate(PlayerEvent::VolumeChanged { volume: 1234 });
        assert_eq!(update, Some(PlayerUpdate::Volume(1234)));
    }

    #[test]
    fn irrelevant_events_are_dropped() {
        let update = translate(PlayerEvent::ShuffleChanged { shuffle: true });
        assert_eq!(update, None);
    }
}
