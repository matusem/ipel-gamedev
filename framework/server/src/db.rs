use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct GameInstanceStore {
    pool: SqlitePool,
}

impl GameInstanceStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn now_secs() -> i64 {
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
        lobby_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        let config_s = String::from_utf8_lossy(config);
        let now = Self::now_secs();
        sqlx::query(
            r#"INSERT INTO game_instances (id, game_type, config, state, status, updated_at, lobby_id, started_at)
               VALUES (?, ?, ?, ?, 'active', ?, ?, ?)"#,
        )
        .bind(id.to_string())
        .bind(game_type)
        .bind(config_s.as_ref())
        .bind(state_json)
        .bind(now)
        .bind(lobby_id.map(|u| u.to_string()))
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

    pub async fn finish_game_record(
        &self,
        id: Uuid,
        state_json: &str,
        result_json: &str,
        player_scores_json: &str,
        seats_snapshot_json: &str,
    ) -> Result<(), sqlx::Error> {
        let now = Self::now_secs();
        sqlx::query(
            r#"UPDATE game_instances SET
                 state = ?,
                 status = 'finished',
                 finished_at = ?,
                 result_json = ?,
                 player_scores_json = ?,
                 seats_snapshot_json = ?,
                 updated_at = ?
               WHERE id = ?"#,
        )
        .bind(state_json)
        .bind(now)
        .bind(result_json)
        .bind(player_scores_json)
        .bind(seats_snapshot_json)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FinishedGameRow {
    pub id: Uuid,
    pub game_type: String,
    pub lobby_id: Option<Uuid>,
    pub finished_at: i64,
    pub started_at: Option<i64>,
    pub result_json: String,
    pub player_scores_json: String,
    pub seats_snapshot_json: String,
}

fn map_finished_row(r: sqlx::sqlite::SqliteRow) -> Option<FinishedGameRow> {
    let id_s: String = r.get(0);
    let id = Uuid::parse_str(&id_s).ok()?;
    let lobby: Option<String> = r.get(2);
    let finished: Option<i64> = r.get(3);
    let started: Option<i64> = r.try_get(7).ok();
    let result_j: Option<String> = r.get(4);
    let scores_j: Option<String> = r.get(5);
    let seats_j: Option<String> = r.get(6);
    Some(FinishedGameRow {
        id,
        game_type: r.get(1),
        lobby_id: lobby.and_then(|s| Uuid::parse_str(&s).ok()),
        finished_at: finished.unwrap_or(0),
        started_at: started,
        result_json: result_j.unwrap_or_else(|| "{}".into()),
        player_scores_json: scores_j.unwrap_or_else(|| "{}".into()),
        seats_snapshot_json: seats_j.unwrap_or_else(|| "[]".into()),
    })
}

const FINISHED_SELECT: &str = r#"SELECT id, game_type, lobby_id, finished_at, result_json, player_scores_json, seats_snapshot_json, started_at
           FROM game_instances"#;

pub async fn get_finished_game(
    pool: &SqlitePool,
    id: Uuid,
) -> Result<Option<FinishedGameRow>, sqlx::Error> {
    let row = sqlx::query(&format!("{FINISHED_SELECT} WHERE id = ? AND status = 'finished'"))
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;
    Ok(row.and_then(map_finished_row))
}

pub async fn list_recent_finished_games(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<FinishedGameRow>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "{FINISHED_SELECT} WHERE status = 'finished' AND finished_at IS NOT NULL ORDER BY finished_at DESC LIMIT ?"
    ))
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(map_finished_row).collect())
}

pub async fn list_finished_games_by_type(
    pool: &SqlitePool,
    game_type: &str,
    limit: i64,
) -> Result<Vec<FinishedGameRow>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "{FINISHED_SELECT} WHERE status = 'finished' AND finished_at IS NOT NULL AND game_type = ? ORDER BY finished_at DESC LIMIT ?"
    ))
    .bind(game_type)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(map_finished_row).collect())
}

