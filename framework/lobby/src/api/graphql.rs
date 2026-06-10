use crate::models::{GamesListData, LobbiesData, LOBBIES_QUERY, USER_ID_KEY};
use dioxus::prelude::*;
use gloo_net::http::{Request, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;

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
        return Err(format!("API error HTTP {status}: {text}"));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| {
        format!("Invalid JSON from API (HTTP {status}): {e}. {DEV_API_HINT}")
    })?;
    if let Some(errs) = v.get("errors").and_then(|x| x.as_array()) {
        if !errs.is_empty() {
            return Err(serde_json::to_string(errs).unwrap_or_else(|_| "GraphQL errors".into()));
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
    if let Some(id) = stored_user_id() {
        let enc = urlencoding::encode(&id);
        base.push_str("?token=");
        base.push_str(enc.as_ref());
    }
    base
}

pub fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

pub fn stored_user_id() -> Option<String> {
    local_storage()
        .and_then(|s| s.get_item(USER_ID_KEY).ok().flatten())
        .filter(|x| !x.is_empty())
}

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
    if let Some(ref id) = stored_user_id() {
        req = req.header("Authorization", &format!("Bearer {}", id));
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
