use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use librespot::connect::{ConnectConfig, LoadRequest, LoadRequestOptions, PlayingTrack, Spirc};
use librespot::core::Session;
use librespot::core::authentication::Credentials;
use librespot::metadata::audio::UniqueFields;
use librespot::playback::config::{AudioFormat, PlayerConfig};
use librespot::playback::mixer::{Mixer, MixerConfig};
use librespot::playback::player::{Player, PlayerEvent, PlayerEventChannel};
use librespot::playback::{audio_backend, mixer};
use thiserror::Error;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tokio::time::{self, sleep_until, timeout, timeout_at};

use crate::message::Message;
use crate::spotify::reconnect::Reconnector;
use crate::spotify::session::SessionHandle;

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
    Reconnect,
}

#[derive(Debug, Error)]
pub enum PlayerError {
    #[error("no audio backend available")]
    NoBackend,

    #[error("no mixer available")]
    NoMixer,

    #[error("could not create mixer: {0}")]
    Mixer(#[source] librespot::core::Error),

    #[error("could not start spotify connect device: {0}")]
    Spirc(#[source] librespot::core::Error),

    #[error("no cached credentials; restart lightify to log in again")]
    NoCredentials,

    #[error("connection attempt timed out")]
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerUpdate {
    TrackChanged {
        name: String,
        artists: Vec<String>,
        album: String,
        duration_ms: u32,
        uri: String,
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
    Disconnected,
    Reconnected,
    ConnectionLost(Option<String>),
}

pub struct PlayerHandle {
    pub commands: UnboundedSender<PlayerCommand>,
    task: JoinHandle<()>,
}

impl PlayerHandle {
    pub async fn shutdown(self) {
        let PlayerHandle { commands, mut task } = self;
        drop(commands);
        match timeout(Duration::from_secs(10), &mut task).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::error!("player task failed: {error}"),
            Err(_) => {
                tracing::warn!("player task did not stop in time");
                task.abort();
                let _ = task.await;
            }
        }
    }
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const QUIT_TIMEOUT: Duration = Duration::from_secs(2);
const SESSION_CHECK: Duration = Duration::from_secs(1);

pub const DEFAULT_VOLUME: u16 = (u16::MAX as u32 * 69 / 100) as u16;

type SpircTask = Pin<Box<dyn Future<Output = ()> + Send>>;
type Attempt = Pin<Box<dyn Future<Output = Result<Connected, PlayerError>> + Send>>;

struct Connected {
    spirc: Spirc,
    task: SpircTask,
    session: Session,
}

enum Connection {
    Up(Connected),
    Draining {
        task: SpircTask,
        deadline: time::Instant,
    },
    Waiting {
        until: time::Instant,
    },
    Connecting {
        attempt: Attempt,
        manual: bool,
    },
    Down,
}

enum ConnectionEvent {
    ConnectionDead {
        task_finished: bool,
    },
    Drained {
        in_time: bool,
    },
    RetryDue,
    AttemptFinished {
        result: Result<Connected, PlayerError>,
        manual: bool,
    },
}

impl Connection {
    async fn event(&mut self) -> ConnectionEvent {
        match self {
            Connection::Up(Connected { task, session, .. }) => {
                let mut check = tokio::time::interval(SESSION_CHECK);
                loop {
                    tokio::select! {
                        () = task.as_mut() => {
                            return ConnectionEvent::ConnectionDead { task_finished: true };
                        }
                        _ = check.tick() => {
                            if session.is_invalid() {
                                return ConnectionEvent::ConnectionDead { task_finished: false };
                            }
                        }
                    }
                }
            }
            Connection::Draining { task, deadline } => {
                let in_time = timeout_at(*deadline, task.as_mut()).await.is_ok();
                ConnectionEvent::Drained { in_time }
            }
            Connection::Waiting { until } => {
                sleep_until(*until).await;
                ConnectionEvent::RetryDue
            }
            Connection::Connecting { attempt, manual } => ConnectionEvent::AttemptFinished {
                result: attempt.as_mut().await,
                manual: *manual,
            },
            Connection::Down => std::future::pending().await,
        }
    }

    async fn quit(self) {
        match self {
            Connection::Up(Connected { spirc, task, .. }) => {
                if let Err(error) = spirc.shutdown() {
                    tracing::error!("spirc shutdown failed: {error}");
                }
                if timeout(QUIT_TIMEOUT, task).await.is_err() {
                    tracing::warn!("spirc did not shut down in time");
                }
            }
            Connection::Draining { task, deadline } => {
                let deadline = deadline.min(time::Instant::now() + QUIT_TIMEOUT);
                if timeout_at(deadline, task).await.is_err() {
                    tracing::warn!("spirc cleanup did not finish in time");
                }
            }
            Connection::Connecting { .. } | Connection::Waiting { .. } | Connection::Down => {}
        }
    }
}

struct PlayerTask {
    session: SessionHandle,
    player: Arc<Player>,
    mixer: Arc<dyn Mixer>,
    connect_config: ConnectConfig,
    connection: Connection,
    reconnector: Reconnector,
    tx: UnboundedSender<Message>,
}

impl PlayerTask {
    async fn run(
        mut self,
        mut commands: UnboundedReceiver<PlayerCommand>,
        mut events: PlayerEventChannel,
    ) {
        loop {
            tokio::select! {
                command = commands.recv() => match command {
                    Some(command) => self.handle_command(command),
                    None => break,
                },
                Some(event) = events.recv() => {
                    if let Some(update) = translate(event) {
                        let _ = self.tx.send(Message::Player(update));
                    }
                }
                event = self.connection.event() => self.handle_connection_event(event),
            }
        }
        self.shutdown().await;
    }

    fn handle_command(&mut self, command: PlayerCommand) {
        match command {
            PlayerCommand::Reconnect => match self.connection {
                Connection::Waiting { .. } | Connection::Down => {
                    tracing::info!("manual reconnect");
                    self.connection = Connection::Connecting {
                        attempt: self.attempt(),
                        manual: true,
                    };
                }
                Connection::Up(_) | Connection::Draining { .. } | Connection::Connecting { .. } => {
                    tracing::info!("reconnect requested while busy; ignored");
                }
            },
            command => match &self.connection {
                Connection::Up(Connected { spirc, .. }) => {
                    if let Err(error) = handle(spirc, command) {
                        tracing::error!("player command failed: {error}");
                    }
                }
                _ => match &command {
                    PlayerCommand::Load { uris, .. } => {
                        tracing::warn!(tracks = uris.len(), "dropped load: not connected");
                    }
                    other => tracing::warn!(?other, "dropped: not connected"),
                },
            },
        }
    }

    async fn shutdown(&mut self) {
        std::mem::replace(&mut self.connection, Connection::Down)
            .quit()
            .await;
    }

    fn attempt(&self) -> Attempt {
        let session = self.session.clone();
        let player = self.player.clone();
        let mixer = self.mixer.clone();
        let config = ConnectConfig {
            initial_volume: mixer.volume(),
            ..self.connect_config.clone()
        };
        Box::pin(async move {
            timeout(CONNECT_TIMEOUT, reconnect(session, player, mixer, config))
                .await
                .unwrap_or(Err(PlayerError::Timeout))
        })
    }

    fn handle_connection_event(&mut self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::ConnectionDead { task_finished } => {
                tracing::warn!("connection to spotify dropped");
                self.player.stop();
                let _ = self.tx.send(Message::Player(PlayerUpdate::Disconnected));
                let old = std::mem::replace(&mut self.connection, Connection::Down);
                self.connection = match old {
                    Connection::Up(Connected { task, .. }) if !task_finished => {
                        tracing::info!("waiting for spirc cleanup");
                        Connection::Draining {
                            task,
                            deadline: time::Instant::now() + SHUTDOWN_TIMEOUT,
                        }
                    }
                    _ => self.schedule_retry(),
                };
            }
            ConnectionEvent::Drained { in_time } => {
                if in_time {
                    tracing::info!("spirc cleanup finished");
                } else {
                    tracing::warn!("spirc cleanup timed out; dropped");
                }
                self.connection = self.schedule_retry();
            }
            ConnectionEvent::RetryDue => {
                self.connection = Connection::Connecting {
                    attempt: self.attempt(),
                    manual: false,
                };
            }
            ConnectionEvent::AttemptFinished {
                result: Ok(connected),
                ..
            } => {
                tracing::info!("reconnected");
                self.reconnector.reset();
                self.connection = Connection::Up(connected);
                let _ = self.tx.send(Message::Player(PlayerUpdate::Reconnected));
            }
            ConnectionEvent::AttemptFinished {
                result: Err(error),
                manual,
            } => {
                tracing::error!("reconnect failed: {error}");
                let hopeless = matches!(error, PlayerError::NoCredentials);
                self.connection = if manual || hopeless {
                    self.give_up(Some(error.to_string()))
                } else {
                    self.schedule_retry()
                };
            }
        }
    }

    fn schedule_retry(&mut self) -> Connection {
        match self.reconnector.next_delay(Instant::now()) {
            Some(delay) => {
                tracing::info!(?delay, "next reconnect attempt");
                Connection::Waiting {
                    until: time::Instant::now() + delay,
                }
            }
            None => self.give_up(None),
        }
    }

    fn give_up(&self, error: Option<String>) -> Connection {
        tracing::error!("not reconnecting automatically");
        let _ = self
            .tx
            .send(Message::Player(PlayerUpdate::ConnectionLost(error)));
        Connection::Down
    }
}

async fn reconnect(
    handle: SessionHandle,
    player: Arc<Player>,
    mixer: Arc<dyn Mixer>,
    config: ConnectConfig,
) -> Result<Connected, PlayerError> {
    let credentials = handle.credentials().ok_or(PlayerError::NoCredentials)?;
    let session = handle.reconnect();
    player.set_session(session.clone());
    connect(config, session, credentials, player, mixer).await
}

async fn connect(
    config: ConnectConfig,
    session: Session,
    credentials: Credentials,
    player: Arc<Player>,
    mixer: Arc<dyn Mixer>,
) -> Result<Connected, PlayerError> {
    let (spirc, task) = Spirc::new(config, session.clone(), credentials, player, mixer)
        .await
        .map_err(PlayerError::Spirc)?;
    Ok(Connected {
        spirc,
        task: Box::pin(task),
        session,
    })
}

pub async fn start(
    session: SessionHandle,
    credentials: Credentials,
    tx: UnboundedSender<Message>,
) -> Result<PlayerHandle, PlayerError> {
    let backend = audio_backend::find(None).ok_or(PlayerError::NoBackend)?;

    let mixer_builder = mixer::find(None).ok_or(PlayerError::NoMixer)?;
    let mixer = mixer_builder(MixerConfig::default()).map_err(PlayerError::Mixer)?;

    let current = session.get();
    let player = Player::new(
        PlayerConfig::default(),
        current.clone(),
        mixer.get_soft_volume(),
        move || backend(None, AudioFormat::default()),
    );
    let events = player.get_player_event_channel();

    let connect_config = ConnectConfig {
        name: "lightify".to_string(),
        initial_volume: DEFAULT_VOLUME,
        ..ConnectConfig::default()
    };

    let connected = connect(
        connect_config.clone(),
        current,
        credentials,
        player.clone(),
        mixer.clone(),
    )
    .await?;

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<PlayerCommand>();
    let task = tokio::spawn(
        PlayerTask {
            session,
            player,
            mixer,
            connect_config,
            connection: Connection::Up(connected),
            reconnector: Reconnector::default(),
            tx,
        }
        .run(cmd_rx, events),
    );

    Ok(PlayerHandle {
        commands: cmd_tx,
        task,
    })
}

fn handle(spirc: &Spirc, command: PlayerCommand) -> Result<(), librespot::core::Error> {
    match command {
        PlayerCommand::Load { uris, start_index } => {
            let options = LoadRequestOptions {
                start_playing: true,
                playing_track: Some(PlayingTrack::Index(start_index as u32)),
                ..Default::default()
            };
            spirc.activate()?;
            spirc.load(LoadRequest::from_tracks(uris, options))
        }
        PlayerCommand::PlayPause => spirc.play_pause(),
        PlayerCommand::Next => spirc.next(),
        PlayerCommand::Prev => spirc.prev(),
        PlayerCommand::Seek(position_ms) => spirc.set_position_ms(position_ms),
        PlayerCommand::SetVolume(volume) => spirc.set_volume(volume),
        PlayerCommand::Reconnect => Ok(()),
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
                uri: item.uri,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stuck_spirc() -> SpircTask {
        Box::pin(std::future::pending())
    }

    #[tokio::test(start_paused = true)]
    async fn quit_while_draining_is_bounded_by_quit_timeout() {
        let connection = Connection::Draining {
            task: stuck_spirc(),
            deadline: time::Instant::now() + SHUTDOWN_TIMEOUT,
        };

        let started = tokio::time::Instant::now();
        connection.quit().await;

        assert_eq!(started.elapsed(), QUIT_TIMEOUT);
    }

    #[tokio::test(start_paused = true)]
    async fn quit_while_draining_returns_as_soon_as_cleanup_finishes() {
        let connection = Connection::Draining {
            task: Box::pin(async {}),
            deadline: time::Instant::now() + SHUTDOWN_TIMEOUT,
        };

        let started = tokio::time::Instant::now();
        connection.quit().await;

        assert_eq!(started.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn quit_while_waiting_does_not_wait_out_the_retry() {
        let connection = Connection::Waiting {
            until: time::Instant::now() + Duration::from_secs(60),
        };

        let started = tokio::time::Instant::now();
        connection.quit().await;

        assert_eq!(started.elapsed(), Duration::ZERO);
    }
}
