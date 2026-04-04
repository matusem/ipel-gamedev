use std::pin::Pin;
use std::sync::Arc;

use async_graphql::{Context, Object, Result, Schema, SimpleObject, Subscription};
use futures_util::stream::{self, Stream, StreamExt};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::component_db::ComponentDb;
use crate::db::{self, GameInstanceStore};
use crate::game_db::GameDb;
use crate::game_registry::GameRegistry;
use crate::game_service;

/// Root query.
pub struct QueryRoot;

#[derive(SimpleObject, Clone)]
pub struct UserGql {
    pub id: async_graphql::types::ID,
    pub display_name: String,
    pub created_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct GameTypeGql {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub min_players: u32,
    pub max_players: u32,
    pub description: String,
    pub config_ui_path: Option<String>,
    /// Stringified JSON schema from manifest, if any.
    pub config_schema_json: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct GameInstanceGql {
    pub game_id: String,
    pub game_type: String,
    pub player_identities: Vec<String>,
    pub connected_players: usize,
}

fn map_game_entries(db: &GameDb) -> Vec<GameInstanceGql> {
    db.list_games()
        .into_iter()
        .map(|e| GameInstanceGql {
            game_id: e.game_id,
            game_type: e.game_type,
            player_identities: e.player_identities,
            connected_players: e.connected_players,
        })
        .collect()
}

#[Object]
impl QueryRoot {
    async fn game_types(&self, ctx: &Context<'_>) -> Result<Vec<GameTypeGql>> {
        let reg = ctx.data::<Arc<GameRegistry>>()?;
        Ok(reg
            .game_types()
            .iter()
            .map(|gt| GameTypeGql {
                name: gt.manifest.name.clone(),
                display_name: gt.manifest.display_name.clone(),
                version: gt.manifest.version.clone(),
                min_players: gt.manifest.min_players,
                max_players: gt.manifest.max_players,
                description: gt.manifest.description.clone(),
                config_ui_path: gt.config_ui_path.clone(),
                config_schema_json: gt
                    .manifest
                    .config_schema
                    .as_ref()
                    .and_then(|v| serde_json::to_string(v).ok()),
            })
            .collect())
    }

    async fn game_instances(&self, ctx: &Context<'_>) -> Result<Vec<GameInstanceGql>> {
        let db = ctx.data::<GameDb>()?;
        Ok(map_game_entries(db))
    }

    async fn user(&self, ctx: &Context<'_>, id: async_graphql::types::ID) -> Result<Option<UserGql>> {
        let pool = ctx.data::<SqlitePool>()?;
        let uid = Uuid::parse_str(id.as_str())
            .map_err(|_| async_graphql::Error::new("invalid user id"))?;
        let row = db::get_user(pool, uid)
            .await
            .map_err(|e| async_graphql::Error::new(format!("db: {e}")))?;
        Ok(row.map(|(id, name, created)| UserGql {
            id: id.to_string().into(),
            display_name: name,
            created_at: created,
        }))
    }

    async fn users(&self, ctx: &Context<'_>, limit: Option<i32>) -> Result<Vec<UserGql>> {
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(50).clamp(1, 500) as i64;
        let rows = db::list_users(pool, lim)
            .await
            .map_err(|e| async_graphql::Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|(id, name, created)| UserGql {
                id: id.to_string().into(),
                display_name: name,
                created_at: created,
            })
            .collect())
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn register_user(
        &self,
        ctx: &Context<'_>,
        display_name: String,
    ) -> Result<UserGql> {
        let pool = ctx.data::<SqlitePool>()?;
        let (id, name, created) = db::register_user(pool, &display_name)
            .await
            .map_err(|e| async_graphql::Error::new(format!("db: {e}")))?;
        Ok(UserGql {
            id: id.to_string().into(),
            display_name: name,
            created_at: created,
        })
    }

    async fn create_game(
        &self,
        ctx: &Context<'_>,
        game_type: String,
        config_json: String,
    ) -> Result<async_graphql::types::ID> {
        let component_db = ctx.data::<ComponentDb>()?;
        let game_db = ctx.data::<GameDb>()?;
        let game_store = ctx.data::<Arc<GameInstanceStore>>()?;
        let config = config_json.into_bytes();
        let id = game_service::create_and_spawn_game(
            component_db,
            game_db,
            game_store.clone(),
            game_type,
            config,
        )
        .await
        .map_err(async_graphql::Error::new)?;
        Ok(id.to_string().into())
    }
}

pub struct SubscriptionRoot;

type GameListStream = Pin<Box<dyn Stream<Item = Vec<GameInstanceGql>> + Send>>;

#[Subscription]
impl SubscriptionRoot {
    async fn game_instances_updated(&self, ctx: &Context<'_>) -> Result<GameListStream> {
        let db = ctx.data::<GameDb>()?.clone();
        let rx = db.subscribe_game_list().ok_or_else(|| {
            async_graphql::Error::new("game list subscriptions are not configured")
        })?;
        let first = map_game_entries(&db);
        let tail = stream::unfold((rx, db), |(mut rx, db)| async move {
            match rx.recv().await {
                Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    Some((map_game_entries(&db), (rx, db)))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
            }
        });
        Ok(Box::pin(stream::once(async move { first }).chain(tail)))
    }
}

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn build_schema() -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot).finish()
}
