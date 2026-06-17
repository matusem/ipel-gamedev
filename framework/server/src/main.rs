use actix_files::{Files, NamedFile};
use actix_web::dev::{ServiceRequest, ServiceResponse, fn_service};
use actix_web::guard;
use actix_web::http::header;
use actix_web::{
    App, Error, HttpRequest, HttpResponse, HttpServer, Result as ActixResult, rt, web,
};
use async_graphql::Data as GqlData;
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use server::game_core::Buffer;
use server::game_db::GameDb;
use server::game_registry::GameRegistry;
use server::graphql::{AppSchema, DraftsDir, GamesDir, RequestUser};
use server::lobby_db::LobbyListNotify;
use server::{component_db::ComponentDb, db};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tracing_actix_web::TracingLogger;
use uuid::Uuid;

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

enum GameWsMode {
    Player(Buffer),
    Spectator,
}

struct GameRequestParams {
    game_id: Uuid,
    mode: GameWsMode,
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

        let spectator = params
            .get("spectator")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        if spectator {
            return Ok(Self {
                game_id,
                mode: GameWsMode::Spectator,
            });
        }

        let player = params
            .get("player")
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'player' parameter"))?;

        let player: Buffer = player_identity_from_query_param(player);

        Ok(Self {
            game_id,
            mode: GameWsMode::Player(player),
        })
    }
}

async fn game(
    request: HttpRequest,
    body: web::Payload,
    game_db: web::Data<GameDb>,
) -> Result<HttpResponse, Error> {
    let params = GameRequestParams::parse(&request)?;
    let GameRequestParams { game_id, mode } = params;

    let (response, mut session, stream) = actix_ws::handle(&request, body)?;
    let mut stream = stream
        .aggregate_continuations()
        .max_continuation_size(2_usize.pow(20));

    match mode {
        GameWsMode::Player(player) => {
            let player_state = {
                let mut game_instance = game_db.get_game(game_id)?;
                game_instance.register_player(player.clone())?
            };

            game_db.notify_game_list_changed();

            let initial = String::from_utf8_lossy(&player_state.0);
            let _ = session.text(initial.as_ref()).await;

            tracing::info!(
                game_id = %game_id,
                player_id = ?player_state.2,
                "game websocket player registered"
            );

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
                                    tracing::warn!(game_id = %game_id, error = ?e, "game websocket event channel closed");
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
                                    tracing::warn!(game_id = %game_id, error = ?e, "game websocket protocol error");
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
        }
        GameWsMode::Spectator => {
            let spectator_state = {
                let mut game_instance = game_db.get_game(game_id)?;
                game_instance.register_spectator()?
            };

            game_db.notify_game_list_changed();

            let initial = String::from_utf8_lossy(&spectator_state.0);
            let _ = session.text(initial.as_ref()).await;

            tracing::info!(
                game_id = %game_id,
                spectator_id = ?spectator_state.2,
                "game websocket spectator registered"
            );

            rt::spawn(async move {
                let channel = spectator_state.1;
                let spectator_id = spectator_state.2;

                loop {
                    tokio::select! {
                        spectator_event = channel.receive_event() => {
                            match spectator_event {
                                Ok(buffer) => {
                                    let out = String::from_utf8_lossy(&buffer);
                                    let _ = session.text(out.as_ref()).await;
                                }
                                Err(e) => {
                                    tracing::warn!(game_id = %game_id, error = ?e, "spectator websocket event channel closed");
                                    break;
                                }
                            }
                        }
                        msg = stream.recv() => {
                            match msg {
                                Some(Ok(msg)) => match msg {
                                    actix_ws::AggregatedMessage::Ping(bytes) => {
                                        let _ = session.pong(&bytes).await;
                                    }
                                    actix_ws::AggregatedMessage::Pong(_) => {}
                                    actix_ws::AggregatedMessage::Close(_) => break,
                                    actix_ws::AggregatedMessage::Text(_)
                                    | actix_ws::AggregatedMessage::Binary(_) => {
                                        tracing::debug!(game_id = %game_id, "ignoring inbound action on spectator socket");
                                    }
                                },
                                Some(Err(e)) => {
                                    tracing::warn!(game_id = %game_id, error = ?e, "spectator websocket protocol error");
                                    break;
                                }
                                None => break,
                            }
                        }
                    }
                }

                let _ = session.close(None).await;

                if let Ok(mut game_instance) = game_db.get_game(game_id) {
                    let _ = game_instance.unregister_spectator(spectator_id);
                    game_db.notify_game_list_changed();
                }
            });
        }
    }

    Ok(response)
}

