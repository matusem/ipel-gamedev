use crate::game_core::{self, Buffer, Game, GameCore, Player, TakeActionResult};
use actix_web::ResponseError;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use uuid::Uuid;
use wasmtime::{Engine, Store};
use wasmtime_wasi::p1::WasiP1Ctx;

#[derive(Clone)]
pub struct GameDb(Arc<RwLock<HashMap<Uuid, GameInstance>>>);

#[derive(Debug)]
pub enum GameInstanceError {
    LockFailed,
    GameNotFound,
    PlayerNotInGame,
    GameCore(game_core::GameCoreError),
    Wasm(wasmtime::Error),
}

impl std::fmt::Display for GameInstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use GameInstanceError::*;
        match self {
            LockFailed => write!(f, "Failed to acquire lock on game instance"),
            GameNotFound => write!(f, "Game not found"),
            PlayerNotInGame => write!(f, "Player not in game"),
            GameCore(e) => write!(f, "Game core error: {}", e),
            Wasm(e) => write!(f, "Wasm error: {}", e),
        }
    }
}
impl ResponseError for GameInstanceError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        use GameInstanceError::*;
        match self {
            LockFailed | Wasm(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            PlayerNotInGame | GameCore(_) => actix_web::http::StatusCode::BAD_REQUEST,
            GameNotFound => actix_web::http::StatusCode::NOT_FOUND,
        }
    }
}

#[derive(Clone)]
pub struct PlayerChannel {
    player: Player,
    action_sender: async_channel::Sender<(Player, Buffer)>,
    event_sender: async_channel::Sender<Buffer>,
    event_receiver: async_channel::Receiver<Buffer>,
}

impl PlayerChannel {
    pub fn new(player: Player, action_sender: async_channel::Sender<(Player, Buffer)>) -> Self {
        let (event_sender, event_receiver) = async_channel::unbounded();

        PlayerChannel {
            player,
            action_sender,
            event_sender,
            event_receiver,
        }
    }

    pub async fn send_action(
        &self,
        action: Buffer,
    ) -> Result<(), async_channel::SendError<(Player, Buffer)>> {
        self.action_sender.send((self.player.clone(), action)).await
    }
    
    pub async fn receive_event(&self) -> Result<Buffer, async_channel::RecvError> {
        self.event_receiver.recv().await
    }
}

#[derive(Clone)]
pub struct GameInstance {
    players: Arc<RwLock<HashMap<Uuid, PlayerChannel>>>,
    game: Arc<RwLock<Game>>,
    game_core: Arc<RwLock<GameCore>>,
    action_sender: async_channel::Sender<(Player, Buffer)>,
    action_receiver: async_channel::Receiver<(Player, Buffer)>,
}

impl GameInstance {
    pub fn new(game: Game, game_core: GameCore) -> Self {
        let (action_sender, action_receiver) = async_channel::unbounded();

        Self {
            players: Arc::new(RwLock::new(HashMap::new())),
            game: Arc::new(RwLock::new(game)),
            game_core: Arc::new(RwLock::new(game_core)),
            action_sender,
            action_receiver,
        }
    }

    pub fn get_player_ids(&self) -> Vec<Uuid> {
        self.players
            .read()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    }

    pub async fn run(&mut self, engine: &Engine, mut store: Store<WasiP1Ctx>) {
        while let Ok((player, action)) = self.action_receiver.recv().await {
            let game_core = self.game_core.write().unwrap();
            let mut game = self.game.write().unwrap();
            let players = self.players.write().unwrap();

            println!(
                "Player {:?} wants to play action: {:?}",
                player,
                str::from_utf8(&action).unwrap()
            );

            let result = game_core
                .call_take_action(
                    &mut store,
                    game_core::SerializationFormat::Json,
                    &game,
                    &(player.clone(), action),
                )
                .await
                .map(|result| {
                    result.map_err(|game_core_error| GameInstanceError::GameCore(game_core_error))
                })
                .map_err(|wasm_error| GameInstanceError::Wasm(wasm_error))
                .flatten();

            println!(
                "Action {:?}",
                if let Ok(_) = &result {
                    "succeeded"
                } else {
                    "failed"
                }
            );

            match result {
                Ok(take_action_result) => {
                    let TakeActionResult {
                        new_game_full_state,
                        player_states,
                    } = take_action_result;

                    *game = Game {
                        full_state: new_game_full_state,
                        player_states: player_states
                            .iter()
                            .map(|new_player_state| new_player_state.state.clone())
                            .collect(),
                    };

                    println!(
                        "Action successful, sending events to {} connected players",
                        players.len()
                    );

                    // Send updated player states
                    for (other_player_id, other_player_channel) in players.iter() {
                        let player_events = player_states
                            .iter()
                            .find(|new_player_state| {
                                new_player_state.state.player == other_player_channel.player
                            })
                            .map(|new_player_state| new_player_state.events.clone())
                            .unwrap_or_default();

                        println!(
                            "Sending {} events to player {:?}",
                            player_events.len(),
                            other_player_channel.player
                        );

                        for event in player_events {
                            let _ = other_player_channel.event_sender.send(event).await;
                        }
                    }
                }
                Err(e) => {
                    println!("Error applying action for player {:?}: {}", player, e);
                }
            }
        }
    }

    pub fn register_player(
        &mut self,
        player: Buffer,
    ) -> Result<(Buffer, PlayerChannel, Uuid), GameInstanceError> {
        let uuid = Uuid::new_v4();

        let player_states = &self
            .game
            .read()
            .map_err(|_| GameInstanceError::LockFailed)?
            .player_states;

        let player_state = player_states
            .iter()
            .find(|player_state| player_state.player == player);

        match player_state {
            Some(player_state) => {
                let player_channel = PlayerChannel::new(player.clone(), self.action_sender.clone());

                self.players
                    .write()
                    .map_err(|_| GameInstanceError::LockFailed)?
                    .insert(uuid, player_channel.clone());

                Ok((player_state.state.clone(), player_channel, uuid))
            }
            None => Err(GameInstanceError::PlayerNotInGame),
        }
    }

    pub fn unregister_player(&mut self, player_id: Uuid) -> Result<(), GameInstanceError> {
        self.players
            .write()
            .map_err(|_| GameInstanceError::LockFailed)?
            .remove(&player_id);

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum GameDbError {
    LockFailed,
    NotFound(Uuid),
}

impl std::fmt::Display for GameDbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameDbError::LockFailed => write!(f, "Failed to acquire lock on game database"),
            GameDbError::NotFound(game_id) => write!(f, "Game with ID {} not found", game_id),
        }
    }
}

impl ResponseError for GameDbError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            GameDbError::LockFailed => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            GameDbError::NotFound(_) => actix_web::http::StatusCode::NOT_FOUND,
        }
    }
}

impl GameDb {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    pub fn new_game(&self, game_id: Uuid, game_instance: GameInstance) {
        let mut db = self.0.write().unwrap();
        db.insert(game_id, game_instance);
    }

    pub fn get_game(&self, game_id: Uuid) -> Result<GameInstance, GameDbError> {
        let db = self.0.read().unwrap();
        db.get(&game_id)
            .cloned()
            .ok_or(GameDbError::NotFound(game_id))
    }

    pub fn games(&self) -> Vec<Uuid> {
        let db = self.0.read().unwrap();
        db.keys().cloned().collect()
    }
}
