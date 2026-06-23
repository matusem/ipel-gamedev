use crate::api::{graphql_exec, kick_lobby_player, transfer_lobby_ownership};
use crate::components::lobby::LobbyBotPicker;
use crate::components::ui::{push_toast, use_confirm, use_toast, Avatar, AvatarSize, Icon, ToastKind};
use crate::models::{LobbyDetail, LobbySeat};
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn LobbyPlayerCard(
    seat: LobbySeat,
    lobby_id: String,
    game_type: String,
    owner_user_id: String,
    my_user_id: Option<String>,
    viewer_is_owner: bool,
    in_staging: bool,
    on_detail_updated: EventHandler<LobbyDetail>,
) -> Element {
    let toast = use_toast();
    let confirm = use_confirm();
    let mut picker_open = use_signal(|| false);
    let seat_external = seat.external_bot;

    let taken = seat.claimed_by_user_id.is_some() || seat.bot_id.is_some() || seat.external_bot;
    let is_published_bot = seat.bot_id.is_some() && !seat.external_bot;
    let is_dev_bot = seat.external_bot && seat.external_bot_category.as_deref() == Some("dev_local");
    let is_external_bot = seat.external_bot && seat.external_bot_category.as_deref() == Some("external");
    let is_bot = is_published_bot || seat.external_bot;
    let is_me = my_user_id
        .as_deref()
        .is_some_and(|u| seat.claimed_by_user_id.as_deref() == Some(u));
    let is_host = seat
        .claimed_by_user_id
        .as_deref()
        .is_some_and(|u| u == owner_user_id.as_str());
    let can_transfer = viewer_is_owner && in_staging && taken && !is_me && !is_host && !is_bot;
    let can_kick = can_transfer;
    let display = seat
        .bot_display_name
        .clone()
        .or(seat.claimed_display_name.clone())
        .unwrap_or_else(|| seat.player_identity.clone());
    let avatar_seed = seat
        .bot_avatar_seed
        .clone()
        .or(seat.claimed_by_user_id.clone())
        .or(seat.bot_id.clone())
        .unwrap_or_else(|| seat.player_identity.clone());
    let avatar_url = seat.bot_avatar_url.clone();
    let avatar_size = if is_me {
        AvatarSize::Hero
    } else {
        AvatarSize::Xl
    };

    let card_class = if !taken {
        "lobby-player-card lobby-player-card-open"
    } else if seat.ready {
        "lobby-player-card lobby-player-card-ready"
    } else if is_me {
        "lobby-player-card lobby-player-card-me"
    } else {
        "lobby-player-card lobby-player-card-filled"
    };

    let wrap_class = if is_me {
        "lobby-player-card-wrap lobby-player-card-wrap-me"
    } else {
        "lobby-player-card-wrap lobby-player-card-wrap-other"
    };

    rsx! {
        LobbyBotPicker {
            open: picker_open(),
            on_close: EventHandler::new(move |_| picker_open.set(false)),
            lobby_id: lobby_id.clone(),
            game_type: game_type.clone(),
            seat_index: seat.seat_index,
            player_identity: seat.player_identity.clone(),
            on_detail_updated,
        }

        div {
            class: "{wrap_class}",
            "data-me": if is_me { "true" } else { "false" },
            if taken {
                div { class: "{card_class}",
                    if is_host {
                        span { class: "lobby-player-card-host-badge",
                            Icon { name: "military_tech", filled: true }
                            "HOST"
                        }
                    }
                    if is_published_bot {
                        span { class: "lobby-player-card-host-badge",
                            Icon { name: "smart_toy", filled: true }
                            "BOT"
                        }
                    }
                    if is_dev_bot {
                        span { class: "lobby-player-card-host-badge",
                            Icon { name: "code", filled: true }
                            "DEV BOT"
                        }
                    }
                    if is_external_bot {
                        span { class: "lobby-player-card-host-badge",
                            Icon { name: "cloud", filled: true }
                            "EXTERNAL BOT"
                        }
                    }
                    div { class: "lobby-player-card-portrait",
                        Avatar { seed: avatar_seed, size: avatar_size, image_url: avatar_url }
                    }
                    div { class: "lobby-player-card-meta",
                        p { class: "lobby-player-card-name", "{display}" }
                        p { class: "lobby-player-card-playing-kicker", "playing as" }
                        p { class: "lobby-player-card-slot", "{seat.player_identity}" }
                        if seat.external_bot && in_staging {
                            p { class: "text-body-sm text-on-surface-variant", "waiting for runner" }
                        }
                        div { class: "lobby-player-card-transfer-row",
                            if viewer_is_owner && in_staging && is_bot {
                                {
                                    let lid = lobby_id.clone();
                                    let idx = seat.seat_index;
                                    let toast = toast;
                                    let on_detail_updated = on_detail_updated;
                                    let ext = seat_external;
                                    rsx! {
                                        button {
                                            class: "lobby-player-card-kick",
                                            title: "Remove bot from seat",
                                            onclick: move |_| {
                                                let lid = lid.clone();
                                                let toast = toast;
                                                let on_detail_updated = on_detail_updated;
                                                spawn(async move {
                                                    let q = if ext {
                                                        r#"mutation R($id: ID!, $i: Int!) { releaseExternalBotSeat(lobbyId: $id, seatIndex: $i) { id } }"#
                                                    } else {
                                                        r#"mutation R($id: ID!, $i: Int!) { removeBotFromSeat(lobbyId: $id, seatIndex: $i) { id } }"#
                                                    };
                                                    let vars = serde_json::json!({ "id": lid, "i": idx });
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
                                                            push_toast(toast.show, "Bot removed", ToastKind::Success);
                                                        }
                                                        Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                    }
                                                });
                                            },
                                            Icon { name: "person_remove", filled: false }
                                            if ext { "Release" } else { "Remove" }
                                        }
                                    }
                                }
                            }
                            if can_transfer {
                                {
                                    let lid = lobby_id.clone();
                                    let new_owner = seat.claimed_by_user_id.clone().unwrap_or_default();
                                    let label = display.clone();
                                    rsx! {
                                        button {
                                            class: "lobby-player-card-transfer",
                                            title: "Make this player the lobby host",
                                            onclick: move |_| {
                                                let lid = lid.clone();
                                                let new_owner = new_owner.clone();
                                                let label = label.clone();
                                                let toast = toast;
                                                let confirm = confirm;
                                                let on_detail_updated = on_detail_updated;
                                                spawn(async move {
                                                    if !confirm
                                                        .confirm(format!(
                                                            "Make {label} the lobby host? You will lose host controls.",
                                                        ))
                                                        .await
                                                    {
                                                        return;
                                                    }
                                                    match transfer_lobby_ownership(&lid, &new_owner).await {
                                                        Ok(updated) => {
                                                            on_detail_updated.call(updated);
                                                            push_toast(
                                                                toast.show,
                                                                format!("{label} is now the host"),
                                                                ToastKind::Success,
                                                            );
                                                        }
                                                        Err(e) => {
                                                            push_toast(toast.show, e, ToastKind::Error);
                                                        }
                                                    }
                                                });
                                            },
                                            Icon { name: "swap_horiz", filled: false }
                                            "Make host"
                                        }
                                    }
                                }
                            }
                            if can_kick {
                                {
                                    let lid = lobby_id.clone();
                                    let target = seat.claimed_by_user_id.clone().unwrap_or_default();
                                    let label = display.clone();
                                    rsx! {
                                        button {
                                            class: "lobby-player-card-kick",
                                            title: "Remove this player from the lobby",
                                            onclick: move |_| {
                                                let lid = lid.clone();
                                                let target = target.clone();
                                                let label = label.clone();
                                                let toast = toast;
                                                let confirm = confirm;
                                                let on_detail_updated = on_detail_updated;
                                                spawn(async move {
                                                    if !confirm
                                                        .confirm(format!("Remove {label} from the lobby?"))
                                                        .await
                                                    {
                                                        return;
                                                    }
                                                    match kick_lobby_player(&lid, &target).await {
                                                        Ok(updated) => {
                                                            on_detail_updated.call(updated);
                                                            push_toast(
                                                                toast.show,
                                                                format!("{label} was removed"),
                                                                ToastKind::Success,
                                                            );
                                                        }
                                                        Err(e) => {
                                                            push_toast(toast.show, e, ToastKind::Error);
                                                        }
                                                    }
                                                });
                                            },
                                            Icon { name: "person_remove", filled: false }
                                            "Kick"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if in_staging {
                div { class: "lobby-player-card lobby-player-card-open",
                    button {
                        class: "lobby-player-card-join",
                        onclick: move |_| {
                            let lid = lobby_id.clone();
                            let idx = seat.seat_index;
                            let toast = toast;
                            spawn(async move {
                                let q = "mutation J($id: ID!, $i: Int!) { joinLobby(lobbyId: $id, seatIndex: $i) { id } }";
                                let vars = serde_json::json!({ "id": lid, "i": idx });
                                match graphql_exec::<Value>(q, Some(vars)).await {
                                    Ok(_) => push_toast(toast.show, "Seat claimed", ToastKind::Success),
                                    Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                }
                            });
                        },
                        div { class: "lobby-player-card-portrait lobby-player-card-portrait-empty",
                            span { class: "lobby-player-card-plus", "+" }
                        }
                        div { class: "lobby-player-card-meta",
                            p { class: "lobby-player-card-name lobby-player-card-join-label", "JOIN" }
                            p { class: "lobby-player-card-playing-kicker", "playing as" }
                            p { class: "lobby-player-card-slot", "{seat.player_identity}" }
                            div { class: "lobby-player-card-transfer-row" }
                        }
                    }
                    if !game_type.is_empty() && viewer_is_owner {
                        div { class: "lobby-player-card-open-footer",
                            button {
                                class: "lobby-player-card-add-bot",
                                title: "Add a published bot to this seat",
                                onclick: move |_| picker_open.set(true),
                                Icon { name: "smart_toy", filled: false }
                                "Add bot"
                            }
                        }
                    }
                }
            } else {
                div { class: "{card_class}",
                    div { class: "lobby-player-card-portrait lobby-player-card-portrait-empty opacity-40",
                        span { class: "lobby-player-card-plus", "—" }
                    }
                    div { class: "lobby-player-card-meta",
                        p { class: "lobby-player-card-name lobby-player-card-join-label text-outline", "OPEN" }
                        p { class: "lobby-player-card-playing-kicker", "playing as" }
                        p { class: "lobby-player-card-slot", "{seat.player_identity}" }
                        div { class: "lobby-player-card-transfer-row" }
                    }
                }
            }
        }
    }
}
