use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::{self, GameInstanceStore};
use crate::platform_stats;

#[derive(Clone, Debug)]
pub struct BadgeDefinition {
    pub id: String,
    pub label: String,
    pub tier: String,
    pub description: Option<String>,
    pub sort_order: i32,
}

#[derive(Clone, Debug)]
pub struct BadgeRow {
    pub id: String,
    pub label: String,
    pub tier: String,
    pub locked: bool,
    pub earned_at: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct NotificationRow {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub kind: String,
    pub unread: bool,
    pub created_at: i64,
}

const BADGE_SEEDS: &[(&str, &str, &str, &str, i32)] = &[
    ("veteran", "Veteran", "Gold", "Play 10+ matches", 1),
    ("publisher", "Publisher", "Silver", "Publish your first game", 2),
    ("streak", "Win Streak", "Bronze", "Win 3+ matches", 3),
    ("elite", "Elite Operator", "Elite", "Reach 500 rep score", 4),
    ("architect", "Architect", "Elite", "Publish 3+ games as a developer", 5),
    ("mentor", "Mentor", "Gold", "Help the community (coming soon)", 6),
];

pub async fn ensure_badge_catalog(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for (id, label, tier, desc, order) in BADGE_SEEDS {
        sqlx::query(
            r#"INSERT INTO badge_definitions (id, label, tier, description, sort_order)
               VALUES (?, ?, ?, ?, ?)
               ON CONFLICT(id) DO NOTHING"#,
        )
        .bind(id)
        .bind(label)
        .bind(tier)
        .bind(desc)
        .bind(order)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn insert_notification(
    pool: &SqlitePool,
    user_id: Uuid,
    title: &str,
    body: &str,
    kind: &str,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        r#"INSERT INTO notifications (id, user_id, title, body, kind, read_at, created_at)
           VALUES (?, ?, ?, ?, ?, NULL, ?)"#,
    )
    .bind(id.to_string())
    .bind(user_id.to_string())
    .bind(title)
    .bind(body)
    .bind(kind)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn welcome_notification(pool: &SqlitePool, user_id: Uuid) -> Result<(), sqlx::Error> {
    insert_notification(
        pool,
        user_id,
        "Welcome to IPEL GameDev",
        "Your account is ready — browse games, join a lobby, or open the developer hub.",
        "system",
    )
    .await?;
    Ok(())
}

pub async fn list_notifications(
    pool: &SqlitePool,
    user_id: Uuid,
    limit: usize,
) -> Result<Vec<NotificationRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, Option<i64>, i64)>(
        r#"SELECT id, title, body, kind, read_at, created_at
           FROM notifications
           WHERE user_id = ?
           ORDER BY created_at DESC
           LIMIT ?"#,
    )
    .bind(user_id.to_string())
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|(id, title, body, kind, read_at, created_at)| {
            let id = Uuid::parse_str(&id).ok()?;
            Some(NotificationRow {
                id,
                title,
                body,
                kind,
                unread: read_at.is_none(),
                created_at,
            })
        })
        .collect())
}

pub async fn unread_count(pool: &SqlitePool, user_id: Uuid) -> Result<i32, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read_at IS NULL",
    )
    .bind(user_id.to_string())
    .fetch_one(pool)
    .await?;
    Ok(n as i32)
}

pub async fn mark_read(
    pool: &SqlitePool,
    user_id: Uuid,
    notification_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    let r = sqlx::query(
        "UPDATE notifications SET read_at = ? WHERE id = ? AND user_id = ? AND read_at IS NULL",
    )
    .bind(now)
    .bind(notification_id.to_string())
    .bind(user_id.to_string())
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn mark_all_read(pool: &SqlitePool, user_id: Uuid) -> Result<i32, sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    let r = sqlx::query(
        "UPDATE notifications SET read_at = ? WHERE user_id = ? AND read_at IS NULL",
    )
    .bind(now)
    .bind(user_id.to_string())
    .execute(pool)
    .await?;
    Ok(r.rows_affected() as i32)
}

async fn award_badge(
    pool: &SqlitePool,
    user_id: Uuid,
    badge_id: &str,
) -> Result<(), sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        r#"INSERT INTO user_badges (user_id, badge_id, earned_at)
           VALUES (?, ?, ?)
           ON CONFLICT(user_id, badge_id) DO NOTHING"#,
    )
    .bind(user_id.to_string())
    .bind(badge_id)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Reconcile auto-earned badges from profile stats and roles.
pub async fn sync_auto_badges(pool: &SqlitePool, user_id: Uuid) -> Result<(), sqlx::Error> {
    ensure_badge_catalog(pool).await?;
    let Some(stats) = platform_stats::build_user_profile(pool, user_id).await? else {
        return Ok(());
    };
    if stats.matches_played >= 10 {
        award_badge(pool, user_id, "veteran").await?;
    }
    if stats.games_published >= 1 {
        award_badge(pool, user_id, "publisher").await?;
    }
    if stats.wins >= 3 {
        award_badge(pool, user_id, "streak").await?;
    }
    if stats.rep_score >= 500 {
        award_badge(pool, user_id, "elite").await?;
    }
    if stats.games_published >= 3
        && db::user_has_role(pool, user_id, "developer")
            .await
            .unwrap_or(false)
    {
        award_badge(pool, user_id, "architect").await?;
    }
    Ok(())
}

pub async fn list_badges_for_user(
    pool: &SqlitePool,
    user_id: Uuid,
) -> Result<Vec<BadgeRow>, sqlx::Error> {
    ensure_badge_catalog(pool).await?;
    sync_auto_badges(pool, user_id).await?;

    let rows = sqlx::query_as::<_, (String, String, String, Option<i64>)>(
        r#"SELECT d.id, d.label, d.tier, ub.earned_at
           FROM badge_definitions d
           LEFT JOIN user_badges ub
             ON ub.badge_id = d.id AND ub.user_id = ?
           ORDER BY d.sort_order ASC, d.label ASC"#,
    )
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, label, tier, earned_at)| BadgeRow {
            locked: earned_at.is_none(),
            id,
            label,
            tier,
            earned_at,
        })
        .collect())
}

pub async fn notify_lobby_started(
    pool: &SqlitePool,
    user_ids: &[Uuid],
    game_type: &str,
) -> Result<(), sqlx::Error> {
    for uid in user_ids {
        insert_notification(
            pool,
            *uid,
            "Lobby started",
            &format!("Your {game_type} match is now in progress."),
            "lobby",
        )
        .await?;
    }
    Ok(())
}