pub async fn count_active_players_by_type(
    pool: &SqlitePool,
) -> Result<std::collections::HashMap<String, i32>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT game_type, COUNT(*) FROM game_instances WHERE status = 'active' GROUP BY game_type",
    )
    .fetch_all(pool)
    .await?;
    let mut out = std::collections::HashMap::new();
    for r in rows {
        let game_type: String = r.get(0);
        let count: i64 = r.get(1);
        out.insert(game_type, count as i32);
    }
    Ok(out)
}

pub async fn update_user_display_name(
    pool: &SqlitePool,
    user_id: Uuid,
    display_name: &str,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
        .bind(display_name)
        .bind(user_id.to_string())
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn count_finished_games_since(pool: &SqlitePool, since_ts: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM game_instances WHERE status = 'finished' AND finished_at >= ?",
    )
    .bind(since_ts)
    .fetch_one(pool)
    .await
}

pub async fn list_published_deployments(pool: &SqlitePool, limit: i64) -> Result<Vec<GameDraftRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, upload_id, owner_user_id, game_name, display_name, version, status, manifest_json, report_json, storage_path, created_at, updated_at, published_at FROM game_drafts WHERE status = 'published' ORDER BY published_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(map_draft_row).collect())
}

pub async fn count_published_drafts_for_user(pool: &SqlitePool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM game_drafts WHERE owner_user_id = ? AND status = 'published'",
    )
    .bind(user_id.to_string())
    .fetch_one(pool)
    .await
}

pub async fn count_user_finished_matches(pool: &SqlitePool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let needle = user_id.to_string();
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM game_instances WHERE status = 'finished' AND seats_snapshot_json LIKE ?",
    )
    .bind(format!("%{needle}%"))
    .fetch_one(pool)
    .await
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

#[derive(Debug, Clone)]
pub struct GameDraftRow {
    pub id: Uuid,
    pub upload_id: Uuid,
    pub owner_user_id: Uuid,
    pub game_name: String,
    pub display_name: String,
    pub version: String,
    pub status: String,
    pub manifest_json: String,
    pub report_json: String,
    pub storage_path: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub published_at: Option<i64>,
}

pub async fn user_has_role(pool: &SqlitePool, user_id: Uuid, role: &str) -> Result<bool, sqlx::Error> {
    let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_roles WHERE user_id = ? AND role = ?")
        .bind(user_id.to_string())
        .bind(role)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    Ok(c > 0)
}

pub async fn grant_role(pool: &SqlitePool, user_id: Uuid, role: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO user_roles (user_id, role, created_at) VALUES (?, ?, ?)")
        .bind(user_id.to_string())
        .bind(role)
        .bind(GameInstanceStore::now_secs())
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PublishTokenRow {
    pub user_id: Uuid,
}

fn hash_publish_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone)]
pub struct PublishTokenSummaryRow {
    pub id: Uuid,
    pub label: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
}

pub fn mask_publish_token_id(id: &Uuid) -> String {
    let s = id.to_string();
    let suffix = s.get(28..).unwrap_or(&s);
    format!("gpt_••••{suffix}")
}

pub async fn create_publish_token(
    pool: &SqlitePool,
    user_id: Uuid,
    ttl_days: i64,
    label: Option<&str>,
) -> Result<(Uuid, String, i64), sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    let expires_at = now + (ttl_days.clamp(1, 30) * 24 * 60 * 60);
    let token = format!("gpt_{}", Uuid::new_v4());
    let token_hash = hash_publish_token(&token);
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO publish_tokens (id, user_id, token_hash, expires_at, created_at, revoked_at, label) VALUES (?, ?, ?, ?, ?, NULL, ?)",
    )
    .bind(id.to_string())
    .bind(user_id.to_string())
    .bind(token_hash)
    .bind(expires_at)
    .bind(now)
    .bind(label)
    .execute(pool)
    .await?;
    Ok((id, token, expires_at))
}

