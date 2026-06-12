use crate::api::{graphql_exec, graphql_post, set_lobby_game_type, start_lobby_room_subscription, stored_user_id};
use crate::components::lobby::LobbyRoomBody;
use crate::components::ui::{Avatar, AvatarSize, EmptyState, ErrorBanner, Icon, Skeleton, StatusBadge, status_variant_from_lobby};
use crate::models::{GameTypeInfo, LobbyDetail, PlayOverlay};
use crate::LobbyRoute;
use dioxus::prelude::*;
use serde::Deserialize;

#[component]
pub fn LobbyRoomPage(
    lobby_id: String,
    mut playing: Signal<Option<PlayOverlay>>,
    mut error_msg: Signal<Option<String>>,
) -> Element {
    let nav = use_navigator();
    let mut detail: Signal<Option<LobbyDetail>> = use_signal(|| None);
    let mut game_types: Signal<Vec<GameTypeInfo>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut seats_bootstrapped = use_signal(|| false);

    use_hook(move || {
        let lid_fetch = lobby_id.clone();
        let lid_sub = lobby_id.clone();
        let mut detail_f = detail;
        let mut game_types_f = game_types;
        let mut error_msg_f = error_msg;
        let mut loading = loading;
        start_lobby_room_subscription(lid_sub, detail, error_msg);
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Gt { game_types: Vec<GameTypeInfo> }
            let gt_q = r#"query { gameTypes { name displayName version minPlayers maxPlayers description configUiPath aboutUiPath configSchemaJson coverImageUrl activePlayers featured tags creatorDisplayName avgSessionMins } }"#;
            if let Ok(g) = graphql_post::<Gt>(gt_q).await {
                game_types_f.set(g.game_types);
            }
            let q = r#"query L($id: ID!) { lobby(id: $id) { id ownerUserId ownerDisplayName gameType configJson status gameInstanceId createdAt updatedAt seats { seatIndex playerIdentity claimedByUserId claimedDisplayName ready } messages { id userId displayName body createdAt } } }"#;
            let vars = serde_json::json!({ "id": lid_fetch });
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Ld { lobby: Option<LobbyDetail> }
            match graphql_exec::<Ld>(q, Some(vars)).await {
                Ok(d) => detail_f.set(d.lobby),
                Err(e) => error_msg_f.set(Some(e)),
            }
            loading.set(false);
        });
    });

    use_effect(move || {
        let Some(ref l) = detail() else { return; };
        let user_id = stored_user_id();
        if l.status == "in_game" {
            if let (Some(uid), Some(gid)) = (user_id.as_ref(), l.game_instance_id.as_ref()) {
                if let Some(seat) = l.seats.iter().find(|s| s.claimed_by_user_id.as_deref() == Some(uid.as_str())) {
                    playing.set(Some(PlayOverlay {
                        game_type: l.game_type.clone(),
                        game_id: gid.clone(),
                        player: seat.player_identity.clone(),
                        return_lobby_id: Some(l.id.clone()),
                        spectator: false,
                    }));
                    return;
                }
            }
        }
        let overlay_this_lobby = playing().as_ref().and_then(|p| p.return_lobby_id.as_deref()) == Some(l.id.as_str());
        if overlay_this_lobby && l.status != "in_game" {
            playing.set(None);
        }
    });

    use_effect(move || {
        if seats_bootstrapped() {
            return;
        }
        let Some(ref l) = detail() else { return; };
        let gt = l.game_type.trim();
        if gt.is_empty() || !l.seats.is_empty() {
            return;
        }
        let uid = stored_user_id();
        if uid.as_deref() != Some(l.owner_user_id.as_str()) {
            return;
        }
        seats_bootstrapped.set(true);
        let lid = l.id.clone();
        let gt = gt.to_string();
        let mut detail_f = detail;
        spawn(async move {
            if let Ok(updated) = set_lobby_game_type(&lid, &gt, false).await {
                detail_f.set(Some(updated));
            }
        });
    });

    let d = detail();
    let gt_list = game_types();
    let uid = stored_user_id();
    let mut detail_for_body = detail;

    rsx! {
        div { class: "lobby-room-wrap",
            if let Some(err) = error_msg() {
                ErrorBanner { message: err }
            }
            if loading() {
                div { class: "lobby-arena-skeleton mx-6",
                    Skeleton { class: Some("h-32 w-full rounded-2xl".into()) }
                    Skeleton { class: Some("h-48 w-full rounded-2xl".into()) }
                    Skeleton { class: Some("h-64 w-full rounded-2xl".into()) }
                }
            } else if let Some(ref l) = d {
                header { class: "lobby-command-bar lobby-command-bar-slim",
                    button {
                        class: "lobby-command-back",
                        onclick: move |_| { nav.push(LobbyRoute::LobbiesBrowser {}); },
                        Icon { name: "arrow_back", filled: false }
                        "Lobbies"
                    }
                    div { class: "lobby-command-host shrink-0 ml-auto",
                        Avatar {
                            seed: l.owner_user_id.clone(),
                            size: AvatarSize::Sm,
                            image_url: None,
                        }
                        div { class: "min-w-0 hidden sm:block",
                            p { class: "text-[10px] font-label-caps uppercase text-outline", "Host" }
                            p { class: "text-body-sm font-medium text-on-surface truncate", "{l.owner_display_name}" }
                        }
                    }
                    StatusBadge {
                        label: l.status.clone(),
                        variant: status_variant_from_lobby(
                            &l.status,
                            l.seats.iter().filter(|s| s.claimed_by_user_id.is_some()).count() as i32,
                            l.seats.len() as i32,
                        ),
                    }
                }
                LobbyRoomBody {
                    lobby_for_cols: l.clone(),
                    gt_list: gt_list.clone(),
                    uid: uid.clone(),
                    on_detail_updated: EventHandler::new(move |updated: LobbyDetail| {
                        detail_for_body.set(Some(updated));
                    }),
                }
            } else {
                EmptyState {
                    icon: "search_off",
                    title: "Lobby not found".to_string(),
                    description: "This lobby may have been closed or the link is invalid.".to_string(),
                    cta_label: Some("Browse lobbies".to_string()),
                    on_cta: Some(EventHandler::new(move |_| {
                        nav.push(LobbyRoute::LobbiesBrowser {});
                    })),
                }
            }
        }
    }
}
