use std::pin::Pin;

use async_graphql::{Context, Error, Result, Subscription};
use futures_util::stream::{self, Stream, StreamExt};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::game_db::GameDb;
use crate::lobby_db::{self, LobbyListNotify};

use super::{
    GameInstanceGql, LobbyGql, LobbySummaryGql, lobby_to_gql, map_game_entries, map_summary,
    require_registered_user,
};
pub struct SubscriptionRoot;

type GameListStream = Pin<Box<dyn Stream<Item = Vec<GameInstanceGql>> + Send>>;
type LobbyListStream = Pin<Box<dyn Stream<Item = Vec<LobbySummaryGql>> + Send>>;
type LobbyRoomStream = Pin<Box<dyn Stream<Item = LobbyGql> + Send>>;

#[Subscription]
impl SubscriptionRoot {
    async fn game_instances_updated(&self, ctx: &Context<'_>) -> Result<GameListStream> {
        let db = ctx.data::<GameDb>()?.clone();
        let rx = db
            .subscribe_game_list()
            .ok_or_else(|| Error::new("game list subscriptions are not configured"))?;
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

    async fn lobbies_updated(&self, ctx: &Context<'_>) -> Result<LobbyListStream> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?.clone();
        let notify = ctx.data::<LobbyListNotify>()?.clone();
        let rx = notify.subscribe();
        let rows = lobby_db::list_active_lobbies(&pool)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let first: Vec<LobbySummaryGql> = rows.into_iter().map(map_summary).collect();
        let tail = stream::unfold((rx, pool), |(mut rx, pool)| async move {
            match rx.recv().await {
                Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let vec = lobby_db::list_active_lobbies(&pool)
                        .await
                        .ok()
                        .map(|rows| rows.into_iter().map(map_summary).collect())
                        .unwrap_or_default();
                    Some((vec, (rx, pool)))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
            }
        });
        Ok(Box::pin(stream::once(async move { first }).chain(tail)))
    }

    async fn lobby_updated(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::types::ID,
    ) -> Result<LobbyRoomStream> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?.clone();
        let notify = ctx.data::<LobbyListNotify>()?.clone();
        let lid = Uuid::parse_str(id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let rx = notify.subscribe();
        let first = {
            let row = lobby_db::get_lobby(&pool, lid)
                .await
                .map_err(|e| Error::new(format!("db: {e}")))?;
            let Some(d) = row else {
                return Err(Error::new("lobby not found"));
            };
            lobby_to_gql(&pool, d).await?
        };
        let tail = stream::unfold((rx, pool, lid), |(mut rx, pool, lid)| async move {
            match rx.recv().await {
                Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let item = match lobby_db::get_lobby(&pool, lid).await {
                        Ok(Some(d)) => lobby_to_gql(&pool, d).await.ok(),
                        _ => None,
                    };
                    item.map(|g| (g, (rx, pool, lid)))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
            }
        });
        Ok(Box::pin(stream::once(async move { first }).chain(tail)))
    }
}
