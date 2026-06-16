use std::sync::{Arc, RwLock};

use async_graphql::{Context, Error, Object, Result};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::{self, GameInstanceStore};
use crate::game_db::GameDb;
use crate::game_registry::GameRegistry;
use crate::game_storefront;
use crate::lobby_db;
use crate::platform_stats;
use crate::user_engagement;

use super::{
    ActivityEventGql, AdminCommentGql, AdminPlatformOverviewGql, AdminReviewGql, AdminUserGql,
    BadgeGql, CliReleaseGql, DeploymentGql, FinishedGameGql, GameCommentGql,
    GameDraftGql, GameInstanceGql, GamePatchNoteGql, GameReviewGql, GameScreenshotGql,
    GameSessionGql, GameStorefrontGql, GameTypeGql, LeaderboardEntryGql, LobbyGql, LobbySummaryGql,
    NotificationGql, PlatformManifestGql, PlatformStatsGql, PlayTimeLeaderboardEntryGql,
    PublishTokenSummaryGql, UserGql, UserProfileGql, lobby_to_gql, map_aspect_ratings, map_draft,
    map_finished_row, map_game_entries,     map_summary, require_developer_user,
    require_registered_user, require_superadmin_user, is_superadmin,
};
/// Root query.
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn game_types(&self, ctx: &Context<'_>) -> Result<Vec<GameTypeGql>> {
        let pool = ctx.data::<SqlitePool>()?;
        let reg = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let game_db = ctx.data::<GameDb>()?;
        let game_types: Vec<crate::game_registry::GameType> = {
            let guard = reg
                .read()
                .map_err(|_| Error::new("registry lock poisoned"))?;
            guard.game_types().to_vec()
        };
        let mut live_players: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        for entry in map_game_entries(game_db) {
            *live_players.entry(entry.game_type).or_insert(0) += entry.connected_players as i32;
        }
        let mut out = Vec::new();
        for gt in game_types {
            let name = gt.manifest.name.clone();
            let (featured, creator, tags, avg_mins) =
                game_storefront::catalog_meta_for_game(pool, &name)
                    .await
                    .unwrap_or((false, None, vec![], 10));
            let cover = game_storefront::get_storefront(pool, &name)
                .await
                .ok()
                .flatten()
                .and_then(|sf| sf.screenshots.first().and_then(|s| s.image_url.clone()))
                .or_else(|| game_storefront::default_cover_image(&name));
            out.push(GameTypeGql {
                name: name.clone(),
                display_name: gt.manifest.display_name.clone(),
                version: gt.manifest.version.clone(),
                min_players: gt.manifest.min_players,
                max_players: gt.manifest.max_players,
                description: gt.manifest.description.clone(),
                config_ui_path: gt.config_ui_path.clone(),
                result_ui_path: gt.result_ui_path.clone(),
                about_ui_path: gt.about_ui_path.clone(),
                config_schema_json: gt
                    .manifest
                    .config_schema
                    .as_ref()
                    .and_then(|v| serde_json::to_string(v).ok()),
                cover_image_url: cover,
                active_players: *live_players.get(&name).unwrap_or(&0),
                featured,
                tags,
                creator_display_name: creator,
                avg_session_mins: avg_mins,
            });
        }
        Ok(out)
    }

    async fn game_instances(&self, ctx: &Context<'_>) -> Result<Vec<GameInstanceGql>> {
        let db = ctx.data::<GameDb>()?;
        Ok(map_game_entries(db))
    }

    async fn finished_game(
        &self,
        ctx: &Context<'_>,
        game_id: async_graphql::types::ID,
    ) -> Result<Option<FinishedGameGql>> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let registry = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let gid = Uuid::parse_str(game_id.as_str()).map_err(|_| Error::new("invalid game id"))?;
        let row = db::get_finished_game(pool, gid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(row.map(|r| map_finished_row(r, registry)))
    }

    async fn recent_finished_games(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<FinishedGameGql>> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let registry = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let lim = limit.unwrap_or(15).clamp(1, 100) as i64;
        let rows = db::list_recent_finished_games(pool, lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|r| map_finished_row(r, registry))
            .collect())
    }

    async fn finished_games_by_lobby(
        &self,
        ctx: &Context<'_>,
        lobby_id: async_graphql::types::ID,
        limit: Option<i32>,
    ) -> Result<Vec<FinishedGameGql>> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let registry = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let lim = limit.unwrap_or(50).clamp(1, 100) as i64;
        let rows = db::list_finished_games_by_lobby(pool, lid, lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|r| map_finished_row(r, registry))
            .collect())
    }

    async fn user(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::types::ID,
    ) -> Result<Option<UserGql>> {
        let pool = ctx.data::<SqlitePool>()?;
        let uid = Uuid::parse_str(id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        let row = db::get_user(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(row.map(|(id, name, created)| UserGql {
            id: id.to_string().into(),
            display_name: name,
            created_at: created,
        }))
    }

    async fn users(&self, ctx: &Context<'_>, limit: Option<i32>) -> Result<Vec<UserGql>> {
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(50).clamp(1, 500) as i64;
        let rows = db::list_users(pool, lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|(id, name, created)| UserGql {
                id: id.to_string().into(),
                display_name: name,
                created_at: created,
            })
            .collect())
    }

    async fn lobbies(&self, ctx: &Context<'_>) -> Result<Vec<LobbySummaryGql>> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let rows = lobby_db::list_active_lobbies(pool)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows.into_iter().map(map_summary).collect())
    }

    async fn lobby(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::types::ID,
    ) -> Result<Option<LobbyGql>> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lid = Uuid::parse_str(id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let row = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        match row {
            None => Ok(None),
            Some(d) => Ok(Some(lobby_to_gql(pool, d).await?)),
        }
    }

    /// True when Google OAuth credentials are configured.
    async fn oauth_available(&self) -> bool {
        crate::google_oauth::is_configured()
    }

    async fn is_developer(&self, ctx: &Context<'_>) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let open_uploads = std::env::var("OPEN_DEVELOPER_UPLOADS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if open_uploads {
            return Ok(true);
        }
        if is_superadmin(pool, uid).await.unwrap_or(false) {
            return Ok(true);
        }
        db::user_has_role(pool, uid, "developer")
            .await
            .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn is_superadmin(&self, ctx: &Context<'_>) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        is_superadmin(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn my_game_drafts(&self, ctx: &Context<'_>) -> Result<Vec<GameDraftGql>> {
        let uid = require_developer_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let rows = db::list_game_drafts_for_owner(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows.into_iter().map(map_draft).collect())
    }

    async fn game_draft(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::types::ID,
    ) -> Result<Option<GameDraftGql>> {
        let uid = require_developer_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let did = Uuid::parse_str(id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let row = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let is_sa = is_superadmin(pool, uid).await.unwrap_or(false);
        Ok(row
            .filter(|d| d.owner_user_id == uid || is_sa)
            .map(map_draft))
    }

    async fn my_publish_tokens(&self, ctx: &Context<'_>) -> Result<Vec<PublishTokenSummaryGql>> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let rows = db::list_publish_tokens_for_user(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|r| PublishTokenSummaryGql {
                id: r.id.to_string().into(),
                label: r.label,
                masked_key: db::mask_publish_token_id(&r.id),
                created_at: r.created_at,
                expires_at: r.expires_at,
            })
            .collect())
    }

    async fn finished_games_by_type(
        &self,
        ctx: &Context<'_>,
        game_type: String,
        limit: Option<i32>,
    ) -> Result<Vec<GameSessionGql>> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(20).clamp(1, 100) as i64;
        let rows = db::list_finished_games_by_type(pool, &game_type, lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .iter()
            .map(platform_stats::map_finished_to_session)
            .map(|s| GameSessionGql {
                game_id: s.game_id,
                game_type: s.game_type,
                finished_at: s.finished_at,
                winner_display_name: s.winner_display_name,
                participant_count: s.participant_count,
                duration_secs: s.duration_secs,
            })
            .collect())
    }

    async fn game_leaderboard(
        &self,
        ctx: &Context<'_>,
        game_type: String,
        limit: Option<i32>,
    ) -> Result<Vec<LeaderboardEntryGql>> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(10).clamp(1, 50) as i64;
        let rows = db::list_finished_games_by_type(pool, &game_type, 200)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let entries = platform_stats::compute_leaderboard(&rows, lim as usize);
        Ok(entries
            .into_iter()
            .enumerate()
            .map(|(i, e)| {
                let win_rate = if e.games_played > 0 {
                    ((e.wins as f64 / e.games_played as f64) * 100.0).round() as u32
                } else {
                    0
                };
                LeaderboardEntryGql {
                    rank: (i + 1) as u32,
                    display_name: e.display_name,
                    total_score: e.total_score,
                    wins: e.wins,
                    win_rate_pct: win_rate,
                }
            })
            .collect())
    }

    async fn published_deployments(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<DeploymentGql>> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let reg = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let lim = limit.unwrap_or(20).clamp(1, 100) as i64;
        let rows = db::list_published_deployments(pool, lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let live_versions: std::collections::HashMap<String, String> = reg
            .read()
            .map_err(|_| Error::new("registry lock poisoned"))?
            .game_types()
            .iter()
            .map(|gt| (gt.manifest.name.clone(), gt.manifest.version.clone()))
            .collect();
        Ok(rows
            .into_iter()
            .filter_map(|d| {
                let deployed_at = d.published_at?;
                let status = if live_versions
                    .get(&d.game_name)
                    .is_some_and(|v| v == &d.version)
                {
                    "Live".into()
                } else {
                    "Archived".into()
                };
                Some(DeploymentGql {
                    id: d.id.to_string().into(),
                    game_name: d.game_name,
                    display_name: d.display_name,
                    version: d.version,
                    status,
                    deployed_at,
                })
            })
            .collect())
    }

    /// Full platform release manifest (versions, SDK matrix, CLI requirements).
    async fn platform_manifest(&self) -> Result<PlatformManifestGql> {
        let m = crate::platform_manifest::load_manifest();
        let sdk_versions_json = serde_json::to_string(&m.sdk_versions)
            .map_err(|e| Error::new(format!("serialize sdk versions: {e}")))?;
        Ok(PlatformManifestGql {
            framework_version: m.framework_version.clone(),
            wit_version: m.wit_version.clone(),
            wasmtime_version: m.wasmtime_version.clone(),
            released_at: m.released_at.clone(),
            cli: super::CliReleaseGql {
                version: m.cli.version.clone(),
                min_supported: m.cli.min_supported.clone(),
                released_at: m.cli.released_at.clone(),
                notes: m.cli.notes.clone(),
            },
            sdk_versions_json,
        })
    }

    /// CLI download manifest (subset of `platformManifest`).
    async fn platform_cli_release(&self) -> Result<CliReleaseGql> {
        let m = crate::platform_manifest::load_manifest();
        Ok(super::CliReleaseGql {
            version: m.cli.version.clone(),
            min_supported: m.cli.min_supported.clone(),
            released_at: m.cli.released_at.clone(),
            notes: m.cli.notes.clone(),
        })
    }

    async fn platform_stats(&self, ctx: &Context<'_>) -> Result<PlatformStatsGql> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let reg = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let game_db = ctx.data::<GameDb>()?;
        let lobbies = lobby_db::list_active_lobbies(pool)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let published_game_types = reg
            .read()
            .map_err(|_| Error::new("registry lock poisoned"))?
            .game_types()
            .len() as i32;
        let since = GameInstanceStore::now_secs() - 86_400;
        let finished_games24h = db::count_finished_games_since(pool, since)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))? as i32;
        let active_sessions = map_game_entries(game_db)
            .iter()
            .map(|g| g.connected_players as i32)
            .sum();
        let snapshot = platform_stats::MetricsSnapshot {
            active_lobbies: lobbies.len() as i32,
            published_game_types,
            finished_games24h,
            active_sessions,
        };
        platform_stats::record_metrics_snapshot(pool, &snapshot)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let trends = platform_stats::build_kpi_trends(pool, &snapshot)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .into_iter()
            .map(|t| super::KpiTrendGql {
                label: t.label,
                value: t.value,
                delta_pct: t.delta_pct,
                up: t.up,
            })
            .collect();
        Ok(PlatformStatsGql {
            active_lobbies: snapshot.active_lobbies,
            published_game_types: snapshot.published_game_types,
            finished_games24h: snapshot.finished_games24h,
            active_sessions: snapshot.active_sessions,
            status: "ok".into(),
            trends,
            pro_tip: platform_stats::pro_tip_text(),
        })
    }

    async fn activity_feed(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<ActivityEventGql>> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(12).clamp(1, 50) as usize;
        let rows = platform_stats::build_activity_feed(pool, lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|e| ActivityEventGql {
                actor: e.actor,
                action: e.action,
                target: e.target,
                timestamp: e.timestamp,
            })
            .collect())
    }

    async fn my_profile(&self, ctx: &Context<'_>) -> Result<Option<UserProfileGql>> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let stats = platform_stats::build_user_profile(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(stats.map(|s| UserProfileGql {
            display_name: s.display_name,
            created_at: s.created_at,
            matches_played: s.matches_played,
            games_published: s.games_published,
            wins: s.wins,
            rep_score: s.rep_score,
            avatar_url: s.avatar_url,
        }))
    }

    async fn game_storefront(
        &self,
        ctx: &Context<'_>,
        game_type: String,
    ) -> Result<GameStorefrontGql> {
        let pool = ctx.data::<SqlitePool>()?;
        let can_edit = if let Ok(uid) = require_registered_user(ctx).await {
            let is_sa = is_superadmin(pool, uid).await.unwrap_or(false);
            if is_sa {
                true
            } else {
                game_storefront::user_can_edit_storefront(pool, uid, &game_type)
                    .await
                    .unwrap_or(false)
            }
        } else {
            false
        };
        let sf = game_storefront::ensure_storefront(pool, &game_type)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let aspects = game_storefront::aggregate_aspect_ratings(pool, &game_type)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let reviews = game_storefront::list_reviews(pool, &game_type, 1)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let review_count = game_storefront::list_reviews(pool, &game_type, 500)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .len() as i32;
        let _ = reviews;
        Ok(GameStorefrontGql {
            game_name: sf.game_name,
            short_tagline: sf.short_tagline,
            long_description: sf.long_description,
            screenshots: sf
                .screenshots
                .into_iter()
                .map(|s| GameScreenshotGql {
                    id: s.id,
                    caption: s.caption,
                    gradient: s.gradient,
                    image_url: s.image_url,
                })
                .collect(),
            patch_notes: sf
                .patch_notes
                .into_iter()
                .map(|p| GamePatchNoteGql {
                    version: p.version,
                    date: p.date,
                    title: p.title,
                    body: p.body,
                    tags: p.tags,
                })
                .collect(),
            tags: sf.tags,
            avg_session_mins: sf.avg_session_mins,
            featured: sf.featured,
            creator_display_name: sf.creator_display_name,
            aspect_ratings: map_aspect_ratings(&aspects),
            review_count,
            can_edit,
            updated_at: sf.updated_at,
        })
    }

    async fn game_reviews(
        &self,
        ctx: &Context<'_>,
        game_type: String,
        limit: Option<i32>,
    ) -> Result<Vec<GameReviewGql>> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(20).clamp(1, 50) as i64;
        let rows = game_storefront::list_reviews(pool, &game_type, lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let mut out = Vec::new();
        for r in rows {
            let voted = game_storefront::user_voted_review(pool, r.id, uid)
                .await
                .unwrap_or(false);
            out.push(GameReviewGql {
                id: r.id.to_string().into(),
                display_name: r.display_name,
                body: r.body,
                aspects: map_aspect_ratings(&r.aspects),
                helpful_votes: r.helpful_votes,
                user_has_voted: voted,
                created_at: r.created_at,
            });
        }
        Ok(out)
    }

    async fn game_comments(
        &self,
        ctx: &Context<'_>,
        game_type: String,
        limit: Option<i32>,
    ) -> Result<Vec<GameCommentGql>> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(30).clamp(1, 100) as i64;
        let rows = game_storefront::list_comments(pool, &game_type, lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|c| GameCommentGql {
                id: c.id.to_string().into(),
                display_name: c.display_name,
                body: c.body,
                created_at: c.created_at,
            })
            .collect())
    }

    async fn game_play_time_leaderboard(
        &self,
        ctx: &Context<'_>,
        game_type: String,
        limit: Option<i32>,
    ) -> Result<Vec<PlayTimeLeaderboardEntryGql>> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(10).clamp(1, 50) as i64;
        let sf = game_storefront::ensure_storefront(pool, &game_type)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let rows = db::list_finished_games_by_type(pool, &game_type, 200)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let entries =
            game_storefront::compute_playtime_leaderboard(&rows, sf.avg_session_mins, lim as usize);
        Ok(entries
            .into_iter()
            .enumerate()
            .map(|(i, e)| PlayTimeLeaderboardEntryGql {
                rank: (i + 1) as u32,
                display_name: e.display_name,
                total_mins: e.total_mins,
                sessions: e.sessions,
            })
            .collect())
    }

    async fn my_badges(&self, ctx: &Context<'_>) -> Result<Vec<BadgeGql>> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let rows = user_engagement::list_badges_for_user(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|b| BadgeGql {
                id: b.id,
                label: b.label,
                tier: b.tier,
                locked: b.locked,
                earned_at: b.earned_at,
            })
            .collect())
    }

    async fn my_notifications(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<NotificationGql>> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(20).clamp(1, 50) as usize;
        let rows = user_engagement::list_notifications(pool, uid, lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|n| NotificationGql {
                id: n.id.to_string().into(),
                title: n.title,
                body: n.body,
                kind: n.kind,
                unread: n.unread,
                created_at: n.created_at,
            })
            .collect())
    }

    async fn unread_notification_count(&self, ctx: &Context<'_>) -> Result<i32> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        user_engagement::unread_count(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn admin_platform_overview(&self, ctx: &Context<'_>) -> Result<AdminPlatformOverviewGql> {
        let _uid = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let o = db::admin_platform_overview(pool)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(AdminPlatformOverviewGql {
            user_count: o.user_count,
            draft_count: o.draft_count,
            active_lobbies: o.active_lobbies,
            published_games: o.published_games,
            review_count: o.review_count,
            comment_count: o.comment_count,
        })
    }

    async fn admin_users(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
        search: Option<String>,
    ) -> Result<Vec<AdminUserGql>> {
        let _uid = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(50).clamp(1, 500) as i64;
        let rows = db::search_users(pool, search.as_deref(), lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|u| AdminUserGql {
                id: u.id.to_string().into(),
                display_name: u.display_name,
                created_at: u.created_at,
                roles: u.roles,
                has_password: u.has_password,
            })
            .collect())
    }

    async fn admin_game_drafts(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
        status: Option<String>,
        #[graphql(name = "ownerUserId")] owner_user_id: Option<async_graphql::types::ID>,
    ) -> Result<Vec<GameDraftGql>> {
        let _uid = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(50).clamp(1, 500) as i64;
        let owner = if let Some(id) = owner_user_id {
            Some(
                Uuid::parse_str(id.as_str())
                    .map_err(|_| Error::new("invalid owner user id"))?,
            )
        } else {
            None
        };
        let rows = db::list_all_game_drafts(pool, status.as_deref(), owner, lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows.into_iter().map(map_draft).collect())
    }

    async fn admin_lobbies(
        &self,
        ctx: &Context<'_>,
        status: Option<String>,
    ) -> Result<Vec<LobbySummaryGql>> {
        let _uid = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let rows = lobby_db::list_lobbies_admin(pool, status.as_deref())
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows.into_iter().map(map_summary).collect())
    }

    async fn admin_reviews(
        &self,
        ctx: &Context<'_>,
        game_type: Option<String>,
        limit: Option<i32>,
    ) -> Result<Vec<AdminReviewGql>> {
        let _uid = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(50).clamp(1, 500) as i64;
        let rows = game_storefront::list_all_reviews(pool, game_type.as_deref(), lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|r| AdminReviewGql {
                id: r.id.to_string().into(),
                game_name: r.game_name,
                display_name: r.display_name,
                body: r.body,
                helpful_votes: r.helpful_votes,
                created_at: r.created_at,
            })
            .collect())
    }

    async fn admin_comments(
        &self,
        ctx: &Context<'_>,
        game_type: Option<String>,
        limit: Option<i32>,
    ) -> Result<Vec<AdminCommentGql>> {
        let _uid = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let lim = limit.unwrap_or(50).clamp(1, 500) as i64;
        let rows = game_storefront::list_all_comments(pool, game_type.as_deref(), lim)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|c| AdminCommentGql {
                id: c.id.to_string().into(),
                game_name: c.game_name,
                display_name: c.display_name,
                body: c.body,
                created_at: c.created_at,
            })
            .collect())
    }
}
