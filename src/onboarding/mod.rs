use std::time::{Duration, Instant};

use librespot::core::authentication::Credentials;

use crate::spotify::oauth::SignInError;
use crate::spotify::player::StartFailure;

pub mod run;

#[cfg(test)]
mod tests;

pub const STEPS: usize = 2;
const SIGNED_IN_HOLD: Duration = Duration::from_secs(1);
const COPIED_FOR: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Connecting,
    Welcome {
        saved_login_rejected: bool,
    },
    Waiting {
        url: Option<String>,
        copied_until: Option<Instant>,
    },
    SignedIn {
        since: Instant,
        ready: bool,
    },
    Failed {
        failure: Failure,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    Cancelled,
    PortBusy,
    NoPremium,
    Audio(String),
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
    LinkCopied,
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
    signing_in: bool,
    pub spinner: usize,
    retry: Retry,
    last_tick: Instant,
}

impl Onboarding {
    pub fn new(cached: Option<Credentials>) -> (Self, Vec<Effect>) {
        let (screen, retry, effects) = match cached {
            Some(credentials) => (
                Screen::Connecting,
                Retry::Connect(credentials.clone()),
                vec![Effect::StartPlayer(credentials)],
            ),
            None => (
                Screen::Welcome {
                    saved_login_rejected: false,
                },
                Retry::SignIn,
                Vec::new(),
            ),
        };
        let onboarding = Self {
            screen,
            signing_in: false,
            spinner: 0,
            retry,
            last_tick: Instant::now(),
        };
        (onboarding, effects)
    }

    pub fn progress(&self) -> Option<usize> {
        if !self.signing_in {
            return None;
        }
        match self.screen {
            Screen::Welcome { .. } => None,
            Screen::SignedIn { .. } => Some(2),
            _ => Some(1),
        }
    }

    pub fn copied(&self) -> bool {
        matches!(
            self.screen,
            Screen::Waiting {
                copied_until: Some(_),
                ..
            }
        )
    }

    fn fail(&mut self, failure: Failure) {
        self.screen = Screen::Failed { failure };
    }
}

pub fn update(onboarding: &mut Onboarding, event: Event) -> Vec<Effect> {
    match event {
        Event::Key(Key::Quit) => vec![Effect::Quit],
        Event::Key(key) => on_key(onboarding, key),
        Event::Listening { url } => match &mut onboarding.screen {
            Screen::Waiting { url: shown, .. } => {
                *shown = Some(url.clone());
                vec![Effect::OpenBrowser(url)]
            }
            _ => Vec::new(),
        },
        Event::LinkCopied => {
            if let Screen::Waiting { copied_until, .. } = &mut onboarding.screen {
                *copied_until = Some(onboarding.last_tick + COPIED_FOR);
            }
            Vec::new()
        }
        Event::SignInFinished(result) => on_sign_in(onboarding, result),
        Event::PlayerStarted(result) => on_player(onboarding, result),
        Event::Tick(now) => {
            onboarding.spinner = onboarding.spinner.wrapping_add(1);
            onboarding.last_tick = now;
            if let Screen::Waiting { copied_until, .. } = &mut onboarding.screen
                && copied_until.is_some_and(|until| now >= until)
            {
                *copied_until = None;
            }
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
        (Screen::Waiting { url: Some(url), .. }, Key::Copy) => vec![Effect::CopyLink(url.clone())],
        (Screen::Waiting { url: Some(url), .. }, Key::Open) => {
            vec![Effect::OpenBrowser(url.clone())]
        }
        (Screen::Waiting { .. }, Key::Back) => {
            onboarding.screen = Screen::Welcome {
                saved_login_rejected: false,
            };
            vec![Effect::CancelSignIn]
        }
        (Screen::Failed { .. }, Key::Enter) => match onboarding.retry.clone() {
            Retry::SignIn => start_sign_in(onboarding),
            Retry::Connect(credentials) => {
                onboarding.screen = if onboarding.signing_in {
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
    onboarding.signing_in = true;
    onboarding.screen = Screen::Waiting {
        url: None,
        copied_until: None,
    };
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
            onboarding.retry = Retry::SignIn;
            onboarding.fail(Failure::NoPremium);
            Vec::new()
        }
        Err(StartFailure::Audio(error)) => {
            onboarding.fail(Failure::Audio(error));
            Vec::new()
        }
        Err(StartFailure::Other(error)) => {
            onboarding.fail(Failure::Network(error));
            Vec::new()
        }
        Err(StartFailure::BadCredentials) => {
            onboarding.screen = Screen::Welcome {
                saved_login_rejected: true,
            };
            onboarding.retry = Retry::SignIn;
            Vec::new()
        }
    }
}
