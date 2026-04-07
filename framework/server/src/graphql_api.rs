use std::pin::Pin;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use async_graphql::{Context, Error, Object, Result, Schema, SimpleObject, Subscription};
use base64::Engine;
use futures_util::stream::{self, Stream, StreamExt};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::auth_password;
use crate::component_db::ComponentDb;
use crate::game_upload::{
    publish_staged_game, remove_published_game_dir, validate_and_stage_zip_bytes,
    validate_game_folder_name, write_manifest_to_staged_dir, ValidationReport,
};
use crate::db::{self, FinishedGameRow, GameInstanceStore};
use crate::game_db::GameDb;
use crate::game_registry::GameRegistry;
use crate::game_service;
use crate::lobby_db::{self, LobbyDetail, LobbyListNotify, LobbyMessage, LobbySeat, LobbySummary};

/// Authenticated user from `Authorization: Bearer <uuid>` (guest id from `registerUser`).
#[derive(Clone, Copy, Debug)]
pub struct RequestUser(pub Option<Uuid>);
#[derive(Clone)]
pub struct GamesDir(pub PathBuf);
#[derive(Clone)]
pub struct DraftsDir(pub PathBuf);

fn require_user(ctx: &Context<'_>) -> Result<Uuid> {
    let RequestUser(u) = ctx.data::<RequestUser>()?;
    u.ok_or_else(|| Error::new("login required: send Authorization: Bearer <userId>"))
}

/// Bearer user must exist in `users` (avoids SQLite FK 787 when localStorage id is stale after DB reset).
async fn require_registered_user(ctx: &Context<'_>) -> Result<Uuid> {
    let uid = require_user(ctx)?;
    let pool = ctx.data::<SqlitePool>()?;
    if db::get_user(pool, uid)
        .await
        .map_err(|e| Error::new(format!("db: {e}")))?
        .is_none()
    {
        return Err(Error::new(
            "user id not in database (localStorage may be stale after a DB reset); clear site data or register again",
        ));
    }
    Ok(uid)
}