fn extract_request_user(req: &HttpRequest) -> RequestUser {
    let Some(h) = req.headers().get(header::AUTHORIZATION) else {
        return RequestUser(None);
    };
    let Ok(s) = h.to_str() else {
        return RequestUser(None);
    };
    let rest = s
        .strip_prefix("Bearer ")
        .or_else(|| s.strip_prefix("bearer "))
        .unwrap_or(s);
    RequestUser(Some(rest.trim().to_string()))
}

/// Browser WebSockets cannot set `Authorization`; allow `?token=<uuid>` on `/graphql` WS.
fn extract_request_user_for_ws(req: &HttpRequest) -> RequestUser {
    if let RequestUser(Some(u)) = extract_request_user(req) {
        return RequestUser(Some(u));
    }
    let Some(qs) = req.uri().query() else {
        return RequestUser(None);
    };
    for part in qs.split('&') {
        let Some(v) = part.strip_prefix("token=") else {
            continue;
        };
        return RequestUser(Some(v.to_string()));
    }
    RequestUser(None)
}

async fn health() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(r#"{"status":"ok"}"#))
}

async fn platform_manifest() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(server::platform_manifest::platform_manifest_json()))
}

async fn cli_manifest() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(server::platform_manifest::cli_manifest_json()))
}

async fn graphql_playground() -> ActixResult<HttpResponse> {
    let html = playground_source(
        GraphQLPlaygroundConfig::new("/graphql").subscription_endpoint("/graphql"),
    );
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

async fn graphql_post(
    http: HttpRequest,
    schema: web::Data<AppSchema>,
    pool: web::Data<SqlitePool>,
    game_db: web::Data<GameDb>,
    registry: web::Data<Arc<RwLock<GameRegistry>>>,
    component_db: web::Data<ComponentDb>,
    game_store: web::Data<Arc<db::GameInstanceStore>>,
    lobby_notify: web::Data<LobbyListNotify>,
    games_dir: web::Data<GamesDir>,
    drafts_dir: web::Data<DraftsDir>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let auth = extract_request_user(&http);
    let req = req
        .into_inner()
        .data(pool.get_ref().clone())
        .data(game_db.get_ref().clone())
        .data(registry.get_ref().clone())
        .data(component_db.get_ref().clone())
        .data(game_store.get_ref().clone())
        .data(lobby_notify.get_ref().clone())
        .data(games_dir.get_ref().clone())
        .data(drafts_dir.get_ref().clone())
        .data(auth);
    schema.execute(req).await.into()
}

async fn graphql_ws(
    schema: web::Data<AppSchema>,
    pool: web::Data<SqlitePool>,
    game_db: web::Data<GameDb>,
    registry: web::Data<Arc<RwLock<GameRegistry>>>,
    component_db: web::Data<ComponentDb>,
    game_store: web::Data<Arc<db::GameInstanceStore>>,
    lobby_notify: web::Data<LobbyListNotify>,
    games_dir: web::Data<GamesDir>,
    drafts_dir: web::Data<DraftsDir>,
    req: HttpRequest,
    payload: web::Payload,
) -> Result<HttpResponse, Error> {
    let mut data = GqlData::default();
    data.insert(pool.get_ref().clone());
    data.insert(game_db.get_ref().clone());
    data.insert(registry.get_ref().clone());
    data.insert(component_db.get_ref().clone());
    data.insert(game_store.get_ref().clone());
    data.insert(lobby_notify.get_ref().clone());
    data.insert(games_dir.get_ref().clone());
    data.insert(drafts_dir.get_ref().clone());
    data.insert(extract_request_user_for_ws(&req));

    GraphQLSubscription::new(schema.get_ref().clone())
        .with_data(data)
        .start(&req, payload)
}

async fn game_asset(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    registry: web::Data<Arc<RwLock<GameRegistry>>>,
) -> Result<HttpResponse, Error> {
    let (game_name, tail) = path.into_inner();
    let tail = if tail.is_empty() {
        "index.html".to_string()
    } else {
        tail
    };
    let rel = std::path::Path::new(&tail);
    if rel.is_absolute() || tail.contains("..") {
        return Err(actix_web::error::ErrorBadRequest("invalid asset path"));
    }
    let reg = registry
        .read()
        .map_err(|_| actix_web::error::ErrorInternalServerError("registry lock poisoned"))?;
    let Some(client_dir) = reg.get_client_dir(&game_name) else {
        return Err(actix_web::error::ErrorNotFound("game not found"));
    };
    let full = client_dir.join(rel);
    if !full.is_file() {
        return Err(actix_web::error::ErrorNotFound("asset not found"));
    }
    let file = NamedFile::open_async(full)
        .await
        .map_err(|_| actix_web::error::ErrorNotFound("asset not found"))?;
    let mut res = file.into_response(&req);
    if tail.ends_with(".html") {
        res.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-cache"),
        );
    }
    Ok(res)
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

fn cleanup_old_drafts(drafts_dir: &std::path::Path) {
    let ttl_secs: u64 = std::env::var("DRAFT_RETENTION_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7 * 24 * 60 * 60);
    let now = std::time::SystemTime::now();
    let Ok(entries) = std::fs::read_dir(drafts_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let age = now.duration_since(modified).unwrap_or_default().as_secs();
        if age > ttl_secs {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let _ = dotenvy::dotenv();
    server::logging::init();
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
    let drafts_dir =
        PathBuf::from(std::env::var("DRAFTS_DIR").unwrap_or_else(|_| "./drafts".into()));
    let lobby_dir = PathBuf::from(std::env::var("LOBBY_DIR").unwrap_or_else(|_| "./lobby".into()));
    let lib_dir = PathBuf::from(std::env::var("LIB_DIR").unwrap_or_else(|_| "./client-lib".into()));
    let tools_dir = PathBuf::from(std::env::var("TOOLS_DIR").unwrap_or_else(|_| "./tools".into()));
    let _ = server::platform_manifest::load_manifest();
    let _ = std::fs::create_dir_all(&drafts_dir);
    cleanup_old_drafts(&drafts_dir);

    let (list_tx, _list_rx) = broadcast::channel::<()>(256);
    let game_db = web::Data::new(GameDb::new(Some(list_tx)));
    let (lobby_tx, _lobby_rx) = broadcast::channel::<()>(256);
    let lobby_notify = web::Data::new(LobbyListNotify { tx: lobby_tx });
    let game_store = web::Data::new(Arc::new(db::GameInstanceStore::new(pool.clone())));

    let component_db = ComponentDb::new();
    let registry = GameRegistry::load(&games_dir, &component_db);
    let registry = web::Data::new(Arc::new(RwLock::new(registry)));
    let component_db = web::Data::new(component_db.clone());

    let pool_data = web::Data::new(pool.clone());
    let games_dir_data = web::Data::new(GamesDir(games_dir.clone()));
    let drafts_dir_data = web::Data::new(DraftsDir(drafts_dir.clone()));
    let schema = web::Data::new(server::graphql::build_schema());

    tracing::info!(%host, port, "starting server");

    HttpServer::new(move || {
        let mut app = App::new()
            .wrap(TracingLogger::default())
            .app_data(game_db.clone())
            .app_data(component_db.clone())
            .app_data(registry.clone())
            .app_data(pool_data.clone())
            .app_data(games_dir_data.clone())
            .app_data(drafts_dir_data.clone())
            .app_data(game_store.clone())
            .app_data(lobby_notify.clone())
            .app_data(schema.clone())
            .route("/health", web::get().to(health))
            .route(
                "/internal/deploy",
                web::post().to(server::deploy_webhook::handle_deploy),
            )
            .route("/platform/manifest.json", web::get().to(platform_manifest))
            .route(
                "/tools/gamedev-cli/manifest.json",
                web::get().to(cli_manifest),
            )
            .route("/game", web::get().to(game))
            .route("/games/{game}/{tail:.*}", web::get().to(game_asset))
            .service(
                web::resource("/graphql")
                    .guard(guard::Post())
                    .to(graphql_post),
            )
            .route("/graphql", web::get().to(graphql_ws))
            .route("/graphql/playground", web::get().to(graphql_playground))
            .route("/auth/google", web::get().to(server::google_oauth::google_start))
            .route(
                "/auth/google/callback",
                web::get().to(server::google_oauth::google_callback),
            );

        if lib_dir.exists() {
            app = app.service(Files::new("/lib", &lib_dir));
        }

        let cli_tools = tools_dir.join("gamedev-cli");
        if cli_tools.is_dir() {
            app = app.service(Files::new("/tools/gamedev-cli", &cli_tools));
        }

        if lobby_dir.exists() {
            let lobby_index = lobby_dir.join("index.html");
            app = app.service(
                Files::new("/", lobby_dir.clone())
                    .index_file("index.html")
                    .default_handler(fn_service(move |req: ServiceRequest| {
                        let lobby_index = lobby_index.clone();
                        async move {
                            let (req, _) = req.into_parts();
                            let file = match NamedFile::open_async(lobby_index).await {
                                Ok(f) => f,
                                Err(_) => {
                                    return Ok(ServiceResponse::new(
                                        req,
                                        HttpResponse::NotFound().finish(),
                                    ));
                                }
                            };
                            let res = file.into_response(&req);
                            Ok(ServiceResponse::new(req, res))
                        }
                    })),
            );
        }

        app
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}
