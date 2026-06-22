use crate::bot_core;
use crate::component_db::ComponentDb;
use crate::db::GameInstanceStore;
use crate::friends;
use crate::game_core::{self, Buffer, Game, GameCore, NewPlayerState, Player, TakeActionResult};
use crate::lobby_db::{self, LobbyListNotify};
use actix_web::ResponseError;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::SqlitePool;
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
        "spectator_state": B64.encode(&game.spectator_state),
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
    pub full_state: bool,
}

impl PlayerChannel {
    pub fn new(
        player: Player,
        action_sender: async_channel::Sender<(Player, Buffer)>,
        full_state: bool,
    ) -> Self {
        let (event_sender, event_receiver) = async_channel::unbounded();

        PlayerChannel {
            player,
            action_sender,
            event_sender,
            event_receiver,
            full_state,
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
pub struct SpectatorChannel {
    event_sender: async_channel::Sender<Buffer>,
    event_receiver: async_channel::Receiver<Buffer>,
}

impl SpectatorChannel {
    pub fn new() -> Self {
        let (event_sender, event_receiver) = async_channel::unbounded();
        Self {
            event_sender,
            event_receiver,
        }
    }

    pub async fn receive_event(&self) -> Result<Buffer, async_channel::RecvError> {
        self.event_receiver.recv().await
    }
}

#[derive(Clone, Debug)]
pub struct BotSeatBinding {
    pub bot_slug: String,
    pub player: Buffer,
    pub settings: Buffer,
}

pub fn player_identity_to_buffer(ident: &str) -> Buffer {
    let t = ident.trim();
    if t.starts_with('{') || t.starts_with('[') {
        return t.as_bytes().to_vec();
    }
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        return t.as_bytes().to_vec();
    }
    serde_json::to_vec(&serde_json::Value::String(t.to_string()))
        .unwrap_or_else(|_| t.as_bytes().to_vec())
}

#[derive(Clone)]
pub struct GameRunPersistence {
    pub game_id: Uuid,
    pub store: Arc<GameInstanceStore>,
    pub lobby_id: Option<Uuid>,
    pub pool: SqlitePool,
    pub game_db: GameDb,
    pub lobby_notify: LobbyListNotify,
    pub component_db: ComponentDb,
    pub bot_bindings: Vec<BotSeatBinding>,
}

fn player_identity_utf8(buf: &[u8]) -> String {
    serde_json::from_slice::<String>(buf)
        .unwrap_or_else(|_| String::from_utf8_lossy(buf).into_owned())
}

fn game_over_from_event_json(raw: &[u8]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(raw).ok()?;
    match v.get("GameOver")? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(m) => {
            if m.contains_key("Draw") {
                Some("Draw".into())
            } else if m.contains_key("Win") {
                Some("Win".into())
            } else if m.contains_key("Loss") {
                Some("Loss".into())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// If this action tick ended the game, map seat identity (JSON string) → outcome label (Win / Loss / Draw).
fn terminal_outcomes_this_tick(
    player_states: &[NewPlayerState],
) -> Option<HashMap<String, String>> {
    let mut map = HashMap::new();
    for nps in player_states {
        let ident = player_identity_utf8(&nps.state.player);
        for ev in &nps.events {
            if let Some(out) = game_over_from_event_json(ev) {
                map.insert(ident.clone(), out);
            }
        }
    }
    if map.is_empty() { None } else { Some(map) }
}

fn float_scores_from_outcomes(outcomes: &HashMap<String, String>) -> HashMap<String, f64> {
    let n = outcomes.len().max(1) as f64;
    let all_draw = outcomes.values().all(|x| x == "Draw");
    if all_draw {
        return outcomes.keys().map(|k| (k.clone(), 1.0 / n)).collect();
    }
    let wins = outcomes.values().filter(|x| *x == "Win").count();
    if wins == 1 {
        return outcomes
            .iter()
            .map(|(k, o)| (k.clone(), if o == "Win" { 1.0 } else { 0.0 }))
            .collect();
    }
    outcomes.keys().map(|k| (k.clone(), 1.0 / n)).collect()
}

#[derive(Clone)]
pub struct GameInstance {
    game_type: String,
    players: Arc<RwLock<HashMap<Uuid, PlayerChannel>>>,
    spectators: Arc<RwLock<HashMap<Uuid, SpectatorChannel>>>,
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
            spectators: Arc::new(RwLock::new(HashMap::new())),
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
                        serde_json::from_str::<String>(&raw).unwrap_or_else(|_| raw.to_string())
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

            tracing::debug!(
                player = ?player,
                action = str::from_utf8(&action).unwrap_or("<invalid utf8>"),
                "game action received"
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

            tracing::debug!(
                outcome = if result.is_ok() {
                    "succeeded"
                } else {
                    "failed"
                },
                "game action processed"
            );

            match result {
                Ok(take_action_result) => {
                    let TakeActionResult {
                        new_game_full_state,
                        player_states,
                        spectator_events,
                        spectator_state,
                    } = take_action_result;

                    let new_game = Game {
                        full_state: new_game_full_state,
                        player_states: player_states
                            .iter()
                            .map(|new_player_state| new_player_state.state.clone())
                            .collect(),
                        spectator_state,
                    };

                    let snap = encode_game_snapshot(&new_game).unwrap_or_else(|e| {
                        tracing::error!(error = %e, "encode_game_snapshot failed");
                        "{}".to_string()
                    });

                    *game = new_game;

                    tracing::debug!(
                        connected_players = players.len(),
                        "broadcasting player events"
                    );

                    for (_other_player_id, other_player_channel) in players.iter() {
                        let player_events = player_states
                            .iter()
                            .find(|new_player_state| {
                                new_player_state.state.player == other_player_channel.player
                            });

                        if other_player_channel.full_state {
                            if let Some(nps) = player_events {
                                let _ = other_player_channel
                                    .event_sender
                                    .send(nps.state.state.clone())
                                    .await;
                                for event in &nps.events {
                                    if game_over_from_event_json(event).is_some() {
                                        let _ = other_player_channel
                                            .event_sender
                                            .send(event.clone())
                                            .await;
                                    }
                                }
                            }
                            continue;
                        }

                        let player_events = player_events
                            .map(|new_player_state| new_player_state.events.clone())
                            .unwrap_or_default();

                        tracing::trace!(
                            event_count = player_events.len(),
                            player = ?other_player_channel.player,
                            "sending player events"
                        );

                        for event in player_events {
                            let _ = other_player_channel.event_sender.send(event).await;
                        }
                    }

                    let spectators = self.spectators.read().unwrap();
                    for event in spectator_events {
                        for (_id, spectator_channel) in spectators.iter() {
                            let _ = spectator_channel.event_sender.send(event.clone()).await;
                        }
                    }
                    drop(spectators);

                    drop(players);
                    drop(game);
                    drop(game_core);

                    if let Some(p) = persistence.as_ref() {
                        if let Some(outcomes) = terminal_outcomes_this_tick(&player_states) {
                            let result_json = serde_json::to_string(&json!({
                                "version": 1,
                                "per_player_outcome": outcomes,
                            }))
                            .unwrap_or_else(|_| "{}".to_string());
                            let scores = float_scores_from_outcomes(&outcomes);
                            let scores_json =
                                serde_json::to_string(&scores).unwrap_or_else(|_| "{}".to_string());
                            let seats_json = if let Some(lid) = p.lobby_id {
                                match lobby_db::get_lobby(&p.pool, lid).await {
                                    Ok(Some(detail)) => {
                                        let seats: Vec<_> = detail
                                            .seats
                                            .iter()
                                            .map(|s| {
                                                json!({
                                                    "seat_index": s.seat_index,
                                                    "player_identity": s.player_identity,
                                                    "claimed_by_user_id": s.claimed_by_user_id.map(|u| u.to_string()),
                                                    "claimed_display_name": s.claimed_display_name,
                                                    "bot_id": s.bot_id.map(|u| u.to_string()),
                                                    "bot_display_name": s.bot_display_name,
                                                    "external_bot": s.external_bot,
                                                    "external_bot_category": s.external_bot_category,
                                                    "bot_avatar_seed": s.bot_avatar_seed,
                                                    "bot_avatar_url": s.bot_avatar_url,
                                                    "is_bot": s.bot_id.is_some(),
                                                    "is_transient": s.external_bot
                                                        && s.external_bot_category.as_deref() == Some("dev_local"),
                                                })
                                            })
                                            .collect();
                                        serde_json::to_string(&seats)
                                            .unwrap_or_else(|_| "[]".to_string())
                                    }
                                    _ => "[]".to_string(),
                                }
                            } else {
                                "[]".to_string()
                            };

                            if let Err(e) = p
                                .store
                                .finish_game_record(
                                    p.game_id,
                                    &snap,
                                    &result_json,
                                    &scores_json,
                                    &seats_json,
                                )
                                .await
                            {
                                tracing::error!(game_id = %p.game_id, error = %e, "finish_game_record failed");
                            }
                            if let Some(lid) = p.lobby_id {
                                let _ = lobby_db::mark_lobby_finished(&p.pool, lid).await;
                                p.lobby_notify.ping();
                                if let Ok(Some(detail)) = lobby_db::get_lobby(&p.pool, lid).await {
                                    let game_id_str = p.game_id.to_string();
                                    for seat in &detail.seats {
                                        if let Some(uid) = seat.claimed_by_user_id {
                                            let ident = seat.player_identity.clone();
                                            let kind = outcomes
                                                .get(&ident)
                                                .map(|o| {
                                                    if o == "Win" {
                                                        "game_won"
                                                    } else {
                                                        "game_finished"
                                                    }
                                                })
                                                .unwrap_or("game_finished");
                                            let _ = friends::insert_friend_activity(
                                                &p.pool,
                                                uid,
                                                kind,
                                                &game_id_str,
                                            )
                                            .await;
                                        }
                                    }
                                }
                            }
                            p.game_db.remove_game(p.game_id);
                            p.game_db.notify_game_list_changed();
                            break;
                        }
                    }

                    if let Some(p) = persistence.as_ref() {
                        if let Err(e) = p.store.update_game_state(p.game_id, &snap).await {
                            tracing::error!(game_id = %p.game_id, error = %e, "game state persist failed");
                        }
                        drive_bots(p, &player_states).await;
                    }
                }
                Err(e) => {
                    tracing::warn!(player = ?player, error = %e, "game action failed");
                }
            }
        }
    }

    pub fn register_player(
        &mut self,
        player: Buffer,
        full_state: bool,
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
                let player_channel =
                    PlayerChannel::new(player.clone(), self.action_sender.clone(), full_state);

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

    pub fn register_spectator(
        &mut self,
    ) -> Result<(Buffer, SpectatorChannel, Uuid), GameInstanceError> {
        let uuid = Uuid::new_v4();
        let spectator_state = self
            .game
            .read()
            .map_err(|_| GameInstanceError::LockFailed)?
            .spectator_state
            .clone();
        let channel = SpectatorChannel::new();
        self.spectators
            .write()
            .map_err(|_| GameInstanceError::LockFailed)?
            .insert(uuid, channel.clone());
        Ok((spectator_state, channel, uuid))
    }

    pub async fn unregister_spectator(&mut self, spectator_id: Uuid) -> Result<(), GameInstanceError> {
        self.spectators
            .write()
            .map_err(|_| GameInstanceError::LockFailed)?
            .remove(&spectator_id);
        Ok(())
    }

    pub async fn submit_bot_action(
        &self,
        player: Buffer,
        action: Buffer,
    ) -> Result<(), async_channel::SendError<(Player, Buffer)>> {
        self.action_sender.send((player, action)).await
    }

    pub async fn drive_bots_initial(
        &self,
        component_db: &ComponentDb,
        bindings: &[BotSeatBinding],
    ) {
        if bindings.is_empty() {
            return;
        }
        let player_states: Vec<NewPlayerState> = self
            .game
            .read()
            .ok()
            .map(|g| {
                g.player_states
                    .iter()
                    .map(|ps| NewPlayerState {
                        state: ps.clone(),
                        events: vec![],
                    })
                    .collect()
            })
            .unwrap_or_default();
        let shim = BotDriveShim {
            component_db: component_db.clone(),
            bot_bindings: bindings.to_vec(),
            action_sender: self.action_sender.clone(),
        };
        drive_bots_shim(&shim, &player_states).await;
    }
}

async fn drive_bots(p: &GameRunPersistence, player_states: &[NewPlayerState]) {
    if p.bot_bindings.is_empty() {
        return;
    }
    for binding in &p.bot_bindings {
        let state_buf = player_states
            .iter()
            .find(|nps| nps.state.player == binding.player)
            .map(|nps| nps.state.state.clone())
            .or_else(|| {
                // fallback: unchanged player state from snapshot not in this tick
                None
            });
        let Some(state_buf) = state_buf else {
            continue;
        };
        let Ok((bot, mut store)) = p.component_db.create_game_bot(&binding.bot_slug).await else {
            tracing::warn!(bot = %binding.bot_slug, "bot component not found");
            continue;
        };
        let result = bot
            .call_decide(
                &mut store,
                bot_core::SerializationFormat::Json,
                &binding.settings,
                &state_buf,
            )
            .await;
        match result {
            Ok(Ok(Some(action))) => {
                tracing::debug!(bot = %binding.bot_slug, "bot submitted action");
                if let Ok(gi) = p.game_db.get_game(p.game_id) {
                    let _ = gi
                        .submit_bot_action(binding.player.clone(), action)
                        .await;
                }
            }
            Ok(Ok(None)) => {}
            Ok(Err(e)) => tracing::warn!(bot = %binding.bot_slug, error = ?e, "bot decide error"),
            Err(e) => tracing::warn!(bot = %binding.bot_slug, error = %e, "bot wasm error"),
        }
    }
}

struct BotDriveShim {
    component_db: ComponentDb,
    bot_bindings: Vec<BotSeatBinding>,
    action_sender: async_channel::Sender<(Player, Buffer)>,
}

async fn drive_bots_shim(shim: &BotDriveShim, player_states: &[NewPlayerState]) {
    for binding in &shim.bot_bindings {
        let Some(state_buf) = player_states
            .iter()
            .find(|nps| nps.state.player == binding.player)
            .map(|nps| nps.state.state.clone())
        else {
            continue;
        };
        let Ok((bot, mut store)) = shim.component_db.create_game_bot(&binding.bot_slug).await else {
            continue;
        };
        if let Ok(Ok(Some(action))) = bot
            .call_decide(
                &mut store,
                bot_core::SerializationFormat::Json,
                &binding.settings,
                &state_buf,
            )
            .await
        {
            let _ = shim.action_sender.send((binding.player.clone(), action)).await;
        }
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

    pub fn remove_game(&self, game_id: Uuid) {
        let mut db = self.0.write().unwrap();
        db.games.remove(&game_id);
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
