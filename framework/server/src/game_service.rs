use std::sync::Arc;

use actix_web::rt;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::component_db::ComponentDb;
use crate::db::GameInstanceStore;
use crate::game_core;
use crate::game_db::{BotSeatBinding, GameDb, GameInstance, GameRunPersistence, encode_game_snapshot, player_identity_to_buffer};
use crate::lobby_db::LobbyListNotify;

pub fn player_identities_from_game(game: &game_core::Game) -> Vec<String> {
    game.player_states
        .iter()
        .map(|ps| {
            let raw = String::from_utf8_lossy(&ps.player);
            serde_json::from_str::<String>(&raw).unwrap_or_else(|_| raw.to_string())
        })
        .collect()
}

/// Run WASM `init` once to discover seat identities (pregame lobby) without persisting a game row.
pub async fn preview_init_identities(
    component_db: &ComponentDb,
    game_type: String,
    config: Vec<u8>,
) -> Result<Vec<String>, String> {
    let (game_core, mut store) = component_db
        .create_game_core(&game_type)
        .await
        .map_err(|e| e.to_string())?;
    let game = game_core
        .call_init(&mut store, game_core::SerializationFormat::Json, &config)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("game init: {:?}", e))?;
    Ok(player_identities_from_game(&game))
}

/// Fetch the game's default config bytes (JSON) from the WASM component.
pub async fn default_config(component_db: &ComponentDb, game_type: &str) -> Result<Vec<u8>, String> {
    let (game_core, mut store) = component_db
        .create_game_core(game_type)
        .await
        .map_err(|e| e.to_string())?;
    game_core
        .call_default_config(&mut store, game_core::SerializationFormat::Json)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("default config: {:?}", e))
}

/// Resolve lobby config bytes: use stored config when present, otherwise fetch default from WASM.
pub async fn resolve_lobby_config(
    component_db: &ComponentDb,
    game_type: &str,
    stored_config: Option<&str>,
) -> Result<Vec<u8>, String> {
    if let Some(cfg) = stored_config.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(cfg.as_bytes().to_vec());
    }
    default_config(component_db, game_type).await
}

pub async fn create_and_spawn_game(
    component_db: &ComponentDb,
    game_db: &GameDb,
    game_store: Arc<GameInstanceStore>,
    game_type: String,
    config: Vec<u8>,
    lobby_id: Option<Uuid>,
    pool: SqlitePool,
    lobby_notify: LobbyListNotify,
    bot_bindings: Vec<BotSeatBinding>,
) -> Result<Uuid, String> {
    let engine = component_db.get_engine();
    let (game_core, mut store) = component_db
        .create_game_core(&game_type)
        .await
        .map_err(|e| e.to_string())?;
    let game = game_core
        .call_init(&mut store, game_core::SerializationFormat::Json, &config)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("game init: {:?}", e))?;
    let game_id = Uuid::new_v4();
    let snap = encode_game_snapshot(&game).map_err(|e| e.to_string())?;
    game_store
        .insert_game(game_id, &game_type, &config, &snap, lobby_id)
        .await
        .map_err(|e| e.to_string())?;
    let persistence = Some(GameRunPersistence {
        game_id,
        store: Arc::clone(&game_store),
        lobby_id,
        pool,
        game_db: game_db.clone(),
        lobby_notify,
        component_db: component_db.clone(),
        bot_bindings: bot_bindings.clone(),
    });
    game_db.new_game(game_id, GameInstance::new(game, game_core, game_type));
    let gdb = game_db.clone();
    let cdb = component_db.clone();
    let bindings = bot_bindings;
    rt::spawn(async move {
        if let Ok(mut gi) = gdb.get_game(game_id) {
            gi.drive_bots_initial(&cdb, &bindings).await;
            gi.run(&engine, store, persistence).await;
        }
    });
    Ok(game_id)
}