pub async fn list_publish_tokens_for_user(
    pool: &SqlitePool,
    user_id: Uuid,
) -> Result<Vec<PublishTokenSummaryRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, label, created_at, expires_at FROM publish_tokens WHERE user_id = ? AND revoked_at IS NULL ORDER BY created_at DESC",
    )
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for r in rows {
        let id_s: String = r.get(0);
        let Ok(id) = Uuid::parse_str(&id_s) else {
            continue;
        };
        out.push(PublishTokenSummaryRow {
            id,
            label: r.get(1),
            created_at: r.get(2),
            expires_at: r.get(3),
        });
    }
    Ok(out)
}

pub async fn revoke_publish_token(pool: &SqlitePool, user_id: Uuid, token_id: Uuid) -> Result<bool, sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    let res = sqlx::query(
        "UPDATE publish_tokens SET revoked_at = ? WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
    )
    .bind(now)
    .bind(token_id.to_string())
    .bind(user_id.to_string())
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn resolve_publish_token(
    pool: &SqlitePool,
    token: &str,
) -> Result<Option<PublishTokenRow>, sqlx::Error> {
    let token_hash = hash_publish_token(token);
    let now = GameInstanceStore::now_secs();
    let row = sqlx::query(
        "SELECT user_id, expires_at FROM publish_tokens WHERE token_hash = ? AND revoked_at IS NULL AND expires_at > ?",
    )
    .bind(token_hash)
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|r| {
        let uid_s: String = r.get(0);
        let _expires_at: i64 = r.get(1);
        Some(PublishTokenRow {
            user_id: Uuid::parse_str(&uid_s).ok()?,
        })
    }))
}

pub async fn insert_upload(
    pool: &SqlitePool,
    owner_user_id: Uuid,
    filename: &str,
    status: &str,
    report_json: &str,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        "INSERT INTO game_uploads (id, owner_user_id, filename, status, report_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(owner_user_id.to_string())
    .bind(filename)
    .bind(status)
    .bind(report_json)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(id)
}

pub struct NewDraft<'a> {
    pub upload_id: Uuid,
    pub owner_user_id: Uuid,
    pub game_name: &'a str,
    pub display_name: &'a str,
    pub version: &'a str,
    pub status: &'a str,
    pub manifest_json: &'a str,
    pub report_json: &'a str,
    pub storage_path: &'a str,
}

pub async fn insert_game_draft(pool: &SqlitePool, draft: NewDraft<'_>) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        r#"INSERT INTO game_drafts
           (id, upload_id, owner_user_id, game_name, display_name, version, status, manifest_json, report_json, storage_path, created_at, updated_at, published_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)"#,
    )
    .bind(id.to_string())
    .bind(draft.upload_id.to_string())
    .bind(draft.owner_user_id.to_string())
    .bind(draft.game_name)
    .bind(draft.display_name)
    .bind(draft.version)
    .bind(draft.status)
    .bind(draft.manifest_json)
    .bind(draft.report_json)
    .bind(draft.storage_path)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(id)
}

fn map_draft_row(r: sqlx::sqlite::SqliteRow) -> Option<GameDraftRow> {
    let id_s: String = r.get("id");
    let upload_s: String = r.get("upload_id");
    let owner_s: String = r.get("owner_user_id");
    Some(GameDraftRow {
        id: Uuid::parse_str(&id_s).ok()?,
        upload_id: Uuid::parse_str(&upload_s).ok()?,
        owner_user_id: Uuid::parse_str(&owner_s).ok()?,
        game_name: r.get("game_name"),
        display_name: r.get("display_name"),
        version: r.get("version"),
        status: r.get("status"),
        manifest_json: r.get("manifest_json"),
        report_json: r.get("report_json"),
        storage_path: r.get("storage_path"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        published_at: r.get("published_at"),
    })
}

pub async fn list_game_drafts_for_owner(pool: &SqlitePool, owner_user_id: Uuid) -> Result<Vec<GameDraftRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, upload_id, owner_user_id, game_name, display_name, version, status, manifest_json, report_json, storage_path, created_at, updated_at, published_at FROM game_drafts WHERE owner_user_id = ? ORDER BY created_at DESC",
    )
    .bind(owner_user_id.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(map_draft_row).collect())
}

