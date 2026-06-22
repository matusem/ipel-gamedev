use crate::models::{GamesListData, LobbiesData, LobbyDetail, LOBBIES_QUERY, ACTIVE_GAME_KEY, PlayOverlay, SESSION_TOKEN_KEY, USER_ID_KEY};
use dioxus::prelude::*;
use gloo_net::http::{Request, Response};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

pub const LOBBY_DETAIL_FIELDS: &str = r#"id ownerUserId ownerDisplayName gameType configJson status gameInstanceId createdAt updatedAt seats { seatIndex playerIdentity claimedByUserId claimedDisplayName ready botId botDisplayName externalBot externalBotCategory botAvatarSeed botAvatarUrl botSettingsJson } botRequests { id category label avatarSeed gameSlug contractHash desiredSeatIndex status seatIndex createdAt settingsJson } messages { id userId displayName body createdAt }"#;

pub fn graphql_error_message(err: &str) -> String {
    format_errors_from_str(err).unwrap_or_else(|| err.trim().to_string())
}

fn humanize_graphql_message(msg: &str) -> String {
    let trimmed = msg.trim();
    if trimmed.is_empty() {
        return "Something went wrong".to_string();
    }
    let lower = trimmed.to_lowercase();
    if lower.starts_with("cannot claim seat") {
        return "That seat is unavailable — it's taken, invalid, or you're already seated elsewhere."
            .to_string();
    }
    if lower.starts_with("cannot join lobby") {
        return trimmed.to_string();
    }
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return trimmed.to_string();
    };
    first.to_uppercase().collect::<String>() + chars.as_str()
}

