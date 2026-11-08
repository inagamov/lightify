use super::*;

fn creds() -> Credentials {
    Credentials::with_access_token("t")
}

fn key(onboarding: &mut Onboarding, key: Key) -> Vec<Effect> {
    update(onboarding, Event::Key(key))
}

fn waiting() -> Onboarding {
    let (mut onboarding, _) = Onboarding::new(None);
    key(&mut onboarding, Key::Enter);
    onboarding
}

fn returning() -> Onboarding {
    Onboarding::new(Some(creds())).0
}

#[test]
fn returning_user_connects_without_dots() {
    let (onboarding, effects) = Onboarding::new(Some(creds()));
    assert_eq!(onboarding.screen, Screen::Connecting);
    assert_eq!(effects, vec![Effect::StartPlayer(creds())]);
    assert_eq!(onboarding.progress(), None);
}

#[test]
fn first_run_waits_for_enter() {
    let (mut onboarding, effects) = Onboarding::new(None);
    assert_eq!(onboarding.screen, Screen::Welcome { note: None });
    assert!(effects.is_empty());

    assert_eq!(key(&mut onboarding, Key::Enter), vec![Effect::StartSignIn]);
    assert_eq!(onboarding.screen, Screen::Waiting { url: None });

    let effects = update(&mut onboarding, Event::Listening { url: "u".into() });
    assert_eq!(effects, vec![Effect::OpenBrowser("u".into())]);
    assert_eq!(
        key(&mut onboarding, Key::Copy),
        vec![Effect::CopyLink("u".into())]
    );
}

#[test]
fn back_from_waiting_cancels_sign_in() {
    let mut onboarding = waiting();
    assert_eq!(key(&mut onboarding, Key::Back), vec![Effect::CancelSignIn]);
    assert_eq!(onboarding.screen, Screen::Welcome { note: None });
}

#[test]
fn stale_sign_in_result_is_ignored() {
    let mut onboarding = waiting();
    key(&mut onboarding, Key::Back);
    let effects = update(&mut onboarding, Event::SignInFinished(Ok(creds())));
    assert!(effects.is_empty());
    assert_eq!(onboarding.screen, Screen::Welcome { note: None });
}

#[test]
fn signed_in_holds_for_a_second() {
    let mut onboarding = waiting();
    let t0 = Instant::now();
    update(&mut onboarding, Event::Tick(t0));

    let effects = update(&mut onboarding, Event::SignInFinished(Ok(creds())));
    assert_eq!(effects, vec![Effect::StartPlayer(creds())]);
    assert_eq!(onboarding.progress(), Some(2));

    assert!(update(&mut onboarding, Event::PlayerStarted(Ok(()))).is_empty());
    let half = update(
        &mut onboarding,
        Event::Tick(t0 + Duration::from_millis(500)),
    );
    assert!(half.is_empty());
    let full = update(&mut onboarding, Event::Tick(t0 + Duration::from_secs(1)));
    assert_eq!(full, vec![Effect::Finish]);
}

#[test]
fn no_premium_signs_in_again() {
    for mut onboarding in [returning(), waiting()] {
        if onboarding.screen != Screen::Connecting {
            update(&mut onboarding, Event::SignInFinished(Ok(creds())));
        }
        update(
            &mut onboarding,
            Event::PlayerStarted(Err(StartFailure::NoPremium)),
        );
        assert_eq!(
            onboarding.screen,
            Screen::Failed {
                failure: Failure::NoPremium
            }
        );
        assert_eq!(key(&mut onboarding, Key::Enter), vec![Effect::StartSignIn]);
        assert_eq!(onboarding.screen, Screen::Waiting { url: None });
    }
}

#[test]
fn returning_user_network_failure_retries_the_connection() {
    let mut onboarding = returning();
    update(
        &mut onboarding,
        Event::PlayerStarted(Err(StartFailure::Other("down".into()))),
    );
    assert_eq!(
        key(&mut onboarding, Key::Enter),
        vec![Effect::StartPlayer(creds())]
    );
    assert_eq!(onboarding.screen, Screen::Connecting);
}

#[test]
fn rejected_saved_login_returns_to_welcome() {
    let mut onboarding = returning();
    update(
        &mut onboarding,
        Event::PlayerStarted(Err(StartFailure::BadCredentials)),
    );
    assert_eq!(
        onboarding.screen,
        Screen::Welcome {
            note: Some(Note::SavedLoginRejected)
        }
    );
    assert_eq!(onboarding.progress(), Some(1));
}

#[test]
fn cancelled_sign_in_retries_in_the_browser() {
    let mut onboarding = waiting();
    update(
        &mut onboarding,
        Event::SignInFinished(Err(SignInError::Cancelled)),
    );
    assert_eq!(
        onboarding.screen,
        Screen::Failed {
            failure: Failure::Cancelled
        }
    );

    assert_eq!(key(&mut onboarding, Key::Enter), vec![Effect::StartSignIn]);
    assert_eq!(onboarding.screen, Screen::Waiting { url: None });
}
