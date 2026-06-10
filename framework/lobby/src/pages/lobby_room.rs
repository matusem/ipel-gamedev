use crate::api::{graphql_exec, graphql_post, start_lobby_room_subscription, stored_user_id};
use crate::components::lobby::LobbyRoomBody;
use crate::components::ui::{EmptyState, ErrorBanner, Skeleton, StatusBadge, status_variant_from_lobby};
use crate::models::{game_type_description, game_type_display_title, GameTypeInfo, LobbyDetail, PlayOverlay};
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
    let my_user_id = use_signal(|| stored_user_id());

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
            let gt_q = r#"query { gameTypes { name displayName version minPlayers maxPlayers description configUiPath aboutUiPath configSchemaJson } }"#;
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
        let user_id = my_user_id();
        if l.status == "in_game" {
            if let (Some(uid), Some(gid)) = (user_id.as_ref(), l.game_instance_id.as_ref()) {
                if let Some(seat) = l.seats.iter().find(|s| s.claimed_by_user_id.as_deref() == Some(uid.as_str())) {
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
        let overlay_this_lobby = playing().as_ref().and_then(|p| p.return_lobby_id.as_deref()) == Some(l.id.as_str());
        if overlay_this_lobby && l.status != "in_game" {
            playing.set(None);
        }
    });

    let d = detail();
    let gt_list = game_types();
    let uid = my_user_id();

    rsx! {
        div { class: "lobby-room-wrap",
            if let Some(err) = error_msg() {
                ErrorBanner { message: err }
            }
            if loading() {
                div { class: "lobby-room-grid",
                    for _ in 0..3 {
                        div { class: "lobby-col",
                            Skeleton { class: Some("h-48 w-full".into()) }
                        }
                    }
                }
            } else if let Some(ref l) = d {
                div { class: "section-card flex flex-wrap items-center gap-4 mb-6",
                    button {
                        class: "btn-ghost shrink-0",
                        onclick: move |_| { nav.push(LobbyRoute::LobbiesBrowser {}); },
                        "← Lobbies"
                    }
                    div { class: "min-w-0 flex-1",
                        h1 { class: "font-manrope text-h2 text-on-surface",
                            "{game_type_display_title(&gt_list, &l.game_type)}"
                        }
                        if let Some(ref sd) = game_type_description(&gt_list, &l.game_type) {
                            p { class: "text-body-sm text-on-surface-variant mt-1", "{sd}" }
                        }
                    }
                    StatusBadge {
                        label: l.status.clone(),
                        variant: status_variant_from_lobby(&l.status, l.seats.iter().filter(|s| s.claimed_by_user_id.is_some()).count() as i32, l.seats.len() as i32),
                    }
                }
                LobbyRoomBody {
                    lobby_for_cols: l.clone(),
                    gt_list: gt_list.clone(),
                    uid: uid.clone(),
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
