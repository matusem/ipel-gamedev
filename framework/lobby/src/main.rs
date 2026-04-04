use dioxus::prelude::*;
use futures_util::{SinkExt, StreamExt};
use gloo_events::EventListener;
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

#[derive(Clone, Debug, PartialEq)]
enum AppRoute {
    Home,
    Lobby(String),
    GameResult(String),
}

fn read_hash_route() -> AppRoute {
    let Some(window) = web_sys::window() else {
        return AppRoute::Home;
    };
    let Ok(hash) = window.location().hash() else {
        return AppRoute::Home;
    };
    if let Some(rest) = hash.strip_prefix("#/lobby/") {
        let id = rest.trim().to_string();
        if !id.is_empty() {
            return AppRoute::Lobby(id);
        }
    }
    if let Some(rest) = hash.strip_prefix("#/game/") {
        let id = rest.trim().to_string();
        if !id.is_empty() {
            return AppRoute::GameResult(id);
        }
    }
    AppRoute::Home
}

fn navigate_lobby(id: &str) {
    if let Some(w) = web_sys::window() {
        let _ = w.location().set_hash(&format!("#/lobby/{id}"));
    }
}

fn navigate_game_result(id: &str) {
    if let Some(w) = web_sys::window() {
        let _ = w.location().set_hash(&format!("#/game/{id}"));
    }
}

