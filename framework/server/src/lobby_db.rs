use sqlx::Row;
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Clone)]
pub struct LobbyListNotify {
    pub tx: broadcast::Sender<()>,
}

impl LobbyListNotify {
    pub fn ping(&self) {
        let _ = self.tx.send(());
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }
}

#[derive(Debug, Clone)]
pub struct LobbySummary {
    pub id: Uuid,
    #[allow(dead_code)]
    pub owner_user_id: Uuid,
    pub owner_display_name: String,
    pub game_type: String,
    pub status: String,
    pub game_instance_id: Option<String>,
    pub created_at: i64,
    pub seats_claimed: i64,
    pub seats_total: i64,
}

#[derive(Debug, Clone)]
pub struct LobbySeat {
    pub seat_index: i32,
    pub player_identity: String,
    pub claimed_by_user_id: Option<Uuid>,
    pub claimed_display_name: Option<String>,
    pub ready: bool,
    pub bot_id: Option<Uuid>,
    pub bot_display_name: Option<String>,
    pub external_bot: bool,
    pub external_bot_category: Option<String>,
    pub bot_avatar_seed: Option<String>,
    pub bot_avatar_url: Option<String>,
    pub bot_settings_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LobbyBotRequest {
    pub id: Uuid,
    pub lobby_id: Uuid,
    pub category: String,
    pub requested_by_user_id: Uuid,
    pub requested_by_bot_id: Option<Uuid>,
    pub bot_identity_id: Uuid,
    pub label: String,
    pub avatar_seed: Option<String>,
    pub avatar_url: Option<String>,
    pub game_slug: String,
    pub contract_hash: String,
    pub desired_seat_index: Option<i32>,
    pub status: String,
    pub seat_index: Option<i32>,
    pub connect_token: String,
    pub settings_json: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct LobbyDetail {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub owner_display_name: String,
    pub game_type: String,
    pub config: Option<String>,
    pub status: String,
    pub game_instance_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub seats: Vec<LobbySeat>,
}

#[derive(Debug, Clone)]
pub struct LobbyMessage {
    pub id: Uuid,
    pub user_id: Uuid,
    pub display_name: String,
    pub body: String,
    pub created_at: i64,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Lobbies visible on the home list (not cancelled).
pub async fn list_active_lobbies(pool: &SqlitePool) -> Result<Vec<LobbySummary>, sqlx::Error> {
    let rows = sqlx::query(
        r#"SELECT l.id, l.owner_user_id, u.display_name, l.game_type, l.status, l.game_instance_id,
                  l.created_at,
                  (SELECT COUNT(*) FROM lobby_seats s WHERE s.lobby_id = l.id AND (s.claimed_by_user_id IS NOT NULL OR s.bot_id IS NOT NULL OR s.external_bot != 0)),
                  (SELECT COUNT(*) FROM lobby_seats s WHERE s.lobby_id = l.id)
           FROM pregame_lobbies l
           JOIN users u ON u.id = l.owner_user_id
           WHERE l.status IN ('configuring', 'waiting', 'in_game')
           ORDER BY l.created_at DESC"#,
    )
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for r in rows {
        let id_s: String = r.get(0);
        let owner_s: String = r.get(1);
        if let (Ok(id), Ok(owner)) = (Uuid::parse_str(&id_s), Uuid::parse_str(&owner_s)) {
            out.push(LobbySummary {
                id,
                owner_user_id: owner,
                owner_display_name: r.get(2),
                game_type: r.get(3),
                status: r.get(4),
                game_instance_id: r.get(5),
                created_at: r.get(6),
                seats_claimed: r.get(7),
                seats_total: r.get(8),
            });
        }
    }
    Ok(out)
}

pub async fn list_lobbies_admin(
    pool: &SqlitePool,
    status: Option<&str>,
) -> Result<Vec<LobbySummary>, sqlx::Error> {
    let rows = if let Some(st) = status {
        sqlx::query(
            r#"SELECT l.id, l.owner_user_id, u.display_name, l.game_type, l.status, l.game_instance_id,
                      l.created_at,
                      (SELECT COUNT(*) FROM lobby_seats s WHERE s.lobby_id = l.id AND (s.claimed_by_user_id IS NOT NULL OR s.bot_id IS NOT NULL OR s.external_bot != 0)),
                      (SELECT COUNT(*) FROM lobby_seats s WHERE s.lobby_id = l.id)
               FROM pregame_lobbies l
               JOIN users u ON u.id = l.owner_user_id
               WHERE l.status = ?
               ORDER BY l.created_at DESC"#,
        )
        .bind(st)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"SELECT l.id, l.owner_user_id, u.display_name, l.game_type, l.status, l.game_instance_id,
                      l.created_at,
                      (SELECT COUNT(*) FROM lobby_seats s WHERE s.lobby_id = l.id AND (s.claimed_by_user_id IS NOT NULL OR s.bot_id IS NOT NULL OR s.external_bot != 0)),
                      (SELECT COUNT(*) FROM lobby_seats s WHERE s.lobby_id = l.id)
               FROM pregame_lobbies l
               JOIN users u ON u.id = l.owner_user_id
               ORDER BY l.created_at DESC"#,
        )
        .fetch_all(pool)
        .await?
    };
    let mut out = Vec::new();
    for r in rows {
        let id_s: String = r.get(0);
        let owner_s: String = r.get(1);
        if let (Ok(id), Ok(owner)) = (Uuid::parse_str(&id_s), Uuid::parse_str(&owner_s)) {
            out.push(LobbySummary {
                id,
                owner_user_id: owner,
                owner_display_name: r.get(2),
                game_type: r.get(3),
                status: r.get(4),
                game_instance_id: r.get(5),
                created_at: r.get(6),
                seats_claimed: r.get(7),
                seats_total: r.get(8),
            });
        }
    }
    Ok(out)
}

pub async fn get_lobby(
    pool: &SqlitePool,
    lobby_id: Uuid,
) -> Result<Option<LobbyDetail>, sqlx::Error> {
    let row = sqlx::query(
        r#"SELECT l.id, l.owner_user_id, u.display_name, l.game_type, l.config, l.status,
                  l.game_instance_id, l.created_at, l.updated_at
           FROM pregame_lobbies l
           JOIN users u ON u.id = l.owner_user_id
           WHERE l.id = ?"#,
    )
    .bind(lobby_id.to_string())
    .fetch_optional(pool)
    .await?;
    let Some(r) = row else {
        return Ok(None);
    };
    let id_s: String = r.get(0);
    let owner_s: String = r.get(1);
    let (Ok(id), Ok(owner)) = (Uuid::parse_str(&id_s), Uuid::parse_str(&owner_s)) else {
        return Ok(None);
    };
    let config: Option<String> = r.get(4);
    let seat_rows = sqlx::query(
        r#"SELECT s.seat_index, s.player_identity, s.claimed_by_user_id, u.display_name, s.ready,
                  s.bot_id, s.bot_display_name, s.external_bot, s.external_bot_category,
                  s.bot_avatar_seed, s.bot_avatar_url, s.bot_settings_json
           FROM lobby_seats s
           LEFT JOIN users u ON u.id = s.claimed_by_user_id
           WHERE s.lobby_id = ?
           ORDER BY s.seat_index"#,
    )
    .bind(lobby_id.to_string())
    .fetch_all(pool)
    .await?;
    let mut seats = Vec::new();
    for s in seat_rows {
        let claimed: Option<String> = s.get(2);
        let claimed_uuid = claimed.and_then(|c| Uuid::parse_str(&c).ok());
        let ready_i: i64 = s.get(4);
        let bot_id_s: Option<String> = s.get(5);
        let bot_id = bot_id_s.and_then(|id| Uuid::parse_str(&id).ok());
        let external_bot_i: i64 = s.get(7);
        seats.push(LobbySeat {
            seat_index: s.get(0),
            player_identity: s.get(1),
            claimed_by_user_id: claimed_uuid,
            claimed_display_name: s.get(3),
            ready: ready_i != 0,
            bot_id,
            bot_display_name: s.get(6),
            external_bot: external_bot_i != 0,
            external_bot_category: s.get(8),
            bot_avatar_seed: s.get(9),
            bot_avatar_url: s.get(10),
            bot_settings_json: s.get(11),
        });
    }
    Ok(Some(LobbyDetail {
        id,
        owner_user_id: owner,
        owner_display_name: r.get(2),
        game_type: r.get(3),
        config,
        status: r.get(5),
        game_instance_id: r.get(6),
        created_at: r.get(7),
        updated_at: r.get(8),
        seats,
    }))
}

/// Create lobby: game type only, no seats, `config` NULL, status `configuring`.
pub async fn insert_lobby_skeleton(
    pool: &SqlitePool,
    id: Uuid,
    owner: Uuid,
    game_type: &str,
) -> Result<(), sqlx::Error> {
    let now = now_secs();
    sqlx::query(
        r#"INSERT INTO pregame_lobbies (id, owner_user_id, game_type, config, status, game_instance_id, created_at, updated_at)
           VALUES (?, ?, ?, NULL, 'configuring', NULL, ?, ?)"#,
    )
    .bind(id.to_string())
    .bind(owner.to_string())
    .bind(game_type)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

fn has_claimed_seats(detail: &LobbyDetail) -> bool {
    detail.seats.iter().any(|s| {
        s.claimed_by_user_id.is_some() || s.bot_id.is_some() || s.external_bot
    })
}

/// Owner replaces seats from WASM init preview. Fails if non-empty claims exist unless `force`.
/// Preserves stored `config`; only `game_type` and seats change.
pub async fn owner_replace_game_type_and_seats(
    pool: &SqlitePool,
    lobby_id: Uuid,
    owner: Uuid,
    new_game_type: &str,
    identities: &[String],
    config: Option<&str>,
    force: bool,
) -> Result<(), String> {
    let detail = get_lobby(pool, lobby_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "lobby not found".to_string())?;
    if detail.owner_user_id != owner {
        return Err("only the owner can change game type".into());
    }
    if detail.status == "in_game" {
        return Err("lobby is in game".into());
    }
    if detail.status == "cancelled" {
        return Err("lobby is cancelled".into());
    }
    if new_game_type.trim().is_empty() {
        return Err("game type must not be empty".into());
    }
    if !force && has_claimed_seats(&detail) {
        return Err("seats are claimed; confirm reset or pass force".into());
    }
    if identities.is_empty() {
        return Err("game reported no player seats".into());
    }
    let now = now_secs();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM lobby_seats WHERE lobby_id = ?")
        .bind(lobby_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE pregame_lobbies SET game_type = ?, config = ?, status = 'waiting', updated_at = ? WHERE id = ?",
    )
    .bind(new_game_type)
    .bind(config)
    .bind(now)
    .bind(lobby_id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    for (i, ident) in identities.iter().enumerate() {
        sqlx::query(
            "INSERT INTO lobby_seats (lobby_id, seat_index, player_identity, claimed_by_user_id, ready) VALUES (?, ?, ?, NULL, 0)",
        )
        .bind(lobby_id.to_string())
        .bind(i as i32)
        .bind(ident)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Owner updates config and replaces seats from preview.
pub async fn owner_replace_config_and_seats(
    pool: &SqlitePool,
    lobby_id: Uuid,
    owner: Uuid,
    expected_game_type: &str,
    config: &str,
    identities: &[String],
    force: bool,
) -> Result<(), String> {
    let detail = get_lobby(pool, lobby_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "lobby not found".to_string())?;
    if detail.owner_user_id != owner {
        return Err("only the owner can change config".into());
    }
    if detail.game_type != expected_game_type {
        return Err("game type mismatch".into());
    }
    if detail.status == "in_game" {
        return Err("lobby is in game".into());
    }
    if detail.status == "cancelled" {
        return Err("lobby is cancelled".into());
    }
    if !force && has_claimed_seats(&detail) {
        return Err("seats are claimed; confirm reset or pass force".into());
    }
    if identities.is_empty() {
        return Err("game reported no player seats".into());
    }
    let now = now_secs();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM lobby_seats WHERE lobby_id = ?")
        .bind(lobby_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE pregame_lobbies SET config = ?, status = 'waiting', updated_at = ? WHERE id = ?",
    )
    .bind(config)
    .bind(now)
    .bind(lobby_id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    for (i, ident) in identities.iter().enumerate() {
        sqlx::query(
            "INSERT INTO lobby_seats (lobby_id, seat_index, player_identity, claimed_by_user_id, ready) VALUES (?, ?, ?, NULL, 0)",
        )
        .bind(lobby_id.to_string())
        .bind(i as i32)
        .bind(ident)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn claim_seat(
    pool: &SqlitePool,
    lobby_id: Uuid,
    seat_index: i32,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let existing = sqlx::query(
        "SELECT seat_index FROM lobby_seats WHERE lobby_id = ? AND claimed_by_user_id = ?",
    )
    .bind(lobby_id.to_string())
    .bind(user_id.to_string())
    .fetch_optional(pool)
    .await?;
    let existing_idx: Option<i32> = existing.map(|r| r.get(0));
    if let Some(idx) = existing_idx {
        if idx != seat_index {
            return Ok(false);
        }
        return Ok(true);
    }
    let r = sqlx::query(
        r#"UPDATE lobby_seats SET claimed_by_user_id = ?, ready = 0
           WHERE lobby_id = ? AND seat_index = ? AND claimed_by_user_id IS NULL AND bot_id IS NULL AND external_bot = 0"#,
    )
    .bind(user_id.to_string())
    .bind(lobby_id.to_string())
    .bind(seat_index)
    .execute(pool)
    .await?;
    if r.rows_affected() == 0 {
        return Ok(false);
    }
    sqlx::query("UPDATE pregame_lobbies SET updated_at = ? WHERE id = ?")
        .bind(now_secs())
        .bind(lobby_id.to_string())
        .execute(pool)
        .await?;
    Ok(true)
}

/// Current owner passes host controls to a seated player (`new_owner` must have a claimed seat).
pub async fn transfer_lobby_ownership(
    pool: &SqlitePool,
    lobby_id: Uuid,
    current_owner: Uuid,
    new_owner: Uuid,
) -> Result<(), String> {
    if current_owner == new_owner {
        return Err("cannot transfer ownership to yourself".into());
    }
    let detail = get_lobby(pool, lobby_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "lobby not found".to_string())?;
    if detail.owner_user_id != current_owner {
        return Err("only the owner can transfer the lobby".into());
    }
    if detail.status == "in_game" {
        return Err("cannot transfer ownership while in game".into());
    }
    if detail.status == "cancelled" {
        return Err("lobby is cancelled".into());
    }
    let seated = detail
        .seats
        .iter()
        .any(|s| s.claimed_by_user_id == Some(new_owner));
    if !seated {
        return Err("new owner must have a claimed seat in this lobby".into());
    }
    let now = now_secs();
    let r = sqlx::query(
        "UPDATE pregame_lobbies SET owner_user_id = ?, updated_at = ? WHERE id = ? AND owner_user_id = ?",
    )
    .bind(new_owner.to_string())
    .bind(now)
    .bind(lobby_id.to_string())
    .bind(current_owner.to_string())
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    if r.rows_affected() == 0 {
        return Err("ownership transfer failed".into());
    }
    Ok(())
}

pub async fn release_user_seats(
    pool: &SqlitePool,
    lobby_id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE lobby_seats SET claimed_by_user_id = NULL, ready = 0 WHERE lobby_id = ? AND claimed_by_user_id = ?",
    )
    .bind(lobby_id.to_string())
    .bind(user_id.to_string())
    .execute(pool)
    .await?;
    sqlx::query("UPDATE pregame_lobbies SET updated_at = ? WHERE id = ?")
        .bind(now_secs())
        .bind(lobby_id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn kick_lobby_player(
    pool: &SqlitePool,
    lobby_id: Uuid,
    owner_id: Uuid,
    target_user_id: Uuid,
) -> Result<(), String> {
    if owner_id == target_user_id {
        return Err("cannot kick yourself".into());
    }
    let detail = get_lobby(pool, lobby_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "lobby not found".to_string())?;
    if detail.owner_user_id != owner_id {
        return Err("only the owner can kick players".into());
    }
    if detail.status == "in_game" {
        return Err("cannot kick players while in game".into());
    }
    if detail.status == "cancelled" {
        return Err("lobby is cancelled".into());
    }
    let seated = detail
        .seats
        .iter()
        .any(|s| s.claimed_by_user_id == Some(target_user_id));
    if !seated {
        return Err("player is not in a seat".into());
    }
    release_user_seats(pool, lobby_id, target_user_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Sets `ready` for the seat claimed by `user_id` in this lobby. Returns false if the user has no seat.
pub async fn set_seat_ready(
    pool: &SqlitePool,
    lobby_id: Uuid,
    user_id: Uuid,
    ready: bool,
) -> Result<bool, sqlx::Error> {
    let ready_i: i32 = if ready { 1 } else { 0 };
    let r = sqlx::query(
        "UPDATE lobby_seats SET ready = ? WHERE lobby_id = ? AND claimed_by_user_id = ?",
    )
    .bind(ready_i)
    .bind(lobby_id.to_string())
    .bind(user_id.to_string())
    .execute(pool)
    .await?;
    if r.rows_affected() == 0 {
        return Ok(false);
    }
    sqlx::query("UPDATE pregame_lobbies SET updated_at = ? WHERE id = ?")
        .bind(now_secs())
        .bind(lobby_id.to_string())
        .execute(pool)
        .await?;
    Ok(true)
}

pub async fn mark_lobby_in_game(
    pool: &SqlitePool,
    lobby_id: Uuid,
    game_instance_id: Uuid,
) -> Result<(), sqlx::Error> {
    let now = now_secs();
    sqlx::query(
        "UPDATE pregame_lobbies SET status = 'in_game', game_instance_id = ?, updated_at = ? WHERE id = ?",
    )
    .bind(game_instance_id.to_string())
    .bind(now)
    .bind(lobby_id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

/// After the game instance reports a terminal result; lobby leaves the active list.
pub async fn mark_lobby_finished(pool: &SqlitePool, lobby_id: Uuid) -> Result<(), sqlx::Error> {
    let now = now_secs();
    sqlx::query(
        "UPDATE pregame_lobbies SET status = 'finished', updated_at = ? WHERE id = ? AND status = 'in_game'",
    )
    .bind(now)
    .bind(lobby_id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reopen_lobby_after_game(
    pool: &SqlitePool,
    lobby_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let now = now_secs();
    let r = sqlx::query(
        "UPDATE pregame_lobbies SET status = 'waiting', game_instance_id = NULL, updated_at = ? WHERE id = ? AND status IN ('in_game', 'finished')",
    )
    .bind(now)
    .bind(lobby_id.to_string())
    .execute(pool)
    .await?;
    if r.rows_affected() > 0 {
        sqlx::query("UPDATE lobby_seats SET ready = 0 WHERE lobby_id = ?")
            .bind(lobby_id.to_string())
            .execute(pool)
            .await?;
    }
    Ok(r.rows_affected() > 0)
}

pub async fn cancel_lobby(pool: &SqlitePool, lobby_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE pregame_lobbies SET status = 'cancelled', updated_at = ? WHERE id = ?")
        .bind(now_secs())
        .bind(lobby_id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_lobby_messages(
    pool: &SqlitePool,
    lobby_id: Uuid,
    limit: i64,
) -> Result<Vec<LobbyMessage>, sqlx::Error> {
    let rows = sqlx::query(
        r#"SELECT m.id, m.user_id, u.display_name, m.body, m.created_at
           FROM lobby_messages m
           JOIN users u ON u.id = m.user_id
           WHERE m.lobby_id = ?
           ORDER BY m.created_at DESC
           LIMIT ?"#,
    )
    .bind(lobby_id.to_string())
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for r in rows {
        let id_s: String = r.get(0);
        let uid_s: String = r.get(1);
        if let (Ok(id), Ok(uid)) = (Uuid::parse_str(&id_s), Uuid::parse_str(&uid_s)) {
            out.push(LobbyMessage {
                id,
                user_id: uid,
                display_name: r.get(2),
                body: r.get(3),
                created_at: r.get(4),
            });
        }
    }
    out.reverse();
    Ok(out)
}

pub async fn insert_lobby_message(
    pool: &SqlitePool,
    lobby_id: Uuid,
    user_id: Uuid,
    body: &str,
) -> Result<LobbyMessage, sqlx::Error> {
    let id = Uuid::new_v4();
    let now = now_secs();
    sqlx::query(
        r#"INSERT INTO lobby_messages (id, lobby_id, user_id, body, created_at)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(id.to_string())
    .bind(lobby_id.to_string())
    .bind(user_id.to_string())
    .bind(body)
    .bind(now)
    .execute(pool)
    .await?;
    let row = sqlx::query(
        r#"SELECT m.id, m.user_id, u.display_name, m.body, m.created_at
           FROM lobby_messages m
           JOIN users u ON u.id = m.user_id
           WHERE m.id = ?"#,
    )
    .bind(id.to_string())
    .fetch_one(pool)
    .await?;
    let id_s: String = row.get(0);
    let uid_s: String = row.get(1);
    let lid = Uuid::parse_str(&id_s).unwrap_or(id);
    let uid = Uuid::parse_str(&uid_s).unwrap_or(user_id);
    Ok(LobbyMessage {
        id: lid,
        user_id: uid,
        display_name: row.get(2),
        body: row.get(3),
        created_at: row.get(4),
    })
}

/// First matching user with a password set (display names may duplicate).
pub async fn find_user_by_display_name_and_password(
    pool: &SqlitePool,
    display_name: &str,
) -> Result<Vec<(Uuid, Option<String>)>, sqlx::Error> {
    let rows = sqlx::query("SELECT id, password_hash FROM users WHERE display_name = ?")
        .bind(display_name)
        .fetch_all(pool)
        .await?;
    let mut out = Vec::new();
    for r in rows {
        let id_s: String = r.get(0);
        let hash: Option<String> = r.get(1);
        if let Ok(id) = Uuid::parse_str(&id_s) {
            out.push((id, hash));
        }
    }
    Ok(out)
}

/// Assign a published bot to an open seat (owner or any user in staging lobby).
pub async fn assign_bot_to_seat(
    pool: &SqlitePool,
    lobby_id: Uuid,
    seat_index: i32,
    bot_id: Uuid,
    bot_display_name: &str,
    bot_avatar_seed: Option<&str>,
    bot_avatar_url: Option<&str>,
    bot_settings_json: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        r#"UPDATE lobby_seats SET bot_id = ?, bot_display_name = ?, bot_avatar_seed = ?, bot_avatar_url = ?,
                  bot_settings_json = ?, ready = 1, external_bot = 0, external_bot_category = NULL, external_bot_token = NULL,
                  claimed_by_user_id = NULL
           WHERE lobby_id = ? AND seat_index = ?
             AND claimed_by_user_id IS NULL AND bot_id IS NULL AND external_bot = 0"#,
    )
    .bind(bot_id.to_string())
    .bind(bot_display_name)
    .bind(bot_avatar_seed)
    .bind(bot_avatar_url)
    .bind(bot_settings_json)
    .bind(lobby_id.to_string())
    .bind(seat_index)
    .execute(pool)
    .await?;
    if r.rows_affected() == 0 {
        return Ok(false);
    }
    sqlx::query("UPDATE pregame_lobbies SET updated_at = ? WHERE id = ?")
        .bind(now_secs())
        .bind(lobby_id.to_string())
        .execute(pool)
        .await?;
    Ok(true)
}

pub async fn remove_bot_from_seat(
    pool: &SqlitePool,
    lobby_id: Uuid,
    seat_index: i32,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        r#"UPDATE lobby_seats SET bot_id = NULL, bot_display_name = NULL, bot_avatar_seed = NULL,
                  bot_avatar_url = NULL, bot_settings_json = NULL, external_bot = 0, external_bot_category = NULL,
                  external_bot_token = NULL, ready = 0
           WHERE lobby_id = ? AND seat_index = ? AND (bot_id IS NOT NULL OR external_bot != 0)"#,
    )
    .bind(lobby_id.to_string())
    .bind(seat_index)
    .execute(pool)
    .await?;
    if r.rows_affected() == 0 {
        return Ok(false);
    }
    sqlx::query("UPDATE pregame_lobbies SET updated_at = ? WHERE id = ?")
        .bind(now_secs())
        .bind(lobby_id.to_string())
        .execute(pool)
        .await?;
    Ok(true)
}

fn map_bot_request_row(r: sqlx::sqlite::SqliteRow) -> LobbyBotRequest {
    let id_s: String = r.get(0);
    let lobby_s: String = r.get(1);
    let req_user_s: String = r.get(3);
    let bot_identity_s: String = r.get(5);
    LobbyBotRequest {
        id: Uuid::parse_str(&id_s).unwrap_or_else(|_| Uuid::nil()),
        lobby_id: Uuid::parse_str(&lobby_s).unwrap_or_else(|_| Uuid::nil()),
        category: r.get(2),
        requested_by_user_id: Uuid::parse_str(&req_user_s).unwrap_or_else(|_| Uuid::nil()),
        requested_by_bot_id: r.get::<Option<String>, _>(4)
            .and_then(|s| Uuid::parse_str(&s).ok()),
        bot_identity_id: Uuid::parse_str(&bot_identity_s).unwrap_or_else(|_| Uuid::nil()),
        label: r.get(6),
        avatar_seed: r.get(7),
        avatar_url: r.get(8),
        game_slug: r.get(9),
        contract_hash: r.get(10),
        desired_seat_index: r.get(11),
        status: r.get(12),
        seat_index: r.get(13),
        connect_token: r.get(14),
        settings_json: r.get(15),
        created_at: r.get(16),
    }
}

const BOT_REQUEST_SELECT: &str = r#"SELECT id, lobby_id, category, requested_by_user_id, requested_by_bot_id,
    bot_identity_id, label, avatar_seed, avatar_url, game_slug, contract_hash, desired_seat_index,
    status, seat_index, connect_token, settings_json, created_at FROM lobby_bot_requests"#;

pub async fn create_bot_request(
    pool: &SqlitePool,
    id: Uuid,
    lobby_id: Uuid,
    category: &str,
    requested_by_user_id: Uuid,
    requested_by_bot_id: Option<Uuid>,
    bot_identity_id: Uuid,
    label: &str,
    avatar_seed: Option<&str>,
    avatar_url: Option<&str>,
    game_slug: &str,
    contract_hash: &str,
    desired_seat_index: Option<i32>,
    connect_token: &str,
    settings_json: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO lobby_bot_requests
           (id, lobby_id, category, requested_by_user_id, requested_by_bot_id, bot_identity_id,
            label, avatar_seed, avatar_url, game_slug, contract_hash, desired_seat_index,
            status, seat_index, connect_token, settings_json, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', NULL, ?, ?, ?)"#,
    )
    .bind(id.to_string())
    .bind(lobby_id.to_string())
    .bind(category)
    .bind(requested_by_user_id.to_string())
    .bind(requested_by_bot_id.map(|u| u.to_string()))
    .bind(bot_identity_id.to_string())
    .bind(label)
    .bind(avatar_seed)
    .bind(avatar_url)
    .bind(game_slug)
    .bind(contract_hash)
    .bind(desired_seat_index)
    .bind(connect_token)
    .bind(settings_json)
    .bind(now_secs())
    .execute(pool)
    .await?;
    sqlx::query("UPDATE pregame_lobbies SET updated_at = ? WHERE id = ?")
        .bind(now_secs())
        .bind(lobby_id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_bot_requests(
    pool: &SqlitePool,
    lobby_id: Uuid,
    status: Option<&str>,
) -> Result<Vec<LobbyBotRequest>, sqlx::Error> {
    let rows = if let Some(st) = status {
        sqlx::query(&format!("{BOT_REQUEST_SELECT} WHERE lobby_id = ? AND status = ? ORDER BY created_at"))
            .bind(lobby_id.to_string())
            .bind(st)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query(&format!("{BOT_REQUEST_SELECT} WHERE lobby_id = ? ORDER BY created_at"))
            .bind(lobby_id.to_string())
            .fetch_all(pool)
            .await?
    };
    Ok(rows.into_iter().map(map_bot_request_row).collect())
}

pub async fn get_bot_request(
    pool: &SqlitePool,
    request_id: Uuid,
) -> Result<Option<LobbyBotRequest>, sqlx::Error> {
    let row = sqlx::query(&format!("{BOT_REQUEST_SELECT} WHERE id = ?"))
        .bind(request_id.to_string())
        .fetch_optional(pool)
        .await?;
    Ok(row.map(map_bot_request_row))
}

pub async fn set_bot_request_status(
    pool: &SqlitePool,
    request_id: Uuid,
    status: &str,
    seat_index: Option<i32>,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE lobby_bot_requests SET status = ?, seat_index = ? WHERE id = ? AND status = 'pending'",
    )
    .bind(status)
    .bind(seat_index)
    .bind(request_id.to_string())
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn assign_external_bot_seat(
    pool: &SqlitePool,
    lobby_id: Uuid,
    seat_index: i32,
    bot_identity_id: Uuid,
    label: &str,
    avatar_seed: Option<&str>,
    avatar_url: Option<&str>,
    connect_token: &str,
    category: &str,
    bot_settings_json: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        r#"UPDATE lobby_seats SET bot_id = ?, bot_display_name = ?, bot_avatar_seed = ?, bot_avatar_url = ?,
                  bot_settings_json = ?, external_bot = 1, external_bot_category = ?, external_bot_token = ?, ready = 1,
                  claimed_by_user_id = NULL
           WHERE lobby_id = ? AND seat_index = ?
             AND claimed_by_user_id IS NULL AND bot_id IS NULL AND external_bot = 0"#,
    )
    .bind(bot_identity_id.to_string())
    .bind(label)
    .bind(avatar_seed)
    .bind(avatar_url)
    .bind(bot_settings_json)
    .bind(category)
    .bind(connect_token)
    .bind(lobby_id.to_string())
    .bind(seat_index)
    .execute(pool)
    .await?;
    if r.rows_affected() == 0 {
        return Ok(false);
    }
    sqlx::query("UPDATE pregame_lobbies SET updated_at = ? WHERE id = ?")
        .bind(now_secs())
        .bind(lobby_id.to_string())
        .execute(pool)
        .await?;
    Ok(true)
}

pub async fn release_external_bot_seat(
    pool: &SqlitePool,
    lobby_id: Uuid,
    seat_index: i32,
) -> Result<bool, sqlx::Error> {
    remove_bot_from_seat(pool, lobby_id, seat_index).await
}

/// Validate external bot WS connect token for a player identity in a lobby game.
pub async fn validate_external_bot_token(
    pool: &SqlitePool,
    lobby_id: Uuid,
    player_identity: &str,
    token: &str,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        r#"SELECT external_bot_token FROM lobby_seats
           WHERE lobby_id = ? AND player_identity = ? AND external_bot != 0"#,
    )
    .bind(lobby_id.to_string())
    .bind(player_identity)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .and_then(|r| r.get::<Option<String>, _>(0))
        .is_some_and(|t| t == token))
}

pub async fn lobby_id_for_game_instance(
    pool: &SqlitePool,
    game_instance_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id FROM pregame_lobbies WHERE game_instance_id = ? LIMIT 1",
    )
    .bind(game_instance_id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|r| {
        let s: String = r.get(0);
        Uuid::parse_str(&s).ok()
    }))
}
