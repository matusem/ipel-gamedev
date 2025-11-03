use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, ResponseError, rt, web};
use futures_util::StreamExt as _;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock, mpsc},
    time::Instant,
};
use uuid::Uuid;
use wasmtime::{Config, Engine, Store, component::Component};
use wasmtime_wasi::{WasiCtxBuilder, p1::WasiP1Ctx};

use crate::game_core::{Buffer, Game, GameCore, Player, TakeActionResult};

use base64::{Engine as _, engine::general_purpose::STANDARD};

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

    let (response, mut session, mut message_stream) = actix_ws::handle(&request, body)?;

    let GameRequestParams { game_id, player } = params;

    // let mut stream = stream
    //     .aggregate_continuations()
    //     .max_continuation_size(2_usize.pow(20));

    let (sender, mut receiver) = mpsc::channel::<Buffer>();

    let mut game_instance = game_db.get_game(game_id)?;
    let player_state = game_instance.register_player(player.clone(), sender)?;

    // let mut send_session = session.clone();
    // rt::spawn(async move {
    //     let _ = send_session.text("sup").await;

    //     while let Ok(msg) = receiver.recv() {
    //         // if send_session.text(msg).await.is_err() {
    //         //     break; // Exit if sending fails
    //         // }
    //     }
    // });

    // let mut send_session = session.clone();
    rt::spawn(async move {
        let game_instance = game_instance.clone();
        let _ = session.text(str::from_utf8(&player_state).unwrap()).await;

        while let Some(msg) = message_stream.next().await {
            match msg {
                Ok(actix_ws::Message::Text(text)) => {
                    session.text("echo: ".to_string() + &text).await.unwrap();
                    println!("Received text message: {}", text);

                    let json = STANDARD.decode(text);

                    game_instance
                        .inner()
                        .unwrap()
                        .game_core
                        .call_take_action(
                            &mut game_instance.inner().unwrap().store,
                            game::SerializationFormat::Json,
                            &json.unwrap(),
                            &player,
                        )
                        .await
                        .unwrap()
                        .map_err(|e| {
                            println!("Error applying action: {}", e);
                        })
                        .ok();
                }
                _ => {}
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
    let params = CreateGameParams::parse(&request)?;

    let wasm_bytes = std::fs::read("./target/wasm32-wasip1/release/wasm.wasm")
        .expect("Wasm module not found, build wasm_game first");
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
    game_db.new_game(game_id, GameInstanceInner::new(game, store, game_core));

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(game_id.to_string()))
}

async fn get_games(
    request: HttpRequest,
    game_db: web::Data<GameDb>,
) -> Result<HttpResponse, Error> {
    let games = game_db
        .read()
        .expect("Failed to lock game database")
        .iter()
        .map(|(id, _)| id.to_string())
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&games).unwrap()))
}

type ClientId = u64;

struct GameInstanceInner {
    game: Game,
    game_core: GameCore,
    players: Vec<(Player, mpsc::Sender<Buffer>)>,
}

impl GameInstanceInner {
    fn new(game: Game, game_core: GameCore) -> Self {
        Self {
            game,
            game_core,
            players: vec![],
        }
    }
}

// #[derive(Clone)]
// struct GameInstance {
//     store: Store<WasiP1Ctx>,
//     // inner: Arc<Mutex<GameInstanceInner>>,
//     game: Game,
//     game_core: GameCore,
//     actors: Vec<
//     players: Vec<(Player, mpsc::Sender<Buffer>)>,
// }

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

struct PlayerChannel {
    player: Player,
    sender: mpsc::Sender<Buffer>,
    receiver: mpsc::Receiver<Buffer>,
}

pub struct GameInstance {
    players: Arc<RwLock<HashMap<Uuid, PlayerChannel>>>,
    store: Store<WasiP1Ctx>,
    game: Arc<RwLock<Game>>,
    game_core: GameCore,
}

