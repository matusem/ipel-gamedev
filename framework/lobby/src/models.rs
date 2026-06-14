use serde::{Deserialize, Serialize};

pub const CONFIG_MSG_SOURCE: &str = "upjs-gdd-game-config";
pub const CONFIG_RESULT_SOURCE: &str = "upjs-gdd-game-config-result";
pub const CONFIG_SCHEMA_SOURCE: &str = "upjs-gdd-game-config-schema";
pub const CONFIG_STATE_SOURCE: &str = "upjs-gdd-game-config-state";
pub const USER_ID_KEY: &str = "upjs_gdd_user_id";
pub const SESSION_TOKEN_KEY: &str = "upjs_gdd_session_token";

pub const LOBBIES_QUERY: &str =
    r#"query { lobbies { id gameType status seatsFilled seatsTotal ownerDisplayName gameInstanceId createdAt } }"#;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameTypeInfo {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub min_players: u32,
    pub max_players: u32,
    pub description: String,
    #[serde(default)]
    pub config_ui_path: Option<String>,
    #[serde(default)]
    pub about_ui_path: Option<String>,
    #[serde(default)]
    pub config_schema_json: Option<String>,
    #[serde(default)]
    pub cover_image_url: Option<String>,
    #[serde(default)]
    pub active_players: i32,
    #[serde(default)]
    pub featured: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub creator_display_name: Option<String>,
    #[serde(default)]
    pub avg_session_mins: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub game_id: String,
    pub game_type: String,
    pub player_identities: Vec<String>,
    pub connected_players: usize,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameResultRow {
    pub game_id: String,
    pub game_type: String,
    pub lobby_id: Option<String>,
    pub finished_at: i64,
    pub result_json: String,
    pub player_scores_json: String,
    pub seats_snapshot_json: String,
    #[serde(default)]
    pub result_ui_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedGameResult {
    pub row: GameResultRow,
    pub iframe_src: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFinishedRow {
    pub game_id: String,
    pub game_type: String,
    pub finished_at: i64,
    pub player_scores_json: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbySummary {
    pub id: String,
    pub game_type: String,
    pub status: String,
    pub seats_filled: i32,
    pub seats_total: i32,
    pub owner_display_name: String,
    #[serde(default)]
    pub game_instance_id: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbySeat {
    pub seat_index: i32,
    pub player_identity: String,
    #[serde(default)]
    pub claimed_by_user_id: Option<String>,
    #[serde(default)]
    pub claimed_display_name: Option<String>,
    #[serde(default)]
    pub ready: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyMessage {
    pub id: String,
    pub user_id: String,
    pub display_name: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyDetail {
    pub id: String,
    pub owner_user_id: String,
    pub owner_display_name: String,
    pub game_type: String,
    #[serde(default)]
    pub config_json: Option<String>,
    pub status: String,
    #[serde(default)]
    pub game_instance_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub seats: Vec<LobbySeat>,
    #[serde(default)]
    pub messages: Vec<LobbyMessage>,
}

#[derive(Clone, Debug)]
pub struct PlayOverlay {
    pub game_type: String,
    pub game_id: String,
    pub player: String,
    pub return_lobby_id: Option<String>,
    pub spectator: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterUserRow {
    pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRow {
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub created_at: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionRow {
    pub session_token: String,
    pub user: UserRow,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterUserData {
    pub register_user: AuthSessionRow,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignUpData {
    pub sign_up: AuthSessionRow,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginData {
    pub login_with_password: AuthSessionRow,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbiesData {
    pub lobbies: Vec<LobbySummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamesListData {
    pub game_instances: Vec<GameInfo>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadDiag {
    pub severity: String,
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub hint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadReport {
    pub ok: bool,
    pub errors: i32,
    pub warnings: i32,
    pub infos: i32,
    pub required_index_html: bool,
    pub required_config_html: bool,
    pub required_result_html: bool,
    pub required_about_html: bool,
    pub diagnostics: Vec<UploadDiag>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDraftShort {
    pub id: String,
    pub game_name: String,
    pub display_name: String,
    pub version: String,
    pub status: String,
    pub manifest_json: String,
    pub created_at: i64,
    pub published_at: Option<i64>,
}

pub fn game_type_display_title(types: &[GameTypeInfo], stored_name: &str) -> String {
    let t = stored_name.trim();
    if t.is_empty() {
        return "No game selected yet".to_string();
    }
    types
        .iter()
        .find(|g| g.name == t)
        .map(|g| g.display_name.clone())
        .unwrap_or_else(|| t.to_string())
}

pub fn game_type_description(types: &[GameTypeInfo], stored_name: &str) -> Option<String> {
    let t = stored_name.trim();
    if t.is_empty() {
        return None;
    }
    types.iter().find(|g| g.name == t).and_then(|g| {
        let d = g.description.trim();
        if d.is_empty() {
            None
        } else {
            Some(d.to_string())
        }
    })
}

pub fn game_type_cover_url(gt: &GameTypeInfo) -> Option<String> {
    gt.cover_image_url
        .clone()
        .or_else(|| crate::stub::demo_images::cover_image_url(&gt.name).map(str::to_string))
}

/// Link to the published game's `about.html` (served by the Actix backend).
/// Demo mode lists synthetic game types that are not deployed — no server asset exists.
pub fn game_type_about_url(gt: &GameTypeInfo) -> Option<String> {
    if crate::stub::demo_mode::is_demo_mode() {
        return None;
    }
    gt.about_ui_path
        .as_ref()
        .map(|path| format!("/games/{}/{}", gt.name, path))
}

pub fn lobby_status_dot_class(status: &str, seats_filled: i32, seats_total: i32) -> &'static str {
    let s = status.to_lowercase();
    if s.contains("full") || (seats_total > 0 && seats_filled >= seats_total) {
        "status-dot-full"
    } else if s.contains("waiting") || s.contains("open") {
        "status-dot-online"
    } else {
        "status-dot-away"
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishTokenSummary {
    pub id: String,
    pub label: Option<String>,
    pub masked_key: String,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSession {
    pub game_id: String,
    pub game_type: String,
    pub finished_at: i64,
    pub winner_display_name: Option<String>,
    pub participant_count: u32,
    pub duration_secs: i32,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntry {
    pub rank: u32,
    pub display_name: String,
    pub total_score: i32,
    pub wins: u32,
    pub win_rate_pct: u32,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentRow {
    pub id: String,
    pub game_name: String,
    pub display_name: String,
    pub version: String,
    pub status: String,
    pub deployed_at: i64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KpiTrend {
    pub label: String,
    pub value: String,
    pub delta_pct: String,
    pub up: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformStats {
    pub active_lobbies: i32,
    pub published_game_types: i32,
    pub finished_games24h: i32,
    #[serde(default)]
    pub active_sessions: i32,
    pub status: String,
    #[serde(default)]
    pub trends: Vec<KpiTrend>,
    #[serde(default)]
    pub pro_tip: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEventGql {
    pub actor: String,
    pub action: String,
    pub target: String,
    pub timestamp: i64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub display_name: String,
    pub created_at: i64,
    pub matches_played: u32,
    pub games_published: u32,
    pub wins: u32,
    pub rep_score: u32,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BadgeGql {
    pub id: String,
    pub label: String,
    pub tier: String,
    pub locked: bool,
    #[serde(default)]
    pub earned_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationGql {
    pub id: String,
    pub title: String,
    pub body: String,
    pub kind: String,
    pub unread: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameScreenshot {
    pub id: String,
    pub caption: String,
    pub gradient: String,
    #[serde(default)]
    pub image_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePatchNote {
    pub version: String,
    pub date: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AspectRatings {
    pub gameplay: f32,
    pub balance: f32,
    pub visuals: f32,
    pub social: f32,
    pub depth: f32,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStorefront {
    pub game_name: String,
    pub short_tagline: Option<String>,
    pub long_description: String,
    pub screenshots: Vec<GameScreenshot>,
    pub patch_notes: Vec<GamePatchNote>,
    pub tags: Vec<String>,
    pub avg_session_mins: i32,
    #[serde(default)]
    pub featured: bool,
    #[serde(default)]
    pub creator_display_name: Option<String>,
    pub aspect_ratings: AspectRatings,
    pub review_count: i32,
    pub can_edit: bool,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameReview {
    pub id: String,
    pub display_name: String,
    pub body: String,
    pub aspects: AspectRatings,
    pub helpful_votes: i32,
    #[serde(default)]
    pub user_has_voted: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameComment {
    pub id: String,
    pub display_name: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayTimeEntry {
    pub rank: u32,
    pub display_name: String,
    pub total_mins: i32,
    pub sessions: u32,
}

pub fn format_estimated_match_time(avg_session_mins: i32) -> String {
    if avg_session_mins <= 0 {
        "—".into()
    } else {
        format!("~{avg_session_mins} min")
    }
}

pub fn format_play_time(mins: i32) -> String {
    if mins < 60 {
        format!("{mins}m")
    } else {
        format!("{}h {}m", mins / 60, mins % 60)
    }
}

pub fn format_relative_time(timestamp: i64) -> String {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let secs = (now - timestamp).max(0);
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

pub fn format_session_duration(secs: i32) -> String {
    if secs <= 0 {
        "—".to_string()
    } else if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m {}s", secs / 60, secs % 60)
    }
}

pub fn manifest_description_from_json(manifest_json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(manifest_json)
        .ok()
        .and_then(|v| v.get("description").and_then(|d| d.as_str()).map(str::to_string))
        .unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPlatformOverview {
    pub user_count: i32,
    pub draft_count: i32,
    pub active_lobbies: i32,
    pub published_games: i32,
    pub review_count: i32,
    pub comment_count: i32,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserRow {
    pub id: String,
    pub display_name: String,
    pub created_at: i64,
    pub roles: Vec<String>,
    pub has_password: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminDraftRow {
    pub id: String,
    pub owner_user_id: String,
    pub game_name: String,
    pub display_name: String,
    pub version: String,
    pub status: String,
    pub created_at: i64,
    pub published_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminReviewRow {
    pub id: String,
    pub game_name: String,
    pub display_name: String,
    pub body: String,
    pub helpful_votes: i32,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminCommentRow {
    pub id: String,
    pub game_name: String,
    pub display_name: String,
    pub body: String,
    pub created_at: i64,
}
