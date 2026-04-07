use base64::{engine::general_purpose::STANDARD, Engine};
use dioxus::events::{DragData, FormData};
use dioxus::html::{FileData, HasFileData};
use dioxus::prelude::*;
use futures_util::{SinkExt, StreamExt};
use gloo_events::{EventListener, EventListenerOptions};
use gloo_net::http::Request;
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::Message;
use gloo_timers::future::TimeoutFuture;
use js_sys::{Array, Object, Reflect};
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;

const CONFIG_MSG_SOURCE: &str = "ipel-game-config";
const CONFIG_RESULT_SOURCE: &str = "ipel-game-config-result";
const CONFIG_SCHEMA_SOURCE: &str = "ipel-game-config-schema";
const CONFIG_STATE_SOURCE: &str = "ipel-game-config-state";
const USER_ID_KEY: &str = "ipel_user_id";

fn parse_iframe_config_message(data: &wasm_bindgen::JsValue) -> Option<(String, String)> {
    let s = js_sys::JSON::stringify(data).ok()?.as_string()?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    if v.get("source").and_then(|x| x.as_str()) != Some(CONFIG_MSG_SOURCE) {
        return None;
    }
    let game = v.get("game")?.as_str()?.to_string();
    let config_val = v.get("config")?;
    let config_str = if let Some(s) = config_val.as_str() {
        s.to_string()
    } else {
        config_val.to_string()
    };
    Some((game, config_str))
}

fn post_message_to_source(event: &web_sys::MessageEvent, origin: &str, payload: &JsValue) {
    let Some(src) = event.source() else {
        return;
    };
    let Ok(win) = JsValue::from(src).dyn_into::<web_sys::Window>() else {
        return;
    };
    let _ = win.post_message(payload, origin);
}

fn config_validation_reply(
    event: &web_sys::MessageEvent,
    origin: &str,
    game: &str,
    ok: bool,
    errors: &[String],
) {
    let obj = Object::new();
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("source"),
        &JsValue::from_str(CONFIG_RESULT_SOURCE),
    );
    let _ = Reflect::set(&obj, &JsValue::from_str("game"), &JsValue::from_str(game));
    let _ = Reflect::set(&obj, &JsValue::from_str("ok"), &JsValue::from_bool(ok));
    let arr = Array::new();
    for e in errors {
        arr.push(&JsValue::from_str(e));
    }
    let _ = Reflect::set(&obj, &JsValue::from_str("errors"), &JsValue::from(arr));
    post_message_to_source(event, origin, &JsValue::from(obj));
}

fn post_config_schema_to_window(win: &web_sys::Window, origin: &str, game: &str, schema: &Value) {
    let Ok(schema_js) = js_sys::JSON::parse(&serde_json::to_string(schema).unwrap_or_else(|_| "{}".to_string()))
    else {
        return;
    };
    let obj = Object::new();
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("source"),
        &JsValue::from_str(CONFIG_SCHEMA_SOURCE),
    );
    let _ = Reflect::set(&obj, &JsValue::from_str("game"), &JsValue::from_str(game));
    let _ = Reflect::set(&obj, &JsValue::from_str("schema"), &schema_js);
    let _ = win.post_message(&JsValue::from(obj), origin);
}

fn post_config_state_to_window(win: &web_sys::Window, origin: &str, game: &str, config_json: &str) {
    let trimmed = config_json.trim();
    let config_js = if trimmed.is_empty() {
        JsValue::NULL
    } else {
        js_sys::JSON::parse(trimmed).unwrap_or(JsValue::NULL)
    };
    let obj = Object::new();
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("source"),
        &JsValue::from_str(CONFIG_STATE_SOURCE),
    );
    let _ = Reflect::set(&obj, &JsValue::from_str("game"), &JsValue::from_str(game));
    let _ = Reflect::set(&obj, &JsValue::from_str("config"), &config_js);
    let _ = win.post_message(&JsValue::from(obj), origin);
}

