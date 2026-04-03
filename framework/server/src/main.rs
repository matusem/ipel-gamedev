use crate::game_db::GameInstance;
use crate::{component_db::ComponentDb, game_core::Buffer, game_db::GameDb};
use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, rt, web};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

mod component_db;
mod game_db;

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
    player: Buffer,
}

impl GameRequestParams {
    fn parse(request: &HttpRequest) -> Result<Self, actix_web::Error> {
        let query = request
            .uri()
            .query()
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing query parameters"))?;

        let params: HashMap<String, String> = serde_urlencoded::from_str(query).unwrap();

        let game_id = params
            .get("id")
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'id' parameter"))?;

        let game_id = Uuid::parse_str(game_id)
            .map_err(|_| actix_web::error::ErrorBadRequest("Invalid 'id' format"))?;

        let player = params
            .get("player")
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'player' parameter"))?;

        let player: Buffer = player.as_bytes().to_vec();

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

    let player_state = {
        let mut game_instance = game_db.get_game(game_id)?;
        game_instance.register_player(player.clone())?
    };

    println!("Sending initial state to player");
    let _ = session.text(str::from_utf8(&player_state.0).unwrap()).await;

    println!("Player registered with ID: {:?}", player_state.2);

    rt::spawn(async move {
        loop {
            tokio::select! {
                player_event = player_state.1.receive_event() => {
                    println!("Received event for player: {:?}", player_event);

                    if let Ok(buffer) = player_event {
                        println!("Sending event to player: {}", str::from_utf8(&buffer).unwrap());
                        let _ = session.text(str::from_utf8(&buffer).unwrap()).await;
                    }
                }
                msg = stream.recv() => {
                    if let Some(Ok(msg)) = msg {
                        println!("Received message from player: {:?}", msg);
                        match msg {
                            actix_ws::Message::Text(text) => {
                                let _ = player_state.1.send_action(text.into_bytes().to_vec()).await;
                            }
                            actix_ws::Message::Close(reason) => {
                                break;
                            }
                            _ => {}
                        }
                    }
                    else {
                        break;
                    }
                }
            }
        }

        let _ = session.close(None).await;

        let mut game_instance = game_db.get_game(game_id).unwrap();
        let _ = game_instance.unregister_player(player_state.2);
    });

    Ok(response)
}

struct CreateGameParams {
    game: String,
    config: Buffer,
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

        let config = params
            .get("config")
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'config' parameter"))?
            .to_string();

        let config = config.as_bytes().to_vec();

        Ok(Self { game, config })
    }
}

async fn create_game(
    request: HttpRequest,
    game_db: web::Data<GameDb>,
    component_db: web::Data<ComponentDb>,
) -> Result<HttpResponse, Error> {
    println!("Creating new game...");
    let params = CreateGameParams::parse(&request)?;

    let engine = component_db.get_engine();

    let (game_core, mut store) = component_db
        .create_game_core(&params.game)
        .await
        .map_err(|error| actix_web::error::ErrorNotAcceptable(error))?;

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

#[derive(Serialize, Deserialize)]
struct GameInfo {
    game_id: String,
    players: Vec<String>,
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

    let game_infos: Vec<GameInfo> = games
        .iter()
        .map(|game_id| GameInfo {
            game_id: game_id.clone(),
            players: game_db
                .get_game(Uuid::parse_str(game_id).unwrap())
                .unwrap()
                .get_player_ids()
                .iter()
                .map(|player_id| player_id.to_string())
                .collect(),
        })
        .collect();

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&game_infos).unwrap()))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let game_db = web::Data::new(GameDb::new());

    let wasm_path = std::env::var("WASM_PATH")
        .unwrap_or_else(|_| "./wasm.wasm".into());
    let wasm_bytes = std::fs::read(&wasm_path)
        .unwrap_or_else(|_| panic!("Wasm module not found at '{}', build wasm first", wasm_path));
    let component_db = ComponentDb::new();
    let _ = component_db.insert_components_as_wasm_bytes("tic_tac_toe", &wasm_bytes);
    let component_db = web::Data::new(component_db.clone());

    println!("Starting server on {}:{}", host, port);

    HttpServer::new(move || {
        App::new()
            .app_data(game_db.clone())
            .app_data(component_db.clone())
            .route("/create_game", web::post().to(create_game))
            .route("/game", web::get().to(game))
            .route("/games", web::get().to(get_games))
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}
