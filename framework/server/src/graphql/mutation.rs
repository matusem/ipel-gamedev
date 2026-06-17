use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use async_graphql::{Context, Error, Object, Result};
use base64::Engine;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::auth_password;
use crate::auth_sessions;
use crate::component_db::ComponentDb;
use crate::db::{self, GameInstanceStore};
use crate::game_db::GameDb;
use crate::game_registry::GameRegistry;
use crate::game_service;
use crate::game_storefront::{self, AspectRatings};
use crate::game_upload::{
    ValidationReport, publish_staged_game, remove_published_game_dir, validate_and_stage_zip_bytes,
    validate_game_folder_name, write_manifest_to_staged_dir,
};
use crate::friends::{self, FriendsListNotify};
use crate::lobby_db::{self, LobbyListNotify};
use crate::user_engagement;

use super::{
    AuthSessionGql, DraftsDir, GameCommentGql, GameDraftGql, GameReviewGql, GamesDir, LobbyGql,
    LobbyMessageGql, PublishTokenGql, RequestUser, UploadGameZipResultGql, UserGql, lobby_to_gql,
    map_aspect_ratings, map_draft, map_message, map_validation_report, require_developer_user,
    require_registered_user, require_superadmin_user, is_superadmin,
};

fn draft_slug(draft: &db::GameDraftRow) -> Result<String, Error> {
    if draft.slug.trim().is_empty() {
        return Err(Error::new(
            "draft has no slug; re-upload the game zip to assign a catalog slug",
        ));
    }
    Ok(draft.slug.clone())
}