fn graphql_ws_url() -> String {
    let Some(window) = web_sys::window() else {
        return "ws://localhost/graphql".to_string();
    };
    let location = window.location();
    let protocol = location.protocol().unwrap_or_default();
    let host = location.host().unwrap_or_default();
    let ws_protocol = if protocol == "https:" { "wss:" } else { "ws:" };
    let mut base = format!("{ws_protocol}//{host}/graphql");
    if let Some(id) = stored_user_id() {
        let enc = urlencoding::encode(&id);
        base.push_str("?token=");
        base.push_str(enc.as_ref());
    }
    base
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn stored_user_id() -> Option<String> {
    local_storage()
        .and_then(|s| s.get_item(USER_ID_KEY).ok().flatten())
        .filter(|x| !x.is_empty())
}

async fn graphql_exec_anonymous<T: DeserializeOwned>(query: &str, variables: Option<Value>) -> Result<T, String> {
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
        .map_err(|e| format!("{e}"))?;
    let text = resp.text().await.map_err(|e| format!("{e}"))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{e}"))?;
    if let Some(errs) = v.get("errors").and_then(|x| x.as_array()) {
        if !errs.is_empty() {
            return Err(serde_json::to_string(errs).unwrap_or_else(|_| "GraphQL errors".into()));
        }
    }
    let data = v.get("data").cloned().ok_or_else(|| "missing data".to_string())?;
    serde_json::from_value(data).map_err(|e| format!("{e}"))
}

async fn graphql_exec<T: DeserializeOwned>(query: &str, variables: Option<Value>) -> Result<T, String> {
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
        .map_err(|e| format!("{e}"))?;
    let text = resp.text().await.map_err(|e| format!("{e}"))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{e}"))?;
    if let Some(errs) = v.get("errors").and_then(|x| x.as_array()) {
        if !errs.is_empty() {
            return Err(serde_json::to_string(errs).unwrap_or_else(|_| "GraphQL errors".into()));
        }
    }
    let data = v.get("data").cloned().ok_or_else(|| "missing data".to_string())?;
    serde_json::from_value(data).map_err(|e| format!("{e}"))
}

async fn graphql_post<T: DeserializeOwned>(query: &str) -> Result<T, String> {
    graphql_exec(query, None).await
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameTypeInfo {
    name: String,
    display_name: String,
    version: String,
    min_players: u32,
    max_players: u32,
    description: String,
    #[serde(default)]
    config_ui_path: Option<String>,
    #[serde(default)]
    about_ui_path: Option<String>,
    #[serde(default)]
    config_schema_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameInfo {
    game_id: String,
    game_type: String,
    player_identities: Vec<String>,
    connected_players: usize,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameResultRow {
    game_id: String,
    game_type: String,
    lobby_id: Option<String>,
    finished_at: i64,
    result_json: String,
    player_scores_json: String,
    seats_snapshot_json: String,
    #[serde(default)]
    result_ui_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct LoadedGameResult {
    row: GameResultRow,
    iframe_src: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecentFinishedRow {
    game_id: String,
    game_type: String,
    finished_at: i64,
    player_scores_json: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LobbySummary {
    id: String,
    game_type: String,
    status: String,
    seats_filled: i32,
    seats_total: i32,
    owner_display_name: String,
    #[serde(default)]
    game_instance_id: Option<String>,
    created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LobbySeat {
    seat_index: i32,
    player_identity: String,
    #[serde(default)]
    claimed_by_user_id: Option<String>,
    #[serde(default)]
    claimed_display_name: Option<String>,
    #[serde(default)]
    ready: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LobbyMessage {
    id: String,
    user_id: String,
    display_name: String,
    body: String,
    created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LobbyDetail {
    id: String,
    owner_user_id: String,
    owner_display_name: String,
    game_type: String,
    #[serde(default)]
    config_json: Option<String>,
    status: String,
    #[serde(default)]
    game_instance_id: Option<String>,
    created_at: i64,
    updated_at: i64,
    seats: Vec<LobbySeat>,
    #[serde(default)]
    messages: Vec<LobbyMessage>,
}

#[derive(Clone, Debug)]
struct PlayOverlay {
    game_type: String,
    game_id: String,
    player: String,
    return_lobby_id: Option<String>,
}

#[derive(Clone, Copy)]
struct AppShellContext {
    playing: Signal<Option<PlayOverlay>>,
    error_msg: Signal<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Routable)]
#[rustfmt::skip]
pub enum LobbyRoute {
    #[layout(OverlayLayout)]
    #[route("/", HomePageRoute)]
    Home {},
    #[route("/lobby/:id", LobbyRoomRoute)]
    Lobby { id: String },
    #[route("/game/:id", GameResultRoute)]
    GameResult { id: String },
    #[route("/developer/uploads", DeveloperUploadsRoute)]
    DeveloperUploads {},
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterUserRow {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterUserData {
    register_user: RegisterUserRow,
}

const LOBBIES_QUERY: &str = r#"query { lobbies { id gameType status seatsFilled seatsTotal ownerDisplayName gameInstanceId createdAt } }"#;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LobbiesData {
    lobbies: Vec<LobbySummary>,
}

async fn reload_lobbies(mut s: Signal<Vec<LobbySummary>>, mut err: Signal<Option<String>>) {
    match graphql_post::<LobbiesData>(LOBBIES_QUERY).await {
        Ok(p) => s.set(p.lobbies),
        Err(e) => err.set(Some(e)),
    }
}

/// `graphql-ws` in browsers maps to Apollo **subscriptions-transport-ws**, which uses `type: "data"`.
/// The newer **graphql-transport-ws** subprotocol uses `type: "next"`. async_graphql supports both.
fn gql_ws_is_subscription_result(ty: Option<&str>) -> bool {
    matches!(ty, Some("data") | Some("next"))
}

/// Subscription result messages may include `errors` without usable `data`.
fn gql_ws_payload_data(msg: &Value) -> Option<&Value> {
    let payload = msg.get("payload")?;
    if let Some(errs) = payload.get("errors").and_then(|e| e.as_array()) {
        if !errs.is_empty() {
            return None;
        }
    }
    let data = payload.get("data")?;
    if data.is_null() {
        return None;
    }
    Some(data)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GamesListData {
    game_instances: Vec<GameInfo>,
}

async fn reload_games(mut s: Signal<Vec<GameInfo>>, mut err: Signal<Option<String>>) {
    let q = r#"query { gameInstances { gameId gameType playerIdentities connectedPlayers } }"#;
    match graphql_post::<GamesListData>(q).await {
        Ok(g) => s.set(g.game_instances),
        Err(e) => err.set(Some(e)),
    }
}

fn start_game_instances_subscription(mut games: Signal<Vec<GameInfo>>) {
    spawn(async move {
        let url = graphql_ws_url();
        let Ok(mut ws) = WebSocket::open_with_protocol(&url, "graphql-ws") else {
            return;
        };
        if ws
            .send(Message::Text(r#"{"type":"connection_init"}"#.into()))
            .await
            .is_err()
        {
            return;
        }
        let mut acked = false;
        while let Some(msg) = ws.next().await {
            let Ok(msg) = msg else { break };
            let text = match msg {
                Message::Text(t) => t,
                _ => continue,
            };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            match v.get("type").and_then(|x| x.as_str()) {
                Some("connection_ack") => {
                    acked = true;
                    break;
                }
                Some("connection_error") => return,
                _ if !acked => continue,
                _ => {}
            }
        }
        if !acked {
            return;
        }
        let sub = serde_json::json!({
            "type": "start",
            "id": "games1",
            "payload": {
                "query": "subscription { gameInstancesUpdated { gameId gameType playerIdentities connectedPlayers } }"
            }
        });
        if ws.send(Message::Text(sub.to_string())).await.is_err() {
            return;
        }
        while let Some(msg) = ws.next().await {
            let Ok(msg) = msg else { break };
            let text = match msg {
                Message::Text(t) => t,
                _ => continue,
            };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            if !gql_ws_is_subscription_result(v.get("type").and_then(|x| x.as_str())) {
                continue;
            }
            let Some(data) = gql_ws_payload_data(&v) else {
                continue;
            };
            let Some(raw) = data.get("gameInstancesUpdated").cloned() else {
                continue;
            };
            if let Ok(list) = serde_json::from_value::<Vec<GameInfo>>(raw) {
                games.set(list);
            }
        }
    });
}

fn start_lobbies_subscription(mut list: Signal<Vec<LobbySummary>>, mut err: Signal<Option<String>>) {
    spawn(async move {
        let url = graphql_ws_url();
        let Ok(mut ws) = WebSocket::open_with_protocol(&url, "graphql-ws") else {
            return;
        };
        if ws
            .send(Message::Text(r#"{"type":"connection_init"}"#.into()))
            .await
            .is_err()
        {
            return;
        }
        let mut acked = false;
        while let Some(msg) = ws.next().await {
            let Ok(msg) = msg else { break };
            let text = match msg {
                Message::Text(t) => t,
                _ => continue,
            };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            match v.get("type").and_then(|x| x.as_str()) {
                Some("connection_ack") => {
                    acked = true;
                    break;
                }
                Some("connection_error") => return,
                _ if !acked => continue,
                _ => {}
            }
        }
        if !acked {
            return;
        }
        let sub = serde_json::json!({
            "type": "start",
            "id": "lobbies1",
            "payload": {
                "query": "subscription { lobbiesUpdated { id gameType status seatsFilled seatsTotal ownerDisplayName gameInstanceId createdAt } }"
            }
        });
        if ws.send(Message::Text(sub.to_string())).await.is_err() {
            return;
        }
        while let Some(msg) = ws.next().await {
            let Ok(msg) = msg else { break };
            let text = match msg {
                Message::Text(t) => t,
                _ => continue,
            };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            if !gql_ws_is_subscription_result(v.get("type").and_then(|x| x.as_str())) {
                continue;
            }
            let Some(data) = gql_ws_payload_data(&v) else {
                let mut l = list;
                let mut e = err;
                spawn(async move {
                    reload_lobbies(l, e).await;
                });
                continue;
            };
            let Some(raw) = data.get("lobbiesUpdated").cloned() else {
                let mut l = list;
                let mut e = err;
                spawn(async move {
                    reload_lobbies(l, e).await;
                });
                continue;
            };
            match serde_json::from_value::<Vec<LobbySummary>>(raw) {
                Ok(rows) => list.set(rows),
                Err(_) => {
                    let mut l = list;
                    let mut e = err;
                    spawn(async move {
                        reload_lobbies(l, e).await;
                    });
                }
            }
        }
    });
}

fn start_lobby_room_subscription(
    lobby_id: String,
    mut detail: Signal<Option<LobbyDetail>>,
    mut err: Signal<Option<String>>,
) {
    spawn(async move {
        let url = graphql_ws_url();
        let Ok(mut ws) = WebSocket::open_with_protocol(&url, "graphql-ws") else {
            return;
        };
        if ws
            .send(Message::Text(r#"{"type":"connection_init"}"#.into()))
            .await
            .is_err()
        {
            return;
        }
        let mut acked = false;
        while let Some(msg) = ws.next().await {
            let Ok(msg) = msg else { break };
            let text = match msg {
                Message::Text(t) => t,
                _ => continue,
            };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            match v.get("type").and_then(|x| x.as_str()) {
                Some("connection_ack") => {
                    acked = true;
                    break;
                }
                Some("connection_error") => return,
                _ if !acked => continue,
                _ => {}
            }
        }
        if !acked {
            return;
        }
        let q = r#"subscription L($id: ID!) { lobbyUpdated(id: $id) { id ownerUserId ownerDisplayName gameType configJson status gameInstanceId createdAt updatedAt seats { seatIndex playerIdentity claimedByUserId claimedDisplayName ready } messages { id userId displayName body createdAt } } }"#;
        let sub = serde_json::json!({
            "type": "start",
            "id": "room1",
            "payload": {
                "query": q,
                "variables": { "id": lobby_id }
            }
        });
        if ws.send(Message::Text(sub.to_string())).await.is_err() {
            return;
        }
        while let Some(msg) = ws.next().await {
            let Ok(msg) = msg else { break };
            let text = match msg {
                Message::Text(t) => t,
                _ => continue,
            };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            if !gql_ws_is_subscription_result(v.get("type").and_then(|x| x.as_str())) {
                continue;
            }
            let fetch_room = {
                let lid = lobby_id.clone();
                let mut d = detail;
                let mut e = err;
                move || {
                    spawn(async move {
                        let q = r#"query L($id: ID!) { lobby(id: $id) { id ownerUserId ownerDisplayName gameType configJson status gameInstanceId createdAt updatedAt seats { seatIndex playerIdentity claimedByUserId claimedDisplayName ready } messages { id userId displayName body createdAt } } }"#;
                        let vars = serde_json::json!({ "id": lid });
                        #[derive(Deserialize)]
                        #[serde(rename_all = "camelCase")]
                        struct Ld {
                            lobby: Option<LobbyDetail>,
                        }
                        match graphql_exec::<Ld>(q, Some(vars)).await {
                            Ok(x) => d.set(x.lobby),
                            Err(msg) => e.set(Some(msg)),
                        }
                    });
                }
            };
            let Some(data) = gql_ws_payload_data(&v) else {
                fetch_room();
                continue;
            };
            let Some(raw) = data.get("lobbyUpdated").cloned() else {
                fetch_room();
                continue;
            };
            match serde_json::from_value::<LobbyDetail>(raw) {
                Ok(d) => detail.set(Some(d)),
                Err(_) => fetch_room(),
            }
        }
    });
}

fn get_ws_base() -> String {
    let Some(window) = web_sys::window() else {
        return "ws://localhost/game".to_string();
    };
    let location = window.location();
    let protocol = location.protocol().unwrap_or_default();
    let host = location.host().unwrap_or_default();
    let ws_protocol = if protocol == "https:" { "wss:" } else { "ws:" };
    format!("{}//{}/game", ws_protocol, host)
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn HomePageRoute() -> Element {
    let shell = use_context::<AppShellContext>();
    rsx! {
        HomePage {
            playing: shell.playing,
            error_msg: shell.error_msg,
        }
    }
}

#[component]
fn LobbyRoomRoute(id: String) -> Element {
    let shell = use_context::<AppShellContext>();
    rsx! {
        LobbyRoomPage {
            key: "{id}",
            lobby_id: id,
            playing: shell.playing,
            error_msg: shell.error_msg,
        }
    }
}

#[component]
fn GameResultRoute(id: String) -> Element {
    rsx! {
        GameResultPage {
            key: "{id}",
            game_id: id,
        }
    }
}

#[component]
fn DeveloperUploadsRoute() -> Element {
    rsx! {
        DeveloperUploadsPage {}
    }
}

#[component]
fn OverlayLayout() -> Element {
    let mut shell = use_context::<AppShellContext>();
    let nav = use_navigator();
    rsx! {
        Outlet::<LobbyRoute> {}
        if let Some(p) = (shell.playing)() {
            GamePlayer {
                game_type: p.game_type.clone(),
                game_id: p.game_id.clone(),
                player: p.player.clone(),
                return_lobby_id: p.return_lobby_id.clone(),
                on_close: move |_| {
                    shell.playing.set(None);
                },
                on_navigate_lobby: move |id: String| {
                    nav.push(LobbyRoute::Lobby { id });
                },
            }
        }
    }
}

#[component]
fn AuthedShell(playing: Signal<Option<PlayOverlay>>, error_msg: Signal<Option<String>>) -> Element {
    use_context_provider(|| AppShellContext { playing, error_msg });
    rsx! {
        Router::<LobbyRoute> {}
    }
}

#[component]
fn App() -> Element {
    let mut session_ok: Signal<bool> = use_signal(|| false);
    let mut session_checked: Signal<bool> = use_signal(|| false);
    let mut playing: Signal<Option<PlayOverlay>> = use_signal(|| None);
    let error_msg: Signal<Option<String>> = use_signal(|| None);

    use_effect(move || {
        let mut session_ok = session_ok;
        let mut session_checked = session_checked;
        let mut error_msg = error_msg;
        spawn(async move {
            if stored_user_id().is_none() {
                session_ok.set(false);
                session_checked.set(true);
                return;
            }
            let id = stored_user_id().unwrap();
            let q = r#"query UserExists($id: ID!) { user(id: $id) { id } }"#;
            let vars = serde_json::json!({ "id": id });
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct UserExists {
                user: Option<RegisterUserRow>,
            }
            match graphql_exec_anonymous::<UserExists>(q, Some(vars)).await {
                Ok(p) if p.user.is_some() => session_ok.set(true),
                _ => {
                    if let Some(st) = local_storage() {
                        let _ = st.remove_item(USER_ID_KEY);
                    }
                    session_ok.set(false);
                }
            }
            session_checked.set(true);
            if let Some(e) = error_msg() {
                let _ = e;
            }
            error_msg.set(None);
        });
    });

    rsx! {
        document::Stylesheet {
            href: asset!("/assets/tailwind.css"),
        }
        div { class: "min-h-screen bg-gradient-to-b from-gray-950 via-gray-900 to-indigo-950/35 text-white",
            if !session_checked() {
                div { class: "flex flex-col items-center justify-center min-h-[50vh] gap-3",
                    p { class: "text-sm font-medium text-indigo-200/90", "Checking session…" }
                    p { class: "text-xs text-gray-500", "Hang tight" }
                }
            } else if !session_ok() {
                AuthGate {
                    on_ready: move |_| {
                        session_ok.set(true);
                        session_checked.set(true);
                    }
                }
            } else {
                AuthedShell {
                    playing,
                    error_msg,
                }
            }
        }
    }
}

#[component]
fn AuthGate(on_ready: EventHandler<()>) -> Element {
    let mut guest_name = use_signal(|| "Guest".to_string());
    let mut signup_name = use_signal(|| String::new());
    let mut signup_pass = use_signal(|| String::new());
    let mut login_name = use_signal(|| String::new());
    let mut login_pass = use_signal(|| String::new());
    let mut err = use_signal(|| None::<String>);

    rsx! {
        div { class: "max-w-lg mx-auto px-4 py-12 sm:py-20",
            div { class: "text-center mb-10",
                h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight text-white", "Welcome" }
                p { class: "mt-2 text-sm text-indigo-200/70",
                    "Guest, sign up, or log in to join lobbies."
                }
            }
            if let Some(e) = err() {
                div { class: "rounded-xl border border-red-500/50 bg-red-950/45 px-4 py-3 mb-6 shadow-lg shadow-red-900/20",
                    p { class: "text-sm font-medium text-red-100", "{e}" }
                }
            }
            div { class: "space-y-6",
                div { class: "rounded-2xl border border-indigo-500/25 bg-gray-900/60 p-5 sm:p-6 shadow-xl shadow-black/30 backdrop-blur-sm",
                    div { class: "flex items-center gap-2 mb-4",
                        span { class: "flex h-8 w-8 items-center justify-center rounded-lg bg-indigo-600 text-sm", "→" }
                        h2 { class: "font-semibold text-white", "Continue as guest" }
                    }
                    input {
                        class: "w-full px-3 py-2.5 bg-gray-950/80 border border-gray-600 rounded-xl text-sm text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/50 mb-3",
                        placeholder: "Display name",
                        value: "{guest_name}",
                        oninput: move |e| guest_name.set(e.value()),
                    }
                    button {
                        class: "w-full px-4 py-2.5 rounded-xl bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-white text-sm font-semibold shadow-lg shadow-indigo-900/30",
                        onclick: move |_| {
                            let n = guest_name();
                            spawn(async move {
                                let q = "mutation G($n: String!) { registerUser(displayName: $n) { id } }";
                                let vars = serde_json::json!({ "n": n });
                                match graphql_exec_anonymous::<RegisterUserData>(q, Some(vars)).await {
                                    Ok(data) => {
                                        if let Some(st) = local_storage() {
                                            let _ = st.set_item(USER_ID_KEY, &data.register_user.id);
                                        }
                                        on_ready.call(());
                                    }
                                    Err(e) => err.set(Some(e)),
                                }
                            });
                        },
                        "Continue"
                    }
                }
                div { class: "rounded-2xl border border-emerald-500/25 bg-gray-900/60 p-5 sm:p-6 shadow-xl shadow-black/30 backdrop-blur-sm",
                    div { class: "flex items-center gap-2 mb-4",
                        span { class: "flex h-8 w-8 items-center justify-center rounded-lg bg-emerald-600 text-sm font-bold", "+" }
                        h2 { class: "font-semibold text-white", "Sign up" }
                    }
                    input {
                        class: "w-full px-3 py-2.5 bg-gray-950/80 border border-gray-600 rounded-xl text-sm text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/40 mb-2",
                        placeholder: "Display name",
                        value: "{signup_name}",
                        oninput: move |e| signup_name.set(e.value()),
                    }
                    input {
                        class: "w-full px-3 py-2.5 bg-gray-950/80 border border-gray-600 rounded-xl text-sm text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/40 mb-3",
                        r#type: "password",
                        placeholder: "Password (min 4 chars)",
                        value: "{signup_pass}",
                        oninput: move |e| signup_pass.set(e.value()),
                    }
                    button {
                        class: "w-full px-4 py-2.5 rounded-xl bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-white text-sm font-semibold shadow-lg shadow-emerald-900/25",
                        onclick: move |_| {
                            let n = signup_name();
                            let p = signup_pass();
                            spawn(async move {
                                let q = "mutation SignUp($n: String!, $p: String!) { signUp(displayName: $n, password: $p) { id } }";
                                let vars = serde_json::json!({ "n": n, "p": p });
                                #[derive(Deserialize)]
                                #[serde(rename_all = "camelCase")]
                                struct Wrap {
                                    #[serde(rename = "signUp")]
                                    row: RegisterUserRow,
                                }
                                match graphql_exec_anonymous::<Wrap>(q, Some(vars)).await {
                                    Ok(w) => {
                                        if let Some(st) = local_storage() {
                                            let _ = st.set_item(USER_ID_KEY, &w.row.id);
                                        }
                                        on_ready.call(());
                                    }
                                    Err(e) => err.set(Some(e)),
                                }
                            });
                        },
                        "Create account"
                    }
                }
                div { class: "rounded-2xl border border-gray-600/60 bg-gray-900/60 p-5 sm:p-6 shadow-xl shadow-black/30 backdrop-blur-sm",
                    div { class: "flex items-center gap-2 mb-4",
                        span { class: "flex h-8 w-8 items-center justify-center rounded-lg bg-gray-700 text-sm", "⌘" }
                        h2 { class: "font-semibold text-white", "Log in" }
                    }
                    input {
                        class: "w-full px-3 py-2.5 bg-gray-950/80 border border-gray-600 rounded-xl text-sm text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-gray-500/40 mb-2",
                        placeholder: "Display name",
                        value: "{login_name}",
                        oninput: move |e| login_name.set(e.value()),
                    }
                    input {
                        class: "w-full px-3 py-2.5 bg-gray-950/80 border border-gray-600 rounded-xl text-sm text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-gray-500/40 mb-3",
                        r#type: "password",
                        placeholder: "Password",
                        value: "{login_pass}",
                        oninput: move |e| login_pass.set(e.value()),
                    }
                    button {
                        class: "w-full px-4 py-2.5 rounded-xl border border-gray-500/50 bg-gray-800 hover:bg-gray-700 text-white text-sm font-semibold transition-colors",
                        onclick: move |_| {
                            let n = login_name();
                            let p = login_pass();
                            spawn(async move {
                                let q = "mutation Login($n: String!, $p: String!) { loginWithPassword(displayName: $n, password: $p) { id } }";
                                let vars = serde_json::json!({ "n": n, "p": p });
                                #[derive(Deserialize)]
                                #[serde(rename_all = "camelCase")]
                                struct LoginWrap {
                                    #[serde(rename = "loginWithPassword")]
                                    user: RegisterUserRow,
                                }
                                match graphql_exec_anonymous::<LoginWrap>(q, Some(vars)).await {
                                    Ok(l) => {
                                        if let Some(st) = local_storage() {
                                            let _ = st.set_item(USER_ID_KEY, &l.user.id);
                                        }
                                        let _ = web_sys::window().unwrap().location().reload();
                                    }
                                    Err(e) => err.set(Some(e)),
                                }
                            });
                        },
                        "Log in"
                    }
                }
            }
        }
    }
}

#[component]
fn HomePage(playing: Signal<Option<PlayOverlay>>, mut error_msg: Signal<Option<String>>) -> Element {
    let nav = use_navigator();
    let game_types: Signal<Vec<GameTypeInfo>> = use_signal(Vec::new);
    let games: Signal<Vec<GameInfo>> = use_signal(Vec::new);
    let lobbies: Signal<Vec<LobbySummary>> = use_signal(Vec::new);
    let recent_finished: Signal<Vec<RecentFinishedRow>> = use_signal(Vec::new);
    let loading = use_signal(|| true);
    let mut creating = use_signal(|| false);

    let refresh_games = {
        let mut games = games;
        let mut error_msg = error_msg;
        move || {
            spawn(async move {
                reload_games(games, error_msg).await;
            });
        }
    };

    let refresh_lobbies = {
        let mut lobbies = lobbies;
        let mut error_msg = error_msg;
        move || {
            spawn(async move {
                reload_lobbies(lobbies, error_msg).await;
            });
        }
    };

    // One-shot bootstrap + WS subscriptions (avoid use_effect re-runs stacking connections).
    use_hook(move || {
        let mut game_types = game_types;
        let mut games = games;
        let mut lobbies = lobbies;
        let mut recent_finished = recent_finished;
        let mut error_msg = error_msg;
        let mut loading = loading;
        start_game_instances_subscription(games);
        start_lobbies_subscription(lobbies, error_msg);
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Boot {
                game_types: Vec<GameTypeInfo>,
                game_instances: Vec<GameInfo>,
                lobbies: Vec<LobbySummary>,
                recent_finished_games: Vec<RecentFinishedRow>,
            }
            let q = r#"query {
                gameTypes { name displayName version minPlayers maxPlayers description configUiPath aboutUiPath configSchemaJson }
                gameInstances { gameId gameType playerIdentities connectedPlayers }
                lobbies { id gameType status seatsFilled seatsTotal ownerDisplayName gameInstanceId createdAt }
                recentFinishedGames(limit: 12) { gameId gameType finishedAt playerScoresJson }
            }"#;
            match graphql_post::<Boot>(q).await {
                Ok(data) => {
                    game_types.set(data.game_types);
                    games.set(data.game_instances);
                    lobbies.set(data.lobbies);
                    recent_finished.set(data.recent_finished_games);
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    rsx! {
        div { class: "max-w-5xl mx-auto px-4 sm:px-6 py-8 sm:py-10 space-y-8",
            div { class: "flex flex-col sm:flex-row sm:items-end sm:justify-between gap-6",
                div {
                    h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight text-white", "Game lobby" }
                    p { class: "mt-2 text-sm text-indigo-200/75 max-w-lg",
                        "Create a room, then the owner picks a game and options. Join a lobby from the list below."
                    }
                }
                button {
                    class: "shrink-0 inline-flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-white text-sm font-semibold shadow-lg shadow-emerald-900/25",
                    onclick: move |_| {
                        nav.push(LobbyRoute::DeveloperUploads {});
                    },
                    "Developer uploads"
                }
            }
            if let Some(err) = error_msg() {
                div { class: "rounded-xl border border-red-500/50 bg-red-950/45 px-4 py-3 shadow-lg shadow-red-900/20",
                    p { class: "text-sm font-medium text-red-100", "{err}" }
                }
            }
            if loading() {
                div { class: "flex flex-col items-center justify-center py-16 gap-2",
                    p { class: "text-sm font-medium text-indigo-200/80", "Loading…" }
                    p { class: "text-xs text-gray-500", "Fetching game types and lobbies" }
                }
            } else {
                section { class: "rounded-2xl border border-indigo-500/20 bg-gray-900/50 p-5 sm:p-6 shadow-xl shadow-black/30 backdrop-blur-sm",
                    div { class: "flex items-center gap-2 mb-1",
                        span { class: "flex h-8 w-8 items-center justify-center rounded-lg bg-indigo-600 text-sm", "+" }
                        h2 { class: "text-lg font-semibold text-white", "Create lobby" }
                    }
                    p { class: "text-gray-400 text-sm mb-4",
                        "Opens an empty lobby. Only you (the owner) can choose the game and settings inside the room."
                    }
                    button {
                        class: "px-4 py-2.5 rounded-xl bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-sm font-semibold text-white shadow-lg shadow-indigo-900/30 disabled:opacity-40 disabled:shadow-none",
                        disabled: creating(),
                        onclick: move |_| {
                            creating.set(true);
                            let nav = nav;
                            spawn(async move {
                                let q = "mutation { createLobby { id } }";
                                #[derive(Deserialize)]
                                #[serde(rename_all = "camelCase")]
                                struct Cr { create_lobby: RegisterUserRow }
                                match graphql_exec::<Cr>(q, None).await {
                                    Ok(c) => {
                                        nav.push(LobbyRoute::Lobby {
                                            id: c.create_lobby.id,
                                        });
                                    }
                                    Err(e) => {
                                        let _ = web_sys::window().unwrap().alert_with_message(&e);
                                    }
                                }
                                creating.set(false);
                            });
                        },
                        if creating() { "Creating…" } else { "Create lobby" }
                    }
                }
                section { class: "rounded-2xl border border-gray-700/70 bg-gray-900/40 p-5 sm:p-6 shadow-lg shadow-black/25 backdrop-blur-sm",
                    div { class: "mb-4",
                        h2 { class: "text-lg font-semibold text-white", "Games on this server" }
                        p { class: "text-sm text-gray-500 mt-1",
                            "Same look as live tables — these are the game types you can run in a lobby (owner picks one in the room)."
                        }
                    }
                    if game_types().is_empty() {
                        p { class: "text-sm text-gray-500", "No games published yet. Developers can upload builds from Developer uploads." }
                    }
                    div { class: "space-y-3",
                        for gt in game_types() {
                            GameTypeCatalogCard { gt: gt.clone() }
                        }
                    }
                }
                section { class: "rounded-2xl border border-gray-700/70 bg-gray-900/40 p-5 sm:p-6 shadow-lg shadow-black/25 backdrop-blur-sm",
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-lg font-semibold text-white", "Lobbies" }
                        button {
                            class: "px-3 py-1.5 rounded-lg border border-gray-600 bg-gray-800/80 hover:bg-gray-700 text-sm text-gray-200",
                            onclick: move |_| refresh_lobbies(),
                            "Refresh"
                        }
                    }
                    if lobbies().is_empty() {
                        p { class: "text-sm text-gray-500", "No active lobbies yet." }
                    }
                    div { class: "space-y-3",
                        for lob in lobbies() {
                            {
                                let types = game_types();
                                let title = game_type_display_title(&types, &lob.game_type);
                                let desc = game_type_description(&types, &lob.game_type);
                                let lid_open = lob.id.clone();
                                rsx! {
                            div { class: "rounded-xl border border-gray-600/60 bg-gray-950/50 p-4 flex justify-between items-center gap-3 shadow-md",
                                div { class: "min-w-0",
                                    p { class: "font-medium text-white", "{title}" }
                                    if let Some(ref d) = desc {
                                        p { class: "text-xs text-gray-500 mt-1 line-clamp-2", "{d}" }
                                    }
                                    p { class: "text-xs text-gray-400 mt-1",
                                        "{lob.owner_display_name} · {lob.seats_filled}/{lob.seats_total} · {lob.status}"
                                    }
                                }
                                button {
                                    class: "px-3 py-1.5 rounded-lg bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-sm font-medium text-white shrink-0 shadow-md shadow-indigo-900/20",
                                    onclick: move |_| {
                                        nav.push(LobbyRoute::Lobby { id: lid_open.clone() });
                                    },
                                    "Open"
                                }
                            }
                                }
                            }
                        }
                    }
                }
                section { class: "rounded-2xl border border-gray-700/70 bg-gray-900/40 p-5 sm:p-6 shadow-lg shadow-black/25 backdrop-blur-sm",
                    h2 { class: "text-lg font-semibold text-white mb-4", "Recent results" }
                    if recent_finished().is_empty() {
                        p { class: "text-sm text-gray-500", "No finished games recorded yet." }
                    }
                    div { class: "space-y-2",
                        for it in recent_finished() {
                            {
                                let gid = it.game_id.clone();
                                let gt = it.game_type.clone();
                                let sc = it.player_scores_json.clone();
                                rsx! {
                                    div { class: "rounded-xl border border-gray-600/50 bg-gray-950/40 p-3 flex justify-between gap-2 items-start",
                                        div { class: "min-w-0",
                                            p { class: "font-medium text-gray-100", "{gt}" }
                                            p { class: "text-xs text-gray-500 font-mono break-all mt-1", "{sc}" }
                                        }
                                        button {
                                            class: "px-2.5 py-1 rounded-lg bg-indigo-600/90 hover:bg-indigo-500 text-xs font-medium text-white shrink-0",
                                            onclick: move |_| {
                                                nav.push(LobbyRoute::GameResult { id: gid.clone() });
                                            },
                                            "Details"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                section { class: "rounded-2xl border border-gray-700/70 bg-gray-900/40 p-5 sm:p-6 shadow-lg shadow-black/25 backdrop-blur-sm",
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-lg font-semibold text-white", "Active games" }
                        button {
                            class: "px-3 py-1.5 rounded-lg border border-gray-600 bg-gray-800/80 hover:bg-gray-700 text-sm text-gray-200",
                            onclick: move |_| refresh_games(),
                            "Refresh"
                        }
                    }
                    if games().is_empty() {
                        p { class: "text-sm text-gray-500", "No active games." }
                    }
                    div { class: "space-y-3",
                        for game in games() {
                            GameCard {
                                game: game.clone(),
                                catalog: game_types(),
                                playing,
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Chat state is isolated so typing does not re-render the config iframe column.
#[component]
fn LobbyChatPanel(lobby_id: String, messages: Vec<LobbyMessage>) -> Element {
    let mut draft = use_signal(|| String::new());
    let lid_send = lobby_id.clone();
    rsx! {
        h4 { class: "text-xs font-semibold uppercase tracking-wide text-indigo-300/80 mt-6 mb-2", "Chat" }
        div { class: "lobby-chat-messages",
            for m in messages.iter().rev().take(40).rev() {
                p {
                    key: "{m.id}",
                    class: "text-xs text-gray-200 mb-1.5 leading-relaxed",
                    span { class: "text-indigo-300/90 font-medium", "{m.display_name}: " }
                    "{m.body}"
                }
            }
        }
        textarea {
            class: "w-full mt-2 px-3 py-2 bg-gray-950/80 border border-gray-600 rounded-xl text-xs text-white placeholder-gray-500 min-h-[3.25rem] focus:outline-none focus:ring-2 focus:ring-indigo-500/35",
            placeholder: "Message…",
            value: "{draft()}",
            oninput: move |e| draft.set(e.value()),
        }
        button {
            class: "mt-2 px-3 py-1.5 rounded-lg bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-xs font-semibold text-white shadow-md shadow-indigo-900/25",
            onclick: move |_| {
                let body = draft();
                if body.trim().is_empty() { return; }
                let lid = lid_send.clone();
                spawn(async move {
                    let q = "mutation P($id: ID!, $b: String!) { postLobbyMessage(lobbyId: $id, body: $b) { id } }";
                    let vars = serde_json::json!({ "id": lid, "b": body });
                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                });
                draft.set(String::new());
            },
            "Send"
        }
    }
}

#[component]
fn GameCard(
    game: GameInfo,
    catalog: Vec<GameTypeInfo>,
    mut playing: Signal<Option<PlayOverlay>>,
) -> Element {
    let game_type = game.game_type.clone();
    let title = game_type_display_title(&catalog, &game.game_type);
    let desc = game_type_description(&catalog, &game.game_type);
    rsx! {
        div { class: "rounded-xl border border-emerald-500/20 bg-gray-950/50 p-4 flex items-center justify-between flex-wrap gap-3 shadow-md",
            div { class: "min-w-0",
                p { class: "font-medium text-white",
                    "{title}"
                    span { class: "text-gray-500 text-sm ml-2 font-mono", "{game.game_id}" }
                }
                if let Some(ref d) = desc {
                    p { class: "text-xs text-gray-500 mt-1 line-clamp-2", "{d}" }
                }
                p { class: "text-xs text-gray-400 mt-1",
                    "Connected: {game.connected_players} / {game.player_identities.len()}"
                }
            }
            div { class: "flex flex-wrap gap-2",
                for identity in game.player_identities.clone() {
                    {
                        let gt = game_type.clone();
                        let gid = game.game_id.clone();
                        let pid = identity.clone();
                        rsx! {
                            button {
                                class: "px-3 py-1.5 rounded-lg bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-sm font-medium text-white shadow-md shadow-emerald-900/20",
                                onclick: move |_| {
                                    playing.set(Some(PlayOverlay {
                                        game_type: gt.clone(),
                                        game_id: gid.clone(),
                                        player: pid.clone(),
                                        return_lobby_id: None,
                                    }));
                                },
                                "Join as {pid}"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn game_type_display_title(types: &[GameTypeInfo], stored_name: &str) -> String {
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

fn game_type_description(types: &[GameTypeInfo], stored_name: &str) -> Option<String> {
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

fn game_type_about_url(gt: &GameTypeInfo) -> Option<String> {
    gt.about_ui_path
        .as_ref()
        .map(|path| format!("/games/{}/{}", gt.name, path))
}

#[component]
fn GameTypeCatalogCard(gt: GameTypeInfo) -> Element {
    let desc = gt.description.trim();
    let about_url = game_type_about_url(&gt);
    rsx! {
        div { class: "rounded-xl border border-emerald-500/20 bg-gray-950/50 p-4 flex flex-col sm:flex-row sm:items-start sm:justify-between gap-3 shadow-md",
            div { class: "min-w-0 flex-1",
                p { class: "font-medium text-white text-base", "{gt.display_name}" }
                p { class: "text-xs text-gray-500 font-mono mt-0.5", "{gt.name} · v{gt.version}" }
                p { class: "text-xs text-indigo-200/70 mt-1", "{gt.min_players}–{gt.max_players} players" }
                if !desc.is_empty() {
                    p { class: "text-sm text-gray-300 mt-2 leading-relaxed", "{desc}" }
                }
            }
            div { class: "shrink-0 self-start flex items-center gap-2",
                if let Some(url) = about_url {
                    a {
                        class: "rounded-lg border border-indigo-500/50 bg-indigo-950/60 hover:bg-indigo-900/60 px-3 py-1.5 text-xs font-semibold text-indigo-100",
                        href: "{url}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "Info"
                    }
                }
                div { class: "rounded-lg border border-gray-600/80 bg-gray-800/80 px-3 py-1.5 text-xs text-gray-400",
                    "Join a lobby to play"
                }
            }
        }
    }
}

#[derive(Clone)]
struct LobbyConfigListenBridge(Rc<LobbyConfigListenInner>);

struct LobbyConfigListenInner {
    cell: Rc<RefCell<String>>,
    _listener: EventListener,
}

impl LobbyConfigListenBridge {
    fn new(init: String, game_type: String) -> Self {
        let cell = Rc::new(RefCell::new(init));
        let cell_for_ev = cell.clone();
        let win = web_sys::window().expect("window");
        let origin = win.location().origin().unwrap_or_default();
        let gtype = game_type.clone();
        let listener = EventListener::new(&win, "message", move |event| {
            let Some(event) = event.dyn_ref::<web_sys::MessageEvent>() else {
                return;
            };
            if event.origin() != origin {
                return;
            }
            let Some((game, config_str)) = parse_iframe_config_message(&event.data()) else {
                return;
            };
            if game != gtype {
                return;
            }
            if serde_json::from_str::<Value>(config_str.trim()).is_ok() {
                *cell_for_ev.borrow_mut() = config_str.clone();
                config_validation_reply(event, &origin, &game, true, &[]);
            } else {
                config_validation_reply(
                    event,
                    &origin,
                    &game,
                    false,
                    &[String::from("invalid JSON")],
                );
            }
        });
        Self(Rc::new(LobbyConfigListenInner {
            cell,
            _listener: listener,
        }))
    }

    fn cell(&self) -> Rc<RefCell<String>> {
        self.0.cell.clone()
    }
}

/// Config iframe talks to the parent only via `postMessage` (preview JSON). Mutations go through
/// **Apply configuration** so the raw `window` listener never spawns GraphQL work (avoids WASM runtime issues).
#[component]
fn LobbyConfigPanel(
    lobby_id: String,
    game_type: String,
    iframe_src: String,
    schema_json: Option<String>,
    read_only: bool,
    server_config_json: Option<String>,
) -> Element {
    let init_cfg = server_config_json
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "null".to_string());

    let init_preview = init_cfg.clone();
    let mut preview = use_signal(move || init_preview.clone());

    let draft_cell = use_hook({
        let init = init_cfg.clone();
        let gt = game_type.clone();
        move || LobbyConfigListenBridge::new(init.clone(), gt.clone())
    })
    .cell();

    let draft_for_poll = draft_cell.clone();
    use_hook(move || {
        let c = draft_for_poll.clone();
        let mut pv = preview;
        spawn(async move {
            let mut last = String::new();
            loop {
                TimeoutFuture::new(120).await;
                let cur = c.borrow().clone();
                if cur != last {
                    last = cur.clone();
                    pv.set(cur);
                }
            }
        });
    });

    let schema_json_mount = schema_json.clone();
    let game_mount = game_type.clone();
    let cfg_push = server_config_json
        .clone()
        .unwrap_or_else(|| "null".to_string());

    let lobby_id_apply = lobby_id.clone();
    let draft_for_apply = draft_cell.clone();

    rsx! {
        iframe {
            class: if read_only { "config-iframe pointer-events-none opacity-90" } else { "config-iframe" },
            src: "{iframe_src}",
            title: "Game config",
            onmounted: move |evt| {
                let Some(schema_str) = schema_json_mount.clone() else { return; };
                let Ok(schema) = serde_json::from_str::<Value>(&schema_str) else { return; };
                let game = game_mount.clone();
                let origin = web_sys::window()
                    .and_then(|w| w.location().origin().ok())
                    .unwrap_or_default();
                let cfg_line = cfg_push.clone();
                let Some(el) = evt.data().downcast::<web_sys::Element>().cloned() else {
                    return;
                };
                let Ok(iframe_el) = el.dyn_into::<web_sys::HtmlIFrameElement>() else {
                    return;
                };
                let iframe_for_load = iframe_el.clone();
                let game_l = game.clone();
                let schema_l = schema.clone();
                let origin_l = origin.clone();
                let cfg_l = cfg_line.clone();
                let _load_listener = EventListener::new(&iframe_el, "load", move |_| {
                    if let Some(w) = iframe_for_load.content_window() {
                        post_config_schema_to_window(&w, &origin_l, &game_l, &schema_l);
                        post_config_state_to_window(&w, &origin_l, &game_l, &cfg_l);
                    }
                });
                std::mem::forget(_load_listener);
                if let Some(w) = iframe_el.content_window() {
                    post_config_schema_to_window(&w, &origin, &game, &schema);
                    post_config_state_to_window(&w, &origin, &game, &cfg_line);
                }
            }
        }
        p { class: "text-xs text-gray-500 mt-2",
            "Preview JSON (from the config panel). Saving is separate — use Apply below."
        }
        textarea {
            class: "config-preview-json w-full mt-2 min-h-[6rem] text-xs font-mono bg-gray-950/90 border border-indigo-500/15 rounded-xl p-3 text-gray-300 ring-1 ring-white/5",
            readonly: true,
            value: "{preview()}",
        }
        if !read_only {
            button {
                class: "mt-3 px-4 py-2.5 rounded-xl bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-sm font-semibold text-white shadow-lg shadow-indigo-900/25",
                onclick: move |_| {
                    let cfg = draft_for_apply.borrow().clone();
                    let lid = lobby_id_apply.clone();
                    spawn(async move {
                        let q = "mutation U($id: ID!, $c: String!, $f: Boolean!) { updateLobbyConfig(lobbyId: $id, configJson: $c, force: $f) { id } }";
                        let vars = serde_json::json!({ "id": lid, "c": cfg, "f": false });
                        let r = graphql_exec::<Value>(q, Some(vars)).await;
                        if r.is_err() {
                            let force = web_sys::window()
                                .map(|w| {
                                    w.confirm_with_message(
                                        "Config change needs resetting seats. Apply and reset claims?",
                                    )
                                    .unwrap_or(false)
                                })
                                .unwrap_or(false);
                            if force {
                                let vars2 = serde_json::json!({ "id": lid, "c": cfg, "f": true });
                                let _ = graphql_exec::<Value>(q, Some(vars2)).await;
                            }
                        }
                    });
                },
                "Apply configuration"
            }
        } else {
            p { class: "text-xs text-amber-200/80 mt-3 rounded-lg border border-amber-500/25 bg-amber-950/30 px-3 py-2",
                "Read-only: only the lobby owner can change configuration."
            }
        }
    }
}

#[component]
fn LobbyRoomBody(
    lobby_for_cols: LobbyDetail,
    gt_list: Vec<GameTypeInfo>,
    uid: Option<String>,
) -> Element {
    let nav = use_navigator();
    let is_owner = uid.as_deref() == Some(lobby_for_cols.owner_user_id.as_str());
    let lobby_id_start = lobby_for_cols.id.clone();
    let lobby_id_cancel = lobby_for_cols.id.clone();
    let lobby_id_chat_panel = lobby_for_cols.id.clone();
    let lobby_id_mark_ready = lobby_for_cols.id.clone();
    let lobby_id_mark_unready = lobby_for_cols.id.clone();
    let no_game_yet = lobby_for_cols.game_type.trim().is_empty();
    let selected_gt = gt_list
        .iter()
        .find(|g| g.name == lobby_for_cols.game_type)
        .cloned();
    let iframe_src = selected_gt.as_ref().and_then(|g| {
        g.config_ui_path
            .as_ref()
            .map(|p| format!("/games/{}/{}", g.name, p))
    });
    let schema_json = selected_gt
        .as_ref()
        .and_then(|g| g.config_schema_json.clone());
    let read_only = !is_owner;
    let config_panel_key = format!(
        "{}|{}|{}",
        lobby_for_cols.id,
        lobby_for_cols.game_type,
        lobby_for_cols.config_json.as_deref().unwrap_or("null")
    );
    let total = lobby_for_cols.seats.len();
    let claimed = lobby_for_cols
        .seats
        .iter()
        .filter(|s| s.claimed_by_user_id.is_some())
        .count();
    let ready_count = lobby_for_cols
        .seats
        .iter()
        .filter(|s| s.claimed_by_user_id.is_some() && s.ready)
        .count();
    let all_ready = total > 0 && claimed == total && ready_count == claimed;
    let can_start = is_owner
        && total > 0
        && claimed == total
        && all_ready
        && (lobby_for_cols.status == "waiting" || lobby_for_cols.status == "configuring");
    let in_staging = lobby_for_cols.status == "waiting" || lobby_for_cols.status == "configuring";
    let user_in_seat = uid.as_ref().is_some_and(|u| {
        lobby_for_cols
            .seats
            .iter()
            .any(|s| s.claimed_by_user_id.as_deref() == Some(u.as_str()))
    });
    let my_seat_ready = uid
        .as_ref()
        .and_then(|u| {
            lobby_for_cols
                .seats
                .iter()
                .find(|s| s.claimed_by_user_id.as_deref() == Some(u.as_str()))
        })
        .map(|s| s.ready)
        .unwrap_or(false);
    let lobby_id_default_config = lobby_for_cols.id.clone();
    let lobby_finished = lobby_for_cols.status == "finished";
    let game_id_for_results_btn = lobby_for_cols.game_instance_id.clone();
    let lobby_id_reopen_finished = lobby_for_cols.id.clone();

    rsx! {
        div {
        if lobby_finished {
            div { class: "mb-6 p-4 sm:p-5 rounded-2xl bg-gradient-to-r from-amber-950/60 to-orange-950/40 border border-amber-500/35 shadow-lg shadow-amber-900/20",
                p { class: "text-amber-100 font-semibold mb-3", "This match is over." }
                div { class: "flex flex-wrap gap-2",
                    if let Some(g) = game_id_for_results_btn.clone() {
                        button {
                            class: "px-3 py-1.5 rounded-lg bg-amber-600 hover:bg-amber-500 text-sm font-medium text-gray-950 shadow-md",
                            onclick: move |_| {
                                nav.push(LobbyRoute::GameResult { id: g.clone() });
                            },
                            "View results"
                        }
                    }
                    if is_owner {
                        button {
                            class: "px-3 py-1.5 rounded-lg border border-gray-500/50 bg-gray-800 hover:bg-gray-700 text-sm font-medium",
                            onclick: move |_| {
                                let lid = lobby_id_reopen_finished.clone();
                                spawn(async move {
                                    let q = "mutation R($id: ID!) { reopenLobbyAfterGame(lobbyId: $id) }";
                                    let vars = serde_json::json!({ "id": lid });
                                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                });
                            },
                            "Play again (reset lobby)"
                        }
                    }
                }
            }
        }
        div { class: "lobby-room-grid",
            div { class: "lobby-col lobby-col-types",
                h3 { class: "text-xs font-semibold uppercase tracking-wide text-indigo-300/80 mb-3", "Game type" }
                if no_game_yet {
                    p { class: "text-xs text-gray-500 mb-3 leading-relaxed",
                        if is_owner {
                            "Choose a game below. Configuration and player seats appear after you select one."
                        } else {
                            "Waiting for the lobby owner to choose a game."
                        }
                    }
                }
                div { class: "lobby-type-list",
                    for gt in gt_list.iter().cloned() {
                        {
                            let active = !no_game_yet && gt.name == lobby_for_cols.game_type;
                            let desc = gt.description.trim();
                            let about_url = game_type_about_url(&gt);
                            let lid_set = lobby_for_cols.id.clone();
                            let gtn = gt.name.clone();
                            rsx! {
                                div { class: "space-y-2",
                                    button {
                                        class: if active { "lobby-type-btn active" } else { "lobby-type-btn" },
                                        disabled: !is_owner,
                                        onclick: move |_| {
                                            if !is_owner { return; }
                                            let lid = lid_set.clone();
                                            let gtn = gtn.clone();
                                            spawn(async move {
                                                let q = "mutation S($id: ID!, $t: String!, $f: Boolean!) { setLobbyGameType(lobbyId: $id, gameType: $t, force: $f) { id } }";
                                                let vars = serde_json::json!({ "id": lid, "t": gtn, "f": false });
                                                let r = graphql_exec::<Value>(q, Some(vars)).await;
                                                if r.is_err() {
                                                    let force = web_sys::window()
                                                        .map(|w| {
                                                            w.confirm_with_message(
                                                                "Changing type resets seats if claimed. Continue?",
                                                            )
                                                            .unwrap_or(false)
                                                        })
                                                        .unwrap_or(false);
                                                    if force {
                                                        let vars = serde_json::json!({ "id": lid, "t": gtn, "f": true });
                                                        let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                                    }
                                                }
                                            });
                                        },
                                        span { class: "font-medium", "{gt.display_name}" }
                                        if !desc.is_empty() {
                                            span { class: "mt-1 text-xs text-gray-400 leading-snug line-clamp-4", "{desc}" }
                                        }
                                    }
                                    if let Some(url) = about_url {
                                        a {
                                            class: "inline-flex rounded-md border border-indigo-500/40 bg-indigo-950/45 px-2 py-1 text-[11px] text-indigo-100 hover:bg-indigo-900/50",
                                            href: "{url}",
                                            target: "_blank",
                                            rel: "noopener noreferrer",
                                            "Open game info and rules"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "lobby-col lobby-col-config",
                h3 { class: "text-xs font-semibold uppercase tracking-wide text-indigo-300/80 mb-3", "Configuration" }
                if no_game_yet {
                    p { class: "text-gray-500 text-sm leading-relaxed",
                        "Select a game in the first column. The config editor loads here when the game provides one."
                    }
                } else if let Some(src) = iframe_src {
                    LobbyConfigPanel {
                        key: "{config_panel_key}",
                        lobby_id: lobby_for_cols.id.clone(),
                        game_type: lobby_for_cols.game_type.clone(),
                        iframe_src: src,
                        schema_json: schema_json.clone(),
                        read_only,
                        server_config_json: lobby_for_cols.config_json.clone(),
                    }
                } else {
                    p { class: "text-gray-500 text-sm mb-2",
                        "This game has no config UI. Initialize seats with default config, or pick another type that has a config editor."
                    }
                    if is_owner && lobby_for_cols.seats.is_empty() {
                        button {
                            class: "px-3 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-sm font-medium text-white shadow-md",
                            onclick: move |_| {
                                let lid = lobby_id_default_config.clone();
                                spawn(async move {
                                    let q = "mutation U($id: ID!, $c: String!, $f: Boolean!) { updateLobbyConfig(lobbyId: $id, configJson: $c, force: $f) { id } }";
                                    let vars = serde_json::json!({ "id": lid, "c": "null", "f": false });
                                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                });
                            },
                            "Use default config (create seats)"
                        }
                    }
                }
            }
            div { class: "lobby-col lobby-col-seats",
                h3 { class: "text-xs font-semibold uppercase tracking-wide text-indigo-300/80 mb-3", "Players" }
                p { class: "text-xs text-gray-400 mb-3",
                    "{claimed}/{total} seats taken"
                    if total > 0 && claimed > 0 {
                        " · "
                        "{ready_count}/{claimed} ready"
                    }
                }
                if in_staging && total > 0 {
                    p { class: "text-[11px] text-gray-500 mb-3 leading-relaxed",
                        "Take a free seat, then mark Ready. The host can start only when every seat is filled and everyone is ready."
                    }
                }
                div { class: "space-y-2 mb-4",
                    for seat in lobby_for_cols.seats.clone() {
                        {
                            let lid_join = lobby_for_cols.id.clone();
                            let idx = seat.seat_index;
                            let taken = seat.claimed_by_user_id.is_some();
                            let label = seat
                                .claimed_display_name
                                .clone()
                                .unwrap_or_else(|| "free".into());
                            let seat_ready = seat.ready;
                            rsx! {
                                div { class: "flex flex-wrap items-center gap-x-2 gap-y-1 text-sm rounded-lg border border-gray-700/50 bg-gray-950/40 px-2 py-1.5",
                                    span { class: "text-gray-400 w-24 truncate font-mono text-xs", "{seat.player_identity}" }
                                    if taken {
                                        span { class: "text-gray-200", "{label}" }
                                        if seat_ready {
                                            span { class: "text-emerald-400 text-xs font-medium", "· Ready" }
                                        } else {
                                            span { class: "text-amber-400/90 text-xs font-medium", "· Not ready" }
                                        }
                                    } else {
                                        button {
                                            class: "px-2 py-0.5 rounded-md bg-indigo-600 hover:bg-indigo-500 text-xs font-medium text-white",
                                            onclick: move |_| {
                                                let lid = lid_join.clone();
                                                spawn(async move {
                                                    let q = "mutation J($id: ID!, $i: Int!) { joinLobby(lobbyId: $id, seatIndex: $i) { id } }";
                                                    let vars = serde_json::json!({ "id": lid, "i": idx });
                                                    match graphql_exec::<Value>(q, Some(vars)).await {
                                                        Ok(_) => {}
                                                        Err(e) => {
                                                            let _ = web_sys::window()
                                                                .and_then(|w| w.alert_with_message(&e).ok());
                                                        }
                                                    }
                                                });
                                            },
                                            "Take seat"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if user_in_seat && in_staging {
                    div { class: "mb-4 rounded-xl border border-gray-700/60 bg-gray-950/50 px-3 py-3",
                        p { class: "text-[11px] font-medium text-gray-400 uppercase tracking-wide mb-2", "Your readiness" }
                        div { class: "flex flex-wrap gap-2",
                            button {
                                class: "px-3 py-1.5 rounded-lg bg-emerald-700 hover:bg-emerald-600 text-xs font-semibold text-white shadow-sm disabled:opacity-40 disabled:shadow-none",
                                disabled: my_seat_ready,
                                onclick: move |_| {
                                    let lid = lobby_id_mark_ready.clone();
                                    spawn(async move {
                                        let q = "mutation R($id: ID!, $r: Boolean!) { setLobbySeatReady(lobbyId: $id, ready: $r) { id } }";
                                        let vars = serde_json::json!({ "id": lid, "r": true });
                                        let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                    });
                                },
                                "Ready"
                            }
                            button {
                                class: "px-3 py-1.5 rounded-lg bg-gray-700 hover:bg-gray-600 text-xs font-medium text-gray-100 disabled:opacity-40",
                                disabled: !my_seat_ready,
                                onclick: move |_| {
                                    let lid = lobby_id_mark_unready.clone();
                                    spawn(async move {
                                        let q = "mutation R($id: ID!, $r: Boolean!) { setLobbySeatReady(lobbyId: $id, ready: $r) { id } }";
                                        let vars = serde_json::json!({ "id": lid, "r": false });
                                        let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                    });
                                },
                                "Not ready"
                            }
                        }
                    }
                }
                div { class: "flex flex-wrap gap-2",
                    if is_owner && in_staging {
                        button {
                            class: "px-3 py-1.5 rounded-lg bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-sm font-semibold text-white shadow-md disabled:opacity-40 disabled:shadow-none",
                            disabled: !can_start,
                            onclick: move |_| {
                                let lid = lobby_id_start.clone();
                                spawn(async move {
                                    let q = "mutation St($id: ID!) { startLobby(lobbyId: $id) }";
                                    let vars = serde_json::json!({ "id": lid });
                                    match graphql_exec::<Value>(q, Some(vars)).await {
                                        Ok(_) => {}
                                        Err(e) => {
                                            let _ = web_sys::window()
                                                .and_then(|w| w.alert_with_message(&e).ok());
                                        }
                                    }
                                });
                            },
                            "Start game"
                        }
                        button {
                            class: "px-3 py-1.5 rounded-lg border border-red-500/40 bg-red-950/70 hover:bg-red-900 text-sm font-medium text-red-100",
                            onclick: move |_| {
                                let lid = lobby_id_cancel.clone();
                                let nav = nav;
                                spawn(async move {
                                    let q = "mutation C($id: ID!) { cancelLobby(lobbyId: $id) }";
                                    let vars = serde_json::json!({ "id": lid });
                                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                    nav.push(LobbyRoute::Home {});
                                });
                            },
                            "Cancel lobby"
                        }
                    }
                    if user_in_seat && in_staging {
                        button {
                            class: "px-3 py-1.5 rounded-lg border border-gray-600 bg-gray-800 hover:bg-gray-700 text-sm",
                            onclick: move |_| {
                                let lid = lobby_for_cols.id.clone();
                                spawn(async move {
                                    let q = "mutation Lv($id: ID!) { leaveLobby(lobbyId: $id) }";
                                    let vars = serde_json::json!({ "id": lid });
                                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                });
                            },
                            "Leave seat"
                        }
                    }
                }
                LobbyChatPanel {
                    lobby_id: lobby_id_chat_panel,
                    messages: lobby_for_cols.messages.clone(),
                }
            }
        }
        }
    }
}

#[component]
fn GameResultPage(game_id: String) -> Element {
    let nav = use_navigator();
    let mut loaded: Signal<Option<LoadedGameResult>> = use_signal(|| None);
    let mut err: Signal<Option<String>> = use_signal(|| None);
    let mut done: Signal<bool> = use_signal(|| false);
    let gid_fetch = game_id.clone();
    use_hook(move || {
        let gid_fetch = gid_fetch.clone();
        let mut loaded = loaded;
        let mut err = err;
        let mut done = done;
        spawn(async move {
            let q = r#"query G($id: ID!) { finishedGame(gameId: $id) { gameId gameType lobbyId finishedAt resultJson playerScoresJson seatsSnapshotJson resultUiPath } }"#;
            let vars = serde_json::json!({ "id": gid_fetch });
            #[derive(Deserialize)]
            struct Wrap {
                #[serde(rename = "finishedGame")]
                finished_game: Option<GameResultRow>,
            }
            match graphql_exec::<Wrap>(q, Some(vars)).await {
                Ok(w) => {
                    if let Some(r) = w.finished_game {
                        let iframe_src = r.result_ui_path.as_ref().and_then(|path| {
                            let result_v: Value =
                                serde_json::from_str(&r.result_json).unwrap_or(Value::Null);
                            let scores_v: Value =
                                serde_json::from_str(&r.player_scores_json).unwrap_or(Value::Null);
                            let seats_v: Value =
                                serde_json::from_str(&r.seats_snapshot_json).unwrap_or(Value::Null);
                            let payload = serde_json::json!({
                                "gameId": &r.game_id,
                                "gameType": &r.game_type,
                                "finishedAt": r.finished_at,
                                "lobbyId": &r.lobby_id,
                                "result": result_v,
                                "scores": scores_v,
                                "seats": seats_v,
                            });
                            let s = payload.to_string();
                            let enc = urlencoding::encode(&s);
                            Some(format!(
                                "/games/{}/{}?payload={}",
                                r.game_type, path, enc
                            ))
                        });
                        loaded.set(Some(LoadedGameResult {
                            row: r,
                            iframe_src,
                        }));
                    } else {
                        loaded.set(None);
                    }
                }
                Err(e) => err.set(Some(e)),
            }
            done.set(true);
        });
    });

    rsx! {
        div { class: "max-w-4xl mx-auto px-4 sm:px-6 py-8 sm:py-10 space-y-6",
            button {
                class: "inline-flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-800/90 border border-gray-600 text-gray-200 text-sm hover:bg-gray-700 hover:border-gray-500 transition-colors",
                onclick: move |_| {
                    nav.push(LobbyRoute::Home {});
                },
                "← Home"
            }
            div {
                h1 { class: "text-2xl sm:text-3xl font-bold tracking-tight text-white", "Game result" }
                p { class: "text-gray-500 text-sm mt-1 font-mono break-all", "{game_id}" }
            }
            if let Some(e) = err() {
                div { class: "rounded-xl border border-red-500/50 bg-red-950/45 px-4 py-3 shadow-lg shadow-red-900/20",
                    p { class: "text-sm font-medium text-red-100", "{e}" }
                }
            } else if !done() {
                p { class: "text-sm text-indigo-200/80", "Loading…" }
            } else if let Some(ld) = loaded() {
                div { class: "rounded-2xl border border-gray-700/70 bg-gray-900/50 p-5 sm:p-6 shadow-xl shadow-black/30 backdrop-blur-sm space-y-4",
                    p { class: "text-gray-400 text-sm", "Finished at UNIX {ld.row.finished_at}" }
                    if let Some(lid) = ld.row.lobby_id.clone() {
                        p { class: "text-sm text-gray-300",
                            "Lobby: "
                            button {
                                class: "text-indigo-400 hover:text-indigo-300 hover:underline font-mono text-xs",
                                onclick: move |_| {
                                    nav.push(LobbyRoute::Lobby { id: lid.clone() });
                                },
                                "{lid}"
                            }
                        }
                    }
                    if let Some(src) = ld.iframe_src.clone() {
                        p { class: "text-xs text-gray-500", "Game-specific view (client/result.html)" }
                        iframe {
                            class: "w-full min-h-[32rem] border border-indigo-500/20 rounded-xl bg-gray-950 shadow-inner ring-1 ring-white/5",
                            src: src,
                        }
                    } else {
                        p { class: "text-amber-200/90 text-sm rounded-lg border border-amber-500/25 bg-amber-950/30 px-3 py-2",
                            "This game type has no client/result.html — raw payload below."
                        }
                    }
                    details { class: "text-sm rounded-xl border border-gray-700/50 bg-gray-950/40 p-4",
                        summary { class: "cursor-pointer text-indigo-200/80 font-medium", "Raw JSON" }
                        h2 { class: "text-base font-semibold mt-4 mb-2 text-white", "Scores (float)" }
                        pre { class: "text-xs bg-gray-950 border border-gray-700/80 rounded-xl p-3 overflow-x-auto mb-4 text-gray-300",
                            "{ld.row.player_scores_json}"
                        }
                        h2 { class: "text-base font-semibold mt-4 mb-2 text-white", "Outcome (JSON)" }
                        pre { class: "text-xs bg-gray-950 border border-gray-700/80 rounded-xl p-3 overflow-x-auto mb-4 text-gray-300",
                            "{ld.row.result_json}"
                        }
                        h2 { class: "text-base font-semibold mt-4 mb-2 text-white", "Seats at finish" }
                        pre { class: "text-xs bg-gray-950 border border-gray-700/80 rounded-xl p-3 overflow-x-auto text-gray-300",
                            "{ld.row.seats_snapshot_json}"
                        }
                    }
                }
            } else {
                p { class: "text-sm text-gray-500", "No saved result for this game (wrong id or not finished yet)." }
            }
        }
    }
}

#[component]
fn LobbyRoomPage(
    lobby_id: String,
    mut playing: Signal<Option<PlayOverlay>>,
    mut error_msg: Signal<Option<String>>,
) -> Element {
    let nav = use_navigator();
    let mut detail: Signal<Option<LobbyDetail>> = use_signal(|| None);
    let mut game_types: Signal<Vec<GameTypeInfo>> = use_signal(Vec::new);
    let my_user_id = use_signal(|| stored_user_id());

    use_hook(move || {
        let lid_fetch = lobby_id.clone();
        let lid_sub = lobby_id.clone();
        let mut detail_f = detail;
        let mut game_types_f = game_types;
        let mut error_msg_f = error_msg;
        start_lobby_room_subscription(lid_sub, detail, error_msg);
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Gt {
                game_types: Vec<GameTypeInfo>,
            }
            let gt_q = r#"query { gameTypes { name displayName version minPlayers maxPlayers description configUiPath aboutUiPath configSchemaJson } }"#;
            if let Ok(g) = graphql_post::<Gt>(gt_q).await {
                game_types_f.set(g.game_types);
            }
            let q = r#"query L($id: ID!) { lobby(id: $id) { id ownerUserId ownerDisplayName gameType configJson status gameInstanceId createdAt updatedAt seats { seatIndex playerIdentity claimedByUserId claimedDisplayName ready } messages { id userId displayName body createdAt } } }"#;
            let vars = serde_json::json!({ "id": lid_fetch });
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Ld {
                lobby: Option<LobbyDetail>,
            }
            match graphql_exec::<Ld>(q, Some(vars)).await {
                Ok(d) => detail_f.set(d.lobby),
                Err(e) => error_msg_f.set(Some(e)),
            }
        });
    });

    use_effect(move || {
        let Some(ref l) = detail() else {
            return;
        };
        let user_id = my_user_id();
        if l.status == "in_game" {
            if let (Some(uid), Some(gid)) = (user_id.as_ref(), l.game_instance_id.as_ref()) {
                if let Some(seat) = l
                    .seats
                    .iter()
                    .find(|s| s.claimed_by_user_id.as_deref() == Some(uid.as_str()))
                {
                    playing.set(Some(PlayOverlay {
                        game_type: l.game_type.clone(),
                        game_id: gid.clone(),
                        player: seat.player_identity.clone(),
                        return_lobby_id: Some(l.id.clone()),
                    }));
                    return;
                }
            }
        }
        let overlay_this_lobby = playing()
            .as_ref()
            .and_then(|p| p.return_lobby_id.as_deref())
            == Some(l.id.as_str());
        if overlay_this_lobby && l.status != "in_game" {
            playing.set(None);
        }
    });

    let d = detail();
    let gt_list = game_types();
    let uid = my_user_id();

    rsx! {
        div { class: "lobby-room-wrap px-4 sm:px-6 py-6 sm:py-8 max-w-[1400px] mx-auto",
            div { class: "flex flex-wrap items-center gap-4 mb-8 rounded-2xl border border-gray-700/60 bg-gray-900/40 px-4 py-3 shadow-lg shadow-black/20 backdrop-blur-sm",
                button {
                    class: "inline-flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-800/90 border border-gray-600 text-gray-200 text-sm hover:bg-gray-700 hover:border-gray-500 transition-colors shrink-0",
                    onclick: move |_| {
                        nav.push(LobbyRoute::Home {});
                    },
                    "← Home"
                }
                if let Some(ref l) = d {
                    div { class: "min-w-0 flex-1",
                        h1 { class: "text-lg sm:text-xl font-semibold text-white",
                            "{game_type_display_title(&gt_list, &l.game_type)}"
                        }
                        if let Some(ref sd) = game_type_description(&gt_list, &l.game_type) {
                            p { class: "text-sm text-gray-400 mt-1 leading-snug", "{sd}" }
                        }
                        p { class: "text-xs text-gray-500 font-mono mt-1.5 break-all", "Lobby {l.id}" }
                    }
                }
            }
            if d.is_none() {
                div { class: "flex flex-col items-center py-12 gap-2",
                    p { class: "text-sm font-medium text-indigo-200/80", "Loading lobby…" }
                    p { class: "text-xs text-gray-500", "Subscribing to room updates" }
                }
            } else if let Some(lob) = d {
                LobbyRoomBody {
                    lobby_for_cols: lob.clone(),
                    gt_list: gt_list.clone(),
                    uid: uid.clone(),
                }
            }
        }
    }
}

#[component]
fn GamePlayer(
    game_type: String,
    game_id: String,
    player: String,
    return_lobby_id: Option<String>,
    on_close: EventHandler<()>,
    on_navigate_lobby: EventHandler<String>,
) -> Element {
    let ws_base = get_ws_base();
    let player_q = urlencoding::encode(&player);
    let iframe_src = format!(
        "/games/{game_type}/?ws={ws_base}&id={game_id}&player={player_q}"
    );
    let ret = return_lobby_id.clone();

    rsx! {
        div { class: "flex flex-col h-screen fixed inset-0 z-50 bg-gradient-to-b from-gray-950 via-gray-900 to-indigo-950/30",
            div { class: "flex items-center gap-4 px-4 py-3 border-b border-indigo-500/20 bg-gray-900/80 backdrop-blur-md shadow-lg shadow-black/40",
                button {
                    class: "px-3 py-1.5 rounded-lg border border-gray-600 bg-gray-800 hover:bg-gray-700 text-sm font-medium text-white",
                    onclick: move |_| {
                        let lid = ret.clone();
                        spawn(async move {
                            if let Some(ref id) = lid {
                                let q = "mutation R($id: ID!) { reopenLobbyAfterGame(lobbyId: $id) }";
                                let vars = serde_json::json!({ "id": id });
                                let _ = graphql_exec::<Value>(q, Some(vars)).await;
                            }
                            on_close.call(());
                            if let Some(id) = lid {
                                on_navigate_lobby.call(id);
                            }
                        });
                    },
                    "Back to lobby"
                }
                span { class: "text-indigo-200/80 text-sm",
                    "Playing "
                    span { class: "font-semibold text-white", "{game_type}" }
                    " as "
                    span { class: "text-emerald-300/90 font-mono text-xs sm:text-sm", "{player}" }
                }
            }
            iframe {
                class: "flex-1 w-full border-0 bg-gray-950",
                src: "{iframe_src}",
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadDiag {
    severity: String,
    code: String,
    message: String,
    path: Option<String>,
    hint: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadReport {
    ok: bool,
    errors: i32,
    warnings: i32,
    infos: i32,
    required_index_html: bool,
    required_config_html: bool,
    required_result_html: bool,
    required_about_html: bool,
    diagnostics: Vec<UploadDiag>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct GameDraftShort {
    id: String,
    game_name: String,
    display_name: String,
    version: String,
    status: String,
    manifest_json: String,
    created_at: i64,
    published_at: Option<i64>,
}

fn manifest_description_from_json(manifest_json: &str) -> String {
    serde_json::from_str::<Value>(manifest_json)
        .ok()
        .and_then(|v| {
            v.get("description")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default()
}

#[component]
fn DeveloperDraftRow(
    draft: GameDraftShort,
    mut err: Signal<Option<String>>,
    on_refresh: EventHandler<()>,
) -> Element {
    let desc0 = manifest_description_from_json(&draft.manifest_json);
    let mut publish_name = use_signal(|| draft.game_name.clone());
    let mut publish_display = use_signal(|| draft.display_name.clone());
    let mut publish_version = use_signal(|| draft.version.clone());
    let mut publish_desc = use_signal(|| desc0);
    let mut saving = use_signal(|| false);

    use_effect({
        let d = draft.clone();
        move || {
            publish_name.set(d.game_name.clone());
            publish_display.set(d.display_name.clone());
            publish_version.set(d.version.clone());
            publish_desc.set(manifest_description_from_json(&d.manifest_json));
        }
    });

    let status = draft.status.clone();
    let id_pub = draft.id.clone();
    let id_unpub = draft.id.clone();
    let id_disc = draft.id.clone();
    let id_save = draft.id.clone();

    rsx! {
        div { class: "{draft_card_classes(&draft.status)}",
            div { class: "flex-1 min-w-0 space-y-3",
                div {
                    div { class: "flex flex-wrap items-center gap-2 mb-1",
                        span { class: "px-2.5 py-0.5 rounded-md text-[10px] font-bold uppercase tracking-wide {draft_status_style(&draft.status).0}",
                            "{draft.status}"
                        }
                        span { class: "text-white font-semibold", "{draft.display_name}" }
                        span { class: "text-gray-500 text-sm", "({draft.game_name})" }
                        span { class: "text-indigo-300/90 text-sm font-mono", "v{draft.version}" }
                    }
                    p { class: "text-xs text-gray-500 font-mono",
                        "created {draft.created_at}"
                        if draft.published_at.is_some() {
                            " · published {draft.published_at.unwrap_or(0)}"
                        }
                    }
                }
                if status == "ready" {
                    p { class: "text-xs text-gray-400",
                        "Adjust how the game appears in the lobby, then save before publishing. Folder name uses "
                        span { class: "font-mono text-indigo-200/90", "name" }
                        " (letters, digits, "
                        span { class: "font-mono", "_ -" }
                        " only)."
                    }
                    div { class: "grid sm:grid-cols-2 gap-3",
                        label { class: "block space-y-1",
                            span { class: "text-[11px] uppercase tracking-wide text-gray-500", "name (folder id)" }
                            input {
                                class: "w-full px-3 py-2 bg-gray-950/80 border border-gray-600 rounded-lg text-sm text-white font-mono",
                                value: "{publish_name()}",
                                oninput: move |e| publish_name.set(e.value()),
                            }
                        }
                        label { class: "block space-y-1",
                            span { class: "text-[11px] uppercase tracking-wide text-gray-500", "display name" }
                            input {
                                class: "w-full px-3 py-2 bg-gray-950/80 border border-gray-600 rounded-lg text-sm text-white",
                                value: "{publish_display()}",
                                oninput: move |e| publish_display.set(e.value()),
                            }
                        }
                        label { class: "block space-y-1",
                            span { class: "text-[11px] uppercase tracking-wide text-gray-500", "version" }
                            input {
                                class: "w-full px-3 py-2 bg-gray-950/80 border border-gray-600 rounded-lg text-sm text-white font-mono",
                                value: "{publish_version()}",
                                oninput: move |e| publish_version.set(e.value()),
                            }
                        }
                    }
                    label { class: "block space-y-1",
                        span { class: "text-[11px] uppercase tracking-wide text-gray-500", "description" }
                        textarea {
                            class: "w-full px-3 py-2 bg-gray-950/80 border border-gray-600 rounded-lg text-sm text-white min-h-[4rem]",
                            value: "{publish_desc()}",
                            oninput: move |e| publish_desc.set(e.value()),
                        }
                    }
                    button {
                        class: "px-4 py-2 rounded-lg border border-indigo-500/50 bg-indigo-950/50 text-indigo-100 text-xs font-semibold hover:bg-indigo-900/50 disabled:opacity-40",
                        disabled: saving(),
                        onclick: move |_| {
                            saving.set(true);
                            let id = id_save.clone();
                            let n = publish_name();
                            let dn = publish_display();
                            let v = publish_version();
                            let d = publish_desc();
                            spawn(async move {
                                let q = r#"mutation U($id: ID!, $n: String!, $dn: String!, $v: String!, $d: String!) {
                                    updateGameDraftManifest(draftId: $id, name: $n, displayName: $dn, version: $v, description: $d) { id }
                                }"#;
                                let vars = serde_json::json!({
                                    "id": id,
                                    "n": n,
                                    "dn": dn,
                                    "v": v,
                                    "d": d,
                                });
                                match graphql_exec::<Value>(q, Some(vars)).await {
                                    Ok(_) => {
                                        err.set(None);
                                        on_refresh.call(());
                                    }
                                    Err(e) => err.set(Some(e)),
                                }
                                saving.set(false);
                            });
                        },
                        if saving() { "Saving…" } else { "Save manifest fields" }
                    }
                }
            }
            div { class: "flex flex-wrap gap-2 shrink-0",
                button {
                    class: "px-4 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold shadow-md shadow-emerald-900/30 disabled:opacity-35 disabled:shadow-none transition-colors",
                    disabled: draft.status != "ready",
                    onclick: move |_| {
                        let id2 = id_pub.clone();
                        spawn(async move {
                            let q = "mutation P($id: ID!) { publishGameDraft(draftId: $id) { id } }";
                            let vars = serde_json::json!({ "id": id2 });
                            match graphql_exec::<Value>(q, Some(vars)).await {
                                Ok(_) => {
                                    err.set(None);
                                    on_refresh.call(());
                                }
                                Err(e) => err.set(Some(e)),
                            }
                        });
                    },
                    "Publish"
                }
                button {
                    class: "px-4 py-2 rounded-lg border border-amber-500/70 bg-amber-950/50 text-amber-100 text-xs font-semibold hover:bg-amber-900/45 disabled:opacity-35 disabled:shadow-none transition-colors",
                    disabled: draft.status != "published",
                    onclick: move |_| {
                        let id2 = id_unpub.clone();
                        let Some(win) = web_sys::window() else {
                            return;
                        };
                        let Ok(true) = win.confirm_with_message(
                            "Remove this game from the live lobby? Players will not see it until someone publishes again.",
                        ) else {
                            return;
                        };
                        spawn(async move {
                            let q = "mutation U($id: ID!) { unpublishGameDraft(draftId: $id) { id status } }";
                            let vars = serde_json::json!({ "id": id2 });
                            match graphql_exec::<Value>(q, Some(vars)).await {
                                Ok(_) => {
                                    err.set(None);
                                    on_refresh.call(());
                                }
                                Err(e) => err.set(Some(e)),
                            }
                        });
                    },
                    "Take down"
                }
                button {
                    class: "px-4 py-2 rounded-lg border border-red-500/60 bg-red-950/40 text-red-200 text-xs font-semibold hover:bg-red-900/50 disabled:opacity-35 transition-colors",
                    disabled: draft.status == "published",
                    onclick: move |_| {
                        let id2 = id_disc.clone();
                        spawn(async move {
                            let q = "mutation D($id: ID!) { discardGameDraft(draftId: $id) }";
                            let vars = serde_json::json!({ "id": id2 });
                            match graphql_exec::<Value>(q, Some(vars)).await {
                                Ok(_) => {
                                    err.set(None);
                                    on_refresh.call(());
                                }
                                Err(e) => err.set(Some(e)),
                            }
                        });
                    },
                    "Discard"
                }
            }
        }
    }
}

fn upload_diag_panel_class(severity: &str) -> &'static str {
    match severity {
        "error" => "border-l-4 border-l-red-500 bg-red-950/40 border border-red-900/50 rounded-r-lg",
        "warning" => "border-l-4 border-l-amber-400 bg-amber-950/35 border border-amber-900/40 rounded-r-lg",
        "info" => "border-l-4 border-l-sky-500 bg-sky-950/30 border border-sky-900/40 rounded-r-lg",
        _ => "border-l-4 border-l-gray-500 bg-gray-800/60 border border-gray-700 rounded-r-lg",
    }
}

fn upload_diag_badge_class(severity: &str) -> &'static str {
    match severity {
        "error" => "bg-red-600 text-white",
        "warning" => "bg-amber-500 text-gray-900",
        "info" => "bg-sky-600 text-white",
        _ => "bg-gray-600 text-gray-100",
    }
}

fn upload_file_check_class(ok: bool) -> &'static str {
    if ok {
        "flex items-center gap-2 rounded-lg border border-emerald-700/60 bg-emerald-950/40 px-3 py-2 text-sm text-emerald-100"
    } else {
        "flex items-center gap-2 rounded-lg border border-red-700/60 bg-red-950/40 px-3 py-2 text-sm text-red-100"
    }
}

fn draft_status_style(status: &str) -> (&'static str, &'static str) {
    match status {
        "ready" => ("bg-violet-600 text-white", "border-l-violet-500"),
        "published" => ("bg-emerald-600 text-white", "border-l-emerald-500"),
        "discarded" => ("bg-gray-600 text-gray-200", "border-l-gray-500"),
        _ => ("bg-slate-600 text-white", "border-l-slate-500"),
    }
}

fn draft_card_classes(status: &str) -> String {
    let (_, border) = draft_status_style(status);
    format!(
        "rounded-xl border border-gray-700/80 bg-gray-800/90 p-4 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 shadow-lg border-l-4 {border}"
    )
}

fn spawn_read_zip_file(
    fd: FileData,
    mut zip_base64: Signal<String>,
    mut filename: Signal<String>,
    mut file_status: Signal<String>,
    mut err: Signal<Option<String>>,
) {
    spawn(async move {
        match fd.read_bytes().await {
            Ok(bytes) => {
                let name = fd.name();
                let b64 = STANDARD.encode(&bytes);
                let status = format!("{name} — {} bytes (ready)", bytes.len());
                filename.set(name);
                zip_base64.set(b64);
                file_status.set(status);
                err.set(None);
            }
            Err(e) => {
                zip_base64.set(String::new());
                file_status.set("No file selected".to_string());
                err.set(Some(format!("Failed to read zip: {e}")));
            }
        }
    });
}

#[component]
fn DeveloperUploadsPage() -> Element {
    let nav = use_navigator();
    let mut is_dev = use_signal(|| None::<bool>);
    let mut err = use_signal(|| None::<String>);
    let mut filename = use_signal(|| "game.zip".to_string());
    let mut zip_base64 = use_signal(String::new);
    let file_status = use_signal(|| "No file selected".to_string());
    let mut uploading = use_signal(|| false);
    let mut report = use_signal(|| None::<UploadReport>);
    let mut drafts = use_signal(Vec::<GameDraftShort>::new);
    let mut zip_drag_over = use_signal(|| false);

    // Prevent the browser from opening/downloading dragged files on this page (drops on our zone are handled in ondrop).
    let _global_file_drag_guard: Rc<(EventListener, EventListener)> = use_hook(move || {
        let win = web_sys::window().expect("window");
        let doc = win.document().expect("document");
        let opts = EventListenerOptions::enable_prevent_default();
        let drag_over = EventListener::new_with_options(&doc, "dragover", opts.clone(), |e: &web_sys::Event| {
            e.prevent_default();
        });
        let drop_doc = EventListener::new_with_options(&doc, "drop", opts, |e: &web_sys::Event| {
            e.prevent_default();
        });
        Rc::new((drag_over, drop_doc))
    });

    let refresh_drafts = {
        let mut drafts = drafts;
        let mut err = err;
        move || {
            spawn(async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Wrap {
                    my_game_drafts: Vec<GameDraftShort>,
                }
                let q = "query { myGameDrafts { id gameName displayName version status manifestJson createdAt publishedAt } }";
                match graphql_post::<Wrap>(q).await {
                    Ok(v) => drafts.set(v.my_game_drafts),
                    Err(e) => err.set(Some(e)),
                }
            });
        }
    };

    use_hook(move || {
        let mut is_dev = is_dev;
        let mut err = err;
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct P {
                is_developer: bool,
            }
            let q = "query { isDeveloper }";
            match graphql_post::<P>(q).await {
                Ok(v) => is_dev.set(Some(v.is_developer)),
                Err(e) => {
                    is_dev.set(Some(false));
                    err.set(Some(e));
                }
            }
        });
        refresh_drafts();
    });

    rsx! {
        div { class: "max-w-5xl mx-auto px-4 sm:px-6 py-8 sm:py-10 space-y-8",
                div { class: "flex flex-col sm:flex-row sm:items-end sm:justify-between gap-4",
                    div {
                        button {
                            class: "mb-2 inline-flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-800/90 border border-gray-600 text-gray-200 text-sm hover:bg-gray-700 hover:border-gray-500 transition-colors",
                            onclick: move |_| {
                                nav.push(LobbyRoute::Home {});
                            },
                            "← Home"
                        }
                        h1 { class: "text-3xl sm:text-4xl font-bold tracking-tight text-white",
                            "Developer uploads"
                        }
                        p { class: "mt-2 text-sm text-indigo-200/80 max-w-xl",
                            "Package a game as zip, validate against the server, then publish drafts when checks pass."
                        }
                    }
                    div { class: "hidden sm:block h-14 w-px bg-gradient-to-b from-transparent via-indigo-500/50 to-transparent" }
                    div { class: "rounded-xl border border-indigo-500/30 bg-indigo-950/50 px-4 py-3 text-xs text-indigo-100/90 max-w-xs",
                        p { class: "font-semibold text-indigo-200 mb-1", "Expected layout" }
                        p { class: "font-mono text-[11px] leading-relaxed text-indigo-100/70",
                            "manifest.json · logic.wasm · client/index|config|result|about.html"
                        }
                    }
                }

                if let Some(e) = err() {
                    div { class: "rounded-xl border border-red-500/60 bg-red-950/50 px-4 py-3 shadow-lg shadow-red-900/20",
                        div { class: "flex items-start gap-3",
                            span { class: "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-red-600 text-sm font-bold text-white",
                                "!"
                            }
                            div {
                                p { class: "text-sm font-semibold text-red-100", "Something went wrong" }
                                p { class: "mt-1 text-sm text-red-200/90 leading-relaxed", "{e}" }
                            }
                        }
                    }
                }

                if is_dev() == Some(false) {
                    div { class: "rounded-xl border border-amber-500/40 bg-amber-950/40 px-5 py-4",
                        p { class: "text-amber-100 font-medium", "Developer access required" }
                        p { class: "mt-2 text-sm text-amber-100/80",
                            "Grant the developer role, or set OPEN_DEVELOPER_UPLOADS=true for open uploads."
                        }
                    }
                } else {
                    section { class: "rounded-2xl border border-gray-700/80 bg-gray-900/60 backdrop-blur-sm p-6 sm:p-8 shadow-xl shadow-black/40",
                        div { class: "flex items-center gap-3 mb-2",
                            span { class: "flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-600 text-lg", "📦" }
                            div {
                                h2 { class: "text-xl font-semibold text-white", "Upload game zip" }
                                p { class: "text-sm text-gray-400", "Drag & drop or browse — then validate on the server." }
                            }
                        }
                        div {
                            class: "relative mt-6 min-h-[11rem] rounded-2xl border-2 border-dashed transition-all duration-200 overflow-hidden",
                            class: if zip_drag_over() {
                                "border-indigo-300 bg-gradient-to-br from-indigo-950/70 via-violet-950/50 to-indigo-900/40 shadow-[0_0_0_1px_rgba(165,180,252,0.45),0_0_28px_rgba(129,140,248,0.35)] ring-2 ring-indigo-400/40"
                            } else {
                                "border-indigo-500/35 bg-gradient-to-br from-gray-950/80 to-indigo-950/30 hover:border-indigo-400/55 hover:from-gray-900/90 hover:to-indigo-950/50"
                            },
                            div { class: "pointer-events-none px-6 py-10 text-center",
                                p { class: "text-base text-gray-200 font-medium mb-1", "Drop your .zip here" }
                                p { class: "text-xs text-gray-500 mb-5", "Release package at repo root (not a folder of zips)" }
                                span { class: "inline-flex items-center gap-2 px-5 py-2.5 rounded-xl bg-indigo-600 text-white text-sm font-semibold shadow-lg shadow-indigo-900/40",
                                    "Browse files…"
                                }
                            }
                            input {
                                id: "dev-zip-upload",
                                class: "absolute inset-0 z-10 h-full w-full cursor-pointer opacity-0",
                                r#type: "file",
                                accept: ".zip,application/zip,application/x-zip-compressed",
                                ondragenter: move |evt: Event<DragData>| {
                                    evt.prevent_default();
                                    zip_drag_over.set(true);
                                },
                                ondragleave: move |evt: Event<DragData>| {
                                    evt.prevent_default();
                                    zip_drag_over.set(false);
                                },
                                ondragover: move |evt: Event<DragData>| {
                                    evt.prevent_default();
                                    evt.data().data_transfer().set_drop_effect("copy");
                                },
                                ondrop: move |evt: Event<DragData>| {
                                    evt.prevent_default();
                                    evt.stop_propagation();
                                    zip_drag_over.set(false);
                                    let files = evt.data().files();
                                    let Some(fd) = files.into_iter().next() else {
                                        return;
                                    };
                                    spawn_read_zip_file(
                                        fd,
                                        zip_base64,
                                        filename,
                                        file_status,
                                        err,
                                    );
                                },
                                onchange: move |evt: Event<FormData>| {
                                    zip_drag_over.set(false);
                                    let files = evt.data().files();
                                    let Some(fd) = files.into_iter().next() else {
                                        return;
                                    };
                                    spawn_read_zip_file(
                                        fd,
                                        zip_base64,
                                        filename,
                                        file_status,
                                        err,
                                    );
                                },
                            }
                        }
                        div { class: "mt-4 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3",
                            p { class: "text-sm text-gray-300 flex items-center gap-2",
                                span { class: "inline-block h-2 w-2 rounded-full shrink-0",
                                    class: if zip_base64().trim().is_empty() { "bg-gray-600" } else { "bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.6)]" },
                                }
                                span { class: "font-mono text-xs sm:text-sm text-gray-200", "{file_status()}" }
                            }
                            button {
                                class: "inline-flex justify-center items-center gap-2 px-5 py-2.5 rounded-xl bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-white text-sm font-semibold shadow-lg shadow-indigo-900/30 disabled:opacity-40 disabled:cursor-not-allowed disabled:shadow-none transition-all",
                                disabled: uploading() || zip_base64().trim().is_empty(),
                                onclick: move |_| {
                                    let n = filename();
                                    let payload = zip_base64();
                                    uploading.set(true);
                                    err.set(None);
                                    spawn(async move {
                                        #[derive(Deserialize)]
                                        #[serde(rename_all = "camelCase")]
                                        struct R {
                                            upload_game_zip: UploadResp,
                                        }
                                        #[derive(Deserialize)]
                                        #[serde(rename_all = "camelCase")]
                                        struct UploadResp {
                                            report: UploadReport,
                                        }
                                        let q = "mutation Upload($f: String!, $z: String!) { uploadGameZip(filename: $f, zipBase64: $z) { report { ok errors warnings infos requiredIndexHtml requiredConfigHtml requiredResultHtml requiredAboutHtml diagnostics { severity code message path hint } } } }";
                                        let vars = serde_json::json!({ "f": n, "z": payload.trim() });
                                        match graphql_exec::<R>(q, Some(vars)).await {
                                            Ok(v) => {
                                                report.set(Some(v.upload_game_zip.report));
                                            }
                                            Err(e) => err.set(Some(e)),
                                        }
                                        refresh_drafts();
                                        uploading.set(false);
                                    });
                                },
                                if uploading() {
                                    span { "Validating…" }
                                } else {
                                    span { "Run validation" }
                                }
                            }
                        }
                    }

                    if let Some(rep) = report() {
                        section { class: "rounded-2xl border overflow-hidden shadow-xl shadow-black/30",
                            class: if rep.ok { "border-emerald-600/40 bg-gray-900/70" } else { "border-red-600/45 bg-gray-900/70" },
                            div { class: "px-6 py-4 border-b border-gray-700/80 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 bg-gradient-to-r from-gray-900 to-gray-800/80",
                                div { class: "flex items-center gap-3",
                                    span { class: "flex h-12 w-12 items-center justify-center rounded-xl text-xl font-bold shrink-0",
                                        class: if rep.ok { "bg-emerald-600 text-white" } else { "bg-red-600 text-white" },
                                        if rep.ok { "✓" } else { "✕" }
                                    }
                                    div {
                                        h3 { class: "text-lg font-semibold text-white", "Validation report" }
                                        p { class: "text-sm text-gray-400",
                                            if rep.ok { "Package accepted — draft created if applicable." } else { "Fix errors below and upload again." }
                                        }
                                    }
                                }
                                div { class: "grid grid-cols-3 gap-2 sm:gap-3 w-full sm:w-auto",
                                    div { class: "rounded-lg bg-red-950/60 border border-red-800/50 px-3 py-2 text-center",
                                        p { class: "text-2xl font-bold text-red-300", "{rep.errors}" }
                                        p { class: "text-[10px] uppercase tracking-wider text-red-400/90", "Errors" }
                                    }
                                    div { class: "rounded-lg bg-amber-950/50 border border-amber-800/40 px-3 py-2 text-center",
                                        p { class: "text-2xl font-bold text-amber-200", "{rep.warnings}" }
                                        p { class: "text-[10px] uppercase tracking-wider text-amber-400/90", "Warnings" }
                                    }
                                    div { class: "rounded-lg bg-sky-950/50 border border-sky-800/40 px-3 py-2 text-center",
                                        p { class: "text-2xl font-bold text-sky-200", "{rep.infos}" }
                                        p { class: "text-[10px] uppercase tracking-wider text-sky-400/90", "Infos" }
                                    }
                                }
                            }

                            div { class: "px-6 py-5 border-b border-gray-700/60 bg-gray-950/40",
                                p { class: "text-xs font-semibold uppercase tracking-wide text-gray-500 mb-3", "Required client files" }
                                div { class: "grid sm:grid-cols-4 gap-3",
                                    div { class: upload_file_check_class(rep.required_index_html),
                                        span { class: "text-lg", if rep.required_index_html { "✓" } else { "✕" } }
                                        div {
                                            p { class: "font-mono text-xs font-semibold", "client/index.html" }
                                            p { class: "text-[11px] opacity-80", "Play UI entry" }
                                        }
                                    }
                                    div { class: upload_file_check_class(rep.required_config_html),
                                        span { class: "text-lg", if rep.required_config_html { "✓" } else { "✕" } }
                                        div {
                                            p { class: "font-mono text-xs font-semibold", "client/config.html" }
                                            p { class: "text-[11px] opacity-80", "Lobby config iframe" }
                                        }
                                    }
                                    div { class: upload_file_check_class(rep.required_result_html),
                                        span { class: "text-lg", if rep.required_result_html { "✓" } else { "✕" } }
                                        div {
                                            p { class: "font-mono text-xs font-semibold", "client/result.html" }
                                            p { class: "text-[11px] opacity-80", "Post-game screen" }
                                        }
                                    }
                                    div { class: upload_file_check_class(rep.required_about_html),
                                        span { class: "text-lg", if rep.required_about_html { "✓" } else { "✕" } }
                                        div {
                                            p { class: "font-mono text-xs font-semibold", "client/about.html" }
                                            p { class: "text-[11px] opacity-80", "Game info and rules" }
                                        }
                                    }
                                }
                            }

                            div { class: "px-6 py-5",
                                p { class: "text-xs font-semibold uppercase tracking-wide text-gray-500 mb-3", "Diagnostics" }
                                div { class: "space-y-2 max-h-80 overflow-y-auto pr-1",
                                    for d in rep.diagnostics {
                                        div { class: "pl-1 {upload_diag_panel_class(&d.severity)}",
                                            div { class: "p-3 sm:p-4",
                                                div { class: "flex flex-wrap items-center gap-2 mb-2",
                                                    span { class: "px-2 py-0.5 rounded text-[10px] font-bold uppercase tracking-wide {upload_diag_badge_class(&d.severity)}",
                                                        "{d.severity}"
                                                    }
                                                    span { class: "font-mono text-xs text-gray-300", "{d.code}" }
                                                }
                                                p { class: "text-sm text-gray-100 leading-snug", "{d.message}" }
                                                if let Some(ref pth) = d.path {
                                                    p { class: "mt-2 text-xs font-mono text-gray-400 bg-black/25 rounded px-2 py-1 inline-block", "{pth}" }
                                                }
                                                if let Some(ref h) = d.hint {
                                                    p { class: "mt-2 text-xs text-gray-300/90 border-l-2 border-gray-500 pl-2", "{h}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    section { class: "rounded-2xl border border-gray-700/80 bg-gray-900/50 p-6 sm:p-8 shadow-lg shadow-black/20",
                        div { class: "flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 mb-6",
                            div {
                                h2 { class: "text-xl font-semibold text-white", "My drafts" }
                                p { class: "text-sm text-gray-500 mt-1", "Publish to go live in the lobby game list." }
                            }
                            button {
                                class: "self-start sm:self-auto px-4 py-2 rounded-xl border border-gray-600 bg-gray-800 text-gray-200 text-sm font-medium hover:bg-gray-700 hover:border-gray-500 transition-colors",
                                onclick: move |_| refresh_drafts(),
                                "Refresh list"
                            }
                        }
                        if drafts().is_empty() {
                            div { class: "rounded-xl border border-dashed border-gray-600 bg-gray-950/50 py-12 text-center",
                                p { class: "text-gray-500 text-sm", "No drafts yet — validate a zip to create one." }
                            }
                        } else {
                            div { class: "space-y-3",
                                for d in drafts() {
                                    DeveloperDraftRow {
                                        key: "{d.id}",
                                        draft: d.clone(),
                                        err,
                                        on_refresh: move |_| refresh_drafts(),
                                    }
                                }
                            }
                        }
                    }
                }
            }
    }
}
