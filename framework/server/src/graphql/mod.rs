mod mutation;
mod query;
mod subscription;
mod types;

pub use mutation::MutationRoot;
pub use query::QueryRoot;
pub use subscription::SubscriptionRoot;
pub use types::*;

use std::path::PathBuf;

use async_graphql::{Context, Error, Result, Schema};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::auth_sessions;
use crate::db;

/// Authenticated principal from `Authorization: Bearer <session_token|publish_token>`.
#[derive(Clone, Debug)]
pub struct RequestUser(pub Option<String>);
#[derive(Clone)]
pub struct GamesDir(pub PathBuf);
#[derive(Clone)]
pub struct DraftsDir(pub PathBuf);

pub(crate) async fn require_user(ctx: &Context<'_>) -> Result<Uuid> {
    let RequestUser(raw) = ctx.data::<RequestUser>()?;
    let Some(raw) = raw.as_ref() else {
        return Err(Error::new("login required: send Authorization: Bearer <sessionToken>"));
    };
    let pool = ctx.data::<SqlitePool>()?;
    if let Some(uid) = auth_sessions::resolve_session(pool, raw)
        .await
        .map_err(|e| Error::new(format!("db: {e}")))?
    {
        return Ok(uid);
    }
    if let Some(tok) = db::resolve_publish_token(pool, raw)
        .await
        .map_err(|e| Error::new(format!("db: {e}")))?
    {
        return Ok(tok.user_id);
    }
    let legacy = std::env::var("ALLOW_LEGACY_BEARER_UUID")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if legacy {
        if let Ok(uid) = Uuid::parse_str(raw) {
            return Ok(uid);
        }
    }
    Err(Error::new("invalid or expired bearer token"))
}

/// Bearer user must exist in `users` (avoids SQLite FK 787 when localStorage id is stale after DB reset).
pub(crate) async fn require_registered_user(ctx: &Context<'_>) -> Result<Uuid> {
    let uid = require_user(ctx).await?;
    let pool = ctx.data::<SqlitePool>()?;
    if db::get_user(pool, uid)
        .await
        .map_err(|e| Error::new(format!("db: {e}")))?
        .is_none()
    {
        return Err(Error::new(
            "user id not in database (localStorage may be stale after a DB reset); clear site data or register again",
        ));
    }
    Ok(uid)
}

pub(crate) async fn require_developer_user(ctx: &Context<'_>) -> Result<Uuid> {
    let uid = require_registered_user(ctx).await?;
    let pool = ctx.data::<SqlitePool>()?;
    let open_uploads = std::env::var("OPEN_DEVELOPER_UPLOADS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if open_uploads || db::user_has_role(pool, uid, "developer").await.unwrap_or(false) {
        return Ok(uid);
    }
    Err(Error::new("developer permission required"))
}

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn build_schema() -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot).finish()
}