async fn require_developer_user(ctx: &Context<'_>) -> Result<Uuid> {
    let uid = require_registered_user(ctx).await?;
    let pool = ctx.data::<SqlitePool>()?;
    let open_uploads = std::env::var("OPEN_DEVELOPER_UPLOADS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(true);
    if open_uploads || db::user_has_role(pool, uid, "developer").await.unwrap_or(false) {
        return Ok(uid);
    }
    Err(Error::new("developer permission required"))
}

#[derive(SimpleObject, Clone)]
pub struct UserGql {
    pub id: async_graphql::types::ID,
    pub display_name: String,
    pub created_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct GameTypeGql {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub min_players: u32,
    pub max_players: u32,
    pub description: String,
    pub config_ui_path: Option<String>,
    pub result_ui_path: Option<String>,
    pub about_ui_path: Option<String>,
    pub config_schema_json: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct GameInstanceGql {
    pub game_id: String,
    pub game_type: String,
    pub player_identities: Vec<String>,
    pub connected_players: usize,
}

#[derive(SimpleObject, Clone)]
pub struct ValidationDiagnosticGql {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub hint: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct ValidationReportGql {
    pub ok: bool,
    pub errors: i32,
    pub warnings: i32,
    pub infos: i32,
    pub required_index_html: bool,
    pub required_config_html: bool,
    pub required_result_html: bool,
    pub required_about_html: bool,
    pub diagnostics: Vec<ValidationDiagnosticGql>,
}

#[derive(SimpleObject, Clone)]
pub struct GameDraftGql {
    pub id: async_graphql::types::ID,
    pub upload_id: async_graphql::types::ID,
    pub owner_user_id: async_graphql::types::ID,
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

#[derive(SimpleObject, Clone)]
pub struct UploadGameZipResultGql {
    pub upload_id: async_graphql::types::ID,
    pub draft: Option<GameDraftGql>,
    pub report: ValidationReportGql,
}

#[derive(SimpleObject, Clone)]
pub struct FinishedGameGql {
    pub game_id: String,
    pub game_type: String,
    pub lobby_id: Option<String>,
    pub finished_at: i64,
    pub result_json: String,
    pub player_scores_json: String,
    pub seats_snapshot_json: String,
    pub result_ui_path: Option<String>,
}

fn map_finished_row(r: FinishedGameRow, registry: &Arc<RwLock<GameRegistry>>) -> FinishedGameGql {
    let result_ui_path = registry
        .read()
        .ok()
        .map(|reg| {
            reg.game_types()
                .iter()
                .find(|gt| gt.manifest.name == r.game_type)
                .and_then(|gt| gt.result_ui_path.clone())
        })
        .flatten();
    FinishedGameGql {
        game_id: r.id.to_string(),
        game_type: r.game_type,
        lobby_id: r.lobby_id.map(|u| u.to_string()),
        finished_at: r.finished_at,
        result_json: r.result_json,
        player_scores_json: r.player_scores_json,
        seats_snapshot_json: r.seats_snapshot_json,
        result_ui_path,
    }
}

#[derive(SimpleObject, Clone)]
pub struct LobbySeatGql {
    pub seat_index: i32,
    pub player_identity: String,
    pub claimed_by_user_id: Option<async_graphql::types::ID>,
    pub claimed_display_name: Option<String>,
    pub ready: bool,
}

#[derive(SimpleObject, Clone)]
pub struct LobbyMessageGql {
    pub id: async_graphql::types::ID,
    pub user_id: async_graphql::types::ID,
    pub display_name: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct LobbySummaryGql {
    pub id: async_graphql::types::ID,
    pub game_type: String,
    pub status: String,
    pub seats_filled: i32,
    pub seats_total: i32,
    pub owner_display_name: String,
    pub game_instance_id: Option<String>,
    pub created_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct LobbyGql {
    pub id: async_graphql::types::ID,
    pub owner_user_id: async_graphql::types::ID,
    pub owner_display_name: String,
    pub game_type: String,
    pub config_json: Option<String>,
    pub status: String,
    pub game_instance_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub seats: Vec<LobbySeatGql>,
    pub messages: Vec<LobbyMessageGql>,
}

fn map_seat(s: LobbySeat) -> LobbySeatGql {
    LobbySeatGql {
        seat_index: s.seat_index,
        player_identity: s.player_identity,
        claimed_by_user_id: s.claimed_by_user_id.map(|u| u.to_string().into()),
        claimed_display_name: s.claimed_display_name,
        ready: s.ready,
    }
}

fn map_message(m: LobbyMessage) -> LobbyMessageGql {
    LobbyMessageGql {
        id: m.id.to_string().into(),
        user_id: m.user_id.to_string().into(),
        display_name: m.display_name,
        body: m.body,
        created_at: m.created_at,
    }
}

fn map_summary(s: LobbySummary) -> LobbySummaryGql {
    LobbySummaryGql {
        id: s.id.to_string().into(),
        game_type: s.game_type,
        status: s.status,
        seats_filled: s.seats_claimed as i32,
        seats_total: s.seats_total as i32,
        owner_display_name: s.owner_display_name,
        game_instance_id: s.game_instance_id,
        created_at: s.created_at,
    }
}

fn map_validation_report(report: ValidationReport) -> ValidationReportGql {
    ValidationReportGql {
        ok: report.ok,
        errors: report.errors as i32,
        warnings: report.warnings as i32,
        infos: report.infos as i32,
        required_index_html: report.required_index_html,
        required_config_html: report.required_config_html,
        required_result_html: report.required_result_html,
        required_about_html: report.required_about_html,
        diagnostics: report
            .diagnostics
            .into_iter()
            .map(|d| ValidationDiagnosticGql {
                severity: d.severity,
                code: d.code,
                message: d.message,
                path: d.path,
                hint: d.hint,
            })
            .collect(),
    }
}

fn map_draft(d: db::GameDraftRow) -> GameDraftGql {
    GameDraftGql {
        id: d.id.to_string().into(),
        upload_id: d.upload_id.to_string().into(),
        owner_user_id: d.owner_user_id.to_string().into(),
        game_name: d.game_name,
        display_name: d.display_name,
        version: d.version,
        status: d.status,
        manifest_json: d.manifest_json,
        report_json: d.report_json,
        storage_path: d.storage_path,
        created_at: d.created_at,
        updated_at: d.updated_at,
        published_at: d.published_at,
    }
}

async fn lobby_to_gql(pool: &SqlitePool, d: LobbyDetail) -> Result<LobbyGql> {
    let msgs = lobby_db::list_lobby_messages(pool, d.id, 100)
        .await
        .map_err(|e| Error::new(format!("db: {e}")))?;
    let seats = d.seats.into_iter().map(map_seat).collect();
    let messages = msgs.into_iter().map(map_message).collect();
    Ok(LobbyGql {
        id: d.id.to_string().into(),
        owner_user_id: d.owner_user_id.to_string().into(),
        owner_display_name: d.owner_display_name,
        game_type: d.game_type,
        config_json: d.config,
        status: d.status,
        game_instance_id: d.game_instance_id,
        created_at: d.created_at,
        updated_at: d.updated_at,
        seats,
        messages,
    })
}

fn map_game_entries(db: &GameDb) -> Vec<GameInstanceGql> {
    db.list_games()
        .into_iter()
        .map(|e| GameInstanceGql {
            game_id: e.game_id,
            game_type: e.game_type,
            player_identities: e.player_identities,
            connected_players: e.connected_players,
        })
        .collect()
}

/// Root query.
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn game_types(&self, ctx: &Context<'_>) -> Result<Vec<GameTypeGql>> {
        let reg = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let guard = reg
            .read()
            .map_err(|_| Error::new("registry lock poisoned"))?;
        Ok(guard
            .game_types()
            .iter()
            .map(|gt| GameTypeGql {
                name: gt.manifest.name.clone(),
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
            })
            .collect())
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

    async fn user(&self, ctx: &Context<'_>, id: async_graphql::types::ID) -> Result<Option<UserGql>> {
        let pool = ctx.data::<SqlitePool>()?;
        let uid = Uuid::parse_str(id.as_str())
            .map_err(|_| Error::new("invalid user id"))?;
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

    async fn lobby(&self, ctx: &Context<'_>, id: async_graphql::types::ID) -> Result<Option<LobbyGql>> {
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

    /// Always false until an OAuth provider is integrated.
    async fn oauth_available(&self) -> bool {
        false
    }

    async fn is_developer(&self, ctx: &Context<'_>) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let open_uploads = std::env::var("OPEN_DEVELOPER_UPLOADS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true);
        if open_uploads {
            return Ok(true);
        }
        db::user_has_role(pool, uid, "developer")
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
        Ok(row
            .filter(|d| d.owner_user_id == uid)
            .map(map_draft))
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn grant_myself_developer(&self, ctx: &Context<'_>) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        db::grant_role(pool, uid, "developer")
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(true)
    }

    async fn register_user(
        &self,
        ctx: &Context<'_>,
        display_name: String,
    ) -> Result<UserGql> {
        let pool = ctx.data::<SqlitePool>()?;
        let (id, name, created) = db::register_user(pool, &display_name)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(UserGql {
            id: id.to_string().into(),
            display_name: name,
            created_at: created,
        })
    }

    async fn sign_up(
        &self,
        ctx: &Context<'_>,
        display_name: String,
        password: String,
    ) -> Result<UserGql> {
        let pool = ctx.data::<SqlitePool>()?;
        let _ = ctx;
        if display_name.trim().is_empty() {
            return Err(Error::new("display name required"));
        }
        if password.len() < 4 {
            return Err(Error::new("password too short"));
        }
        let hash = auth_password::hash_password(&password).map_err(Error::new)?;
        let (id, name, created) = db::sign_up(pool, display_name.trim(), &hash)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(UserGql {
            id: id.to_string().into(),
            display_name: name,
            created_at: created,
        })
    }

    /// Set or replace password for the current Bearer user (Argon2 hash in DB).
    async fn set_password(&self, ctx: &Context<'_>, password: String) -> Result<bool> {
        let pool = ctx.data::<SqlitePool>()?;
        let uid = require_registered_user(ctx).await?;
        if password.len() < 4 {
            return Err(Error::new("password too short"));
        }
        let hash = auth_password::hash_password(&password).map_err(Error::new)?;
        db::set_password_hash(pool, uid, &hash)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(true)
    }

    /// True if the Bearer user has this password on file.
    async fn verify_password(&self, ctx: &Context<'_>, password: String) -> Result<bool> {
        let pool = ctx.data::<SqlitePool>()?;
        let uid = require_registered_user(ctx).await?;
        let hash = db::get_password_hash(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(hash
            .map(|h| auth_password::verify_password(&password, &h))
            .unwrap_or(false))
    }

    /// Log in by display name and password; returns the matching user (first row with password).
    async fn login_with_password(
        &self,
        ctx: &Context<'_>,
        display_name: String,
        password: String,
    ) -> Result<UserGql> {
        let pool = ctx.data::<SqlitePool>()?;
        let candidates = lobby_db::find_user_by_display_name_and_password(pool, &display_name)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        for (id, hash_opt) in candidates {
            let Some(hash) = hash_opt else {
                continue;
            };
            if auth_password::verify_password(&password, &hash) {
                let row = db::get_user(pool, id)
                    .await
                    .map_err(|e| Error::new(format!("db: {e}")))?
                    .ok_or_else(|| Error::new("user vanished"))?;
                return Ok(UserGql {
                    id: row.0.to_string().into(),
                    display_name: row.1,
                    created_at: row.2,
                });
            }
        }
        Err(Error::new("invalid credentials"))
    }

    async fn upload_game_zip(
        &self,
        ctx: &Context<'_>,
        filename: String,
        zip_base64: String,
    ) -> Result<UploadGameZipResultGql> {
        let uid = require_developer_user(ctx).await?;
        println!("upload_game_zip: user={} filename={}", uid, filename);
        let pool = ctx.data::<SqlitePool>()?;
        let component_db = ctx.data::<ComponentDb>()?;
        let drafts_dir = &ctx.data::<DraftsDir>()?.0;
        let games_dir = &ctx.data::<GamesDir>()?.0;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(zip_base64.as_bytes())
            .map_err(|e| Error::new(format!("invalid base64 payload: {e}")))?;

        let validation = validate_and_stage_zip_bytes(
            &bytes,
            component_db,
            drafts_dir,
            Some(games_dir.as_path()),
        )
        .await;
        match validation {
            Ok(ok) => {
                println!(
                    "upload_game_zip: validation ok game={} version={}",
                    ok.manifest.name, ok.manifest.version
                );
                let report_json = serde_json::to_string(&ok.report)
                    .map_err(|e| Error::new(format!("serialize report: {e}")))?;
                let upload_id = db::insert_upload(pool, uid, &filename, "validated", &report_json)
                    .await
                    .map_err(|e| Error::new(format!("db: {e}")))?;
                let manifest_json = serde_json::to_string(&ok.manifest)
                    .map_err(|e| Error::new(format!("serialize manifest: {e}")))?;
                let taken = db::count_game_drafts_name_version_active(
                    pool,
                    &ok.manifest.name,
                    &ok.manifest.version,
                    None,
                )
                .await
                .map_err(|e| Error::new(format!("db: {e}")))?;
                if taken > 0 {
                    return Err(Error::new(format!(
                        "Game name {:?} with version {:?} is already used by another draft or published record. Change the manifest (or edit the draft after upload) and try again.",
                        ok.manifest.name, ok.manifest.version
                    )));
                }
                let draft_id = db::insert_game_draft(
                    pool,
                    db::NewDraft {
                        upload_id,
                        owner_user_id: uid,
                        game_name: &ok.manifest.name,
                        display_name: &ok.manifest.display_name,
                        version: &ok.manifest.version,
                        status: "ready",
                        manifest_json: &manifest_json,
                        report_json: &report_json,
                        storage_path: &ok.staged_dir.to_string_lossy(),
                    },
                )
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("UNIQUE") {
                        Error::new(
                            "A draft or published record already uses this game name and version.",
                        )
                    } else {
                        Error::new(format!("db: {msg}"))
                    }
                })?;
                let draft = db::get_game_draft(pool, draft_id)
                    .await
                    .map_err(|e| Error::new(format!("db: {e}")))?
                    .map(map_draft);
                Ok(UploadGameZipResultGql {
                    upload_id: upload_id.to_string().into(),
                    draft,
                    report: map_validation_report(ok.report),
                })
            }
            Err(report_err) => {
                println!("upload_game_zip: validation failed for user={}", uid);
                let report: ValidationReport = serde_json::from_str(&report_err).unwrap_or(ValidationReport {
                    ok: false,
                    errors: 1,
                    warnings: 0,
                    infos: 0,
                    required_index_html: false,
                    required_config_html: false,
                    required_result_html: false,
                    required_about_html: false,
                    diagnostics: vec![crate::game_upload::ValidationDiagnostic {
                        severity: "error".to_string(),
                        code: "E_UPLOAD_VALIDATION_FAILED".to_string(),
                        message: report_err.clone(),
                        path: None,
                        hint: None,
                    }],
                });
                let report_json = serde_json::to_string(&report)
                    .unwrap_or_else(|_| "{\"ok\":false}".to_string());
                let upload_id = db::insert_upload(pool, uid, &filename, "rejected", &report_json)
                    .await
                    .map_err(|e| Error::new(format!("db: {e}")))?;
                Ok(UploadGameZipResultGql {
                    upload_id: upload_id.to_string().into(),
                    draft: None,
                    report: map_validation_report(report),
                })
            }
        }
    }

    async fn publish_game_draft(
        &self,
        ctx: &Context<'_>,
        draft_id: async_graphql::types::ID,
    ) -> Result<GameDraftGql> {
        let uid = require_developer_user(ctx).await?;
        println!("publish_game_draft: user={} draft={}", uid, draft_id.as_str());
        let pool = ctx.data::<SqlitePool>()?;
        let games_dir = &ctx.data::<GamesDir>()?.0;
        let registry = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let component_db = ctx.data::<ComponentDb>()?;
        let did = Uuid::parse_str(draft_id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let draft = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found"))?;
        if draft.owner_user_id != uid {
            return Err(Error::new("not your draft"));
        }
        if draft.status != "ready" {
            return Err(Error::new("draft is not publishable"));
        }
        validate_game_folder_name(&draft.game_name).map_err(|m| Error::new(m.to_string()))?;
        let staged = PathBuf::from(&draft.storage_path);
        publish_staged_game(&staged, games_dir, &draft.game_name).map_err(Error::new)?;
        db::mark_draft_published(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        {
            let mut reg = registry
                .write()
                .map_err(|_| Error::new("registry lock poisoned"))?;
            reg.reload(games_dir, component_db);
        }
        let out = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found after publish"))?;
        Ok(map_draft(out))
    }

    /// Take down the live game for this draft's folder name when this draft is the latest published
    /// build for that name; otherwise only this row is demoted to `ready` (a newer published version
    /// still owns the live folder).
    async fn unpublish_game_draft(
        &self,
        ctx: &Context<'_>,
        draft_id: async_graphql::types::ID,
    ) -> Result<GameDraftGql> {
        let uid = require_developer_user(ctx).await?;
        println!("unpublish_game_draft: user={} draft={}", uid, draft_id.as_str());
        let pool = ctx.data::<SqlitePool>()?;
        let games_dir = &ctx.data::<GamesDir>()?.0;
        let registry = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let component_db = ctx.data::<ComponentDb>()?;
        let did = Uuid::parse_str(draft_id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let draft = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found"))?;
        if draft.owner_user_id != uid {
            return Err(Error::new("not your draft"));
        }
        if draft.status != "published" {
            return Err(Error::new("draft is not published"));
        }
        validate_game_folder_name(&draft.game_name).map_err(|m| Error::new(m.to_string()))?;

        let max_pa = db::max_published_at_for_game_name(pool, &draft.game_name)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let my_pa = draft.published_at;
        let this_is_latest_published = matches!((my_pa, max_pa), (Some(t), Some(m)) if t == m);

        if this_is_latest_published {
            remove_published_game_dir(games_dir, &draft.game_name).map_err(Error::new)?;
            db::demote_all_published_for_game_name(pool, &draft.game_name)
                .await
                .map_err(|e| Error::new(format!("db: {e}")))?;
        } else {
            db::demote_single_published_draft(pool, did)
                .await
                .map_err(|e| Error::new(format!("db: {e}")))?;
        }

        {
            let mut reg = registry
                .write()
                .map_err(|_| Error::new("registry lock poisoned"))?;
            reg.reload(games_dir, component_db);
        }

        let out = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found after unpublish"))?;
        Ok(map_draft(out))
    }

    /// Update manifest identity fields on a **ready** draft (writes `manifest.json` in draft storage).
    async fn update_game_draft_manifest(
        &self,
        ctx: &Context<'_>,
        draft_id: async_graphql::types::ID,
        name: String,
        display_name: String,
        version: String,
        description: String,
    ) -> Result<GameDraftGql> {
        let uid = require_developer_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let did = Uuid::parse_str(draft_id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let draft = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found"))?;
        if draft.owner_user_id != uid {
            return Err(Error::new("not your draft"));
        }
        if draft.status != "ready" {
            return Err(Error::new("only ready drafts can be edited"));
        }
        validate_game_folder_name(&name).map_err(|m| Error::new(m.to_string()))?;
        let dn = display_name.trim();
        let ver = version.trim();
        if dn.is_empty() {
            return Err(Error::new("display_name must not be empty"));
        }
        if ver.is_empty() {
            return Err(Error::new("version must not be empty"));
        }
        let mut manifest: crate::game_registry::GameManifest =
            serde_json::from_str(&draft.manifest_json)
                .map_err(|e| Error::new(format!("stored manifest is invalid: {e}")))?;
        manifest.name = name.trim().to_string();
        manifest.display_name = dn.to_string();
        manifest.version = ver.to_string();
        manifest.description = description;
        let manifest_json = serde_json::to_string(&manifest)
            .map_err(|e| Error::new(format!("serialize manifest: {e}")))?;
        let clash = db::count_game_drafts_name_version_active(pool, &manifest.name, &manifest.version, Some(did))
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        if clash > 0 {
            return Err(Error::new(
                "Another draft or published record already uses this name and version. Pick a different combination.",
            ));
        }
        let staged = PathBuf::from(&draft.storage_path);
        if !staged.is_dir() {
            return Err(Error::new("draft storage is missing on disk"));
        }
        write_manifest_to_staged_dir(&staged, &manifest).map_err(Error::new)?;
        let updated = db::update_game_draft_manifest_columns(
            pool,
            did,
            uid,
            &manifest.name,
            &manifest.display_name,
            &manifest.version,
            &manifest_json,
        )
        .await
        .map_err(|e| Error::new(format!("db: {e}")))?;
        if !updated {
            return Err(Error::new("failed to update draft"));
        }
        let out = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found after update"))?;
        Ok(map_draft(out))
    }

    async fn discard_game_draft(
        &self,
        ctx: &Context<'_>,
        draft_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let uid = require_developer_user(ctx).await?;
        println!("discard_game_draft: user={} draft={}", uid, draft_id.as_str());
        let pool = ctx.data::<SqlitePool>()?;
        let did = Uuid::parse_str(draft_id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let draft = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found"))?;
        if draft.owner_user_id != uid {
            return Err(Error::new("not your draft"));
        }
        if draft.status == "published" {
            return Err(Error::new(
                "cannot discard a published draft; take it down from the lobby first",
            ));
        }
        db::mark_draft_discarded(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let p = PathBuf::from(draft.storage_path);
        if p.exists() {
            let _ = std::fs::remove_dir_all(&p);
        }
        Ok(true)
    }

    async fn create_game(
        &self,
        ctx: &Context<'_>,
        game_type: String,
        config_json: String,
    ) -> Result<async_graphql::types::ID> {
        let component_db = ctx.data::<ComponentDb>()?;
        let game_db = ctx.data::<GameDb>()?;
        let game_store = ctx.data::<Arc<GameInstanceStore>>()?;
        let pool = ctx.data::<SqlitePool>()?.clone();
        let notify = ctx.data::<LobbyListNotify>()?.clone();
        let config = config_json.into_bytes();
        let id = game_service::create_and_spawn_game(
            component_db,
            game_db,
            game_store.clone(),
            game_type,
            config,
            None,
            pool,
            notify,
        )
        .await
        .map_err(Error::new)?;
        Ok(id.to_string().into())
    }

    /// New lobby with no game yet (`game_type` empty until owner calls `setLobbyGameType`).
    /// Optional `game_type` is legacy; omit or pass empty and choose the game inside the lobby.
    async fn create_lobby(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "gameType")] game_type: Option<String>,
    ) -> Result<LobbyGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lobby_id = Uuid::new_v4();
        let gt = game_type.unwrap_or_default();
        lobby_db::insert_lobby_skeleton(pool, lobby_id, uid, &gt)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        notify.ping();
        let detail = lobby_db::get_lobby(pool, lobby_id)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby missing after insert"))?;
        lobby_to_gql(pool, detail).await
    }

    async fn set_lobby_game_type(
        &self,
        ctx: &Context<'_>,
        lobby_id: async_graphql::types::ID,
        game_type: String,
        force: bool,
    ) -> Result<LobbyGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let component_db = ctx.data::<ComponentDb>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        let config_bytes = detail
            .config
            .as_deref()
            .unwrap_or("null")
            .as_bytes()
            .to_vec();
        let identities =
            game_service::preview_init_identities(component_db, game_type.clone(), config_bytes)
                .await
                .map_err(Error::new)?;
        lobby_db::owner_replace_game_type_and_seats(pool, lid, uid, &game_type, &identities, force)
            .await
            .map_err(Error::new)?;
        notify.ping();
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        lobby_to_gql(pool, detail).await
    }

    async fn update_lobby_config(
        &self,
        ctx: &Context<'_>,
        lobby_id: async_graphql::types::ID,
        config_json: String,
        force: bool,
    ) -> Result<LobbyGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let component_db = ctx.data::<ComponentDb>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        let gt = detail.game_type.clone();
        let config = config_json.into_bytes();
        let identities =
            game_service::preview_init_identities(component_db, gt.clone(), config.clone())
                .await
                .map_err(Error::new)?;
        let config_s = String::from_utf8_lossy(&config).to_string();
        lobby_db::owner_replace_config_and_seats(pool, lid, uid, &gt, &config_s, &identities, force)
            .await
            .map_err(Error::new)?;
        notify.ping();
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        lobby_to_gql(pool, detail).await
    }

    async fn join_lobby(
        &self,
        ctx: &Context<'_>,
        lobby_id: async_graphql::types::ID,
        seat_index: i32,
    ) -> Result<LobbyGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        match lobby_db::claim_seat(pool, lid, seat_index, uid).await {
            Ok(true) => {}
            Ok(false) => {
                return Err(Error::new(
                    "cannot claim seat (taken, invalid index, or you already have another seat in this lobby)",
                ));
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("UNIQUE") || msg.contains("unique") {
                    return Err(Error::new(
                        "you already occupy a seat in this lobby",
                    ));
                }
                return Err(Error::new(format!("db: {msg}")));
            }
        }
        notify.ping();
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        lobby_to_gql(pool, detail).await
    }

    async fn set_lobby_seat_ready(
        &self,
        ctx: &Context<'_>,
        lobby_id: async_graphql::types::ID,
        ready: bool,
    ) -> Result<LobbyGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        if detail.status != "waiting" && detail.status != "configuring" {
            return Err(Error::new("cannot change ready status in this lobby state"));
        }
        let ok = lobby_db::set_seat_ready(pool, lid, uid, ready)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        if !ok {
            return Err(Error::new("you must take a seat before setting ready"));
        }
        notify.ping();
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        lobby_to_gql(pool, detail).await
    }

    async fn leave_lobby(&self, ctx: &Context<'_>, lobby_id: async_graphql::types::ID) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        lobby_db::release_user_seats(pool, lid, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        notify.ping();
        Ok(true)
    }

    async fn start_lobby(
        &self,
        ctx: &Context<'_>,
        lobby_id: async_graphql::types::ID,
    ) -> Result<async_graphql::types::ID> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let component_db = ctx.data::<ComponentDb>()?;
        let game_db = ctx.data::<GameDb>()?;
        let game_store = ctx.data::<Arc<GameInstanceStore>>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        if detail.owner_user_id != uid {
            return Err(Error::new("only the owner can start"));
        }
        if detail.status != "waiting" && detail.status != "configuring" {
            return Err(Error::new("lobby cannot be started in this state"));
        }
        if detail.game_type.trim().is_empty() {
            return Err(Error::new("choose a game type in the lobby before starting"));
        }
        let total = detail.seats.len();
        let claimed = detail
            .seats
            .iter()
            .filter(|s| s.claimed_by_user_id.is_some())
            .count();
        if total == 0 {
            return Err(Error::new("no seats — set game type and config first"));
        }
        if claimed != total {
            return Err(Error::new(format!(
                "all seats must be claimed ({claimed}/{total})"
            )));
        }
        if detail
            .seats
            .iter()
            .any(|s| s.claimed_by_user_id.is_some() && !s.ready)
        {
            return Err(Error::new(
                "every seated player must be ready before starting",
            ));
        }
        let config = detail
            .config
            .as_deref()
            .unwrap_or("null")
            .as_bytes()
            .to_vec();
        let gid = game_service::create_and_spawn_game(
            component_db,
            game_db,
            game_store.clone(),
            detail.game_type,
            config,
            Some(lid),
            pool.clone(),
            notify.clone(),
        )
        .await
        .map_err(Error::new)?;
        lobby_db::mark_lobby_in_game(pool, lid, gid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        notify.ping();
        Ok(gid.to_string().into())
    }

    async fn cancel_lobby(&self, ctx: &Context<'_>, lobby_id: async_graphql::types::ID) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        if detail.owner_user_id != uid {
            return Err(Error::new("only the owner can cancel"));
        }
        if detail.status == "in_game" {
            return Err(Error::new("cannot cancel while in game"));
        }
        if detail.status == "cancelled" {
            return Err(Error::new("lobby already cancelled"));
        }
        lobby_db::cancel_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        notify.ping();
        Ok(true)
    }

    async fn reopen_lobby_after_game(
        &self,
        ctx: &Context<'_>,
        lobby_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        if detail.owner_user_id != uid {
            return Err(Error::new("only the owner can reopen the lobby"));
        }
        let ok = lobby_db::reopen_lobby_after_game(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        if ok {
            notify.ping();
        }
        Ok(ok)
    }

    async fn post_lobby_message(
        &self,
        ctx: &Context<'_>,
        lobby_id: async_graphql::types::ID,
        body: String,
    ) -> Result<LobbyMessageGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        if detail.status == "cancelled" {
            return Err(Error::new("cannot chat in a cancelled lobby"));
        }
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return Err(Error::new("empty message"));
        }
        if trimmed.len() > 2000 {
            return Err(Error::new("message too long"));
        }
        let m = lobby_db::insert_lobby_message(pool, lid, uid, trimmed)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        notify.ping();
        Ok(map_message(m))
    }
}

pub struct SubscriptionRoot;

type GameListStream = Pin<Box<dyn Stream<Item = Vec<GameInstanceGql>> + Send>>;
type LobbyListStream = Pin<Box<dyn Stream<Item = Vec<LobbySummaryGql>> + Send>>;
type LobbyRoomStream = Pin<Box<dyn Stream<Item = LobbyGql> + Send>>;

#[Subscription]
impl SubscriptionRoot {
    async fn game_instances_updated(&self, ctx: &Context<'_>) -> Result<GameListStream> {
        let db = ctx.data::<GameDb>()?.clone();
        let rx = db.subscribe_game_list().ok_or_else(|| {
            Error::new("game list subscriptions are not configured")
        })?;
        let first = map_game_entries(&db);
        let tail = stream::unfold((rx, db), |(mut rx, db)| async move {
            match rx.recv().await {
                Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    Some((map_game_entries(&db), (rx, db)))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
            }
        });
        Ok(Box::pin(stream::once(async move { first }).chain(tail)))
    }

    async fn lobbies_updated(&self, ctx: &Context<'_>) -> Result<LobbyListStream> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?.clone();
        let notify = ctx.data::<LobbyListNotify>()?.clone();
        let rx = notify.subscribe();
        let rows = lobby_db::list_active_lobbies(&pool)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let first: Vec<LobbySummaryGql> = rows.into_iter().map(map_summary).collect();
        let tail = stream::unfold((rx, pool), |(mut rx, pool)| async move {
            match rx.recv().await {
                Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let vec = lobby_db::list_active_lobbies(&pool)
                        .await
                        .ok()
                        .map(|rows| rows.into_iter().map(map_summary).collect())
                        .unwrap_or_default();
                    Some((vec, (rx, pool)))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
            }
        });
        Ok(Box::pin(stream::once(async move { first }).chain(tail)))
    }

    async fn lobby_updated(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::types::ID,
    ) -> Result<LobbyRoomStream> {
        let _uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?.clone();
        let notify = ctx.data::<LobbyListNotify>()?.clone();
        let lid = Uuid::parse_str(id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let rx = notify.subscribe();
        let first = {
            let row = lobby_db::get_lobby(&pool, lid)
                .await
                .map_err(|e| Error::new(format!("db: {e}")))?;
            let Some(d) = row else {
                return Err(Error::new("lobby not found"));
            };
            lobby_to_gql(&pool, d).await?
        };
        let tail = stream::unfold((rx, pool, lid), |(mut rx, pool, lid)| async move {
            match rx.recv().await {
                Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let item = match lobby_db::get_lobby(&pool, lid).await {
                        Ok(Some(d)) => lobby_to_gql(&pool, d).await.ok(),
                        _ => None,
                    };
                    item.map(|g| (g, (rx, pool, lid)))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
            }
        });
        Ok(Box::pin(stream::once(async move { first }).chain(tail)))
    }
}

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn build_schema() -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot).finish()
}
