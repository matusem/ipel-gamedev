use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, rt, web};
use actix_ws::AggregatedMessage;
use base64::engine::general_purpose::PAD;
use futures_util::StreamExt as _;
use rmp_serde::{Deserializer as RmpDeserializer, Serializer as RmpSerializer};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    f32::consts::E,
    sync::{Arc, Mutex, RwLock, mpsc},
};
use uuid::Uuid;
use wasmtime::{
    Config, Engine, Linker, Module, Store,
    component::{Component, bindgen},
};
use wasmtime_wasi::{WasiCtxBuilder, p1::WasiP1Ctx};

use crate::{
    game_core::{Buffer, Game, GameCore, Player},
    wasm::Wasm,
};

mod wasm;

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

        use base64::Engine;
        use base64::engine::general_purpose::STANDARD;

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

    let (response, mut session, mut stream) = actix_ws::handle(&request, body)?;

    // let mut stream = stream
    //     .aggregate_continuations()
    //     .max_continuation_size(2_usize.pow(20));

    let (sender, mut receiver) = mpsc::channel::<Buffer>();

    let player = params.player.clone();

    {
        let mut games = game_db.write().expect("Failed to lock game database");
        games
            .get_mut(&params.game_id)
            .ok_or_else(|| actix_web::error::ErrorNotFound("Game not found"))?
            .lock()
            .unwrap()
            .register_player(player.clone(), sender);
    };

    // {
    //     let mut games = game_db.write().expect("Failed to lock game database");
    //     games
    //         .get_mut(&params.game_id)
    //         .ok_or_else(|| actix_web::error::ErrorNotFound("Game not found"))?
    //         .lock()
    //         .unwrap()
    //         .game
    //         .player_states
    //         .iter()
    //         .for_each(|a| println!("{:?}", str::from_utf8(&a.player).unwrap()));
    // };

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
        {
            let games = game_db.read().expect("Failed to lock game database");
            let game_instance = games
                .get(&params.game_id)
                .ok_or_else(|| actix_web::error::ErrorNotFound("Game not found"))
                .unwrap()
                .lock()
                .unwrap();

            let player_state = game_instance
                .game
                .player_states
                .iter()
                .find(|player_state| player_state.player == player.clone())
                .unwrap();

            let _ = session
                .text(str::from_utf8(&player_state.state).unwrap())
                .await;
        }

        while let Some(msg) = stream.next().await {
            match msg {
                Ok(actix_ws::Message::Text(text)) => {
                    session.text("echo: ".to_string() + &text).await.unwrap();
                    println!("Received text message: {}", text);
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
) -> Result<HttpResponse, Error> {
    let params = CreateGameParams::parse(&request)?;

    let mut config = Config::default();
    config.async_support(true);
    let engine = Engine::new(&config).unwrap();

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
    let game_db_clone = game_db.clone();
    {
        let mut db = game_db_clone.write().unwrap();
        db.insert(
            game_id.clone(),
            Arc::new(Mutex::new(GameInstance::new(game, store, game_core))),
        );
    }

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

struct GameInstance {
    game: Game,
    store: Store<WasiP1Ctx>,
    game_core: GameCore,
    players: Vec<(Player, mpsc::Sender<Buffer>)>,
}

impl GameInstance {
    fn new(game: Game, store: Store<WasiP1Ctx>, game_core: GameCore) -> Self {
        Self {
            game,
            store,
            game_core,
            players: vec![],
        }
    }

    fn register_player(&mut self, player: Player, sender: mpsc::Sender<Buffer>) {
        self.players.push((player, sender));
    }
}

type GameDb = Arc<RwLock<HashMap<Uuid, Arc<Mutex<GameInstance>>>>>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let game_db: GameDb = Arc::new(RwLock::new(HashMap::new()));
    let game_db = web::Data::new(game_db.clone());

    HttpServer::new(move || {
        App::new()
            .app_data(game_db.clone())
            .route("/create_game", web::post().to(create_game))
            .route("/game", web::get().to(game))
            .route("/games", web::get().to(get_games))
    })
    .bind(("127.0.0.1", 80))?
    .run()
    .await
}
