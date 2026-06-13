//! Privacy boundary checks for spectator derivation.

use crate::{
    Action, Config, Event, FullState, GameCore, InGameEvent, PlayerAction, PlayerState,
    SpectatorState,
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
enum Seat {
    A,
    B,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SecretConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecretState {
    secret: u8,
    public_tick: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PublicView {
    public_tick: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Reveal;

impl Action<SecretGame> for Reveal {
    type Error = String;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SeatState {
    seat: Seat,
}

impl PlayerState<SecretGame> for SeatState {
    fn init(_config: &SecretConfig, seat: Seat) -> Self {
        Self { seat }
    }

    fn get_player(&self) -> Seat {
        self.seat
    }

    fn can_take_action(&self, _action: &Reveal) -> Result<(), String> {
        Ok(())
    }

    fn apply_event(&mut self, _event: &u8) {}
}

impl Config<SecretGame> for SecretConfig {
    type ValidationError = String;

    fn validate(&self) -> Result<(), Self::ValidationError> {
        Ok(())
    }

    fn get_players(&self) -> Vec<Seat> {
        vec![Seat::A, Seat::B]
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SecretGame;

impl GameCore for SecretGame {
    type Config = SecretConfig;
    type State = SecretState;
    type Action = Reveal;
    type Player = Seat;
    type PlayerState = SeatState;
    type Event = ();
    type PlayerEvent = u8;
    type Result = u8;
    type PlayerResult = u8;
    type SpectatorEvent = u8;
    type SpectatorResult = u8;
    type SpectatorState = PublicView;

    fn init(_config: &Self::Config) -> Self::State {
        SecretState {
            secret: 42,
            public_tick: 0,
        }
    }

    fn take_action(
        state: &mut Self::State,
        _player_action: PlayerAction<Self>,
    ) -> Vec<Self::Event> {
        state.secret = state.secret.wrapping_add(1);
        state.public_tick = state.public_tick.saturating_add(1);
        vec![]
    }

    fn check_game_over(_state: &Self::State) -> Option<Self::Result> {
        None
    }

    fn derive_player_event(
        state: &Self::State,
        _player: &Self::Player,
        _event: &InGameEvent<Self>,
    ) -> Option<Self::PlayerEvent> {
        Some(state.secret)
    }

    fn derive_player_result(
        _state: &Self::State,
        _player: &Self::Player,
        result: &Self::Result,
    ) -> Self::PlayerResult {
        *result
    }

    fn derive_spectator_event(
        state: &Self::State,
        _event: &InGameEvent<Self>,
    ) -> Option<Self::SpectatorEvent> {
        Some(state.public_tick)
    }

    fn derive_spectator_result(
        _state: &Self::State,
        result: &Self::Result,
    ) -> Self::SpectatorResult {
        *result
    }

    fn scores_at_end(_result: &Self::Result) -> Vec<(Self::Player, f64)> {
        vec![(Seat::A, 0.5), (Seat::B, 0.5)]
    }
}

impl SpectatorState<SecretGame> for PublicView {
    fn init(_config: &SecretConfig) -> Self {
        Self { public_tick: 0 }
    }

    fn apply_event(&mut self, event: &u8) {
        self.public_tick = *event;
    }
}

#[test]
fn spectator_events_do_not_expose_player_only_fields() {
    let mut fs = FullState {
        config: SecretConfig,
        state: SecretGame::init(&SecretConfig),
        actions_made: vec![],
    };
    let ps = SeatState::init(&SecretConfig, Seat::A);
    let applied = SecretGame::apply_action(
        &mut fs,
        PlayerAction {
            player: Seat::A,
            action: Reveal,
        },
        &ps,
    )
    .expect("action ok");

    let player_ev = applied
        .player_events
        .get(&Seat::A)
        .and_then(|v| v.first())
        .expect("player event");
    let spectator_ev = applied.spectator_events.first().expect("spectator event");

    match (player_ev, spectator_ev) {
        (crate::PlayerEvent::Event(secret), crate::SpectatorEvent::Event(public)) => {
            assert_eq!(*secret, 43, "player sees updated secret");
            assert_eq!(*public, 1, "spectator sees public tick only");
            assert_ne!(*secret, *public);
        }
        _ => panic!("unexpected event envelope"),
    }

    let spec_map = SecretGame::build_spectator_events_map(
        &fs,
        &[Event::InGameEvent(InGameEvent::PlayerAction(
            PlayerAction {
                player: Seat::A,
                action: Reveal,
            },
        ))],
    );
    assert_eq!(spec_map.len(), 1);
    if let crate::SpectatorEvent::Event(v) = &spec_map[0] {
        assert_eq!(*v, 1);
    } else {
        panic!("expected visible spectator event");
    }
}