fn extract_error_messages(value: &Value) -> Vec<String> {
    match value {
        Value::Array(arr) => arr
            .iter()
            .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
            .map(humanize_graphql_message)
            .collect(),
        Value::Object(obj) => {
            if let Some(msg) = obj.get("message").and_then(|m| m.as_str()) {
                return vec![humanize_graphql_message(msg)];
            }
            if let Some(errs) = obj.get("errors") {
                return extract_error_messages(errs);
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn format_errors_value(errs: &Value) -> String {
    let messages = extract_error_messages(errs);
    if messages.is_empty() {
        return "Request failed".to_string();
    }
    messages.join("; ")
}

fn format_errors_from_str(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        let messages = extract_error_messages(&v);
        if !messages.is_empty() {
            return Some(messages.join("; "));
        }
    }
    if let Some(json_start) = trimmed.find('[') {
        if let Ok(v) = serde_json::from_str::<Value>(&trimmed[json_start..]) {
            let messages = extract_error_messages(&v);
            if !messages.is_empty() {
                return Some(messages.join("; "));
            }
        }
    }
    None
}

pub fn lobby_mutation_needs_force(err: &str) -> bool {
    let msg = graphql_error_message(err);
    msg.contains("seats are claimed") || msg.contains("confirm reset")
}

pub async fn fetch_lobby_detail(lobby_id: &str) -> Result<Option<LobbyDetail>, String> {
    let q = format!(
        "query L($id: ID!) {{ lobby(id: $id) {{ {} }} }}",
        LOBBY_DETAIL_FIELDS
    );
    let vars = serde_json::json!({ "id": lobby_id });
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Ld {
        lobby: Option<LobbyDetail>,
    }
    Ok(graphql_exec::<Ld>(&q, Some(vars)).await?.lobby)
}

pub async fn set_lobby_game_type(
    lobby_id: &str,
    game_type: &str,
    force: bool,
) -> Result<LobbyDetail, String> {
    let q = format!(
        "mutation S($id: ID!, $t: String!, $f: Boolean!) {{ setLobbyGameType(lobbyId: $id, gameType: $t, force: $f) {{ {} }} }}",
        LOBBY_DETAIL_FIELDS
    );
    let vars = serde_json::json!({ "id": lobby_id, "t": game_type, "f": force });
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SetGt {
        set_lobby_game_type: LobbyDetail,
    }
    Ok(graphql_exec::<SetGt>(&q, Some(vars))
        .await?
        .set_lobby_game_type)
}

pub async fn transfer_lobby_ownership(
    lobby_id: &str,
    new_owner_user_id: &str,
) -> Result<LobbyDetail, String> {
    let q = format!(
        "mutation T($id: ID!, $u: ID!) {{ transferLobbyOwnership(lobbyId: $id, newOwnerUserId: $u) {{ {} }} }}",
        LOBBY_DETAIL_FIELDS
    );
    let vars = serde_json::json!({ "id": lobby_id, "u": new_owner_user_id });
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Tr {
        transfer_lobby_ownership: LobbyDetail,
    }
    Ok(graphql_exec::<Tr>(&q, Some(vars))
        .await?
        .transfer_lobby_ownership)
}

pub async fn kick_lobby_player(lobby_id: &str, user_id: &str) -> Result<LobbyDetail, String> {
    let q = format!(
        "mutation K($id: ID!, $u: ID!) {{ kickLobbyPlayer(lobbyId: $id, userId: $u) {{ {} }} }}",
        LOBBY_DETAIL_FIELDS
    );
    let vars = serde_json::json!({ "id": lobby_id, "u": user_id });
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Kr {
        kick_lobby_player: LobbyDetail,
    }
    Ok(graphql_exec::<Kr>(&q, Some(vars))
        .await?
        .kick_lobby_player)
}

/// Creates a lobby and, when `game_type` is provided, selects that game and materializes seats.
pub async fn create_lobby_with_game(game_type: Option<&str>) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Cr {
        create_lobby: crate::models::RegisterUserRow,
    }
    let q = if game_type.is_some() {
        "mutation C($gt: String) { createLobby(gameType: $gt) { id } }"
    } else {
        "mutation { createLobby { id } }"
    };
    let vars = game_type.map(|gt| serde_json::json!({ "gt": gt }));
    let id = graphql_exec::<Cr>(q, vars).await?.create_lobby.id;
    if let Some(gt) = game_type.filter(|s| !s.trim().is_empty()) {
        let _ = set_lobby_game_type(&id, gt, false).await?;
    }
    Ok(id)
}

const DEV_API_HINT: &str =
    "Backend not reachable on :8081. Run scripts/dev-backend.ps1 (or: $env:PORT=8081; cargo run -p server), then retry.";

async fn parse_graphql_response<T: DeserializeOwned>(resp: Response) -> Result<T, String> {
    let status = resp.status();
    let ok = resp.ok();
    let text = resp.text().await.map_err(|e| format!("{e}"))?;
    if text.trim().is_empty() {
        return Err(format!(
            "API returned empty response (HTTP {status}). {DEV_API_HINT}"
        ));
    }
    if !ok {
        if let Ok(v) = serde_json::from_str::<Value>(&text) {
            if let Some(errs) = v.get("errors") {
                return Err(format_errors_value(errs));
            }
        }
        return Err(format!(
            "API error HTTP {status}: {}",
            graphql_error_message(&text)
        ));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| {
        format!("Invalid JSON from API (HTTP {status}): {e}. {DEV_API_HINT}")
    })?;
    if let Some(errs) = v.get("errors").and_then(|x| x.as_array()) {
        if !errs.is_empty() {
            return Err(format_errors_value(&Value::Array(errs.clone())));
        }
    }
    let data = v
        .get("data")
        .cloned()
        .ok_or_else(|| "missing data".to_string())?;
    serde_json::from_value(data).map_err(|e| format!("{e}"))
}

/// Host for browser WebSocket connections.
/// `dx serve` on :8080 proxies HTTP to the Actix server on :8081, but its WS proxy is unreliable,
/// so in local dev we connect WebSockets directly to the backend port.
fn ws_host() -> String {
    let Some(window) = web_sys::window() else {
        return "localhost".to_string();
    };
    let location = window.location();
    let port = location.port().unwrap_or_default();
    if port == "8080" {
        return "127.0.0.1:8081".to_string();
    }
    location.host().unwrap_or_else(|_| "localhost".to_string())
}

pub fn graphql_ws_url() -> String {
    let Some(window) = web_sys::window() else {
        return "ws://localhost/graphql".to_string();
    };
    let location = window.location();
    let protocol = location.protocol().unwrap_or_default();
    let host = ws_host();
    let ws_protocol = if protocol == "https:" { "wss:" } else { "ws:" };
    let mut base = format!("{ws_protocol}//{host}/graphql");
    if let Some(token) = stored_session_token() {
        let enc = urlencoding::encode(&token);
        base.push_str("?token=");
        base.push_str(enc.as_ref());
    }
    base
}

pub fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

pub fn stored_session_token() -> Option<String> {
    local_storage()
        .and_then(|s| s.get_item(SESSION_TOKEN_KEY).ok().flatten())
        .filter(|x| !x.is_empty())
}

pub fn store_auth_session(session_token: &str, user_id: &str) {
    if let Some(st) = local_storage() {
        let _ = st.set_item(SESSION_TOKEN_KEY, session_token);
        let _ = st.set_item(USER_ID_KEY, user_id);
    }
}

pub fn clear_auth_session() {
    if let Some(st) = local_storage() {
        let _ = st.remove_item(SESSION_TOKEN_KEY);
        let _ = st.remove_item(USER_ID_KEY);
    }
}

pub fn stored_user_id() -> Option<String> {
    local_storage()
        .and_then(|s| s.get_item(USER_ID_KEY).ok().flatten())
        .filter(|x| !x.is_empty())
}

pub fn session_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok()?
}

pub fn store_active_overlay(overlay: &PlayOverlay) {
    if let Ok(json) = serde_json::to_string(overlay) {
        if let Some(st) = session_storage() {
            let _ = st.set_item(ACTIVE_GAME_KEY, &json);
        }
    }
}

pub fn stored_active_overlay() -> Option<PlayOverlay> {
    session_storage()
        .and_then(|s| s.get_item(ACTIVE_GAME_KEY).ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
}

pub fn clear_active_overlay() {
    if let Some(st) = session_storage() {
        let _ = st.remove_item(ACTIVE_GAME_KEY);
    }
}

pub const FRIENDS_PAGE_QUERY: &str = r#"query {
    myFriends { userId displayName avatarUrl online since }
    pendingFriendRequests { userId displayName avatarUrl createdAt }
    sentFriendRequests { userId displayName avatarUrl createdAt }
    pendingFriendRequestCount
    lobbies { id gameType status ownerDisplayName seatsFilled seatsTotal }
}"#;

pub async fn graphql_exec_anonymous<T: DeserializeOwned>(
    query: &str,
    variables: Option<Value>,
) -> Result<T, String> {
    if crate::stub::demo_mode::is_demo_mode() {
        return crate::stub::demo_api::demo_graphql(query, variables).await;
    }
    let mut body = serde_json::json!({ "query": query });
    if let Some(v) = variables {
        body["variables"] = v;
    }
    let resp = Request::post("/graphql")
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| format!("{e}"))?
        .send()
        .await
        .map_err(|e| format!("Cannot reach API: {e}. {DEV_API_HINT}"))?;
    parse_graphql_response(resp).await
}

