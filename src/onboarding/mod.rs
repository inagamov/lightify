use std::time::{Duration, Instant};

use librespot::core::authentication::Credentials;

use crate::spotify::oauth::SignInError;
use crate::spotify::player::StartFailure;

pub mod run;

#[cfg(test)]
mod tests;

pub const STEPS: usize = 2;
pub const SIGNED_IN_HOLD: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Connecting,
    Welcome { note: Option<Note> },
    Waiting { url: Option<String> },
    SignedIn { since: Instant, ready: bool },
    Failed { failure: Failure },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    SavedLoginRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    Cancelled,
    PortBusy,
    NoPremium,
    Network(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Copy,
    Open,
    Back,
    Quit,
}

#[derive(Debug)]
pub enum Event {
    Key(Key),
    Listening { url: String },
    SignInFinished(Result<Credentials, SignInError>),
    PlayerStarted(Result<(), StartFailure>),
    Tick(Instant),
}

#[derive(Debug, PartialEq)]
pub enum Effect {
    StartSignIn,
    CancelSignIn,
    OpenBrowser(String),
    CopyLink(String),
    StartPlayer(Credentials),
    Finish,
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
enum Retry {
    SignIn,
    Connect(Credentials),
}

#[derive(Debug)]
pub struct Onboarding {
    pub screen: Screen,
    pub first_run: bool,
    pub spinner: usize,
    retry: Retry,
    last_tick: Instant,
}

impl Onboarding {
    pub fn new(cached: Option<Credentials>) -> (Self, Vec<Effect>) {
        let last_tick = Instant::now();
        match cached {
            Some(credentials) => (
                Self {
                    screen: Screen::Connecting,
                    first_run: false,
                    spinner: 0,
                    retry: Retry::Connect(credentials.clone()),
                    last_tick,
                },
                vec![Effect::StartPlayer(credentials)],
            ),
            None => (
                Self {
                    screen: Screen::Welcome { note: None },
                    first_run: true,
                    spinner: 0,
                    retry: Retry::SignIn,
                    last_tick,
                },
                Vec::new(),
            ),
        }
    }

    pub fn progress(&self) -> Option<usize> {
        if !self.first_run {
            return None;
        }
        match self.screen {
            Screen::SignedIn { .. } => Some(2),
            _ => Some(1),
        }
    }

    fn fail(&mut self, failure: Failure) {
        self.screen = Screen::Failed { failure };
    }
}

pub fn update(onboarding: &mut Onboarding, event: Event) -> Vec<Effect> {
    match event {
        Event::Key(Key::Quit) => vec![Effect::CancelSignIn, Effect::Quit],
        Event::Key(key) => on_key(onboarding, key),
        Event::Listening { url } => match &mut onboarding.screen {
            Screen::Waiting { url: shown } => {
                *shown = Some(url.clone());
                vec![Effect::OpenBrowser(url)]
            }
            _ => Vec::new(),
        },
        Event::SignInFinished(result) => on_sign_in(onboarding, result),
        Event::PlayerStarted(result) => on_player(onboarding, result),
        Event::Tick(now) => {
            onboarding.spinner = onboarding.spinner.wrapping_add(1);
            onboarding.last_tick = now;
            match onboarding.screen {
                Screen::SignedIn { since, ready: true }
                    if now.duration_since(since) >= SIGNED_IN_HOLD =>
                {
                    vec![Effect::Finish]
                }
                _ => Vec::new(),
            }
        }
    }
}

fn on_key(onboarding: &mut Onboarding, key: Key) -> Vec<Effect> {
    match (&onboarding.screen, key) {
        (Screen::Welcome { .. }, Key::Enter) => start_sign_in(onboarding),
        (Screen::Waiting { url: Some(url) }, Key::Copy) => vec![Effect::CopyLink(url.clone())],
        (Screen::Waiting { url: Some(url) }, Key::Open) => vec![Effect::OpenBrowser(url.clone())],
        (Screen::Waiting { .. }, Key::Back) => {
            onboarding.screen = Screen::Welcome { note: None };
            vec![Effect::CancelSignIn]
        }
        (Screen::Failed { .. }, Key::Enter) => match onboarding.retry.clone() {
            Retry::SignIn => start_sign_in(onboarding),
            Retry::Connect(credentials) => {
                onboarding.screen = if onboarding.first_run {
                    Screen::SignedIn {
                        since: onboarding.last_tick,
                        ready: false,
                    }
                } else {
                    Screen::Connecting
                };
                vec![Effect::StartPlayer(credentials)]
            }
        },
        _ => Vec::new(),
    }
}

fn start_sign_in(onboarding: &mut Onboarding) -> Vec<Effect> {
    onboarding.screen = Screen::Waiting { url: None };
    vec![Effect::StartSignIn]
}

fn on_sign_in(
    onboarding: &mut Onboarding,
    result: Result<Credentials, SignInError>,
) -> Vec<Effect> {
    if !matches!(onboarding.screen, Screen::Waiting { .. }) {
        return Vec::new();
    }
    match result {
        Ok(credentials) => {
            onboarding.screen = Screen::SignedIn {
                since: onboarding.last_tick,
                ready: false,
            };
            onboarding.retry = Retry::Connect(credentials.clone());
            vec![Effect::StartPlayer(credentials)]
        }
        Err(error) => {
            onboarding.retry = Retry::SignIn;
            onboarding.fail(match error {
                SignInError::Cancelled => Failure::Cancelled,
                SignInError::PortBusy => Failure::PortBusy,
                other => Failure::Network(other.to_string()),
            });
            Vec::new()
        }
    }
}

fn on_player(onboarding: &mut Onboarding, result: Result<(), StartFailure>) -> Vec<Effect> {
    let connecting = matches!(onboarding.screen, Screen::Connecting);
    let signed_in = matches!(onboarding.screen, Screen::SignedIn { .. });
    if !connecting && !signed_in {
        return Vec::new();
    }
    match result {
        Ok(()) if connecting => vec![Effect::Finish],
        Ok(()) => {
            if let Screen::SignedIn { ready, .. } = &mut onboarding.screen {
                *ready = true;
            }
            Vec::new()
        }
        Err(StartFailure::NoPremium) => {
            onboarding.fail(Failure::NoPremium);
            Vec::new()
        }
        Err(StartFailure::Other(error)) => {
            onboarding.fail(Failure::Network(error));
            Vec::new()
        }
        Err(StartFailure::BadCredentials) => {
            onboarding.screen = Screen::Welcome {
                note: Some(Note::SavedLoginRejected),
            };
            onboarding.first_run = true;
            onboarding.retry = Retry::SignIn;
            Vec::new()
        }
    }
}
