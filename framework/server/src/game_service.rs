use std::sync::Arc;

use actix_web::rt;
use uuid::Uuid;

use crate::component_db::ComponentDb;
use crate::db::GameInstanceStore;
use crate::game_core;
use crate::game_db::{encode_game_snapshot, GameDb, GameInstance, GameRunPersistence};

pub async fn create_and_spawn_game(
    component_db: &ComponentDb,
    game_db: &GameDb,
    game_store: Arc<GameInstanceStore>,
    game_type: String,
    config: Vec<u8>,
) -> Result<Uuid, String> {
    let engine = component_db.get_engine();
    let (game_core, mut store) = component_db
        .create_game_core(&game_type)
        .await
        .map_err(|e| e.to_string())?;
    let game = game_core
        .call_init(
            &mut store,
            game_core::SerializationFormat::Json,
            &config,
        )
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("game init: {:?}", e))?;
    let game_id = Uuid::new_v4();
    let snap = encode_game_snapshot(&game).map_err(|e| e.to_string())?;
    game_store
        .insert_game(game_id, &game_type, &config, &snap)
        .await
        .map_err(|e| e.to_string())?;
    let persistence = Some(GameRunPersistence {
        game_id,
        store: Arc::clone(&game_store),
    });
    game_db.new_game(
        game_id,
        GameInstance::new(game, game_core, game_type),
    );
    let gdb = game_db.clone();
    rt::spawn(async move {
        if let Ok(mut gi) = gdb.get_game(game_id) {
            gi.run(&engine, store, persistence).await;
        }
    });
    Ok(game_id)
}
