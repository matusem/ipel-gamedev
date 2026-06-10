use crate::api::graphql::{graphql_exec, graphql_ws_url, reload_lobbies};
use crate::models::{GameInfo, LobbyDetail, LobbySummary};
use dioxus::prelude::*;
use futures_util::{SinkExt, StreamExt};
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::Message;
use serde::Deserialize;
use serde_json::Value;

/// `graphql-ws` in browsers maps to Apollo **subscriptions-transport-ws**, which uses `type: "data"`.
/// The newer **graphql-transport-ws** subprotocol uses `type: "next"`. async_graphql supports both.
pub fn gql_ws_is_subscription_result(ty: Option<&str>) -> bool {
    matches!(ty, Some("data") | Some("next"))
}

/// Subscription result messages may include `errors` without usable `data`.
pub fn gql_ws_payload_data(msg: &Value) -> Option<&Value> {
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

async fn connect_graphql_ws() -> Option<WebSocket> {
    let url = graphql_ws_url();
    let Ok(mut ws) = WebSocket::open_with_protocol(&url, "graphql-ws") else {
        return None;
    };
    if ws
        .send(Message::Text(r#"{"type":"connection_init"}"#.into()))
        .await
        .is_err()
    {
        return None;
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
            Some("connection_error") => return None,
            _ if !acked => continue,
            _ => {}
        }
    }
    if acked { Some(ws) } else { None }
}

pub fn start_game_instances_subscription(mut games: Signal<Vec<GameInfo>>) {
    spawn(async move {
        let Some(mut ws) = connect_graphql_ws().await else {
            return;
        };
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

pub fn start_lobbies_subscription(
    mut list: Signal<Vec<LobbySummary>>,
    mut err: Signal<Option<String>>,
) {
    if crate::stub::demo_mode::is_demo_mode() {
        return;
    }
    spawn(async move {
        let Some(mut ws) = connect_graphql_ws().await else {
            return;
        };
        let sub = serde_json::json!({
            "type": "start",
            "id": "lobbies1",
            "payload": {
                "query": "subscription { lobbiesUpdated { id gameType status seatsFilled seatsTotal ownerDisplayName gameInstanceId createdAt } }"
            }
        });
        if ws.send(Message::Text(sub.to_string())).await.is_err() {
            return;
        };
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

pub fn start_lobby_room_subscription(
    lobby_id: String,
    mut detail: Signal<Option<LobbyDetail>>,
    mut err: Signal<Option<String>>,
) {
    if crate::stub::demo_mode::is_demo_mode() {
        return;
    }
    spawn(async move {
        let Some(mut ws) = connect_graphql_ws().await else {
            return;
        };
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
