use crate::game_core::{Buffer, Game, GameCore, Player, TakeActionResult};
use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, ResponseError, rt, web};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use uuid::Uuid;
use wasmtime::{Config, Engine, Store, component::Component};
use wasmtime_wasi::{WasiCtxBuilder, p1::WasiP1Ctx};

mod game_core {
    use wasmtime::component::bindgen;

    bindgen!({
        path: "../test.wit",
        world: "game-core",
        imports: { default: async | trappable },
        exports: { default: async }
    });
}

struct GameRequestParams {
    game_id: Uuid,
    player: Vec<u8>,
}

impl GameRequestParams {
    fn parse(request: &HttpRequest) -> Result<Self, actix_web::Error> {
        let query = request
            .uri()
            .query()
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing query parameters"))?;

        let params: HashMap<String, String> = serde_urlencoded::from_str(query).unwrap();

        let game_id = params
            .get("game-id")
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'game-id' parameter"))?;

        let game_id = Uuid::parse_str(game_id)
            .map_err(|_| actix_web::error::ErrorBadRequest("Invalid 'game-id' format"))?;

        let player = params
            .get("player")
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'player' parameter"))?;

        let player: Vec<u8> = STANDARD
            .decode(player)
            .map_err(|_| actix_web::error::ErrorBadRequest("Invalid base64 string"))?;

        Ok(Self { game_id, player })
    }
}

async fn game(
    request: HttpRequest,
    body: web::Payload,
    game_db: web::Data<GameDb>,
) -> Result<HttpResponse, Error> {
    let params = GameRequestParams::parse(&request)?;
    let GameRequestParams { game_id, player } = params;

    let (response, mut session, mut stream) = actix_ws::handle(&request, body)?;

    // let mut stream = message_stream
    //     .aggregate_continuations()
    //     .max_continuation_size(2_usize.pow(20));

    let mut game_instance = game_db.get_game(game_id)?;
    let player_state = game_instance.register_player(player.clone())?;

    println!("Sending initial state to player");
    let _ = session.text(str::from_utf8(&player_state.0).unwrap()).await;

    println!("Player registered with ID: {:?}", player_state.2);

    rt::spawn(async move {
        loop {
            tokio::select! {
                player_event = player_state.1.event_receiver.recv() => {
                    println!("Received event for player: {:?}", player_event);

                    if let Ok(buffer) = player_event {
                        println!("Sending event to player: {}", str::from_utf8(&buffer).unwrap());
                        let _ = session.text(str::from_utf8(&buffer).unwrap()).await.is_err();
                    }
                }
                msg = stream.recv() => {
                    if let Some(Ok(msg)) = msg {
                        println!("Received message from player: {:?}", msg);
                        if let actix_ws::Message::Text(text) = msg {
                            let _ = player_state.1.action_sender.send((player.clone(), text.into_bytes().to_vec())).await;
                        }
                    }
                    else {
                        break;
                    }
                }
            }
        }

        let _ = session.close(None).await;
    });

    Ok(response)
}

struct CreateGameParams {
    game: String,
    config: Vec<u8>,
}

impl CreateGameParams {
    fn parse(request: &HttpRequest) -> Result<Self, actix_web::Error> {
        let query = request
            .uri()
            .query()
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing query parameters"))?;

        let params: HashMap<String, String> = serde_urlencoded::from_str(query).unwrap();

        let game = params
            .get("game")
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'game' parameter"))?
            .to_string();

        let config_base64 = params
            .get("config_base64")
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'config_base64' parameter"))?
            .to_string();

        use base64::Engine;
        use base64::engine::general_purpose::STANDARD;

        let config_base64: Vec<u8> = STANDARD
            .decode(config_base64)
            .map_err(|_| actix_web::error::ErrorBadRequest("Invalid base64 string"))?;

        println!("Config: {:?}", config_base64);

        Ok(Self {
            game,
            config: config_base64,
        })
    }
}

