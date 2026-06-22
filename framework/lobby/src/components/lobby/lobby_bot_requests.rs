use crate::api::graphql_exec;
use crate::components::ui::{push_toast, use_toast, Chip, Icon, ToastKind};
use crate::models::{LobbyBotRequest, LobbyDetail};
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn LobbyBotRequestCards(
    lobby_id: String,
    game_type: String,
    contract_hash: Option<String>,
    requests: Vec<LobbyBotRequest>,
    is_owner: bool,
    seats: Vec<crate::models::LobbySeat>,
    on_detail_updated: EventHandler<LobbyDetail>,
) -> Element {
    if !is_owner {
        return rsx! {};
    }
    let pending: Vec<_> = requests
        .into_iter()
        .filter(|r| r.status == "pending")
        .collect();
    if pending.is_empty() {
        return rsx! {};
    }

    let toast = use_toast();

    rsx! {
        div { class: "lobby-bot-requests mb-4 space-y-2",
            p { class: "text-label-caps font-label-caps text-outline uppercase",
                "Bot seat requests ({pending.len()})"
            }
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
                    rsx! {
                        div { class: "section-card p-4 flex flex-col sm:flex-row sm:items-center gap-3",
                            div { class: "flex-1 min-w-0",
                                div { class: "flex flex-wrap gap-2 mb-1",
                                    Chip { label: badge.to_string(), muted: false }
                                    if mismatch {
                                        Chip { label: "CONTRACT MISMATCH".to_string(), muted: true }
                                    }
                                }
                                p { class: "font-medium text-on-surface", "{label}" }
                                p { class: "text-body-sm text-on-surface-variant",
                                    "Game: {gt} · hash {hash_short}…"
                                }
                                if let Some(d) = desired {
                                    p { class: "text-body-sm text-on-surface-variant", "Requested seat {d + 1}" }
                                }
                            }
                            div { class: "flex gap-2 shrink-0",
                                button {
                                    class: "btn-primary",
                                    onclick: move |_| {
                                        let lid = lid_approve.clone();
                                        let rid = rid_approve.clone();
                                        let seats = seats.clone();
                                        let toast = toast;
                                        let on_detail_updated = on_detail_updated;
                                        spawn(async move {
                                            let seat_index = desired.unwrap_or_else(|| {
                                                seats.iter()
                                                    .find(|s| s.bot_id.is_none() && !s.external_bot && s.claimed_by_user_id.is_none())
                                                    .map(|s| s.seat_index)
                                                    .unwrap_or(0)
                                            });
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
                                    class: "btn-ghost",
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
