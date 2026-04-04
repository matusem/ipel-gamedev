use crate::db::GameInstanceStore;
use crate::game_core::{self, Buffer, Game, GameCore, Player, TakeActionResult};
use actix_web::ResponseError;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::broadcast;
use uuid::Uuid;
use wasmtime::{Engine, Store};
use wasmtime_wasi::p1::WasiP1Ctx;

/// JSON snapshot of WASM `game` (base64 for binary buffers) for DB durability / future replay.
pub fn encode_game_snapshot(game: &Game) -> Result<String, serde_json::Error> {
    let player_states: Vec<_> = game
        .player_states
        .iter()
        .map(|ps| {
            json!({
                "player": B64.encode(&ps.player),
                "state": B64.encode(&ps.state),
            })
        })
        .collect();
    let v = json!({
        "full_state": B64.encode(&game.full_state),
        "player_states": player_states,
    });
    serde_json::to_string(&v)
}

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
pub struct GameRunPersistence {
    pub game_id: Uuid,
    pub store: Arc<GameInstanceStore>,
}

#[derive(Clone)]
pub struct GameInstance {
    game_type: String,
    players: Arc<RwLock<HashMap<Uuid, PlayerChannel>>>,
    game: Arc<RwLock<Game>>,
    game_core: Arc<RwLock<GameCore>>,
    action_sender: async_channel::Sender<(Player, Buffer)>,
    action_receiver: async_channel::Receiver<(Player, Buffer)>,
}

impl GameInstance {
    pub fn new(game: Game, game_core: GameCore, game_type: String) -> Self {
        let (action_sender, action_receiver) = async_channel::unbounded();

        Self {
            game_type,
            players: Arc::new(RwLock::new(HashMap::new())),
            game: Arc::new(RwLock::new(game)),
            game_core: Arc::new(RwLock::new(game_core)),
            action_sender,
            action_receiver,
        }
    }

    pub fn game_type(&self) -> &str {
        &self.game_type
    }

    pub fn player_identities(&self) -> Vec<String> {
        self.game
            .read()
            .map(|game| {
                game.player_states
                    .iter()
                    .map(|ps| {
                        let raw = String::from_utf8_lossy(&ps.player);
                        serde_json::from_str::<String>(&raw)
                            .unwrap_or_else(|_| raw.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn connected_player_count(&self) -> usize {
        self.players.read().map(|p| p.len()).unwrap_or(0)
    }

    pub async fn run(
        &mut self,
        _engine: &Engine,
        mut store: Store<WasiP1Ctx>,
        persistence: Option<GameRunPersistence>,
    ) {
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

                    let new_game = Game {
                        full_state: new_game_full_state,
                        player_states: player_states
                            .iter()
                            .map(|new_player_state| new_player_state.state.clone())
                            .collect(),
                    };

                    let snap = encode_game_snapshot(&new_game).ok();

                    *game = new_game;

                    println!(
                        "Action successful, sending events to {} connected players",
                        players.len()
                    );

                    for (_other_player_id, other_player_channel) in players.iter() {
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

                    drop(players);
                    drop(game);
                    drop(game_core);

                    if let (Some(p), Some(json)) = (persistence.as_ref(), snap) {
                        if let Err(e) = p.store.update_game_state(p.game_id, &json).await {
                            eprintln!("game_instances state persist failed: {e}");
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

struct GameDbInner {
    games: HashMap<Uuid, GameInstance>,
    list_notify: Option<broadcast::Sender<()>>,
}

#[derive(Clone)]
pub struct GameDb(Arc<RwLock<GameDbInner>>);

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
    pub fn new(list_notify: Option<broadcast::Sender<()>>) -> Self {
        Self(Arc::new(RwLock::new(GameDbInner {
            games: HashMap::new(),
            list_notify,
        })))
    }

    fn ping_list_subscribers(&self) {
        if let Ok(guard) = self.0.read() {
            if let Some(tx) = &guard.list_notify {
                let _ = tx.send(());
            }
        }
    }

    /// For GraphQL subscriptions; `None` if the server was built without a list broadcast channel.
    pub fn subscribe_game_list(&self) -> Option<broadcast::Receiver<()>> {
        self.0
            .read()
            .ok()
            .and_then(|g| g.list_notify.as_ref().map(|s| s.subscribe()))
    }

    pub fn notify_game_list_changed(&self) {
        self.ping_list_subscribers();
    }

    pub fn new_game(&self, game_id: Uuid, game_instance: GameInstance) {
        let mut db = self.0.write().unwrap();
        db.games.insert(game_id, game_instance);
        drop(db);
        self.ping_list_subscribers();
    }

    pub fn get_game(&self, game_id: Uuid) -> Result<GameInstance, GameDbError> {
        let db = self.0.read().unwrap();
        db.games
            .get(&game_id)
            .cloned()
            .ok_or(GameDbError::NotFound(game_id))
    }

    pub fn games(&self) -> Vec<Uuid> {
        let db = self.0.read().unwrap();
        db.games.keys().cloned().collect()
    }

    pub fn list_games(&self) -> Vec<GameListEntry> {
        let db = self.0.read().unwrap();
        db.games
            .iter()
            .map(|(id, instance)| GameListEntry {
                game_id: id.to_string(),
                game_type: instance.game_type().to_string(),
                player_identities: instance.player_identities(),
                connected_players: instance.connected_player_count(),
            })
            .collect()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GameListEntry {
    pub game_id: String,
    pub game_type: String,
    pub player_identities: Vec<String>,
    pub connected_players: usize,
}