async fn create_game(
    request: HttpRequest,
    game_db: web::Data<GameDb>,
    engine: web::Data<Engine>,
) -> Result<HttpResponse, Error> {
    println!("Creating new game...");
    let params = CreateGameParams::parse(&request)?;

    let wasm_path = std::env::var("WASM_PATH")
        .unwrap_or_else(|_| "./wasm.wasm".into());
    let wasm_bytes = std::fs::read(&wasm_path)
        .unwrap_or_else(|_| panic!("Wasm module not found at '{}', build wasm first", wasm_path));
    let component = Component::new(&engine, &wasm_bytes).unwrap();

    let mut linker = wasmtime::component::Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker).unwrap();

    let mut store = Store::new(&engine, WasiCtxBuilder::new().build_p1());
    let game_core = GameCore::instantiate_async(&mut store, &component, &linker)
        .await
        .unwrap();

    let game = game_core
        .call_init(
            &mut store,
            game_core::SerializationFormat::Json,
            &params.config,
        )
        .await
        .map_err(|error| actix_web::error::ErrorNotAcceptable(error))?
        .map_err(|error| actix_web::error::ErrorNotAcceptable(error))?;

    let game_id = Uuid::new_v4();
    game_db.new_game(game_id, GameInstance::new(game, game_core));

    rt::spawn(async move {
        let mut game = game_db.get_game(game_id).unwrap();
        game.run(&engine, store).await;
    });

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(game_id.to_string()))
}

async fn get_games(
    request: HttpRequest,
    game_db: web::Data<GameDb>,
) -> Result<HttpResponse, Error> {
    println!("Getting games...");

    let games = game_db
        .games()
        .iter()
        .map(|uuid| uuid.to_string())
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&games).unwrap()))
}

type ClientId = u64;

#[derive(Clone)]
struct GameDb(Arc<RwLock<HashMap<Uuid, GameInstance>>>);

#[derive(Debug)]
enum GameInstanceError {
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
struct PlayerChannel {
    player: Player,
    action_sender: async_channel::Sender<(Player, Buffer)>,
    event_sender: async_channel::Sender<Buffer>,
    event_receiver: async_channel::Receiver<Buffer>,
}

impl PlayerChannel {
    fn new(player: Player, action_sender: async_channel::Sender<(Player, Buffer)>) -> Self {
        let (event_sender, event_receiver) = async_channel::unbounded();

        PlayerChannel {
            player,
            action_sender,
            event_sender,
            event_receiver,
        }
    }

    async fn send_action(
        &self,
        action: Buffer,
    ) -> Result<(), async_channel::SendError<(Player, Buffer)>> {
        self.action_sender.send((self.player.clone(), action)).await
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
    fn new(game: Game, game_core: GameCore) -> Self {
        let (action_sender, action_receiver) = async_channel::unbounded();

        Self {
            players: Arc::new(RwLock::new(HashMap::new())),
            game: Arc::new(RwLock::new(game)),
            game_core: Arc::new(RwLock::new(game_core)),
            action_sender,
            action_receiver,
        }
    }

    fn get_player_ids(&self) -> Vec<Uuid> {
        self.players
            .read()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    }

    async fn run(&mut self, engine: &Engine, mut store: Store<WasiP1Ctx>) {
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

    fn register_player(
        &mut self,
        player: Vec<u8>,
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
}

#[derive(Clone, Debug)]
enum GameDbError {
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
    fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    fn new_game(&self, game_id: Uuid, game_instance: GameInstance) {
        let mut db = self.0.write().unwrap();
        db.insert(game_id, game_instance);
    }

    fn get_game(&self, game_id: Uuid) -> Result<GameInstance, GameDbError> {
        let db = self.0.read().unwrap();
        db.get(&game_id)
            .cloned()
            .ok_or(GameDbError::NotFound(game_id))
    }

    fn games(&self) -> Vec<Uuid> {
        let db = self.0.read().unwrap();
        db.keys().cloned().collect()
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let game_db = GameDb::new();
    let game_db = web::Data::new(game_db.clone());

    let engine = {
        let mut config = Config::default();
        config.async_support(true);
        web::Data::new(Engine::new(&config).unwrap())
    };

    println!("Starting server on {}:{}", host, port);

    HttpServer::new(move || {
        App::new()
            .app_data(game_db.clone())
            .app_data(engine.clone())
            .route("/create_game", web::post().to(create_game))
            .route("/game", web::get().to(game))
            .route("/games", web::get().to(get_games))
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}
