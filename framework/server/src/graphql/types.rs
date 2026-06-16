use std::sync::{Arc, RwLock};

use async_graphql::{Error, Result, SimpleObject};
use sqlx::SqlitePool;

use crate::db::{self, FinishedGameRow};
use crate::game_db::GameDb;
use crate::game_registry::GameRegistry;
use crate::game_storefront::AspectRatings;
use crate::game_upload::ValidationReport;
use crate::lobby_db::{self, LobbyDetail, LobbyMessage, LobbySeat, LobbySummary};
#[derive(SimpleObject, Clone)]
pub struct AuthSessionGql {
    pub session_token: String,
    pub expires_at: i64,
    pub user: UserGql,
}

#[derive(SimpleObject, Clone)]
pub struct KpiTrendGql {
    pub label: String,
    pub value: String,
    pub delta_pct: String,
    pub up: bool,
}

#[derive(SimpleObject, Clone)]
pub struct UserGql {
    pub id: async_graphql::types::ID,
    pub display_name: String,
    pub created_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct PublishTokenGql {
    pub token: String,
    pub user_id: async_graphql::types::ID,
    pub expires_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct PublishTokenSummaryGql {
    pub id: async_graphql::types::ID,
    pub label: Option<String>,
    pub masked_key: String,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct GameSessionGql {
    pub game_id: String,
    pub game_type: String,
    pub finished_at: i64,
    pub winner_display_name: Option<String>,
    pub participant_count: u32,
    pub duration_secs: i32,
}

#[derive(SimpleObject, Clone)]
pub struct LeaderboardEntryGql {
    pub rank: u32,
    pub display_name: String,
    pub total_score: i32,
    pub wins: u32,
    pub win_rate_pct: u32,
}

#[derive(SimpleObject, Clone)]
pub struct DeploymentGql {
    pub id: async_graphql::types::ID,
    pub game_name: String,
    pub display_name: String,
    pub version: String,
    pub status: String,
    pub deployed_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct CliReleaseGql {
    pub version: String,
    pub min_supported: String,
    pub released_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct PlatformManifestGql {
    pub framework_version: String,
    pub wit_version: String,
    pub wasmtime_version: Option<String>,
    pub released_at: Option<String>,
    pub cli: CliReleaseGql,
    pub sdk_versions_json: String,
}

#[derive(SimpleObject, Clone)]
pub struct PlatformStatsGql {
    pub active_lobbies: i32,
    pub published_game_types: i32,
    pub finished_games24h: i32,
    pub active_sessions: i32,
    pub status: String,
    pub trends: Vec<KpiTrendGql>,
    pub pro_tip: String,
}

#[derive(SimpleObject, Clone)]
pub struct ActivityEventGql {
    pub actor: String,
    pub action: String,
    pub target: String,
    pub timestamp: i64,
}

#[derive(SimpleObject, Clone)]
pub struct UserProfileGql {
    pub display_name: String,
    pub created_at: i64,
    pub matches_played: u32,
    pub games_published: u32,
    pub wins: u32,
    pub rep_score: u32,
}

#[derive(SimpleObject, Clone)]
pub struct BadgeGql {
    pub id: String,
    pub label: String,
    pub tier: String,
    pub locked: bool,
    pub earned_at: Option<i64>,
}

#[derive(SimpleObject, Clone)]
pub struct NotificationGql {
    pub id: async_graphql::types::ID,
    pub title: String,
    pub body: String,
    pub kind: String,
    pub unread: bool,
    pub created_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct GameScreenshotGql {
    pub id: String,
    pub caption: String,
    pub gradient: String,
    #[graphql(name = "imageUrl")]
    pub image_url: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct GamePatchNoteGql {
    pub version: String,
    pub date: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
}

#[derive(SimpleObject, Clone)]
pub struct AspectRatingsGql {
    pub gameplay: f32,
    pub balance: f32,
    pub visuals: f32,
    pub social: f32,
    pub depth: f32,
}

#[derive(SimpleObject, Clone)]
pub struct GameStorefrontGql {
    pub game_name: String,
    pub short_tagline: Option<String>,
    pub long_description: String,
    pub screenshots: Vec<GameScreenshotGql>,
    pub patch_notes: Vec<GamePatchNoteGql>,
    pub tags: Vec<String>,
    pub avg_session_mins: i32,
    pub featured: bool,
    pub creator_display_name: Option<String>,
    pub aspect_ratings: AspectRatingsGql,
    pub review_count: i32,
    pub can_edit: bool,
    pub updated_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct GameReviewGql {
    pub id: async_graphql::types::ID,
    pub display_name: String,
    pub body: String,
    pub aspects: AspectRatingsGql,
    pub helpful_votes: i32,
    pub user_has_voted: bool,
    pub created_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct GameCommentGql {
    pub id: async_graphql::types::ID,
    pub display_name: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct AdminUserGql {
    pub id: async_graphql::types::ID,
    pub display_name: String,
    pub created_at: i64,
    pub roles: Vec<String>,
    pub has_password: bool,
}

#[derive(SimpleObject, Clone)]
pub struct AdminPlatformOverviewGql {
    pub user_count: i32,
    pub draft_count: i32,
    pub active_lobbies: i32,
    pub published_games: i32,
    pub review_count: i32,
    pub comment_count: i32,
}

#[derive(SimpleObject, Clone)]
pub struct AdminReviewGql {
    pub id: async_graphql::types::ID,
    pub game_name: String,
    pub display_name: String,
    pub body: String,
    pub helpful_votes: i32,
    pub created_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct AdminCommentGql {
    pub id: async_graphql::types::ID,
    pub game_name: String,
    pub display_name: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(SimpleObject, Clone)]
pub struct PlayTimeLeaderboardEntryGql {
    pub rank: u32,
    pub display_name: String,
    pub total_mins: i32,
    pub sessions: u32,
}

pub(crate) fn map_aspect_ratings(a: &AspectRatings) -> AspectRatingsGql {
    AspectRatingsGql {
        gameplay: a.gameplay,
        balance: a.balance,
        visuals: a.visuals,
        social: a.social,
        depth: a.depth,
    }
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
    pub cover_image_url: Option<String>,
    pub active_players: i32,
    pub featured: bool,
    pub tags: Vec<String>,
    pub creator_display_name: Option<String>,
    pub avg_session_mins: i32,
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

pub(crate) fn map_finished_row(
    r: FinishedGameRow,
    registry: &Arc<RwLock<GameRegistry>>,
) -> FinishedGameGql {
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

pub(crate) fn map_seat(s: LobbySeat) -> LobbySeatGql {
    LobbySeatGql {
        seat_index: s.seat_index,
        player_identity: s.player_identity,
        claimed_by_user_id: s.claimed_by_user_id.map(|u| u.to_string().into()),
        claimed_display_name: s.claimed_display_name,
        ready: s.ready,
    }
}

pub(crate) fn map_message(m: LobbyMessage) -> LobbyMessageGql {
    LobbyMessageGql {
        id: m.id.to_string().into(),
        user_id: m.user_id.to_string().into(),
        display_name: m.display_name,
        body: m.body,
        created_at: m.created_at,
    }
}

pub(crate) fn map_summary(s: LobbySummary) -> LobbySummaryGql {
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

pub(crate) fn map_validation_report(report: ValidationReport) -> ValidationReportGql {
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

pub(crate) fn map_draft(d: db::GameDraftRow) -> GameDraftGql {
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

pub(crate) async fn lobby_to_gql(pool: &SqlitePool, d: LobbyDetail) -> Result<LobbyGql> {
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

pub(crate) fn map_game_entries(db: &GameDb) -> Vec<GameInstanceGql> {
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
