use crate::game_db::GameDb;
use crate::game_registry::GameRegistry;
use crate::graphql_api::AppSchema;
use crate::{component_db::ComponentDb, game_core::Buffer};
use actix_files::Files;
use actix_web::guard;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Result as ActixResult, rt};
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::Data as GqlData;
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

mod component_db;
mod db;
mod game_db;
mod game_registry;
mod game_service;
mod graphql_api;

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
    serde_json::to_vec(&serde_json::Value::String(t.to_string()))
        .unwrap_or_else(|_| t.as_bytes().to_vec())
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

    game_db.notify_game_list_changed();

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
        game_db.notify_game_list_changed();
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
    game_store: web::Data<Arc<db::GameInstanceStore>>,
) -> Result<HttpResponse, Error> {
    println!("Creating new game...");
    let params = CreateGameParams::parse(&request)?;

    let game_id = game_service::create_and_spawn_game(
        component_db.get_ref(),
        game_db.get_ref(),
        game_store.get_ref().clone(),
        params.game,
        params.config,
    )
    .await
    .map_err(|e| actix_web::error::ErrorNotAcceptable(e))?;

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

async fn graphql_playground() -> ActixResult<HttpResponse> {
    let html = playground_source(GraphQLPlaygroundConfig::new("/graphql").subscription_endpoint("/graphql"));
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

async fn graphql_post(
    schema: web::Data<AppSchema>,
    pool: web::Data<SqlitePool>,
    game_db: web::Data<GameDb>,
    registry: web::Data<Arc<GameRegistry>>,
    component_db: web::Data<ComponentDb>,
    game_store: web::Data<Arc<db::GameInstanceStore>>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let req = req
        .into_inner()
        .data(pool.get_ref().clone())
        .data(game_db.get_ref().clone())
        .data(registry.get_ref().clone())
        .data(component_db.get_ref().clone())
        .data(game_store.get_ref().clone());
    schema.execute(req).await.into()
}

async fn graphql_ws(
    schema: web::Data<AppSchema>,
    pool: web::Data<SqlitePool>,
    game_db: web::Data<GameDb>,
    registry: web::Data<Arc<GameRegistry>>,
    component_db: web::Data<ComponentDb>,
    game_store: web::Data<Arc<db::GameInstanceStore>>,
    req: HttpRequest,
    payload: web::Payload,
) -> Result<HttpResponse, Error> {
    let mut data = GqlData::default();
    data.insert(pool.get_ref().clone());
    data.insert(game_db.get_ref().clone());
    data.insert(registry.get_ref().clone());
    data.insert(component_db.get_ref().clone());
    data.insert(game_store.get_ref().clone());

    GraphQLSubscription::new(schema.get_ref().clone())
        .with_data(data)
        .start(&req, payload)
}

/// Create parent dirs for file-backed SQLite so `sqlite:///abs/path.db` works in Docker.
///
/// `sqlite:///app/data/app.db` becomes `///app/...` after the `sqlite:` prefix; stripping *all*
/// leading `/` wrongly yields a relative path (`app/...`) and `mkdir` targets the wrong place
/// while SQLx still opens the absolute file → SQLite error 14.
fn ensure_sqlite_parent_dir(database_url: &str) {
    let Some(rest) = database_url.strip_prefix("sqlite:") else {
        return;
    };
    if rest.starts_with(":memory:") {
        return;
    }
    let path = if rest.starts_with("///") {
        std::path::PathBuf::from(format!("/{}", rest.trim_start_matches('/')))
    } else if rest.starts_with("//") {
        // Authority-style URL (unusual for our deployments); avoid guessing a filesystem path.
        return;
    } else {
        std::path::PathBuf::from(rest)
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./data/app.db".to_string());

    ensure_sqlite_parent_dir(&database_url);

    let pool = db::connect_and_migrate(&database_url)
        .await
        .expect("database connect/migrate");

    let games_dir = PathBuf::from(std::env::var("GAMES_DIR").unwrap_or_else(|_| "./games".into()));
    let lobby_dir = PathBuf::from(std::env::var("LOBBY_DIR").unwrap_or_else(|_| "./lobby".into()));
    let lib_dir = PathBuf::from(std::env::var("LIB_DIR").unwrap_or_else(|_| "./lib".into()));

    let (list_tx, _list_rx) = broadcast::channel::<()>(256);
    let game_db = web::Data::new(GameDb::new(Some(list_tx)));
    let game_store = web::Data::new(Arc::new(db::GameInstanceStore::new(pool.clone())));

    let component_db = ComponentDb::new();
    let registry = GameRegistry::load(&games_dir, &component_db);
    let registry = web::Data::new(Arc::new(registry));
    let component_db = web::Data::new(component_db.clone());

    let pool_data = web::Data::new(pool.clone());
    let schema = web::Data::new(graphql_api::build_schema());

    println!("Starting server on {}:{}", host, port);

    HttpServer::new(move || {
        let mut app = App::new()
            .app_data(game_db.clone())
            .app_data(component_db.clone())
            .app_data(registry.clone())
            .app_data(pool_data.clone())
            .app_data(game_store.clone())
            .app_data(schema.clone())
            .route("/api/create_game", web::post().to(create_game))
            .route("/api/game_types", web::get().to(get_game_types))
            .route("/api/games", web::get().to(get_games))
            .route("/game", web::get().to(game))
            .service(
                web::resource("/graphql")
                    .guard(guard::Post())
                    .to(graphql_post),
            )
            .route("/graphql", web::get().to(graphql_ws))
            .route("/graphql/playground", web::get().to(graphql_playground));

        for gt in registry.get_ref().game_types() {
            if gt.client_dir.exists() {
                let route = format!("/games/{}", gt.manifest.name);
                app = app.service(
                    Files::new(&route, &gt.client_dir).index_file("index.html"),
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
