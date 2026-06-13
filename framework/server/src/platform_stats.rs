use crate::db::{self, FinishedGameRow};
use crate::lobby_db;
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LeaderboardEntry {
    pub display_name: String,
    pub total_score: i32,
    pub wins: u32,
    pub games_played: u32,
}

#[derive(Debug, Clone)]
pub struct GameSessionSummary {
    pub game_id: String,
    pub game_type: String,
    pub finished_at: i64,
    pub winner_display_name: Option<String>,
    pub participant_count: u32,
    pub duration_secs: i32,
}

#[derive(Debug, Clone)]
pub struct ActivityEventRow {
    pub actor: String,
    pub action: String,
    pub target: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct UserProfileStats {
    pub display_name: String,
    pub created_at: i64,
    pub matches_played: u32,
    pub games_published: u32,
    pub wins: u32,
    pub rep_score: u32,
}

pub fn identity_display_map(seats_json: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(v) = serde_json::from_str::<Value>(seats_json) else {
        return map;
    };
    let Some(arr) = v.as_array() else {
        return map;
    };
    for seat in arr {
        let identity = seat
            .get("player_identity")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let name = seat
            .get("claimed_display_name")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&identity)
            .to_string();
        if !identity.is_empty() {
            map.insert(identity, name);
        }
    }
    map
}

fn winner_from_result(result_json: &str, id_to_name: &HashMap<String, String>) -> Option<String> {
    let Ok(v) = serde_json::from_str::<Value>(result_json) else {
        return None;
    };
    let outcomes = v.get("per_player_outcome")?.as_object()?;
    let mut winners = Vec::new();
    for (pid, outcome) in outcomes {
        if outcome.as_str() == Some("Win") {
            winners.push(id_to_name.get(pid).cloned().unwrap_or_else(|| pid.clone()));
        }
    }
    if winners.len() == 1 {
        Some(winners.remove(0))
    } else {
        None
    }
}

pub fn map_finished_to_session(row: &FinishedGameRow) -> GameSessionSummary {
    let id_to_name = identity_display_map(&row.seats_snapshot_json);
    let participant_count = serde_json::from_str::<Value>(&row.seats_snapshot_json)
        .ok()
        .and_then(|v| v.as_array().map(|a| a.len() as u32))
        .unwrap_or(0);
    let duration_secs = row
        .started_at
        .map(|start| (row.finished_at - start).max(0) as i32)
        .unwrap_or(0);
    GameSessionSummary {
        game_id: row.id.to_string(),
        game_type: row.game_type.clone(),
        finished_at: row.finished_at,
        winner_display_name: winner_from_result(&row.result_json, &id_to_name),
        participant_count,
        duration_secs,
    }
}

pub fn compute_leaderboard(rows: &[FinishedGameRow], limit: usize) -> Vec<LeaderboardEntry> {
    let mut by_name: HashMap<String, LeaderboardEntry> = HashMap::new();
    for row in rows {
        let id_to_name = identity_display_map(&row.seats_snapshot_json);
        let winner = winner_from_result(&row.result_json, &id_to_name);
        let Ok(scores) = serde_json::from_str::<HashMap<String, f64>>(&row.player_scores_json)
        else {
            continue;
        };
        for (pid, score) in scores {
            let name = id_to_name.get(&pid).cloned().unwrap_or_else(|| pid.clone());
            let entry = by_name.entry(name.clone()).or_insert(LeaderboardEntry {
                display_name: name.clone(),
                total_score: 0,
                wins: 0,
                games_played: 0,
            });
            entry.total_score += (score * 1000.0).round() as i32;
            entry.games_played += 1;
            if winner.as_deref() == Some(name.as_str()) {
                entry.wins += 1;
            }
        }
    }
    let mut list: Vec<_> = by_name.into_values().collect();
    list.sort_by(|a, b| b.wins.cmp(&a.wins).then(b.total_score.cmp(&a.total_score)));
    list.truncate(limit);
    list
}