async fn issue_auth_session(pool: &SqlitePool, user: UserGql) -> Result<AuthSessionGql> {
    let uid = Uuid::parse_str(user.id.as_str()).map_err(|_| Error::new("invalid user id"))?;
    let (token, expires_at) = auth_sessions::create_session(pool, uid)
        .await
        .map_err(|e| Error::new(format!("db: {e}")))?;
    Ok(AuthSessionGql {
        session_token: token,
        expires_at,
        user,
    })
}
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn grant_myself_developer(&self, ctx: &Context<'_>) -> Result<bool> {
        let allowed = std::env::var("ADMIN_GRANT_DEVELOPER")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if !allowed {
            return Err(Error::new("developer self-grant is disabled"));
        }
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        db::grant_role(pool, uid, "developer")
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let _ = user_engagement::insert_notification(
            pool,
            uid,
            "Developer access",
            "Upload console is available for your account.",
            "system",
        )
        .await;
        Ok(true)
    }

    async fn register_user(
        &self,
        ctx: &Context<'_>,
        display_name: String,
    ) -> Result<AuthSessionGql> {
        let pool = ctx.data::<SqlitePool>()?;
        let (id, name, created) = db::register_user(pool, &display_name)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let _ = user_engagement::welcome_notification(pool, id).await;
        issue_auth_session(
            pool,
            UserGql {
                id: id.to_string().into(),
                display_name: name,
                created_at: created,
            },
        )
        .await
    }

    async fn create_publish_token(
        &self,
        ctx: &Context<'_>,
        ttl_days: Option<i32>,
        label: Option<String>,
    ) -> Result<PublishTokenGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let (_id, token, expires_at) =
            db::create_publish_token(pool, uid, ttl_days.unwrap_or(7) as i64, label.as_deref())
                .await
                .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(PublishTokenGql {
            token,
            user_id: uid.to_string().into(),
            expires_at,
        })
    }

    async fn revoke_publish_token(
        &self,
        ctx: &Context<'_>,
        token_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let tid = Uuid::parse_str(token_id.as_str()).map_err(|_| Error::new("invalid token id"))?;
        db::revoke_publish_token(pool, uid, tid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn update_game_storefront(
        &self,
        ctx: &Context<'_>,
        game_type: String,
        short_tagline: Option<String>,
        long_description: String,
        screenshots_json: String,
        patch_notes_json: String,
        tags_json: String,
        avg_session_mins: Option<i32>,
    ) -> Result<bool> {
        let uid = require_developer_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        if !game_storefront::user_can_edit_storefront(pool, uid, &game_type)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            && !is_superadmin(pool, uid).await.unwrap_or(false)
        {
            return Err(Error::new("you do not own a draft for this game"));
        }
        let _ = game_storefront::ensure_storefront(pool, &game_type)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        game_storefront::update_storefront(
            pool,
            &game_type,
            short_tagline,
            long_description,
            &screenshots_json,
            &patch_notes_json,
            &tags_json,
            avg_session_mins.unwrap_or(10).clamp(1, 180),
        )
        .await
        .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn submit_game_review(
        &self,
        ctx: &Context<'_>,
        game_type: String,
        body: String,
        gameplay: f32,
        balance: f32,
        visuals: f32,
        social: f32,
        depth: f32,
    ) -> Result<GameReviewGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let user = db::get_user(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("user not found"))?;
        let aspects = AspectRatings {
            gameplay: gameplay.clamp(1.0, 5.0),
            balance: balance.clamp(1.0, 5.0),
            visuals: visuals.clamp(1.0, 5.0),
            social: social.clamp(1.0, 5.0),
            depth: depth.clamp(1.0, 5.0),
        };
        let r =
            game_storefront::submit_review(pool, &game_type, uid, &user.1, body.trim(), &aspects)
                .await
                .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(GameReviewGql {
            id: r.id.to_string().into(),
            display_name: r.display_name,
            body: r.body,
            aspects: map_aspect_ratings(&r.aspects),
            helpful_votes: r.helpful_votes,
            user_has_voted: false,
            created_at: r.created_at,
        })
    }

    async fn mark_review_helpful(
        &self,
        ctx: &Context<'_>,
        review_id: async_graphql::types::ID,
    ) -> Result<GameReviewGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let rid =
            Uuid::parse_str(review_id.as_str()).map_err(|_| Error::new("invalid review id"))?;
        let r = game_storefront::mark_review_helpful(pool, rid, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let voted = game_storefront::user_voted_review(pool, r.id, uid)
            .await
            .unwrap_or(true);
        Ok(GameReviewGql {
            id: r.id.to_string().into(),
            display_name: r.display_name,
            body: r.body,
            aspects: map_aspect_ratings(&r.aspects),
            helpful_votes: r.helpful_votes,
            user_has_voted: voted,
            created_at: r.created_at,
        })
    }

    async fn update_display_name(
        &self,
        ctx: &Context<'_>,
        display_name: String,
    ) -> Result<UserGql> {
        let pool = ctx.data::<SqlitePool>()?;
        let uid = require_registered_user(ctx).await?;
        let name = display_name.trim();
        if name.is_empty() {
            return Err(Error::new("display name required"));
        }
        db::update_user_display_name(pool, uid, name)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let row = db::get_user(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("user not found"))?;
        Ok(UserGql {
            id: row.0.to_string().into(),
            display_name: row.1,
            created_at: row.2,
        })
    }

    async fn set_avatar_url(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "avatarUrl")] avatar_url: Option<String>,
    ) -> Result<bool> {
        let pool = ctx.data::<SqlitePool>()?;
        let uid = require_registered_user(ctx).await?;
        let url = avatar_url.as_deref().map(str::trim).filter(|u| !u.is_empty());
        db::set_user_avatar_url(pool, uid, url)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn logout(&self, ctx: &Context<'_>) -> Result<bool> {
        let pool = ctx.data::<SqlitePool>()?;
        let RequestUser(raw) = ctx.data::<RequestUser>()?;
        let Some(raw) = raw.as_ref() else {
            return Ok(false);
        };
        auth_sessions::revoke_session(pool, raw)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn submit_game_comment(
        &self,
        ctx: &Context<'_>,
        game_type: String,
        body: String,
    ) -> Result<GameCommentGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let user = db::get_user(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("user not found"))?;
        if body.trim().is_empty() {
            return Err(Error::new("comment cannot be empty"));
        }
        let c = game_storefront::submit_comment(pool, &game_type, uid, &user.1, body.trim())
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(GameCommentGql {
            id: c.id.to_string().into(),
            display_name: c.display_name,
            body: c.body,
            created_at: c.created_at,
        })
    }

    async fn sign_up(
        &self,
        ctx: &Context<'_>,
        display_name: String,
        password: String,
    ) -> Result<AuthSessionGql> {
        let pool = ctx.data::<SqlitePool>()?;
        let _ = ctx;
        if display_name.trim().is_empty() {
            return Err(Error::new("display name required"));
        }
        if password.len() < 8 {
            return Err(Error::new("password must be at least 8 characters"));
        }
        let hash = auth_password::hash_password(&password).map_err(Error::new)?;
        let (id, name, created) = db::sign_up(pool, display_name.trim(), &hash)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        issue_auth_session(
            pool,
            UserGql {
                id: id.to_string().into(),
                display_name: name,
                created_at: created,
            },
        )
        .await
    }

    /// Set or replace password for the current Bearer user (Argon2 hash in DB).
    async fn set_password(&self, ctx: &Context<'_>, password: String) -> Result<bool> {
        let pool = ctx.data::<SqlitePool>()?;
        let uid = require_registered_user(ctx).await?;
        if password.len() < 8 {
            return Err(Error::new("password must be at least 8 characters"));
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
    ) -> Result<AuthSessionGql> {
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
                return issue_auth_session(
                    pool,
                    UserGql {
                        id: row.0.to_string().into(),
                        display_name: row.1,
                        created_at: row.2,
                    },
                )
                .await;
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
        tracing::info!(user_id = %uid, filename = %filename, "upload game zip");
        let pool = ctx.data::<SqlitePool>()?;
        let component_db = ctx.data::<ComponentDb>()?;
        let drafts_dir = &ctx.data::<DraftsDir>()?.0;
        let games_dir = &ctx.data::<GamesDir>()?.0;
        let registry = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(zip_base64.as_bytes())
            .map_err(|e| Error::new(format!("invalid base64 payload: {e}")))?;

        let validation = validate_and_stage_zip_bytes(
            &bytes,
            component_db,
            drafts_dir,
            None,
            None,
        )
        .await;
        match validation {
            Ok(ok) => {
                let game = db::get_or_create_game(
                    pool,
                    uid,
                    &ok.manifest.name,
                    &ok.manifest.display_name,
                    games_dir.as_path(),
                )
                .await
                .map_err(|e| Error::new(format!("db: {e}")))?;
                tracing::info!(
                    user_id = %uid,
                    manifest_name = %ok.manifest.name,
                    slug = %game.slug,
                    version = %ok.manifest.version,
                    "upload game zip validated"
                );
                let report_json = serde_json::to_string(&ok.report)
                    .map_err(|e| Error::new(format!("serialize report: {e}")))?;
                let upload_id = db::insert_upload(pool, uid, &filename, "validated", &report_json)
                    .await
                    .map_err(|e| Error::new(format!("db: {e}")))?;
                let manifest_json = serde_json::to_string(&ok.manifest)
                    .map_err(|e| Error::new(format!("serialize manifest: {e}")))?;
                let taken = db::count_game_drafts_owner_name_version_active(
                    pool,
                    uid,
                    &ok.manifest.name,
                    &ok.manifest.version,
                    None,
                )
                .await
                .map_err(|e| Error::new(format!("db: {e}")))?;
                if taken > 0 {
                    return Err(Error::new(format!(
                        "You already have a draft or published record for game {:?} version {:?}. Bump the version in manifest.json and try again.",
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
                        slug: &game.slug,
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
                            "A draft or published record already uses this game name and version for your account.",
                        )
                    } else {
                        Error::new(format!("db: {msg}"))
                    }
                })?;

                let mut publish_warning = None;
                if let Err(e) = validate_game_folder_name(&game.slug) {
                    publish_warning = Some(format!("Auto-publish skipped: {e}"));
                } else if !db::game_owner_matches_slug(pool, &game.slug, uid)
                    .await
                    .unwrap_or(false)
                {
                    publish_warning =
                        Some("Auto-publish skipped: slug ownership mismatch.".to_string());
                } else {
                    match publish_staged_game(&ok.staged_dir, games_dir, &game.slug) {
                        Ok(_) => {
                            if let Err(e) = db::mark_draft_published(pool, draft_id).await {
                                publish_warning =
                                    Some(format!("Auto-publish failed (db): {e}"));
                            } else if let Err(e) = db::update_game_current_version(
                                pool,
                                &game.slug,
                                &ok.manifest.version,
                            )
                            .await
                            {
                                publish_warning =
                                    Some(format!("Auto-publish failed (version): {e}"));
                            } else {
                                let mut reg = registry
                                    .write()
                                    .map_err(|_| Error::new("registry lock poisoned"))?;
                                reg.reload(games_dir, component_db);
                            }
                        }
                        Err(e) => {
                            publish_warning = Some(format!("Auto-publish failed: {e}"));
                        }
                    }
                }

                let draft = db::get_game_draft(pool, draft_id)
                    .await
                    .map_err(|e| Error::new(format!("db: {e}")))?
                    .map(map_draft);
                Ok(UploadGameZipResultGql {
                    upload_id: upload_id.to_string().into(),
                    draft,
                    report: map_validation_report(ok.report),
                    publish_warning,
                })
            }
            Err(report_err) => {
                tracing::warn!(user_id = %uid, "upload game zip validation failed");
                let report: ValidationReport =
                    serde_json::from_str(&report_err).unwrap_or(ValidationReport {
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
                let report_json =
                    serde_json::to_string(&report).unwrap_or_else(|_| "{\"ok\":false}".to_string());
                let upload_id = db::insert_upload(pool, uid, &filename, "rejected", &report_json)
                    .await
                    .map_err(|e| Error::new(format!("db: {e}")))?;
                Ok(UploadGameZipResultGql {
                    upload_id: upload_id.to_string().into(),
                    draft: None,
                    report: map_validation_report(report),
                    publish_warning: None,
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
        tracing::info!(user_id = %uid, draft_id = %draft_id.as_str(), "publish game draft");
        let pool = ctx.data::<SqlitePool>()?;
        let games_dir = &ctx.data::<GamesDir>()?.0;
        let registry = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let component_db = ctx.data::<ComponentDb>()?;
        let did = Uuid::parse_str(draft_id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let draft = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found"))?;
        if draft.owner_user_id != uid
            && !is_superadmin(pool, uid).await.unwrap_or(false)
        {
            return Err(Error::new("not your draft"));
        }
        if draft.status != "ready" {
            return Err(Error::new("draft is not publishable"));
        }
        let slug = draft_slug(&draft)?;
        if !db::game_owner_matches_slug(pool, &slug, draft.owner_user_id)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
        {
            return Err(Error::new("you do not own this game slug"));
        }
        validate_game_folder_name(&slug).map_err(|m| Error::new(m.to_string()))?;
        let staged = PathBuf::from(&draft.storage_path);
        publish_staged_game(&staged, games_dir, &slug).map_err(Error::new)?;
        db::mark_draft_published(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        db::update_game_current_version(pool, &slug, &draft.version)
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
        tracing::info!(user_id = %uid, draft_id = %draft_id.as_str(), "unpublish game draft");
        let pool = ctx.data::<SqlitePool>()?;
        let games_dir = &ctx.data::<GamesDir>()?.0;
        let registry = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let component_db = ctx.data::<ComponentDb>()?;
        let did = Uuid::parse_str(draft_id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let draft = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found"))?;
        if draft.owner_user_id != uid
            && !is_superadmin(pool, uid).await.unwrap_or(false)
        {
            return Err(Error::new("not your draft"));
        }
        if draft.status != "published" {
            return Err(Error::new("draft is not published"));
        }
        let slug = draft_slug(&draft)?;
        validate_game_folder_name(&slug).map_err(|m| Error::new(m.to_string()))?;

        let max_pa = db::max_published_at_for_slug(pool, &slug)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let my_pa = draft.published_at;
        let this_is_latest_published = matches!((my_pa, max_pa), (Some(t), Some(m)) if t == m);

        if this_is_latest_published {
            remove_published_game_dir(games_dir, &slug).map_err(Error::new)?;
            db::demote_all_published_for_slug(pool, &slug)
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
        let games_dir = &ctx.data::<GamesDir>()?.0;
        let did = Uuid::parse_str(draft_id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let draft = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found"))?;
        if draft.owner_user_id != uid
            && !is_superadmin(pool, uid).await.unwrap_or(false)
        {
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
        let is_sa = is_superadmin(pool, uid).await.unwrap_or(false);
        let owner_for_sql = if is_sa { draft.owner_user_id } else { uid };
        let clash = db::count_game_drafts_owner_name_version_active(
            pool,
            owner_for_sql,
            &manifest.name,
            &manifest.version,
            Some(did),
        )
        .await
        .map_err(|e| Error::new(format!("db: {e}")))?;
        if clash > 0 {
            return Err(Error::new(
                "Another draft or published record already uses this name and version for this account. Pick a different combination.",
            ));
        }
        let game = db::get_or_create_game(
            pool,
            owner_for_sql,
            &manifest.name,
            &manifest.display_name,
            games_dir.as_path(),
        )
        .await
        .map_err(|e| Error::new(format!("db: {e}")))?;
        let staged = PathBuf::from(&draft.storage_path);
        if !staged.is_dir() {
            return Err(Error::new("draft storage is missing on disk"));
        }
        write_manifest_to_staged_dir(&staged, &manifest).map_err(Error::new)?;
        let updated = db::update_game_draft_manifest_columns(
            pool,
            did,
            owner_for_sql,
            &manifest.name,
            &manifest.display_name,
            &manifest.version,
            &game.slug,
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
        tracing::info!(user_id = %uid, draft_id = %draft_id.as_str(), "discard game draft");
        let pool = ctx.data::<SqlitePool>()?;
        let did = Uuid::parse_str(draft_id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let draft = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found"))?;
        if draft.owner_user_id != uid
            && !is_superadmin(pool, uid).await.unwrap_or(false)
        {
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
        let config_bytes = game_service::default_config(component_db, &game_type)
            .await
            .map_err(Error::new)?;
        let config_s = String::from_utf8_lossy(&config_bytes).to_string();
        let identities =
            game_service::preview_init_identities(component_db, game_type.clone(), config_bytes)
                .await
                .map_err(Error::new)?;
        lobby_db::owner_replace_game_type_and_seats(
            pool,
            lid,
            uid,
            &game_type,
            &identities,
            Some(&config_s),
            force,
        )
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
        lobby_db::owner_replace_config_and_seats(
            pool,
            lid,
            uid,
            &gt,
            &config_s,
            &identities,
            force,
        )
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
                    return Err(Error::new("you already occupy a seat in this lobby"));
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
        if detail.status != "waiting" && detail.status != "configuring" && detail.status != "finished" {
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

    async fn transfer_lobby_ownership(
        &self,
        ctx: &Context<'_>,
        lobby_id: async_graphql::types::ID,
        #[graphql(name = "newOwnerUserId")] new_owner_user_id: async_graphql::types::ID,
    ) -> Result<LobbyGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let new_owner = Uuid::parse_str(new_owner_user_id.as_str())
            .map_err(|_| Error::new("invalid user id"))?;
        lobby_db::transfer_lobby_ownership(pool, lid, uid, new_owner)
            .await
            .map_err(Error::new)?;
        notify.ping();
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        lobby_to_gql(pool, detail).await
    }

    async fn kick_lobby_player(
        &self,
        ctx: &Context<'_>,
        lobby_id: async_graphql::types::ID,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
    ) -> Result<LobbyGql> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let target = Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        lobby_db::kick_lobby_player(pool, lid, uid, target)
            .await
            .map_err(Error::new)?;
        notify.ping();
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        lobby_to_gql(pool, detail).await
    }

    async fn leave_lobby(
        &self,
        ctx: &Context<'_>,
        lobby_id: async_graphql::types::ID,
    ) -> Result<bool> {
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
        if detail.owner_user_id != uid
            && !is_superadmin(pool, uid).await.unwrap_or(false)
        {
            return Err(Error::new("only the owner can start"));
        }
        if detail.status != "waiting" && detail.status != "configuring" {
            return Err(Error::new("lobby cannot be started in this state"));
        }
        if detail.game_type.trim().is_empty() {
            return Err(Error::new(
                "choose a game type in the lobby before starting",
            ));
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
        let game_type = detail.game_type.clone();
        let seated_users: Vec<Uuid> = detail
            .seats
            .iter()
            .filter_map(|s| s.claimed_by_user_id)
            .collect();
        let config = game_service::resolve_lobby_config(
            component_db,
            &game_type,
            detail.config.as_deref(),
        )
        .await
        .map_err(Error::new)?;
        let gid = game_service::create_and_spawn_game(
            component_db,
            game_db,
            game_store.clone(),
            game_type.clone(),
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
        let _ = user_engagement::notify_lobby_started(pool, &seated_users, &game_type).await;
        let _ = friends::insert_friend_activity(pool, uid, "lobby_created", &lid.to_string()).await;
        notify.ping();
        Ok(gid.to_string().into())
    }

    async fn cancel_lobby(
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
        if detail.owner_user_id != uid
            && !is_superadmin(pool, uid).await.unwrap_or(false)
        {
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
        if detail.owner_user_id != uid
            && !is_superadmin(pool, uid).await.unwrap_or(false)
        {
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

    async fn mark_notification_read(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::types::ID,
    ) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let nid =
            Uuid::parse_str(id.as_str()).map_err(|_| Error::new("invalid notification id"))?;
        user_engagement::mark_read(pool, uid, nid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn mark_all_notifications_read(&self, ctx: &Context<'_>) -> Result<i32> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        user_engagement::mark_all_read(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn admin_grant_role(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
        role: String,
    ) -> Result<bool> {
        let _actor = require_superadmin_user(ctx).await?;
        let role = role.trim().to_lowercase();
        if role != "developer" && role != "superadmin" {
            return Err(Error::new("role must be developer or superadmin"));
        }
        let pool = ctx.data::<SqlitePool>()?;
        let uid = Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        if db::get_user(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .is_none()
        {
            return Err(Error::new("user not found"));
        }
        db::grant_role(pool, uid, &role)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(true)
    }

    async fn admin_revoke_role(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
        role: String,
    ) -> Result<bool> {
        let _actor = require_superadmin_user(ctx).await?;
        let role = role.trim().to_lowercase();
        if role != "developer" && role != "superadmin" {
            return Err(Error::new("role must be developer or superadmin"));
        }
        let pool = ctx.data::<SqlitePool>()?;
        let uid = Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        db::revoke_role(pool, uid, &role)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn admin_update_user_display_name(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
        display_name: String,
    ) -> Result<UserGql> {
        let _actor = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let uid = Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        let name = display_name.trim();
        if name.is_empty() {
            return Err(Error::new("display name required"));
        }
        db::update_user_display_name(pool, uid, name)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let row = db::get_user(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("user not found"))?;
        Ok(UserGql {
            id: row.0.to_string().into(),
            display_name: row.1,
            created_at: row.2,
        })
    }

    async fn admin_revoke_user_sessions(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
    ) -> Result<i32> {
        let _actor = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let uid = Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        let n = auth_sessions::revoke_all_sessions_for_user(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(n as i32)
    }

    async fn admin_revoke_user_publish_tokens(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
    ) -> Result<i32> {
        let _actor = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let uid = Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        let n = db::revoke_all_publish_tokens_for_user(pool, uid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        Ok(n as i32)
    }

    async fn admin_discard_game_draft(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "draftId")] draft_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let _actor = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let did = Uuid::parse_str(draft_id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let draft = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found"))?;
        if draft.status == "published" {
            return Err(Error::new("cannot discard a published draft; unpublish first"));
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

    async fn admin_publish_game_draft(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "draftId")] draft_id: async_graphql::types::ID,
    ) -> Result<GameDraftGql> {
        let _actor = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let games_dir = &ctx.data::<GamesDir>()?.0;
        let registry = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let component_db = ctx.data::<ComponentDb>()?;
        let did = Uuid::parse_str(draft_id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let draft = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found"))?;
        if draft.status != "ready" {
            return Err(Error::new("draft is not publishable"));
        }
        let slug = draft_slug(&draft)?;
        validate_game_folder_name(&slug).map_err(|m| Error::new(m.to_string()))?;
        let staged = PathBuf::from(&draft.storage_path);
        publish_staged_game(&staged, games_dir, &slug).map_err(Error::new)?;
        db::mark_draft_published(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        db::update_game_current_version(pool, &slug, &draft.version)
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

    async fn admin_unpublish_game_draft(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "draftId")] draft_id: async_graphql::types::ID,
    ) -> Result<GameDraftGql> {
        let _actor = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let games_dir = &ctx.data::<GamesDir>()?.0;
        let registry = ctx.data::<Arc<RwLock<GameRegistry>>>()?;
        let component_db = ctx.data::<ComponentDb>()?;
        let did = Uuid::parse_str(draft_id.as_str()).map_err(|_| Error::new("invalid draft id"))?;
        let draft = db::get_game_draft(pool, did)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("draft not found"))?;
        if draft.status != "published" {
            return Err(Error::new("draft is not published"));
        }
        let slug = draft_slug(&draft)?;
        validate_game_folder_name(&slug).map_err(|m| Error::new(m.to_string()))?;
        let max_pa = db::max_published_at_for_slug(pool, &slug)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?;
        let my_pa = draft.published_at;
        let this_is_latest_published = matches!((my_pa, max_pa), (Some(t), Some(m)) if t == m);
        if this_is_latest_published {
            remove_published_game_dir(games_dir, &slug).map_err(Error::new)?;
            db::demote_all_published_for_slug(pool, &slug)
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

    async fn admin_cancel_lobby(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "lobbyId")] lobby_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let _actor = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<LobbyListNotify>()?;
        let lid = Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        let detail = lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
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

    async fn admin_delete_review(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "reviewId")] review_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let _actor = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let rid =
            Uuid::parse_str(review_id.as_str()).map_err(|_| Error::new("invalid review id"))?;
        game_storefront::delete_review(pool, rid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn admin_delete_comment(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "commentId")] comment_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let _actor = require_superadmin_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let cid =
            Uuid::parse_str(comment_id.as_str()).map_err(|_| Error::new("invalid comment id"))?;
        game_storefront::delete_comment(pool, cid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))
    }

    async fn send_friend_request(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<FriendsListNotify>()?;
        let target =
            Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        friends::send_friend_request(pool, uid, target)
            .await
            .map_err(|e| Error::new(e.to_string()))?;
        notify.ping();
        Ok(true)
    }

    async fn accept_friend_request(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<FriendsListNotify>()?;
        let from =
            Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        friends::accept_friend_request(pool, uid, from)
            .await
            .map_err(|e| Error::new(e.to_string()))?;
        notify.ping();
        Ok(true)
    }

    async fn decline_friend_request(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<FriendsListNotify>()?;
        let from =
            Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        friends::decline_friend_request(pool, uid, from)
            .await
            .map_err(|e| Error::new(e.to_string()))?;
        notify.ping();
        Ok(true)
    }

    async fn cancel_friend_request(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<FriendsListNotify>()?;
        let to = Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        friends::cancel_friend_request(pool, uid, to)
            .await
            .map_err(|e| Error::new(e.to_string()))?;
        notify.ping();
        Ok(true)
    }

    async fn remove_friend(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<FriendsListNotify>()?;
        let friend =
            Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        friends::remove_friend(pool, uid, friend)
            .await
            .map_err(|e| Error::new(e.to_string()))?;
        notify.ping();
        Ok(true)
    }

    async fn block_user(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "userId")] user_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let notify = ctx.data::<FriendsListNotify>()?;
        let target =
            Uuid::parse_str(user_id.as_str()).map_err(|_| Error::new("invalid user id"))?;
        friends::block_user(pool, uid, target)
            .await
            .map_err(|e| Error::new(e.to_string()))?;
        notify.ping();
        Ok(true)
    }

    async fn invite_friend_to_lobby(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "friendUserId")] friend_user_id: async_graphql::types::ID,
        #[graphql(name = "lobbyId")] lobby_id: async_graphql::types::ID,
    ) -> Result<bool> {
        let uid = require_registered_user(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let friend = Uuid::parse_str(friend_user_id.as_str())
            .map_err(|_| Error::new("invalid friend user id"))?;
        let lid =
            Uuid::parse_str(lobby_id.as_str()).map_err(|_| Error::new("invalid lobby id"))?;
        lobby_db::get_lobby(pool, lid)
            .await
            .map_err(|e| Error::new(format!("db: {e}")))?
            .ok_or_else(|| Error::new("lobby not found"))?;
        friends::invite_friend_to_lobby(pool, uid, friend, lid)
            .await
            .map_err(|e| Error::new(e.to_string()))?;
        Ok(true)
    }
}
