use crate::api::{graphql_exec, kick_lobby_player, transfer_lobby_ownership};
use crate::components::ui::{push_toast, use_toast, Avatar, AvatarSize, Icon, ToastKind};
use crate::models::{LobbyDetail, LobbySeat};
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn LobbyPlayerCard(
    seat: LobbySeat,
    lobby_id: String,
    owner_user_id: String,
    my_user_id: Option<String>,
    viewer_is_owner: bool,
    in_staging: bool,
    on_detail_updated: EventHandler<LobbyDetail>,
) -> Element {
    let toast = use_toast();
    let taken = seat.claimed_by_user_id.is_some();
    let is_me = my_user_id
        .as_deref()
        .is_some_and(|u| seat.claimed_by_user_id.as_deref() == Some(u));
    let is_host = seat
        .claimed_by_user_id
        .as_deref()
        .is_some_and(|u| u == owner_user_id.as_str());
    let can_transfer = viewer_is_owner
        && in_staging
        && taken
        && !is_me
        && !is_host;
    let can_kick = can_transfer;
    let display = seat
        .claimed_display_name
        .clone()
        .unwrap_or_else(|| seat.player_identity.clone());
    let avatar_seed = seat
        .claimed_by_user_id
        .clone()
        .unwrap_or_else(|| seat.player_identity.clone());
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
                    div { class: "lobby-player-card-portrait",
                        Avatar { seed: avatar_seed, size: avatar_size, image_url: None }
                    }
                    div { class: "lobby-player-card-meta",
                        p { class: "lobby-player-card-name", "{display}" }
                        p { class: "lobby-player-card-playing-kicker", "playing as" }
                        p { class: "lobby-player-card-slot", "{seat.player_identity}" }
                        div { class: "lobby-player-card-transfer-row",
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
                                                let on_detail_updated = on_detail_updated;
                                                let confirm = web_sys::window()
                                                    .map(|w| {
                                                        w.confirm_with_message(&format!(
                                                            "Make {label} the lobby host? You will lose host controls.",
                                                        ))
                                                        .unwrap_or(false)
                                                    })
                                                    .unwrap_or(false);
                                                if !confirm {
                                                    return;
                                                }
                                                spawn(async move {
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
                                                let on_detail_updated = on_detail_updated;
                                                let confirm = web_sys::window()
                                                    .map(|w| {
                                                        w.confirm_with_message(&format!(
                                                            "Remove {label} from the lobby?",
                                                        ))
                                                        .unwrap_or(false)
                                                    })
                                                    .unwrap_or(false);
                                                if !confirm {
                                                    return;
                                                }
                                                spawn(async move {
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
