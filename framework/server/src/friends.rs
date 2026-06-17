use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use sqlx::Row;
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::db::GameInstanceStore;
use crate::user_engagement;

const ONLINE_TTL: Duration = Duration::from_secs(90);

#[derive(Clone)]
pub struct FriendsListNotify {
    pub tx: broadcast::Sender<()>,
}

impl FriendsListNotify {
    pub fn ping(&self) {
        let _ = self.tx.send(());
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }
}

#[derive(Clone, Default)]
pub struct OnlineTracker(Arc<RwLock<HashMap<Uuid, Instant>>>);

impl OnlineTracker {
    pub fn heartbeat(&self, user_id: Uuid) {
        if let Ok(mut guard) = self.0.write() {
            guard.insert(user_id, Instant::now());
        }
    }

    pub fn is_online(&self, user_id: Uuid) -> bool {
        let Ok(guard) = self.0.read() else {
            return false;
        };
        guard
            .get(&user_id)
            .map(|t| t.elapsed() < ONLINE_TTL)
            .unwrap_or(false)
    }
}

#[derive(Debug)]
pub enum FriendError {
    Sqlx(sqlx::Error),
    NotFound,
    NotFriends,
    SelfAction,
    AlreadyExists,
    Blocked,
    InvalidState(String),
}

impl fmt::Display for FriendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FriendError::Sqlx(e) => write!(f, "db: {e}"),
            FriendError::NotFound => write!(f, "user not found"),
            FriendError::NotFriends => write!(f, "not friends"),
            FriendError::SelfAction => write!(f, "cannot perform this action on yourself"),
            FriendError::AlreadyExists => write!(f, "friendship already exists"),
            FriendError::Blocked => write!(f, "blocked"),
            FriendError::InvalidState(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<sqlx::Error> for FriendError {
    fn from(value: sqlx::Error) -> Self {
        FriendError::Sqlx(value)
    }
}

#[derive(Clone, Debug)]
pub struct FriendRow {
    pub user_id: Uuid,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub since: i64,
}

#[derive(Clone, Debug)]
pub struct FriendRequestRow {
    pub user_id: Uuid,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug)]
pub struct UserSearchRow {
    pub id: Uuid,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub friendship_status: Option<String>,
}

#[derive(Clone, Debug)]
pub struct FriendActivityRow {
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_avatar_url: Option<String>,
    pub kind: String,
    pub target: String,
    pub timestamp: i64,
}

fn ordered_pair(a: Uuid, b: Uuid) -> (String, String) {
    let sa = a.to_string();
    let sb = b.to_string();
    if sa < sb {
        (sa, sb)
    } else {
        (sb, sa)
    }
}

async fn user_exists(pool: &SqlitePool, user_id: Uuid) -> Result<bool, sqlx::Error> {
    let row = sqlx::query("SELECT 1 FROM users WHERE id = ?")
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

async fn get_display_name(pool: &SqlitePool, user_id: Uuid) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query("SELECT display_name FROM users WHERE id = ?")
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get::<String, _>(0)))
}

async fn friendship_status_between(
    pool: &SqlitePool,
    user_id: Uuid,
    other_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let (a, b) = ordered_pair(user_id, other_id);
    let row = sqlx::query("SELECT status FROM friendships WHERE user_a = ? AND user_b = ?")
        .bind(&a)
        .bind(&b)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get::<String, _>(0)))
}