impl GameInstance {
    fn new(engine: &Engine, game: Game, game_core: GameCore) -> Self {
        let store = Store::new(&engine, WasiCtxBuilder::new().build_p1());

        Self {
            players: Arc::new(RwLock::new(HashMap::new())),
            store,
            game: Arc::new(RwLock::new(game)),
            game_core,
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

    async fn run(&mut self) {
        loop {
            let player_ids = self.get_player_ids();

            for player_id in player_ids {
                let players = self.players.write().unwrap();
                let mut player_channel = players.get(&player_id).unwrap();

                while let Ok(action) = player_channel.receiver.try_recv() {
                    let result = self
                        .game_core
                        .call_take_action(
                            &mut self.store,
                            game_core::SerializationFormat::Json,
                            &self.game.read().unwrap(),
                            &(player_channel.player.clone(), action),
                        )
                        .await
                        .map(|result| {
                            result.map_err(|game_core_error| {
                                GameInstanceError::GameCore(game_core_error)
                            })
                        })
                        .map_err(|wasm_error| GameInstanceError::Wasm(wasm_error))
                        .flatten();

                    match result {
                        Ok(take_action_result) => {
                            let mut game = self.game.write().unwrap();

                            let new_game_full_state = take_action_result.new_game_full_state;

                            game.full_state.clear();

                            // Send updated player states
                            for (other_player_id, other_player_channel) in
                                self.players.read().unwrap().iter()
                            {
                                let player_state = game
                                    .player_states
                                    .iter()
                                    .find(|ps| ps.player == other_player_channel.player)
                                    .map(|ps| ps.state.clone())
                                    .unwrap_or_default();

                                let _ = other_player_channel.sender.send(player_state);
                            }
                        }
                        Err(e) => {
                            println!("Error applying action for player {}: {}", player_id, e);
                        }
                    }
                }
            }
        }
    }

    fn register_player(&mut self, player: Vec<u8>) -> Result<(Buffer, P), GameInstanceError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| GameInstanceError::LockFailed)?;

        let player_states = &inner.game.player_states;

        let player_state = player_states
            .iter()
            .find(|player_state| *player_state.player == player)
            .cloned();

        match player_state {
            Some(player_state) => {
                inner.players.push((player_state.player, sender));
                Ok(player_state.state)
            }
            None => Err(GameInstanceError::PlayerNotInGame),
        }
    }

    async fn try_apply_action(
        &mut self,
        player: Player,
        action: Buffer,
    ) -> Result<TakeActionResult, GameInstanceError> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| GameInstanceError::LockFailed)?;

        let inner = self
            .inner
            .lock()
            .map_err(|_| GameInstanceError::LockFailed)?;

        let action = inner.game_core.call_take_action(
            store.data_mut(),
            game_core::SerializationFormat::Json,
            &inner.game,
            &(player, action),
        );

        let result = action
            .await
            .map(|result| {
                result.map_err(|game_core_error| GameInstanceError::GameCore(game_core_error))
            })
            .map_err(|wasm_error| GameInstanceError::Wasm(wasm_error))
            .flatten();

        result
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

    fn get_game(&self, game_id: Uuid) -> Result<GameInstance, GetGameError> {
        let db = self.0.read().unwrap();
        db.get(&game_id)
            .cloned()
            .ok_or(GetGameError::NotFound(game_id))
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let game_db = GameDb::new();
    let game_db = web::Data::new(game_db.clone());

    let engine = {
        let mut config = Config::default();
        config.async_support(true);
        Engine::new(&config).unwrap()
    };

    HttpServer::new(move || {
        App::new()
            .app_data(game_db.clone())
            .app_data(engine.clone())
            .route("/create_game", web::post().to(create_game))
            .route("/game", web::get().to(game))
            .route("/games", web::get().to(get_games))
    })
    .bind(("127.0.0.1", 80))?
    .run()
    .await
}