pub async fn get_game_draft(pool: &SqlitePool, draft_id: Uuid) -> Result<Option<GameDraftRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, upload_id, owner_user_id, game_name, display_name, version, status, manifest_json, report_json, storage_path, created_at, updated_at, published_at FROM game_drafts WHERE id = ?",
    )
    .bind(draft_id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(map_draft_row))
}

pub async fn mark_draft_published(pool: &SqlitePool, draft_id: Uuid) -> Result<(), sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    sqlx::query("UPDATE game_drafts SET status = 'published', published_at = ?, updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(now)
        .bind(draft_id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_draft_discarded(pool: &SqlitePool, draft_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE game_drafts SET status = 'discarded', updated_at = ? WHERE id = ?")
        .bind(GameInstanceStore::now_secs())
        .bind(draft_id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

/// Latest `published_at` among drafts with `status = 'published'` for this folder name (`game_name`).
pub async fn max_published_at_for_game_name(
    pool: &SqlitePool,
    game_name: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, Option<i64>>(
        "SELECT MAX(published_at) FROM game_drafts WHERE game_name = ? AND status = 'published'",
    )
    .bind(game_name)
    .fetch_one(pool)
    .await
}

/// Set every `published` draft for `game_name` back to `ready` and clear `published_at`
/// (live folder was removed; no row should stay `published` for that folder id).
pub async fn demote_all_published_for_game_name(pool: &SqlitePool, game_name: &str) -> Result<(), sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        "UPDATE game_drafts SET status = 'ready', published_at = NULL, updated_at = ? WHERE game_name = ? AND status = 'published'",
    )
    .bind(now)
    .bind(game_name)
    .execute(pool)
    .await?;
    Ok(())
}

/// Set a single `published` draft back to `ready` (older published row while a newer version is live).
pub async fn demote_single_published_draft(pool: &SqlitePool, draft_id: Uuid) -> Result<(), sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        "UPDATE game_drafts SET status = 'ready', published_at = NULL, updated_at = ? WHERE id = ? AND status = 'published'",
    )
    .bind(now)
    .bind(draft_id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

/// Counts drafts that reserve `game_name` + `version` in `ready` or `published` status.
pub async fn count_game_drafts_name_version_active(
    pool: &SqlitePool,
    game_name: &str,
    version: &str,
    exclude_draft_id: Option<Uuid>,
) -> Result<i64, sqlx::Error> {
    match exclude_draft_id {
        Some(ex) => {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM game_drafts WHERE game_name = ? AND version = ? AND status IN ('ready', 'published') AND id != ?",
            )
            .bind(game_name)
            .bind(version)
            .bind(ex.to_string())
            .fetch_one(pool)
            .await
        }
        None => {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM game_drafts WHERE game_name = ? AND version = ? AND status IN ('ready', 'published')",
            )
            .bind(game_name)
            .bind(version)
            .fetch_one(pool)
            .await
        }
    }
}

pub async fn update_game_draft_manifest_columns(
    pool: &SqlitePool,
    draft_id: Uuid,
    owner_user_id: Uuid,
    game_name: &str,
    display_name: &str,
    version: &str,
    manifest_json: &str,
) -> Result<bool, sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    let r = sqlx::query(
        "UPDATE game_drafts SET game_name = ?, display_name = ?, version = ?, manifest_json = ?, updated_at = ? WHERE id = ? AND owner_user_id = ? AND status = 'ready'",
    )
    .bind(game_name)
    .bind(display_name)
    .bind(version)
    .bind(manifest_json)
    .bind(now)
    .bind(draft_id.to_string())
    .bind(owner_user_id.to_string())
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}