pub async fn send_friend_request(
    pool: &SqlitePool,
    from_id: Uuid,
    to_id: Uuid,
) -> Result<(), FriendError> {
    if from_id == to_id {
        return Err(FriendError::SelfAction);
    }
    if !user_exists(pool, to_id).await? {
        return Err(FriendError::NotFound);
    }
    let (a, b) = ordered_pair(from_id, to_id);
    if let Some(status) = friendship_status_between(pool, from_id, to_id).await? {
        return match status.as_str() {
            "accepted" => Err(FriendError::AlreadyExists),
            "pending" => Err(FriendError::AlreadyExists),
            "blocked" => Err(FriendError::Blocked),
            _ => Err(FriendError::InvalidState(status)),
        };
    }
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        r#"INSERT INTO friendships (user_a, user_b, status, requested_by, created_at, accepted_at)
           VALUES (?, ?, 'pending', ?, ?, NULL)"#,
    )
    .bind(&a)
    .bind(&b)
    .bind(from_id.to_string())
    .bind(now)
    .execute(pool)
    .await?;
    let from_name = get_display_name(pool, from_id)
        .await?
        .unwrap_or_else(|| "Someone".into());
    let _ = user_engagement::insert_notification(
        pool,
        to_id,
        &format!("{from_name} sent you a friend request"),
        &from_id.to_string(),
        "friend_request",
    )
    .await?;
    Ok(())
}

pub async fn accept_friend_request(
    pool: &SqlitePool,
    user_id: Uuid,
    from_id: Uuid,
) -> Result<(), FriendError> {
    let (a, b) = ordered_pair(user_id, from_id);
    let row = sqlx::query(
        "SELECT status, requested_by FROM friendships WHERE user_a = ? AND user_b = ?",
    )
    .bind(&a)
    .bind(&b)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Err(FriendError::NotFound);
    };
    let status: String = row.get(0);
    let requested_by: String = row.get(1);
    if status != "pending" {
        return Err(FriendError::InvalidState(
            "no pending request to accept".into(),
        ));
    }
    if requested_by == user_id.to_string() {
        return Err(FriendError::InvalidState(
            "cannot accept your own outgoing request".into(),
        ));
    }
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        "UPDATE friendships SET status = 'accepted', accepted_at = ? WHERE user_a = ? AND user_b = ?",
    )
    .bind(now)
    .bind(&a)
    .bind(&b)
    .execute(pool)
    .await?;
    let accepter_name = get_display_name(pool, user_id)
        .await?
        .unwrap_or_else(|| "Someone".into());
    let _ = user_engagement::insert_notification(
        pool,
        from_id,
        "Friend request accepted",
        &format!("{accepter_name} accepted your friend request"),
        "friend_accepted",
    )
    .await?;
    let friend_name = get_display_name(pool, from_id)
        .await?
        .unwrap_or_else(|| "Friend".into());
    insert_friend_activity(pool, user_id, "friend_added", &friend_name).await?;
    insert_friend_activity(pool, from_id, "friend_added", &accepter_name).await?;
    Ok(())
}

pub async fn decline_friend_request(
    pool: &SqlitePool,
    user_id: Uuid,
    from_id: Uuid,
) -> Result<(), FriendError> {
    let (a, b) = ordered_pair(user_id, from_id);
    let res = sqlx::query(
        "DELETE FROM friendships WHERE user_a = ? AND user_b = ? AND status = 'pending' AND requested_by = ?",
    )
    .bind(&a)
    .bind(&b)
    .bind(from_id.to_string())
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(FriendError::NotFound);
    }
    Ok(())
}

pub async fn cancel_friend_request(
    pool: &SqlitePool,
    user_id: Uuid,
    to_id: Uuid,
) -> Result<(), FriendError> {
    let (a, b) = ordered_pair(user_id, to_id);
    let res = sqlx::query(
        "DELETE FROM friendships WHERE user_a = ? AND user_b = ? AND status = 'pending' AND requested_by = ?",
    )
    .bind(&a)
    .bind(&b)
    .bind(user_id.to_string())
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(FriendError::NotFound);
    }
    Ok(())
}

pub async fn remove_friend(
    pool: &SqlitePool,
    user_id: Uuid,
    friend_id: Uuid,
) -> Result<(), FriendError> {
    let (a, b) = ordered_pair(user_id, friend_id);
    let res = sqlx::query(
        "DELETE FROM friendships WHERE user_a = ? AND user_b = ? AND status = 'accepted'",
    )
    .bind(&a)
    .bind(&b)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(FriendError::NotFound);
    }
    Ok(())
}

