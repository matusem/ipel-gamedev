use sha2::{Digest, Sha256};
use sqlx::Row;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::GameInstanceStore;

const SESSION_TTL_SECS: i64 = 30 * 24 * 60 * 60;

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn new_session_token() -> String {
    format!("sess_{}", Uuid::new_v4())
}

pub async fn create_session(pool: &SqlitePool, user_id: Uuid) -> Result<(String, i64), sqlx::Error> {
    let token = new_session_token();
    let now = GameInstanceStore::now_secs();
    let expires_at = now + SESSION_TTL_SECS;
    sqlx::query(
        "INSERT INTO auth_sessions (token_hash, user_id, created_at, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(hash_token(&token))
    .bind(user_id.to_string())
    .bind(now)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok((token, expires_at))
}

pub async fn resolve_session(pool: &SqlitePool, token: &str) -> Result<Option<Uuid>, sqlx::Error> {
    if token.is_empty() {
        return Ok(None);
    }
    let now = GameInstanceStore::now_secs();
    let row =
        sqlx::query("SELECT user_id FROM auth_sessions WHERE token_hash = ? AND expires_at > ?")
            .bind(hash_token(token))
            .bind(now)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|r| {
        let uid: String = r.get(0);
        Uuid::parse_str(&uid).ok()
    }))
}

pub async fn revoke_session(pool: &SqlitePool, token: &str) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM auth_sessions WHERE token_hash = ?")
        .bind(hash_token(token))
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn purge_expired_sessions(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    sqlx::query("DELETE FROM auth_sessions WHERE expires_at <= ?")
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn revoke_all_sessions_for_user(
    pool: &SqlitePool,
    user_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM auth_sessions WHERE user_id = ?")
        .bind(user_id.to_string())
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}