pub async fn graphql_exec<T: DeserializeOwned>(
    query: &str,
    variables: Option<Value>,
) -> Result<T, String> {
    if crate::stub::demo_mode::is_demo_mode() {
        return crate::stub::demo_api::demo_graphql(query, variables).await;
    }
    let mut body = serde_json::json!({ "query": query });
    if let Some(v) = variables {
        body["variables"] = v;
    }
    let body_str = body.to_string();
    let mut req = Request::post("/graphql").header("Content-Type", "application/json");
    if let Some(ref token) = stored_session_token() {
        req = req.header("Authorization", &format!("Bearer {}", token));
    }
    let resp = req
        .body(body_str)
        .map_err(|e| format!("{e}"))?
        .send()
        .await
        .map_err(|e| format!("Cannot reach API: {e}. {DEV_API_HINT}"))?;
    parse_graphql_response(resp).await
}

pub async fn graphql_post<T: DeserializeOwned>(query: &str) -> Result<T, String> {
    graphql_exec(query, None).await
}

pub async fn reload_lobbies(
    mut s: Signal<Vec<crate::models::LobbySummary>>,
    mut err: Signal<Option<String>>,
) {
    match graphql_post::<LobbiesData>(LOBBIES_QUERY).await {
        Ok(p) => s.set(p.lobbies),
        Err(e) => err.set(Some(e)),
    }
}

pub async fn reload_games(
    mut s: Signal<Vec<crate::models::GameInfo>>,
    mut err: Signal<Option<String>>,
) {
    let q = r#"query { gameInstances { gameId gameType playerIdentities connectedPlayers } }"#;
    match graphql_post::<GamesListData>(q).await {
        Ok(g) => s.set(g.game_instances),
        Err(e) => err.set(Some(e)),
    }
}

pub fn get_ws_base() -> String {
    let Some(window) = web_sys::window() else {
        return "ws://localhost/game".to_string();
    };
    let location = window.location();
    let protocol = location.protocol().unwrap_or_default();
    let host = ws_host();
    let ws_protocol = if protocol == "https:" { "wss:" } else { "ws:" };
    format!("{}//{}/game", ws_protocol, host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_graphql_error_array_json() {
        let raw = r#"[{"locations":[{"column":34,"line":1}],"message":"cannot claim seat (taken, invalid index, or you already have another seat in this lobby)","path":["joinLobby"]}]"#;
        let msg = graphql_error_message(raw);
        assert!(msg.contains("seat is unavailable"));
        assert!(!msg.contains("locations"));
    }
}
