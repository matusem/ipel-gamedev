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
                  (SELECT COUNT(*) FROM lobby_seats s WHERE s.lobby_id = l.id AND s.claimed_by_user_id IS NOT NULL),
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
        r#"SELECT s.seat_index, s.player_identity, s.claimed_by_user_id, u.display_name, s.ready
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
        seats.push(LobbySeat {
            seat_index: s.get(0),
            player_identity: s.get(1),
            claimed_by_user_id: claimed_uuid,
            claimed_display_name: s.get(3),
            ready: ready_i != 0,
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
    detail.seats.iter().any(|s| s.claimed_by_user_id.is_some())
}

/// Owner replaces seats from WASM init preview. Fails if non-empty claims exist unless `force`.
/// Preserves stored `config`; only `game_type` and seats change.
pub async fn owner_replace_game_type_and_seats(
    pool: &SqlitePool,
    lobby_id: Uuid,
    owner: Uuid,
    new_game_type: &str,
    identities: &[String],
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
        "UPDATE pregame_lobbies SET game_type = ?, status = 'waiting', updated_at = ? WHERE id = ?",
    )
    .bind(new_game_type)
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
           WHERE lobby_id = ? AND seat_index = ? AND claimed_by_user_id IS NULL"#,
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