pub async fn block_user(
    pool: &SqlitePool,
    user_id: Uuid,
    target_id: Uuid,
) -> Result<(), FriendError> {
    if user_id == target_id {
        return Err(FriendError::SelfAction);
    }
    let (a, b) = ordered_pair(user_id, target_id);
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        r#"INSERT INTO friendships (user_a, user_b, status, requested_by, created_at, accepted_at)
           VALUES (?, ?, 'blocked', ?, ?, NULL)
           ON CONFLICT(user_a, user_b) DO UPDATE SET
             status = 'blocked',
             requested_by = excluded.requested_by,
             accepted_at = NULL"#,
    )
    .bind(&a)
    .bind(&b)
    .bind(user_id.to_string())
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn are_friends(pool: &SqlitePool, a: Uuid, b: Uuid) -> Result<bool, sqlx::Error> {
    Ok(friendship_status_between(pool, a, b)
        .await?
        .as_deref()
        == Some("accepted"))
}

pub async fn list_friends(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<FriendRow>, sqlx::Error> {
    let uid = user_id.to_string();
    let rows = sqlx::query(
        r#"SELECT
             CASE WHEN f.user_a = ? THEN f.user_b ELSE f.user_a END AS friend_id,
             u.display_name,
             u.avatar_url,
             COALESCE(f.accepted_at, f.created_at) AS since
           FROM friendships f
           JOIN users u ON u.id = CASE WHEN f.user_a = ? THEN f.user_b ELSE f.user_a END
           WHERE (f.user_a = ? OR f.user_b = ?) AND f.status = 'accepted'"#,
    )
    .bind(&uid)
    .bind(&uid)
    .bind(&uid)
    .bind(&uid)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for r in rows {
        let sid: String = r.get(0);
        let Ok(fid) = Uuid::parse_str(&sid) else {
            continue;
        };
        out.push(FriendRow {
            user_id: fid,
            display_name: r.get(1),
            avatar_url: r.get(2),
            since: r.get(3),
        });
    }
    Ok(out)
}

pub async fn list_pending_requests(
    pool: &SqlitePool,
    user_id: Uuid,
) -> Result<Vec<FriendRequestRow>, sqlx::Error> {
    let uid = user_id.to_string();
    let rows = sqlx::query(
        r#"SELECT u.id, u.display_name, u.avatar_url, f.created_at
           FROM friendships f
           JOIN users u ON u.id = f.requested_by
           WHERE f.status = 'pending'
             AND f.requested_by != ?
             AND (f.user_a = ? OR f.user_b = ?)"#,
    )
    .bind(&uid)
    .bind(&uid)
    .bind(&uid)
    .fetch_all(pool)
    .await?;
    map_request_rows(rows)
}

pub async fn list_sent_requests(
    pool: &SqlitePool,
    user_id: Uuid,
) -> Result<Vec<FriendRequestRow>, sqlx::Error> {
    let uid = user_id.to_string();
    let rows = sqlx::query(
        r#"SELECT u.id, u.display_name, u.avatar_url, f.created_at
           FROM friendships f
           JOIN users u ON u.id = CASE WHEN f.user_a = ? THEN f.user_b ELSE f.user_a END
           WHERE f.status = 'pending'
             AND f.requested_by = ?
             AND (f.user_a = ? OR f.user_b = ?)"#,
    )
    .bind(&uid)
    .bind(&uid)
    .bind(&uid)
    .bind(&uid)
    .fetch_all(pool)
    .await?;
    map_request_rows(rows)
}

fn map_request_rows(rows: Vec<sqlx::sqlite::SqliteRow>) -> Result<Vec<FriendRequestRow>, sqlx::Error> {
    let mut out = Vec::new();
    for r in rows {
        let sid: String = r.get(0);
        let Ok(uid) = Uuid::parse_str(&sid) else {
            continue;
        };
        out.push(FriendRequestRow {
            user_id: uid,
            display_name: r.get(1),
            avatar_url: r.get(2),
            created_at: r.get(3),
        });
    }
    Ok(out)
}

pub async fn pending_friend_request_count(
    pool: &SqlitePool,
    user_id: Uuid,
) -> Result<i32, sqlx::Error> {
    let uid = user_id.to_string();
    let row = sqlx::query(
        r#"SELECT COUNT(*) FROM friendships
           WHERE status = 'pending'
             AND requested_by != ?
             AND (user_a = ? OR user_b = ?)"#,
    )
    .bind(&uid)
    .bind(&uid)
    .bind(&uid)
    .fetch_one(pool)
    .await?;
    Ok(row.get::<i64, _>(0) as i32)
}

pub async fn search_users_for_friends(
    pool: &SqlitePool,
    user_id: Uuid,
    query: &str,
    limit: i64,
) -> Result<Vec<UserSearchRow>, sqlx::Error> {
    let needle = format!("%{}%", query.trim());
    let uid = user_id.to_string();
    let rows = sqlx::query(
        r#"SELECT u.id, u.display_name, u.avatar_url
           FROM users u
           WHERE u.id != ?
             AND u.display_name LIKE ?
           ORDER BY u.display_name ASC
           LIMIT ?"#,
    )
    .bind(&uid)
    .bind(&needle)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for r in rows {
        let sid: String = r.get(0);
        let Ok(id) = Uuid::parse_str(&sid) else {
            continue;
        };
        let status = friendship_status_between(pool, user_id, id).await?;
        out.push(UserSearchRow {
            id,
            display_name: r.get(1),
            avatar_url: r.get(2),
            friendship_status: status,
        });
    }
    Ok(out)
}

pub async fn insert_friend_activity(
    pool: &SqlitePool,
    user_id: Uuid,
    kind: &str,
    target: &str,
) -> Result<(), sqlx::Error> {
    let id = Uuid::new_v4();
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        "INSERT INTO friend_activity (id, user_id, kind, target, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(user_id.to_string())
    .bind(kind)
    .bind(target)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn friend_activity_feed(
    pool: &SqlitePool,
    user_id: Uuid,
    limit: usize,
) -> Result<Vec<FriendActivityRow>, sqlx::Error> {
    let uid = user_id.to_string();
    let lim = limit as i64;
    let rows = sqlx::query(
        r#"SELECT fa.user_id, u.display_name, u.avatar_url, fa.kind, fa.target, fa.created_at
           FROM friend_activity fa
           JOIN users u ON u.id = fa.user_id
           WHERE fa.user_id IN (
             SELECT CASE WHEN f.user_a = ? THEN f.user_b ELSE f.user_a END
             FROM friendships f
             WHERE (f.user_a = ? OR f.user_b = ?) AND f.status = 'accepted'
           )
           ORDER BY fa.created_at DESC
           LIMIT ?"#,
    )
    .bind(&uid)
    .bind(&uid)
    .bind(&uid)
    .bind(lim)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for r in rows {
        let sid: String = r.get(0);
        let Ok(actor_id) = Uuid::parse_str(&sid) else {
            continue;
        };
        out.push(FriendActivityRow {
            actor_id,
            actor_name: r.get(1),
            actor_avatar_url: r.get(2),
            kind: r.get(3),
            target: r.get(4),
            timestamp: r.get(5),
        });
    }
    Ok(out)
}

pub async fn invite_friend_to_lobby(
    pool: &SqlitePool,
    inviter_id: Uuid,
    friend_id: Uuid,
    lobby_id: Uuid,
) -> Result<(), FriendError> {
    if !are_friends(pool, inviter_id, friend_id).await? {
        return Err(FriendError::NotFriends);
    }
    let inviter_name = get_display_name(pool, inviter_id)
        .await?
        .unwrap_or_else(|| "Someone".into());
    let _ = user_engagement::insert_notification(
        pool,
        friend_id,
        &format!("{inviter_name} invited you to join a lobby"),
        &lobby_id.to_string(),
        "lobby_invite",
    )
    .await?;
    Ok(())
}

pub fn touch_presence(ctx: &async_graphql::Context<'_>, user_id: Uuid) {
    if let Ok(tracker) = ctx.data::<OnlineTracker>() {
        tracker.heartbeat(user_id);
    }
}