fn navigate_home() {
    if let Some(w) = web_sys::window() {
        let _ = w.location().set_hash("");
    }
}

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
        let q = r#"subscription L($id: ID!) { lobbyUpdated(id: $id) { id ownerUserId ownerDisplayName gameType configJson status gameInstanceId createdAt updatedAt seats { seatIndex playerIdentity claimedByUserId claimedDisplayName } messages { id userId displayName body createdAt } } }"#;
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
                        let q = r#"query L($id: ID!) { lobby(id: $id) { id ownerUserId ownerDisplayName gameType configJson status gameInstanceId createdAt updatedAt seats { seatIndex playerIdentity claimedByUserId claimedDisplayName } messages { id userId displayName body createdAt } } }"#;
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
fn App() -> Element {
    let route: Signal<AppRoute> = use_signal(read_hash_route);
    let mut session_ok: Signal<bool> = use_signal(|| false);
    let mut session_checked: Signal<bool> = use_signal(|| false);
    let mut playing: Signal<Option<PlayOverlay>> = use_signal(|| None);
    let error_msg: Signal<Option<String>> = use_signal(|| None);

    use_hook(move || {
        let window = web_sys::window().expect("window");
        let mut r = route;
        let listener = EventListener::new(&window, "hashchange", move |_| {
            r.set(read_hash_route());
        });
        std::mem::forget(listener);
    });

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
        div { class: "min-h-screen bg-gray-900 text-white",
            if !session_checked() {
                p { class: "text-center text-gray-400 py-12", "Checking session…" }
            } else if !session_ok() {
                AuthGate {
                    on_ready: move |_| {
                        session_ok.set(true);
                        session_checked.set(true);
                    }
                }
            } else {
                match route() {
                    AppRoute::Home => rsx! {
                        HomePage {
                            playing,
                            error_msg,
                        }
                    },
                    AppRoute::Lobby(id) => rsx! {
                        LobbyRoomPage {
                            key: "{id}",
                            lobby_id: id,
                            playing,
                            error_msg,
                        }
                    },
                    AppRoute::GameResult(id) => rsx! {
                        GameResultPage {
                            key: "{id}",
                            game_id: id,
                        }
                    },
                }
                if let Some(p) = playing() {
                    GamePlayer {
                        game_type: p.game_type.clone(),
                        game_id: p.game_id.clone(),
                        player: p.player.clone(),
                        return_lobby_id: p.return_lobby_id.clone(),
                        on_close: move |_| {
                            playing.set(None);
                        },
                    }
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
        div { class: "max-w-lg mx-auto px-4 py-16",
            h1 { class: "text-3xl font-bold mb-2 text-center", "Sign in" }
            p { class: "text-gray-500 text-sm text-center mb-8",
                "Choose guest, sign up, or log in to open lobbies."
            }
            if let Some(e) = err() {
                div { class: "bg-red-900/50 border border-red-500 text-red-200 px-4 py-3 rounded mb-6", "{e}" }
            }
            div { class: "space-y-8",
                div { class: "bg-gray-800 rounded-lg p-4 border border-gray-700",
                    h2 { class: "font-semibold mb-3", "Continue as guest" }
                    input {
                        class: "w-full px-2 py-2 bg-gray-700 border border-gray-600 rounded text-sm mb-3",
                        placeholder: "Display name",
                        value: "{guest_name}",
                        oninput: move |e| guest_name.set(e.value()),
                    }
                    button {
                        class: "w-full px-4 py-2 bg-indigo-700 hover:bg-indigo-600 rounded font-medium",
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
                div { class: "bg-gray-800 rounded-lg p-4 border border-gray-700",
                    h2 { class: "font-semibold mb-3", "Sign up" }
                    input {
                        class: "w-full px-2 py-2 bg-gray-700 border border-gray-600 rounded text-sm mb-2",
                        placeholder: "Display name",
                        value: "{signup_name}",
                        oninput: move |e| signup_name.set(e.value()),
                    }
                    input {
                        class: "w-full px-2 py-2 bg-gray-700 border border-gray-600 rounded text-sm mb-3",
                        r#type: "password",
                        placeholder: "Password (min 4 chars)",
                        value: "{signup_pass}",
                        oninput: move |e| signup_pass.set(e.value()),
                    }
                    button {
                        class: "w-full px-4 py-2 bg-emerald-700 hover:bg-emerald-600 rounded font-medium",
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
                div { class: "bg-gray-800 rounded-lg p-4 border border-gray-700",
                    h2 { class: "font-semibold mb-3", "Log in" }
                    input {
                        class: "w-full px-2 py-2 bg-gray-700 border border-gray-600 rounded text-sm mb-2",
                        placeholder: "Display name",
                        value: "{login_name}",
                        oninput: move |e| login_name.set(e.value()),
                    }
                    input {
                        class: "w-full px-2 py-2 bg-gray-700 border border-gray-600 rounded text-sm mb-3",
                        r#type: "password",
                        placeholder: "Password",
                        value: "{login_pass}",
                        oninput: move |e| login_pass.set(e.value()),
                    }
                    button {
                        class: "w-full px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded font-medium",
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
    let game_types: Signal<Vec<GameTypeInfo>> = use_signal(Vec::new);
    let games: Signal<Vec<GameInfo>> = use_signal(Vec::new);
    let lobbies: Signal<Vec<LobbySummary>> = use_signal(Vec::new);
    let recent_finished: Signal<Vec<RecentFinishedRow>> = use_signal(Vec::new);
    let loading = use_signal(|| true);
    let mut create_type = use_signal(|| String::new());
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
                gameTypes { name displayName version minPlayers maxPlayers description configUiPath configSchemaJson }
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
        div { class: "max-w-4xl mx-auto px-4 py-8",
            h1 { class: "text-4xl font-bold mb-8 text-center", "Game Server" }
            if let Some(err) = error_msg() {
                div { class: "bg-red-900/50 border border-red-500 text-red-200 px-4 py-3 rounded mb-6", "{err}" }
            }
            if loading() {
                p { class: "text-center text-gray-400", "Loading…" }
            } else {
                section { class: "mb-10",
                    h2 { class: "text-2xl font-semibold mb-4", "Create lobby" }
                    p { class: "text-gray-500 text-sm mb-3",
                        "Pick a game type only. You will set options in the lobby room."
                    }
                    div { class: "flex flex-wrap gap-2 items-end",
                        select {
                            class: "px-3 py-2 bg-gray-800 border border-gray-700 rounded text-sm min-w-[12rem]",
                            onchange: move |e| create_type.set(e.value()),
                            option { value: "", "Select game type…" }
                            for gt in game_types() {
                                option { value: "{gt.name}", "{gt.display_name}" }
                            }
                        }
                        button {
                            class: "px-4 py-2 bg-indigo-700 hover:bg-indigo-600 rounded text-sm font-medium disabled:opacity-50",
                            disabled: creating() || create_type().is_empty(),
                            onclick: move |_| {
                                let t = create_type();
                                if t.is_empty() { return; }
                                creating.set(true);
                                spawn(async move {
                                    let q = "mutation Create($t: String!) { createLobby(gameType: $t) { id } }";
                                    let vars = serde_json::json!({ "t": t });
                                    #[derive(Deserialize)]
                                    #[serde(rename_all = "camelCase")]
                                    struct Cr { create_lobby: RegisterUserRow }
                                    match graphql_exec::<Cr>(q, Some(vars)).await {
                                        Ok(c) => {
                                            navigate_lobby(&c.create_lobby.id);
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
                }
                section { class: "mb-10",
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-2xl font-semibold", "Lobbies" }
                        button {
                            class: "px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-sm",
                            onclick: move |_| refresh_lobbies(),
                            "Refresh"
                        }
                    }
                    if lobbies().is_empty() {
                        p { class: "text-gray-400", "No active lobbies yet." }
                    }
                    div { class: "space-y-3",
                        for lob in lobbies() {
                            div { class: "bg-gray-800 rounded-lg p-4 border border-gray-700 flex justify-between items-center",
                                div {
                                    p { class: "font-medium", "{lob.game_type}" }
                                    p { class: "text-xs text-gray-400 mt-1",
                                        "{lob.owner_display_name} · {lob.seats_filled}/{lob.seats_total} · {lob.status}"
                                    }
                                }
                                button {
                                    class: "px-3 py-1 bg-indigo-700 hover:bg-indigo-600 rounded text-sm shrink-0",
                                    onclick: move |_| navigate_lobby(&lob.id),
                                    "Open"
                                }
                            }
                        }
                    }
                }
                section { class: "mb-10",
                    h2 { class: "text-2xl font-semibold mb-4", "Recent results" }
                    if recent_finished().is_empty() {
                        p { class: "text-gray-400", "No finished games recorded yet." }
                    }
                    div { class: "space-y-2",
                        for it in recent_finished() {
                            {
                                let gid = it.game_id.clone();
                                let gt = it.game_type.clone();
                                let sc = it.player_scores_json.clone();
                                rsx! {
                                    div { class: "bg-gray-800 rounded p-3 border border-gray-700 flex justify-between gap-2 items-start",
                                        div {
                                            p { class: "font-medium", "{gt}" }
                                            p { class: "text-xs text-gray-500 font-mono break-all", "{sc}" }
                                        }
                                        button {
                                            class: "px-2 py-1 bg-indigo-700 hover:bg-indigo-600 rounded text-xs shrink-0",
                                            onclick: move |_| navigate_game_result(&gid),
                                            "Details"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                section {
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-2xl font-semibold", "Active games" }
                        button {
                            class: "px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-sm",
                            onclick: move |_| refresh_games(),
                            "Refresh"
                        }
                    }
                    if games().is_empty() {
                        p { class: "text-gray-400", "No active games." }
                    }
                    div { class: "space-y-3",
                        for game in games() {
                            GameCard {
                                game: game.clone(),
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
        h4 { class: "text-xs font-semibold text-gray-500 mt-6 mb-2", "Chat" }
        div { class: "lobby-chat-messages",
            for m in messages.iter().rev().take(40).rev() {
                p {
                    key: "{m.id}",
                    class: "text-xs text-gray-300 mb-1",
                    span { class: "text-gray-500", "{m.display_name}: " }
                    "{m.body}"
                }
            }
        }
        textarea {
            class: "w-full mt-2 px-2 py-1 bg-gray-800 border border-gray-600 rounded text-xs min-h-[3rem]",
            placeholder: "Message…",
            value: "{draft()}",
            oninput: move |e| draft.set(e.value()),
        }
        button {
            class: "mt-2 px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-xs",
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
fn GameCard(game: GameInfo, mut playing: Signal<Option<PlayOverlay>>) -> Element {
    let game_type = game.game_type.clone();
    rsx! {
        div { class: "bg-gray-800 rounded-lg p-4 border border-gray-700 flex items-center justify-between flex-wrap gap-2",
            div {
                p { class: "font-medium",
                    "{game.game_type}"
                    span { class: "text-gray-500 text-sm ml-2", "{game.game_id}" }
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
                                class: "px-3 py-1 bg-green-700 hover:bg-green-600 rounded text-sm",
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
            class: "config-preview-json w-full mt-2 min-h-[6rem] text-xs font-mono bg-gray-950 border border-gray-700 rounded p-2 text-gray-300",
            readonly: true,
            value: "{preview()}",
        }
        if !read_only {
            button {
                class: "mt-3 px-4 py-2 bg-indigo-700 hover:bg-indigo-600 rounded text-sm font-medium",
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
            p { class: "text-xs text-gray-500 mt-2", "Read-only: only the lobby owner can change configuration." }
        }
    }
}

#[component]
fn LobbyRoomBody(
    lobby_for_cols: LobbyDetail,
    gt_list: Vec<GameTypeInfo>,
    uid: Option<String>,
    mut playing: Signal<Option<PlayOverlay>>,
) -> Element {
    let is_owner = uid.as_deref() == Some(lobby_for_cols.owner_user_id.as_str());
    let lobby_id_start = lobby_for_cols.id.clone();
    let lobby_id_cancel = lobby_for_cols.id.clone();
    let lobby_id_chat_panel = lobby_for_cols.id.clone();
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
    let can_start = is_owner
        && total > 0
        && claimed == total
        && (lobby_for_cols.status == "waiting" || lobby_for_cols.status == "configuring");
    let in_game = lobby_for_cols.status == "in_game";
    let gid = lobby_for_cols.game_instance_id.clone();
    let play_row: Option<(String, String, String, String)> = if in_game {
        gid.as_ref().and_then(|game_id| {
            uid.as_ref().and_then(|u| {
                lobby_for_cols
                    .seats
                    .iter()
                    .find(|s| s.claimed_by_user_id.as_deref() == Some(u.as_str()))
                    .map(|seat| {
                        (
                            lobby_for_cols.game_type.clone(),
                            game_id.clone(),
                            seat.player_identity.clone(),
                            lobby_for_cols.id.clone(),
                        )
                    })
            })
        })
    } else {
        None
    };
    let in_staging = lobby_for_cols.status == "waiting" || lobby_for_cols.status == "configuring";
    let user_in_seat = uid.as_ref().is_some_and(|u| {
        lobby_for_cols
            .seats
            .iter()
            .any(|s| s.claimed_by_user_id.as_deref() == Some(u.as_str()))
    });
    let lobby_id_default_config = lobby_for_cols.id.clone();
    let lobby_finished = lobby_for_cols.status == "finished";
    let game_id_for_results_btn = lobby_for_cols.game_instance_id.clone();
    let lobby_id_reopen_finished = lobby_for_cols.id.clone();

    rsx! {
        div {
        if lobby_finished {
            div { class: "mb-4 p-4 rounded-lg bg-amber-900/25 border border-amber-700/80",
                p { class: "text-amber-100 font-medium mb-2", "This match is over." }
                div { class: "flex flex-wrap gap-2",
                    if let Some(g) = game_id_for_results_btn.clone() {
                        button {
                            class: "px-3 py-1 bg-amber-700 hover:bg-amber-600 rounded text-sm",
                            onclick: move |_| {
                                navigate_game_result(&g);
                            },
                            "View results"
                        }
                    }
                    if is_owner {
                        button {
                            class: "px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-sm",
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
                h3 { class: "text-sm font-semibold text-gray-400 mb-3", "Game type" }
                div { class: "lobby-type-list",
                    for gt in gt_list.iter().cloned() {
                        {
                            let active = gt.name == lobby_for_cols.game_type;
                            let lid_set = lobby_for_cols.id.clone();
                            let gtn = gt.name.clone();
                            rsx! {
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
                                    "{gt.display_name}"
                                }
                            }
                        }
                    }
                }
            }
            div { class: "lobby-col lobby-col-config",
                h3 { class: "text-sm font-semibold text-gray-400 mb-3", "Configuration" }
                if let Some(src) = iframe_src {
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
                            class: "px-3 py-1 bg-indigo-800 hover:bg-indigo-700 rounded text-sm",
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
                h3 { class: "text-sm font-semibold text-gray-400 mb-3", "Players" }
                p { class: "text-xs text-gray-500 mb-3", "{claimed}/{total} seats claimed" }
                div { class: "space-y-2 mb-4",
                    for seat in lobby_for_cols.seats.clone() {
                        {
                            let lid = lobby_for_cols.id.clone();
                            let idx = seat.seat_index;
                            let taken = seat.claimed_by_user_id.is_some();
                            let label = seat
                                .claimed_display_name
                                .clone()
                                .unwrap_or_else(|| "free".into());
                            rsx! {
                                div { class: "flex items-center gap-2 text-sm",
                                    span { class: "text-gray-500 w-24 truncate", "{seat.player_identity}" }
                                    if taken {
                                        span { "{label}" }
                                    } else {
                                        button {
                                            class: "px-2 py-0.5 bg-indigo-700 hover:bg-indigo-600 rounded text-xs",
                                            onclick: move |_| {
                                                let lid = lid.clone();
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
                                            "Join"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "flex flex-wrap gap-2",
                    if is_owner && in_staging {
                        button {
                            class: "px-3 py-1 bg-emerald-700 hover:bg-emerald-600 rounded text-sm disabled:opacity-40",
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
                            class: "px-3 py-1 bg-red-900 hover:bg-red-800 rounded text-sm",
                            onclick: move |_| {
                                let lid = lobby_id_cancel.clone();
                                spawn(async move {
                                    let q = "mutation C($id: ID!) { cancelLobby(lobbyId: $id) }";
                                    let vars = serde_json::json!({ "id": lid });
                                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                    navigate_home();
                                });
                            },
                            "Cancel lobby"
                        }
                    }
                    if user_in_seat && in_staging {
                        button {
                            class: "px-3 py-1 bg-gray-600 hover:bg-gray-500 rounded text-sm",
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
                    if let Some((gt, gid, pid, lid_ret)) = play_row.clone() {
                        button {
                            class: "px-3 py-1 bg-green-700 hover:bg-green-600 rounded text-sm",
                            onclick: move |_| {
                                playing.set(Some(PlayOverlay {
                                    game_type: gt.clone(),
                                    game_id: gid.clone(),
                                    player: pid.clone(),
                                    return_lobby_id: Some(lid_ret.clone()),
                                }));
                            },
                            "Play"
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
        div { class: "max-w-4xl mx-auto px-4 py-8",
            button {
                class: "px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-sm mb-6",
                onclick: move |_| navigate_home(),
                "← Home"
            }
            h1 { class: "text-2xl font-bold mb-2", "Game result" }
            p { class: "text-gray-500 text-sm mb-6", "{game_id}" }
            if let Some(e) = err() {
                div { class: "bg-red-900/50 border border-red-500 text-red-200 px-4 py-3 rounded", "{e}" }
            } else if !done() {
                p { class: "text-gray-400", "Loading…" }
            } else if let Some(ld) = loaded() {
                p { class: "text-gray-400 text-sm mb-2", "Finished at UNIX {ld.row.finished_at}" }
                if let Some(lid) = ld.row.lobby_id.clone() {
                    p { class: "text-sm mb-4",
                        "Lobby: "
                        button {
                            class: "text-indigo-400 hover:underline",
                            onclick: move |_| navigate_lobby(&lid),
                            "{lid}"
                        }
                    }
                }
                if let Some(src) = ld.iframe_src.clone() {
                    p { class: "text-xs text-gray-500 mb-2", "Game-specific view (client/result.html)" }
                    iframe {
                        class: "w-full min-h-[32rem] border border-gray-700 rounded-lg bg-gray-950 mb-6",
                        src: src,
                    }
                } else {
                    p { class: "text-amber-200/90 text-sm mb-3",
                        "This game type has no client/result.html — raw payload below."
                    }
                }
                details { class: "text-sm",
                    summary { class: "cursor-pointer text-gray-400 mb-2", "Raw JSON" }
                    h2 { class: "text-lg font-semibold mt-2 mb-2", "Scores (float)" }
                    pre { class: "text-xs bg-gray-950 border border-gray-700 rounded p-3 overflow-x-auto mb-4",
                        "{ld.row.player_scores_json}"
                    }
                    h2 { class: "text-lg font-semibold mt-4 mb-2", "Outcome (JSON)" }
                    pre { class: "text-xs bg-gray-950 border border-gray-700 rounded p-3 overflow-x-auto mb-4",
                        "{ld.row.result_json}"
                    }
                    h2 { class: "text-lg font-semibold mt-4 mb-2", "Seats at finish" }
                    pre { class: "text-xs bg-gray-950 border border-gray-700 rounded p-3 overflow-x-auto",
                        "{ld.row.seats_snapshot_json}"
                    }
                }
            } else {
                p { class: "text-gray-400", "No saved result for this game (wrong id or not finished yet)." }
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
            let gt_q = r#"query { gameTypes { name displayName version minPlayers maxPlayers description configUiPath configSchemaJson } }"#;
            if let Ok(g) = graphql_post::<Gt>(gt_q).await {
                game_types_f.set(g.game_types);
            }
            let q = r#"query L($id: ID!) { lobby(id: $id) { id ownerUserId ownerDisplayName gameType configJson status gameInstanceId createdAt updatedAt seats { seatIndex playerIdentity claimedByUserId claimedDisplayName } messages { id userId displayName body createdAt } } }"#;
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

    let d = detail();
    let gt_list = game_types();
    let uid = my_user_id();

    rsx! {
        div { class: "lobby-room-wrap px-4 py-6 max-w-[1400px] mx-auto",
            div { class: "flex items-center gap-4 mb-6",
                button {
                    class: "px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-sm",
                    onclick: move |_| navigate_home(),
                    "← Home"
                }
                if let Some(ref l) = d {
                    h1 { class: "text-xl font-semibold",
                        "{l.game_type}"
                        span { class: "text-gray-500 text-sm ml-2 font-normal", "{l.id}" }
                    }
                }
            }
            if d.is_none() {
                p { class: "text-gray-400", "Loading lobby…" }
            } else if let Some(lob) = d {
                LobbyRoomBody {
                    lobby_for_cols: lob.clone(),
                    gt_list: gt_list.clone(),
                    uid: uid.clone(),
                    playing,
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
) -> Element {
    let ws_base = get_ws_base();
    let player_q = urlencoding::encode(&player);
    let iframe_src = format!(
        "/games/{game_type}/?ws={ws_base}&id={game_id}&player={player_q}"
    );
    let ret = return_lobby_id.clone();

    rsx! {
        div { class: "flex flex-col h-screen fixed inset-0 z-50 bg-gray-900",
            div { class: "flex items-center gap-4 px-4 py-3 bg-gray-800 border-b border-gray-700",
                button {
                    class: "px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-sm",
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
                                navigate_lobby(&id);
                            }
                        });
                    },
                    "Back to lobby"
                }
                span { class: "text-gray-400 text-sm",
                    "Playing {game_type} as {player}"
                }
            }
            iframe {
                class: "flex-1 w-full border-0",
                src: "{iframe_src}",
            }
        }
    }
}
