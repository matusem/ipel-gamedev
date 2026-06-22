use sqlx::Row;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BotRecord {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub version: String,
    pub game_slug: String,
    pub game_version: String,
    pub contract_hash: String,
    pub status: String,
    pub category: String,
    pub avatar_seed: Option<String>,
    pub avatar_url: Option<String>,
    pub settings_schema_json: Option<String>,
    pub settings_json: Option<String>,
    pub created_at: i64,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub async fn insert_bot(
    pool: &SqlitePool,
    id: Uuid,
    owner: Uuid,
    slug: &str,
    display_name: &str,
    version: &str,
    game_slug: &str,
    game_version: &str,
    contract_hash: &str,
    settings_schema_json: Option<&str>,
    settings_json: Option<&str>,
) -> Result<(), sqlx::Error> {
    let avatar_seed = id.to_string();
    sqlx::query(
        r#"INSERT INTO bots (id, owner_user_id, slug, display_name, version, game_slug, game_version, contract_hash, status, category, avatar_seed, avatar_url, settings_schema_json, settings_json, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'published', 'published', ?, NULL, ?, ?, ?)"#,
    )
    .bind(id.to_string())
    .bind(owner.to_string())
    .bind(slug)
    .bind(display_name)
    .bind(version)
    .bind(game_slug)
    .bind(game_version)
    .bind(contract_hash)
    .bind(avatar_seed)
    .bind(settings_schema_json)
    .bind(settings_json)
    .bind(now_secs())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_external_bot(
    pool: &SqlitePool,
    id: Uuid,
    owner: Uuid,
    slug: &str,
    display_name: &str,
    game_slug: &str,
    contract_hash: &str,
    avatar_seed: Option<&str>,
    avatar_url: Option<&str>,
    settings_schema_json: Option<&str>,
    settings_json: Option<&str>,
) -> Result<(), sqlx::Error> {
    let seed = avatar_seed
        .map(|s| s.to_string())
        .unwrap_or_else(|| id.to_string());
    sqlx::query(
        r#"INSERT INTO bots (id, owner_user_id, slug, display_name, version, game_slug, game_version, contract_hash, status, category, avatar_seed, avatar_url, settings_schema_json, settings_json, created_at)
           VALUES (?, ?, ?, ?, '', ?, '', ?, 'published', 'external', ?, ?, ?, ?, ?)"#,
    )
    .bind(id.to_string())
    .bind(owner.to_string())
    .bind(slug)
    .bind(display_name)
    .bind(game_slug)
    .bind(contract_hash)
    .bind(seed)
    .bind(avatar_url)
    .bind(settings_schema_json)
    .bind(settings_json)
    .bind(now_secs())
    .execute(pool)
    .await?;
    Ok(())
}

const BOT_SELECT: &str = "SELECT id, owner_user_id, slug, display_name, version, game_slug, game_version, contract_hash, status, category, avatar_seed, avatar_url, settings_schema_json, settings_json, created_at FROM bots";

pub async fn get_bot_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<BotRecord>, sqlx::Error> {
    let row = sqlx::query(&format!("{BOT_SELECT} WHERE id = ?"))
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;
    Ok(row.map(map_bot_row))
}

pub async fn get_bot_by_slug(pool: &SqlitePool, slug: &str) -> Result<Option<BotRecord>, sqlx::Error> {
    let row = sqlx::query(&format!("{BOT_SELECT} WHERE slug = ?"))
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(map_bot_row))
}

pub async fn list_bots_by_owner(pool: &SqlitePool, owner: Uuid) -> Result<Vec<BotRecord>, sqlx::Error> {
    let rows = sqlx::query(&format!("{BOT_SELECT} WHERE owner_user_id = ? ORDER BY created_at DESC"))
        .bind(owner.to_string())
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(map_bot_row).collect())
}

pub async fn list_compatible_bots(
    pool: &SqlitePool,
    game_slug: &str,
    contract_hash: &str,
) -> Result<Vec<BotRecord>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "{BOT_SELECT} WHERE game_slug = ? AND contract_hash = ? AND status = 'published' AND category = 'published'
         ORDER BY display_name"
    ))
    .bind(game_slug)
    .bind(contract_hash)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(map_bot_row).collect())
}

pub async fn update_bot_settings(
    pool: &SqlitePool,
    bot_id: Uuid,
    owner: Uuid,
    settings_schema_json: Option<&str>,
    settings_json: &str,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        r#"UPDATE bots SET settings_schema_json = COALESCE(?, settings_schema_json), settings_json = ?
           WHERE id = ? AND owner_user_id = ?"#,
    )
    .bind(settings_schema_json)
    .bind(settings_json)
    .bind(bot_id.to_string())
    .bind(owner.to_string())
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

fn map_bot_row(r: sqlx::sqlite::SqliteRow) -> BotRecord {
    let id_s: String = r.get(0);
    let owner_s: String = r.get(1);
    BotRecord {
        id: Uuid::parse_str(&id_s).unwrap_or_else(|_| Uuid::nil()),
        owner_user_id: Uuid::parse_str(&owner_s).unwrap_or_else(|_| Uuid::nil()),
        slug: r.get(2),
        display_name: r.get(3),
        version: r.get(4),
        game_slug: r.get(5),
        game_version: r.get(6),
        contract_hash: r.get(7),
        status: r.get(8),
        category: r.get(9),
        avatar_seed: r.get(10),
        avatar_url: r.get(11),
        settings_schema_json: r.get(12),
        settings_json: r.get(13),
        created_at: r.get(14),
    }
}
