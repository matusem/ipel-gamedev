use crate::game_db::GameInstance;
use crate::game_registry::GameRegistry;
use crate::{component_db::ComponentDb, game_core::Buffer, game_db::GameDb};
use actix_files::Files;
use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, rt, web};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

mod component_db;
mod game_db;
mod game_registry;

mod game_core {
    use wasmtime::component::bindgen;

    bindgen!({
        path: "../test.wit",
        world: "game-core",
        imports: { default: async | trappable },
        exports: { default: async }
    });
}

fn player_identity_from_query_param(param: &str) -> Buffer {
    let t = param.trim();
    if t.starts_with('{') || t.starts_with('[') {
        return t.as_bytes().to_vec();
    }
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        return t.as_bytes().to_vec();
    }
    serde_json::to_vec(&serde_json::Value::String(t.to_string())).unwrap_or_else(|_| t.as_bytes().to_vec())
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

        // Game logic stores identities as JSON (e.g. enum "X" -> bytes b"\"X\""). Plain ?player=X
        // must match those buffers, so we JSON-encode simple tokens; pass-through if already JSON.
        let player: Buffer = player_identity_from_query_param(player);

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

    let (response, mut session, stream) = actix_ws::handle(&request, body)?;
    let mut stream = stream
        .aggregate_continuations()
        .max_continuation_size(2_usize.pow(20));

    let player_state = {
        let mut game_instance = game_db.get_game(game_id)?;
        game_instance.register_player(player.clone())?
    };

    println!("Sending initial state to player");
    let initial = String::from_utf8_lossy(&player_state.0);
    let _ = session.text(initial.as_ref()).await;

    println!("Player registered with ID: {:?}", player_state.2);

    rt::spawn(async move {
        loop {
            tokio::select! {
                player_event = player_state.1.receive_event() => {
                    match player_event {
                        Ok(buffer) => {
                            let out = String::from_utf8_lossy(&buffer);
                            let _ = session.text(out.as_ref()).await;
                        }
                        Err(e) => {
                            eprintln!("game WS event channel closed: {e:?}");
                            break;
                        }
                    }
                }
                msg = stream.recv() => {
                    match msg {
                        Some(Ok(msg)) => match msg {
                            actix_ws::AggregatedMessage::Text(text) => {
                                let _ = player_state
                                    .1
                                    .send_action(text.into_bytes().to_vec())
                                    .await;
                            }
                            actix_ws::AggregatedMessage::Binary(bin) => {
                                let _ = player_state.1.send_action(bin.to_vec()).await;
                            }
                            actix_ws::AggregatedMessage::Ping(bytes) => {
                                let _ = session.pong(&bytes).await;
                            }
                            actix_ws::AggregatedMessage::Pong(_) => {}
                            actix_ws::AggregatedMessage::Close(_) => break,
                        },
                        Some(Err(e)) => {
                            eprintln!("game WS protocol error: {e:?}");
                            break;
                        }
                        None => break,
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
    let game_type = params.game.clone();

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
    game_db.new_game(game_id, GameInstance::new(game, game_core, game_type));

    rt::spawn(async move {
        let mut game = game_db.get_game(game_id).unwrap();
        game.run(&engine, store).await;
    });

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(game_id.to_string()))
}

#[derive(Serialize, Deserialize)]
struct GameTypeInfo {
    name: String,
    display_name: String,
    version: String,
    min_players: u32,
    max_players: u32,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    config_ui_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    config_schema: Option<serde_json::Value>,
}

async fn get_game_types(registry: web::Data<GameRegistry>) -> Result<HttpResponse, Error> {
    let types: Vec<GameTypeInfo> = registry
        .game_types()
        .iter()
        .map(|gt| GameTypeInfo {
            name: gt.manifest.name.clone(),
            display_name: gt.manifest.display_name.clone(),
            version: gt.manifest.version.clone(),
            min_players: gt.manifest.min_players,
            max_players: gt.manifest.max_players,
            description: gt.manifest.description.clone(),
            config_ui_path: gt.config_ui_path.clone(),
            config_schema: gt.manifest.config_schema.clone(),
        })
        .collect();

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&types).unwrap()))
}

#[derive(Serialize, Deserialize)]
struct GameInfo {
    game_id: String,
    game_type: String,
    player_identities: Vec<String>,
    connected_players: usize,
}

async fn get_games(game_db: web::Data<GameDb>) -> Result<HttpResponse, Error> {
    let game_infos = game_db.list_games();

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

    let games_dir = PathBuf::from(std::env::var("GAMES_DIR").unwrap_or_else(|_| "./games".into()));
    let lobby_dir = PathBuf::from(std::env::var("LOBBY_DIR").unwrap_or_else(|_| "./lobby".into()));
    let lib_dir = PathBuf::from(std::env::var("LIB_DIR").unwrap_or_else(|_| "./lib".into()));

    let game_db = web::Data::new(GameDb::new());

    let component_db = ComponentDb::new();
    let registry = GameRegistry::load(&games_dir, &component_db);
    let component_db = web::Data::new(component_db.clone());
    let registry_data = web::Data::new(registry.clone());

    println!("Starting server on {}:{}", host, port);

    HttpServer::new(move || {
        let mut app = App::new()
            .app_data(game_db.clone())
            .app_data(component_db.clone())
            .app_data(registry_data.clone())
            .route("/api/create_game", web::post().to(create_game))
            .route("/api/game_types", web::get().to(get_game_types))
            .route("/api/games", web::get().to(get_games))
            .route("/game", web::get().to(game));

        for gt in registry.game_types() {
            if gt.client_dir.exists() {
                let route = format!("/games/{}", gt.manifest.name);
                app = app.service(
                    Files::new(&route, &gt.client_dir)
                        .index_file("index.html"),
                );
            }
        }

        if lib_dir.exists() {
            app = app.service(Files::new("/lib", &lib_dir));
        }

        if lobby_dir.exists() {
            app = app.service(Files::new("/", &lobby_dir).index_file("index.html"));
        }

        app
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}
