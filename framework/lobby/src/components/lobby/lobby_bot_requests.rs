use std::collections::HashMap;

use crate::api::graphql_exec;
use crate::components::ui::{push_toast, use_toast, Chip, Icon, ToastKind};
use crate::models::{LobbyBotRequest, LobbyDetail, LobbySeat};
use dioxus::prelude::*;
use serde_json::Value;

fn seat_is_open(s: &LobbySeat) -> bool {
    s.claimed_by_user_id.is_none() && s.bot_id.is_none() && !s.external_bot
}

fn default_seat_for_request(seats: &[LobbySeat], desired: Option<i32>) -> i32 {
    if let Some(d) = desired {
        if seats.iter().any(|s| s.seat_index == d && seat_is_open(s)) {
            return d;
        }
    }
    seats
        .iter()
        .find(|s| seat_is_open(s))
        .map(|s| s.seat_index)
        .unwrap_or(0)
}

#[component]
pub fn LobbyBotRequestCards(
    lobby_id: String,
    game_type: String,
    contract_hash: Option<String>,
    requests: Vec<LobbyBotRequest>,
    is_owner: bool,
    seats: Vec<LobbySeat>,
    panel_open: Signal<bool>,
    on_detail_updated: EventHandler<LobbyDetail>,
) -> Element {
    let pending: Vec<_> = requests
        .into_iter()
        .filter(|r| r.status == "pending")
        .collect();
    if pending.is_empty() {
        return rsx! {};
    }

    let mut open = panel_open;
    let toast = use_toast();
    let mut seat_choices: Signal<HashMap<String, i32>> = use_signal(HashMap::new);
    let pending_count = pending.len();

    if open() {
        rsx! {
            div { class: "lobby-bot-panel",
                div { class: "lobby-bot-panel-head",
                    div {
                        p { class: "lobby-section-kicker", "BOT REQUESTS" }
                        p { class: "font-manrope font-semibold text-on-surface text-sm",
                            if is_owner {
                                "{pending_count} pending"
                            } else {
                                "{pending_count} waiting for host"
                            }
                        }
                    }
                    button {
                        class: "lobby-drawer-close",
                        onclick: move |_| open.set(false),
                        Icon { name: "close", filled: false }
                    }
                }
                div { class: "lobby-bot-panel-body",
                    for req in pending {
                        {
                            let lid = lobby_id.clone();
                            let gt = game_type.clone();
                            let ch = contract_hash.clone();
                            let seats = seats.clone();
                            let rid = req.id.clone();
                            let label = req.label.clone();
                            let category = req.category.clone();
                            let req_hash = req.contract_hash.clone();
                            let desired = req.desired_seat_index;
                            let toast = toast;
                            let on_detail_updated = on_detail_updated;
                            let mismatch = ch.as_deref().is_some_and(|h| h != req_hash.as_str());
                            let badge = if category == "external" { "EXTERNAL BOT" } else { "DEV BOT" };
                            let hash_short: String = req_hash.chars().take(8).collect();
                            let lid_approve = lid.clone();
                            let rid_approve = rid.clone();
                            let lid_deny = lid.clone();
                            let rid_deny = rid.clone();
                            let open_seats: Vec<_> = seats
                                .iter()
                                .filter(|s| seat_is_open(s))
                                .cloned()
                                .collect();
                            let default_seat = default_seat_for_request(&seats, desired);
                            let selected_seat = seat_choices()
                                .get(&rid)
                                .copied()
                                .unwrap_or(default_seat);
                            rsx! {
                                div { class: "lobby-bot-request-card",
                                    div { class: "flex flex-wrap gap-2",
                                        Chip { label: badge.to_string(), muted: false }
                                        if mismatch {
                                            Chip { label: "CONTRACT MISMATCH".to_string(), muted: true }
                                        }
                                    }
                                    p { class: "font-medium text-on-surface text-sm", "{label}" }
                                    p { class: "text-body-sm text-on-surface-variant",
                                        "Game: {gt} · hash {hash_short}…"
                                    }
                                    if let Some(d) = desired {
                                        p { class: "text-body-sm text-on-surface-variant",
                                            "Requested seat {d + 1}"
                                        }
                                    }
                                    if !is_owner {
                                        p { class: "text-body-sm text-on-surface-variant",
                                            "Waiting for host approval"
                                        }
                                    }
                                    if is_owner {
                                        div { class: "lobby-bot-request-actions",
                                            if !open_seats.is_empty() {
                                                label { class: "lobby-bot-request-seat-row",
                                                    span { class: "text-on-surface-variant", "Seat" }
                                                    select {
                                                        class: "input-field w-full",
                                                        value: "{selected_seat}",
                                                        onchange: move |evt| {
                                                            if let Ok(v) = evt.value().parse::<i32>() {
                                                                seat_choices.write().insert(rid.clone(), v);
                                                            }
                                                        },
                                                        for seat in open_seats.iter() {
                                                            option {
                                                                value: "{seat.seat_index}",
                                                                "{seat.player_identity} (seat {seat.seat_index + 1})"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            div { class: "lobby-bot-request-buttons",
                                                button {
                                                    class: "btn-primary flex-1 text-sm py-2",
                                                    disabled: open_seats.is_empty(),
                                                    onclick: move |_| {
                                                        let lid = lid_approve.clone();
                                                        let rid = rid_approve.clone();
                                                        let seat_index = seat_choices()
                                                            .get(&rid)
                                                            .copied()
                                                            .unwrap_or(default_seat);
                                                        let toast = toast;
                                                        let on_detail_updated = on_detail_updated;
                                                        spawn(async move {
                                                            let q = r#"mutation A($lid: ID!, $rid: ID!, $seat: Int!) { approveExternalBotSeat(lobbyId: $lid, requestId: $rid, seatIndex: $seat) { id } }"#;
                                                            let vars = serde_json::json!({ "lid": lid, "rid": rid, "seat": seat_index });
                                                            match graphql_exec::<Value>(q, Some(vars)).await {
                                                                Ok(_) => {
                                                                    let detail_q = format!(
                                                                        "query L($id: ID!) {{ lobby(id: $id) {{ {} }} }}",
                                                                        crate::api::graphql::LOBBY_DETAIL_FIELDS
                                                                    );
                                                                    if let Ok(v) = graphql_exec::<Value>(&detail_q, Some(serde_json::json!({ "id": lid }))).await {
                                                                        if let Ok(d) = serde_json::from_value::<LobbyDetail>(v.get("lobby").cloned().unwrap_or(Value::Null)) {
                                                                            on_detail_updated.call(d);
                                                                        }
                                                                    }
                                                                    push_toast(toast.show, "Bot seat approved", ToastKind::Success);
                                                                }
                                                                Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                            }
                                                        });
                                                    },
                                                    "Approve"
                                                }
                                                button {
                                                    class: "btn-ghost px-3",
                                                    onclick: move |_| {
                                                        let lid = lid_deny.clone();
                                                        let rid = rid_deny.clone();
                                                        let toast = toast;
                                                        let on_detail_updated = on_detail_updated;
                                                        spawn(async move {
                                                            let q = r#"mutation D($lid: ID!, $rid: ID!) { denyExternalBotSeat(lobbyId: $lid, requestId: $rid) { id } }"#;
                                                            let vars = serde_json::json!({ "lid": lid, "rid": rid });
                                                            match graphql_exec::<Value>(q, Some(vars)).await {
                                                                Ok(_) => {
                                                                    let detail_q = format!(
                                                                        "query L($id: ID!) {{ lobby(id: $id) {{ {} }} }}",
                                                                        crate::api::graphql::LOBBY_DETAIL_FIELDS
                                                                    );
                                                                    if let Ok(v) = graphql_exec::<Value>(&detail_q, Some(serde_json::json!({ "id": lid }))).await {
                                                                        if let Ok(d) = serde_json::from_value::<LobbyDetail>(v.get("lobby").cloned().unwrap_or(Value::Null)) {
                                                                            on_detail_updated.call(d);
                                                                        }
                                                                    }
                                                                    push_toast(toast.show, "Request denied", ToastKind::Success);
                                                                }
                                                                Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                            }
                                                        });
                                                    },
                                                    Icon { name: "close", filled: false }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        rsx! {
            button {
                class: "lobby-bot-fab",
                onclick: move |_| open.set(true),
                Icon { name: "smart_toy", filled: false }
                span { class: "lobby-bot-fab-label", "Bot requests" }
                span { class: "lobby-bot-fab-badge", "{pending_count}" }
            }
        }
    }
}
