//! Shared WebAssembly host for `GameCore` implementations (wit `game-core` world).
//! Per-game `cdylib` crates depend on this crate and call [`export_game_core!`] with a type alias
//! for `MyHost<TheirGame>`.

#![allow(warnings)]

pub mod bindings;

pub use bindings::{
    Buffer, Game, GameCoreError, NewPlayerState, PlayerAction, PlayerState, SerializationFormat,
    TakeActionResult,
};

use bindings::Guest;
use game::GameCore;
use std::marker::PhantomData;

mod serialization;
use serialization::{get_deserialize as de, get_serialize as se};

/// Type tag for the wit-bindgen `Guest` implementation; use `export!(YourAlias)` where
/// `type YourAlias = MyHost<YourGameCore>;`.
pub struct MyHost<GameCoreT: GameCore> {
    pub _marker: PhantomData<GameCoreT>,
}

impl<GameCoreT: GameCore> Guest for MyHost<GameCoreT> {
    #[allow(async_fn_in_trait)]
    fn init(format: SerializationFormat, config: Buffer) -> Result<Game, GameCoreError> {
        let config: GameCoreT::Config = de(format)(&config).map_err(|error| {
            println!("Failed to initialize game with provided config: {}", error);
            GameCoreError::Deserialize(error)
        })?;

        let state = GameCoreT::try_init(&config).map_err(|error| {
            println!("Failed to initialize game state: {:?}", error);
            match se(format)(&error) {
                Ok(error_buffer) => GameCoreError::GameCore(error_buffer),
                Err(serialize_error) => {
                    println!("Failed to serialize game core error: {}", serialize_error);
                    GameCoreError::Serialize(serialize_error)
                }
            }
        })?;

        println!("Game initialized with config: {:?}", state);

        let player_states: Vec<(GameCoreT::Player, GameCoreT::PlayerState)> =
            game::Config::get_players(&config)
                .iter()
                .map(|&player| {
                    (
                        player,
                        <GameCoreT::PlayerState as game::PlayerState<GameCoreT>>::init(
                            &config, player,
                        ),
                    )
                })
                .collect();

        let game = {
            let full_state = se(format)(&state).map_err(|error| {
                println!("Failed to serialize game state: {}", error);
                GameCoreError::Serialize(error)
            })?;

            let player_states = player_states
                .iter()
                .map(|(player, player_state)| {
                    let player = se(format)(player).map_err(|error| {
                        println!("Failed to serialize player: {}", error);
                        GameCoreError::Serialize(error)
                    })?;

                    let player_state = se(format)(player_state).map_err(|error| {
                        println!("Failed to serialize player state: {}", error);
                        GameCoreError::Serialize(error)
                    })?;

                    Ok(PlayerState {
                        player,
                        state: player_state,
                    })
                })
                .collect::<Result<Vec<PlayerState>, GameCoreError>>()?;

            Game {
                full_state,
                player_states,
            }
        };

        Ok(game)
    }

    #[allow(async_fn_in_trait)]
    fn take_action(
        format: SerializationFormat,
        game: Game,
        player_action: PlayerAction,
    ) -> Result<TakeActionResult, GameCoreError> {
        let mut game_state: game::FullState<GameCoreT> =
            de(format)(&game.full_state).map_err(|error| {
                println!("Failed to deserialize game state: {}", error);
                GameCoreError::Deserialize(error)
            })?;

        let player_states: Vec<(GameCoreT::Player, GameCoreT::PlayerState)> = game
            .player_states
            .iter()
            .map(|player_state| {
                let player: GameCoreT::Player =
                    de(format)(&player_state.player).map_err(|error| {
                        println!("Failed to deserialize player: {}", error);
                        GameCoreError::Deserialize(error)
                    })?;

                let state: GameCoreT::PlayerState =
                    de(format)(&player_state.state).map_err(|error| {
                        println!("Failed to deserialize player state: {}", error);
                        GameCoreError::Deserialize(error)
                    })?;

                Ok((player, state))
            })
            .collect::<Result<Vec<(GameCoreT::Player, GameCoreT::PlayerState)>, GameCoreError>>()?;

        let player: GameCoreT::Player = de(format)(&player_action.0).map_err(|error| {
            println!("Failed to deserialize player: {}", error);
            GameCoreError::Deserialize(error)
        })?;

        let action: GameCoreT::Action = de(format)(&player_action.1).map_err(|error| {
            println!("Failed to deserialize player action: {}", error);
            GameCoreError::Deserialize(error)
        })?;

        let player_action = game::PlayerAction {
            player: player.clone(),
            action,
        };

        let player_state = player_states
            .iter()
            .find(|(p, _)| *p == player)
            .map(|(_, state)| state)
            .ok_or_else(|| {
                println!("Player state not found for player: {:?}", player);
                GameCoreError::Processing("Player state not found".into())
            })?;

        let apply_action_result =
            GameCoreT::apply_action(&mut game_state, player_action, player_state);

        match apply_action_result {
            Ok(result) => {
                let new_game_full_state = se(format)(&game_state).map_err(|error| {
                    println!("Failed to serialize new game state: {}", error);
                    GameCoreError::Serialize(error)
                })?;

                let player_states = player_states
                    .iter()
                    .map(|player_state| {
                        let new_events = result.get(&player_state.0).unwrap();
                        let mut state = player_state.1.clone();
                        for event in new_events {
                            if let game::PlayerEvent::Event(event) = event {
                                game::PlayerState::apply_event(&mut state, event);
                            }
                        }

                        let player = se(format)(&player_state.0).map_err(|error| {
                            println!("Failed to deserialize player: {}", error);
                            GameCoreError::Deserialize(error)
                        })?;

                        let state = se(format)(&state).map_err(|error| {
                            println!("Failed to serialize new player state: {}", error);
                            GameCoreError::Serialize(error)
                        })?;

                        let player_state = PlayerState { player, state };

                        let events = new_events
                            .iter()
                            .map(|event| {
                                se(format)(&event).map_err(|error| {
                                    println!("Failed to serialize new player event: {}", error);
                                    GameCoreError::Serialize(error)
                                })
                            })
                            .collect::<Result<Vec<Buffer>, GameCoreError>>()?;

                        let new_player_state = NewPlayerState {
                            state: player_state,
                            events,
                        };

                        Ok(new_player_state)
                    })
                    .collect::<Result<Vec<NewPlayerState>, GameCoreError>>()?;

                Ok(TakeActionResult {
                    new_game_full_state,
                    player_states,
                })
            }
            Err(error) => {
                println!("Failed to apply player action: {:?}", error);
                match se(format)(&error) {
                    Ok(error_buffer) => return Err(GameCoreError::GameCore(error_buffer)),
                    Err(serialize_error) => {
                        println!("Failed to serialize game core error: {}", serialize_error);
                        return Err(GameCoreError::Serialize(serialize_error));
                    }
                }
            }
        }
    }
}

/// Registers the component exports (`init`, `take-action`). Use a single ident (type alias).
///
/// ```ignore
/// type TicTacToeWorld = MyHost<TicTacToe>;
/// game_wasm_host::export_game_core!(TicTacToeWorld);
/// ```
#[macro_export]
macro_rules! export_game_core {
    ($ty:ident) => {
        $crate::__export_game_core_impl!($ty with_types_in $crate::bindings);
    };
}
