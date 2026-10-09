//! Era transition state machine (driven by the save watcher in a later phase).
//!
//! Automatic mode never touches a running game: it waits for the game process to exit and for
//! the save to be stable before converting.

use serde::{Deserialize, Serialize};

use crate::date::PartialDate;
use crate::settings::TransitionMode;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TransitionState {
    Idle,
    /// A save at/after the transition date exists and is stable.
    Ready { save_date: PartialDate },
    /// Automatic mode: ready, waiting for the game to close.
    WaitingForGameExit { save_date: PartialDate },
    Converting,
    Done,
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// Watcher found a new stable save with this in-game date.
    StableSave { date: PartialDate },
    GameExited,
    UserConvert,
    UserPostpone,
    ConversionFinished,
    ConversionFailed(String),
}

/// What the UI/tray should do after a transition step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Notify,
    StartConversion,
}

pub fn step(
    state: &TransitionState,
    signal: &Signal,
    mode: TransitionMode,
    transition_date: PartialDate,
) -> (TransitionState, Effect) {
    use Signal as S;
    use TransitionState as T;
    match (state, signal) {
        (T::Idle, S::StableSave { date }) if *date >= transition_date => match mode {
            TransitionMode::Manual => (T::Ready { save_date: *date }, Effect::None),
            TransitionMode::Suggested => (T::Ready { save_date: *date }, Effect::Notify),
            TransitionMode::Automatic => (T::WaitingForGameExit { save_date: *date }, Effect::Notify),
        },
        (T::Ready { save_date } | T::WaitingForGameExit { save_date }, S::StableSave { date }) if date > save_date => {
            // Keep tracking the newest qualifying save.
            let next = if matches!(state, T::Ready { .. }) {
                T::Ready { save_date: *date }
            } else {
                T::WaitingForGameExit { save_date: *date }
            };
            (next, Effect::None)
        }
        (T::WaitingForGameExit { .. }, S::GameExited) => (T::Converting, Effect::StartConversion),
        (T::Ready { .. } | T::WaitingForGameExit { .. }, S::UserConvert) => (T::Converting, Effect::StartConversion),
        (T::Ready { .. } | T::WaitingForGameExit { .. }, S::UserPostpone) => (T::Idle, Effect::None),
        (T::Converting, S::ConversionFinished) => (T::Done, Effect::Notify),
        (T::Converting, S::ConversionFailed(r)) => (T::Failed { reason: r.clone() }, Effect::Notify),
        (s, _) => (s.clone(), Effect::None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: PartialDate = PartialDate::ymd(1337, 4, 1);

    #[test]
    fn early_saves_are_ignored() {
        let (s, e) = step(&TransitionState::Idle, &Signal::StableSave { date: PartialDate::year(1300) },
                          TransitionMode::Automatic, T);
        assert_eq!((s, e), (TransitionState::Idle, Effect::None));
    }

    #[test]
    fn automatic_waits_for_game_exit() {
        let d = PartialDate::ymd(1337, 5, 1);
        let (s, e) = step(&TransitionState::Idle, &Signal::StableSave { date: d }, TransitionMode::Automatic, T);
        assert_eq!(s, TransitionState::WaitingForGameExit { save_date: d });
        assert_eq!(e, Effect::Notify);
        let (s, e) = step(&s, &Signal::GameExited, TransitionMode::Automatic, T);
        assert_eq!((s, e), (TransitionState::Converting, Effect::StartConversion));
    }

    #[test]
    fn suggested_notifies_and_waits_for_user() {
        let d = PartialDate::ymd(1338, 1, 1);
        let (s, e) = step(&TransitionState::Idle, &Signal::StableSave { date: d }, TransitionMode::Suggested, T);
        assert_eq!(e, Effect::Notify);
        let (s2, e2) = step(&s, &Signal::GameExited, TransitionMode::Suggested, T);
        assert_eq!((s2.clone(), e2), (s.clone(), Effect::None), "suggested mode never auto-converts");
        let (s3, _) = step(&s, &Signal::UserPostpone, TransitionMode::Suggested, T);
        assert_eq!(s3, TransitionState::Idle);
    }

    #[test]
    fn conversion_outcomes() {
        let (s, _) = step(&TransitionState::Converting, &Signal::ConversionFailed("x".into()), TransitionMode::Manual, T);
        assert_eq!(s, TransitionState::Failed { reason: "x".into() });
    }
}