fn count_wins_for_user(rows: &[FinishedGameRow], user_id: Uuid) -> u32 {
    let uid = user_id.to_string();
    let mut wins = 0u32;
    for row in rows {
        let id_to_name = identity_display_map(&row.seats_snapshot_json);
        let user_display = serde_json::from_str::<Value>(&row.seats_snapshot_json)
            .ok()
            .and_then(|v| v.as_array().cloned())
            .and_then(|arr| {
                arr.into_iter().find_map(|seat| {
                    let claimed = seat.get("claimed_by_user_id")?.as_str()?;
                    if claimed == uid {
                        seat.get("claimed_display_name")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            });
        let Some(user_display) = user_display else {
            continue;
        };
        if winner_from_result(&row.result_json, &id_to_name).as_deref()
            == Some(user_display.as_str())
        {
            wins += 1;
        }
    }
    wins
}

pub async fn build_activity_feed(
    pool: &SqlitePool,
    limit: usize,
) -> Result<Vec<ActivityEventRow>, sqlx::Error> {
    let mut events = Vec::new();

    let finished = db::list_recent_finished_games(pool, 8).await?;
    for row in finished {
        let session = map_finished_to_session(&row);
        let actor = session
            .winner_display_name
            .clone()
            .unwrap_or_else(|| "System".into());
        let action = if session.winner_display_name.is_some() {
            "won"
        } else {
            "finished"
        };
        let short_id = session.game_id.chars().take(8).collect::<String>();
        events.push(ActivityEventRow {
            actor,
            action: action.into(),
            target: format!("{} #{short_id}", session.game_type),
            timestamp: session.finished_at,
        });
    }

    let lobbies = lobby_db::list_active_lobbies(pool).await?;
    for lob in lobbies.into_iter().take(8) {
        events.push(ActivityEventRow {
            actor: lob.owner_display_name.clone(),
            action: "created lobby".into(),
            target: format!(
                "{} #{}",
                lob.game_type,
                lob.id.to_string().chars().take(8).collect::<String>()
            ),
            timestamp: lob.created_at,
        });
    }

    let deployments = db::list_published_deployments(pool, 8).await?;
    for d in deployments {
        if let Some(ts) = d.published_at {
            events.push(ActivityEventRow {
                actor: "System".into(),
                action: "published".into(),
                target: format!("{} v{}", d.display_name, d.version),
                timestamp: ts,
            });
        }
    }

    events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    events.truncate(limit);
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaderboard_aggregates_wins() {
        let rows = vec![FinishedGameRow {
            id: Uuid::new_v4(),
            game_type: "tic_tac_toe".into(),
            lobby_id: None,
            finished_at: 120,
            started_at: Some(60),
            result_json: r#"{"version":1,"per_player_outcome":{"p1":"Win","p2":"Loss"}}"#.into(),
            player_scores_json: r#"{"p1":1.0,"p2":0.0}"#.into(),
            seats_snapshot_json: r#"[{"player_identity":"p1","claimed_display_name":"Alice","claimed_by_user_id":null},{"player_identity":"p2","claimed_display_name":"Bob","claimed_by_user_id":null}]"#.into(),
        }];
        let lb = compute_leaderboard(&rows, 10);
        assert_eq!(lb.len(), 2);
        assert_eq!(lb[0].display_name, "Alice");
        assert_eq!(lb[0].wins, 1);
    }
}

pub async fn build_user_profile(
    pool: &SqlitePool,
    user_id: Uuid,
) -> Result<Option<UserProfileStats>, sqlx::Error> {
    let Some((_, display_name, created_at)) = db::get_user(pool, user_id).await? else {
        return Ok(None);
    };
    let matches = db::count_user_finished_matches(pool, user_id).await? as u32;
    let published = db::count_published_drafts_for_user(pool, user_id).await? as u32;

    let all_finished = db::list_recent_finished_games(pool, 500).await?;
    let needle = user_id.to_string();
    let user_rows: Vec<_> = all_finished
        .into_iter()
        .filter(|r| r.seats_snapshot_json.contains(&needle))
        .collect();
    let wins = count_wins_for_user(&user_rows, user_id);
    let rep_score = wins * 10 + matches;

    Ok(Some(UserProfileStats {
        display_name,
        created_at,
        matches_played: matches,
        games_published: published,
        wins,
        rep_score,
    }))
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub active_lobbies: i32,
    pub published_game_types: i32,
    pub finished_games24h: i32,
    pub active_sessions: i32,
}

#[derive(Debug, Clone)]
pub struct KpiTrendRow {
    pub label: String,
    pub value: String,
    pub delta_pct: String,
    pub up: bool,
}

pub async fn record_metrics_snapshot(
    pool: &SqlitePool,
    snapshot: &MetricsSnapshot,
) -> Result<(), sqlx::Error> {
    let now = db::GameInstanceStore::now_secs();
    let last: Option<i64> = sqlx::query_scalar(
        "SELECT captured_at FROM platform_metrics_snapshots ORDER BY captured_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    if last.is_some_and(|t| now - t < 3600) {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO platform_metrics_snapshots (captured_at, active_lobbies, published_game_types, finished_games24h, active_sessions) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(now)
    .bind(snapshot.active_lobbies)
    .bind(snapshot.published_game_types)
    .bind(snapshot.finished_games24h)
    .bind(snapshot.active_sessions)
    .execute(pool)
    .await?;
    Ok(())
}

fn pct_delta(current: i32, previous: i32) -> (String, bool) {
    if previous <= 0 {
        return if current > 0 {
            ("new".into(), true)
        } else {
            ("—".into(), true)
        };
    }
    let delta = ((current - previous) as f64 / previous as f64) * 100.0;
    let up = delta >= 0.0;
    let sign = if up { "+" } else { "" };
    (format!("{sign}{delta:.0}%"), up)
}

pub async fn build_kpi_trends(
    pool: &SqlitePool,
    current: &MetricsSnapshot,
) -> Result<Vec<KpiTrendRow>, sqlx::Error> {
    let now = db::GameInstanceStore::now_secs();
    let target = now - 86_400;
    let row = sqlx::query(
        "SELECT active_lobbies, published_game_types, finished_games24h, active_sessions FROM platform_metrics_snapshots WHERE captured_at <= ? ORDER BY captured_at DESC LIMIT 1",
    )
    .bind(target)
    .fetch_optional(pool)
    .await?;
    let (prev_lobbies, prev_published, prev_finished, prev_sessions) = row
        .map(|r| {
            (
                r.get::<i32, _>(0),
                r.get::<i32, _>(1),
                r.get::<i32, _>(2),
                r.get::<i32, _>(3),
            )
        })
        .unwrap_or((0, 0, 0, 0));
    let (d0, up0) = pct_delta(current.active_lobbies, prev_lobbies);
    let (d1, up1) = pct_delta(current.published_game_types, prev_published);
    let (d2, up2) = pct_delta(current.finished_games24h, prev_finished);
    Ok(vec![
        KpiTrendRow {
            label: "Active lobbies".into(),
            value: current.active_lobbies.to_string(),
            delta_pct: d0,
            up: up0,
        },
        KpiTrendRow {
            label: "Published games".into(),
            value: current.published_game_types.to_string(),
            delta_pct: d1,
            up: up1,
        },
        KpiTrendRow {
            label: "Finished (24h)".into(),
            value: current.finished_games24h.to_string(),
            delta_pct: d2,
            up: up2,
        },
    ])
}

pub fn pro_tip_text() -> String {
    std::env::var("PLATFORM_PRO_TIP").unwrap_or_else(|_| {
        "Claim a seat and mark Ready before the host launches — staged lobbies won't start until everyone is set.".into()
    })
}
