mod serialization;
use common::{
    Game, ProcessingError, ProcessingInput, ProcessingOutput, ProcessingTransaction,
    SerializedBuffer, TryInit, TryTakeAction, TryTakeActionOutput,
};
use game::{Config, FullState, GameCore, PlayerAction, PlayerState};
use serialization::*;
use std::vec;
use tic_tac_toe::*;

#[unsafe(no_mangle)]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buffer = Vec::with_capacity(size);
    let pointer = buffer.as_mut_ptr();

    std::mem::forget(buffer);

    pointer
}

#[unsafe(no_mangle)]
pub extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, size);
    }
}

fn process<T: ProcessingTransaction + serde::Serialize>(
    input_pointer: *mut u8,
    length: usize,
    output_pointer: *mut u32,
    processing: fn(
        ProcessingInput<T>,
    ) -> Result<<T as ProcessingTransaction>::Output, ProcessingError>,
) -> usize {
    let processing_output: ProcessingOutput<T> = ProcessingOutput(
        deserialize_buffer(input_pointer, length, rmp_deserialize)
            .map_err(|e| ProcessingError::DeserializationError(e))
            .and_then(|input| processing(input)),
    );

    write_buffer(&processing_output, output_pointer, rmp_serialize)
}

#[unsafe(no_mangle)]
pub extern "C" fn try_init(
    input_pointer: *mut u8,
    length: usize,
    output_pointer: *mut u32,
) -> usize {
    process::<TryInit>(
        input_pointer,
        length,
        output_pointer,
        |ProcessingInput { format, payload }| {
            let config: <TicTacToe as GameCore>::Config = get_deserialize(format)(&payload.config)
                .map_err(|e| {
                    println!("Failed to initialize game with provided config: {}", e);
                    ProcessingError::DeserializationError(e)
                })?;

            let state = TicTacToe::try_init(&config).map_err(|e| {
                println!("Failed to initialize game state: {:?}", e);
                let mut error_buffer = vec![];
                match get_serialize(format)(&e, &mut error_buffer) {
                    Ok(_) => ProcessingError::GameCoreError(error_buffer),
                    Err(serialize_error) => {
                        println!("Failed to serialize game core error: {}", serialize_error);
                        ProcessingError::SerializationError(serialize_error)
                    }
                }
            })?;
            println!("Game initialized with config: {:?}", state);

            let player_states: Vec<<TicTacToe as GameCore>::PlayerState> = config
                .get_players()
                .iter()
                .map(|&player| <TicTacToe as GameCore>::PlayerState::init(&config, player))
                .collect();

            let mut game = Game::default();
            get_serialize(format)(&state, &mut game.state).map_err(|e| {
                println!("Failed to serialize game state: {}", e);
                ProcessingError::SerializationError(e)
            })?;
            get_serialize(format)(&player_states, &mut game.player_states).map_err(|e| {
                println!("Failed to serialize player states: {}", e);
                ProcessingError::SerializationError(e)
            })?;

            Ok(game)
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn try_take_action(
    input_pointer: *mut u8,
    length: usize,
    output_pointer: *mut u32,
) -> usize {
    process::<TryTakeAction>(
        input_pointer,
        length,
        output_pointer,
        |ProcessingInput { format, payload }| {
            let mut game_state: FullState<TicTacToe> = get_deserialize(format)(&payload.game.state)
                .map_err(|e| {
                    println!("Failed to deserialize game state: {}", e);
                    ProcessingError::DeserializationError(e)
                })?;
            let mut player_states: Vec<<TicTacToe as GameCore>::PlayerState> =
                get_deserialize(format)(&payload.game.player_states).map_err(|e| {
                    println!("Failed to deserialize player states: {}", e);
                    ProcessingError::DeserializationError(e)
                })?;
            let player_action: PlayerAction<TicTacToe> = get_deserialize(format)(&payload.action)
                .map_err(|e| {
                println!("Failed to deserialize player action: {}", e);
                ProcessingError::DeserializationError(e)
            })?;

            let player_state = player_states
                .iter()
                .find(|player_state| player_state.get_player() == player_action.player)
                .ok_or_else(|| {
                    println!(
                        "Player state not found for player: {:?}",
                        player_action.player
                    );
                    ProcessingError::ProcessingError("Player state not found".into())
                })?;

            let result = player_state
                .can_take_action(&player_action.action)
                .and_then(|_| TicTacToe::apply_action(&mut game_state, player_action, player_state))
                .map_err(|e| {
                    println!("Failed to apply player action: {}", e);
                    let mut error_buffer = vec![];
                    match get_serialize(format)(&e, &mut error_buffer) {
                        Ok(_) => ProcessingError::GameCoreError(error_buffer),
                        Err(serialize_error) => {
                            println!("Failed to serialize game core error: {}", serialize_error);
                            ProcessingError::SerializationError(serialize_error)
                        }
                    }
                })?;

            player_states
                .iter_mut()
                .filter_map(|state| {
                    result
                        .get(&state.get_player())
                        .map(|events| (state, events))
                })
                .for_each(|(player_state, player_events)| {
                    player_events.iter().for_each(|event| {
                        if let game::PlayerEvent::Event(event) = event {
                            player_state.apply_event(event);
                        }
                    });
                });

            let mut output = TryTakeActionOutput {
                game: Game::default(),
                player_events: vec![],
            };

            get_serialize(format)(&game_state.state, &mut output.game.state).map_err(|e| {
                println!("Failed to serialize game state: {}", e);
                ProcessingError::SerializationError(e)
            })?;
            get_serialize(format)(&player_states, &mut output.game.player_states).map_err(|e| {
                println!("Failed to serialize player states: {}", e);
                ProcessingError::SerializationError(e)
            })?;
            for (player, events) in result {
                let mut player_events = (SerializedBuffer::default(), SerializedBuffer::default());
                get_serialize(format)(&player, &mut player_events.0).map_err(|e| {
                    println!("Failed to serialize player: {}", e);
                    ProcessingError::SerializationError(e)
                })?;
                get_serialize(format)(&events, &mut player_events.1).map_err(|e| {
                    println!("Failed to serialize player events: {}", e);
                    ProcessingError::SerializationError(e)
                })?;
            }

            Ok(output)
        },
    )
}
