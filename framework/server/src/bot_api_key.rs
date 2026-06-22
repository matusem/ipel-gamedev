use sha2::{Digest, Sha256};
use sqlx::Row;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::GameInstanceStore;

#[derive(Debug, Clone)]
pub struct BotPrincipal {
    pub bot_id: Uuid,
    pub owner_user_id: Uuid,
    pub game_slug: String,
    pub contract_hash: String,
    pub display_name: String,
    pub avatar_seed: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BotApiKeySummary {
    pub id: Uuid,
    pub prefix: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

fn hash_bot_key(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn now_secs() -> i64 {
    GameInstanceStore::now_secs()
}

pub fn mask_bot_key_prefix(prefix: &str) -> String {
    format!("gbk_••••{prefix}")
}

/// Issue a new API key for an external bot. Returns (key_id, plaintext_key, prefix).
pub async fn issue_key(
    pool: &SqlitePool,
    bot_id: Uuid,
    owner: Uuid,
) -> Result<(Uuid, String, String), sqlx::Error> {
    let id = Uuid::new_v4();
    let suffix: String = id.to_string().chars().skip(28).collect();
    let prefix = suffix;
    let token = format!("gbk_{}", Uuid::new_v4());
    let token_hash = hash_bot_key(&token);
    let now = now_secs();
    sqlx::query(
        r#"INSERT INTO bot_api_keys (id, bot_id, owner_user_id, token_hash, prefix, created_at, last_used_at, revoked_at)
           VALUES (?, ?, ?, ?, ?, ?, NULL, NULL)"#,
    )
    .bind(id.to_string())
    .bind(bot_id.to_string())
    .bind(owner.to_string())
    .bind(token_hash)
    .bind(&prefix)
    .bind(now)
    .execute(pool)
    .await?;
    Ok((id, token, prefix))
}

pub async fn list_keys_for_bot(
    pool: &SqlitePool,
    bot_id: Uuid,
    owner: Uuid,
) -> Result<Vec<BotApiKeySummary>, sqlx::Error> {
    let rows = sqlx::query(
        r#"SELECT id, prefix, created_at, last_used_at FROM bot_api_keys
           WHERE bot_id = ? AND owner_user_id = ? AND revoked_at IS NULL
           ORDER BY created_at DESC"#,
    )
    .bind(bot_id.to_string())
    .bind(owner.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let id_s: String = r.get(0);
            BotApiKeySummary {
                id: Uuid::parse_str(&id_s).unwrap_or_else(|_| Uuid::nil()),
                prefix: r.get(1),
                created_at: r.get(2),
                last_used_at: r.get(3),
            }
        })
        .collect())
}

pub async fn revoke_key(pool: &SqlitePool, owner: Uuid, key_id: Uuid) -> Result<bool, sqlx::Error> {
    let now = now_secs();
    let r = sqlx::query(
        "UPDATE bot_api_keys SET revoked_at = ? WHERE id = ? AND owner_user_id = ? AND revoked_at IS NULL",
    )
    .bind(now)
    .bind(key_id.to_string())
    .bind(owner.to_string())
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn resolve_bot_key(pool: &SqlitePool, token: &str) -> Result<Option<BotPrincipal>, sqlx::Error> {
    let token_hash = hash_bot_key(token);
    let row = sqlx::query(
        r#"SELECT k.bot_id, k.owner_user_id, b.game_slug, b.contract_hash, b.display_name, b.avatar_seed, b.avatar_url
           FROM bot_api_keys k
           JOIN bots b ON b.id = k.bot_id
           WHERE k.token_hash = ? AND k.revoked_at IS NULL AND b.category = 'external'"#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?;
    let Some(r) = row else {
        return Ok(None);
    };
    let now = now_secs();
    let _ = sqlx::query("UPDATE bot_api_keys SET last_used_at = ? WHERE token_hash = ?")
        .bind(now)
        .bind(&token_hash)
        .execute(pool)
        .await;
    let bot_id_s: String = r.get(0);
    let owner_s: String = r.get(1);
    Ok(Some(BotPrincipal {
        bot_id: Uuid::parse_str(&bot_id_s).unwrap_or_else(|_| Uuid::nil()),
        owner_user_id: Uuid::parse_str(&owner_s).unwrap_or_else(|_| Uuid::nil()),
        game_slug: r.get(2),
        contract_hash: r.get(3),
        display_name: r.get(4),
        avatar_seed: r.get(5),
        avatar_url: r.get(6),
    }))
}
