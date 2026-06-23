use crate::api::graphql::{
    fetch_lobby_detail, graphql_ws_url, lobby_room_subscription, reload_lobbies,
};
use crate::models::{GameInfo, LobbyDetail, LobbySummary};
use dioxus::prelude::*;
use futures_util::{SinkExt, StreamExt};
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::Message;
use gloo_timers::future::TimeoutFuture;
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

fn refetch_lobby_room(
    lobby_id: &str,
    mut detail: Signal<Option<LobbyDetail>>,
    mut err: Signal<Option<String>>,
) {
    let lid = lobby_id.to_string();
    spawn(async move {
        match fetch_lobby_detail(&lid).await {
            Ok(d) => detail.set(d),
            Err(msg) => err.set(Some(msg)),
        }
    });
}

fn spawn_lobby_room_poll(
    lobby_id: String,
    mut detail: Signal<Option<LobbyDetail>>,
    mut err: Signal<Option<String>>,
) {
    spawn(async move {
        loop {
            match fetch_lobby_detail(&lobby_id).await {
                Ok(d) => detail.set(d),
                Err(msg) => err.set(Some(msg)),
            }
            TimeoutFuture::new(4_000).await;
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
    // Keep lobby state fresh when WS is down (e.g. dx on a non-8080 port) or events are missed.
    spawn_lobby_room_poll(lobby_id.clone(), detail, err);
    spawn(async move {
        loop {
            let Some(mut ws) = connect_graphql_ws().await else {
                refetch_lobby_room(&lobby_id, detail, err);
                TimeoutFuture::new(5_000).await;
                continue;
            };
            let q = lobby_room_subscription();
            let sub = serde_json::json!({
                "type": "start",
                "id": "room1",
                "payload": {
                    "query": q,
                    "variables": { "id": lobby_id }
                }
            });
            if ws.send(Message::Text(sub.to_string())).await.is_err() {
                refetch_lobby_room(&lobby_id, detail, err);
                TimeoutFuture::new(5_000).await;
                continue;
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
                    refetch_lobby_room(&lobby_id, detail, err);
                    continue;
                };
                let Some(raw) = data.get("lobbyUpdated").cloned() else {
                    refetch_lobby_room(&lobby_id, detail, err);
                    continue;
                };
                match serde_json::from_value::<LobbyDetail>(raw) {
                    Ok(d) => detail.set(Some(d)),
                    Err(_) => refetch_lobby_room(&lobby_id, detail, err),
                }
            }
            refetch_lobby_room(&lobby_id, detail, err);
            TimeoutFuture::new(2_000).await;
        }
    });
}
