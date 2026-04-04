use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone)]
pub struct GameInstanceStore {
    pool: SqlitePool,
}

impl GameInstanceStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn now_secs() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    pub async fn insert_game(
        &self,
        id: Uuid,
        game_type: &str,
        config: &[u8],
        state_json: &str,
    ) -> Result<(), sqlx::Error> {
        let config_s = String::from_utf8_lossy(config);
        let now = Self::now_secs();
        sqlx::query(
            r#"INSERT INTO game_instances (id, game_type, config, state, status, updated_at)
               VALUES (?, ?, ?, ?, 'active', ?)"#,
        )
        .bind(id.to_string())
        .bind(game_type)
        .bind(config_s.as_ref())
        .bind(state_json)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_game_state(&self, id: Uuid, state_json: &str) -> Result<(), sqlx::Error> {
        let now = Self::now_secs();
        sqlx::query(
            "UPDATE game_instances SET state = ?, updated_at = ? WHERE id = ?",
        )
        .bind(state_json)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

pub async fn register_user(
    pool: &SqlitePool,
    display_name: &str,
) -> Result<(Uuid, String, i64), sqlx::Error> {
    let id = Uuid::new_v4();
    let now = GameInstanceStore::now_secs();
    sqlx::query("INSERT INTO users (id, display_name, created_at) VALUES (?, ?, ?)")
        .bind(id.to_string())
        .bind(display_name)
        .bind(now)
        .execute(pool)
        .await?;
    Ok((id, display_name.to_string(), now))
}

/// New account with password hash (single insert).
pub async fn sign_up(
    pool: &SqlitePool,
    display_name: &str,
    password_hash: &str,
) -> Result<(Uuid, String, i64), sqlx::Error> {
    let id = Uuid::new_v4();
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        "INSERT INTO users (id, display_name, password_hash, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(display_name)
    .bind(password_hash)
    .bind(now)
    .execute(pool)
    .await?;
    Ok((id, display_name.to_string(), now))
}

pub async fn get_user(pool: &SqlitePool, id: Uuid) -> Result<Option<(Uuid, String, i64)>, sqlx::Error> {
    let row = sqlx::query("SELECT id, display_name, created_at FROM users WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| {
        let sid: String = r.get(0);
        let name: String = r.get(1);
        let created: i64 = r.get(2);
        (Uuid::parse_str(&sid).unwrap_or(id), name, created)
    }))
}

pub async fn list_users(pool: &SqlitePool, limit: i64) -> Result<Vec<(Uuid, String, i64)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, display_name, created_at FROM users ORDER BY created_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for r in rows {
        let sid: String = r.get(0);
        let name: String = r.get(1);
        let created: i64 = r.get(2);
        if let Ok(uid) = Uuid::parse_str(&sid) {
            out.push((uid, name, created));
        }
    }
    Ok(out)
}

pub async fn connect_and_migrate(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePool::connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn get_password_hash(pool: &SqlitePool, id: Uuid) -> Result<Option<String>, sqlx::Error> {
    let v: Option<Option<String>> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;
    Ok(v.flatten())
}

pub async fn set_password_hash(pool: &SqlitePool, id: Uuid, hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(hash)
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}
