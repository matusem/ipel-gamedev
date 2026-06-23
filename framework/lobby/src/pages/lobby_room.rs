use crate::api::{graphql_exec, graphql_post, set_lobby_game_type, start_lobby_room_subscription, stored_user_id};
use crate::components::lobby::{LobbyResultsModal, LobbyRoomBody};
use crate::components::ui::{EmptyState, ErrorBanner, Skeleton};
use crate::models::{GameTypeInfo, LobbyDetail, PlayOverlay};
use crate::LobbyRoute;
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::Value;

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
    let mut results_open = use_signal(|| false);
    let mut results_initial_game = use_signal(|| None::<String>);
    let mut results_shown_for_game = use_signal(|| None::<String>);
    let mut reopened_for_game = use_signal(|| None::<String>);

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
            let gt_q = format!(
                "query {{ gameTypes {{ {} }} }}",
                crate::models::GAME_TYPES_GQL_FIELDS
            );
            if let Ok(g) = graphql_post::<Gt>(&gt_q).await {
                game_types_f.set(g.game_types);
            }
            let q = crate::api::graphql::lobby_room_query();
            let vars = serde_json::json!({ "id": lid_fetch });
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Ld { lobby: Option<LobbyDetail> }
            match graphql_exec::<Ld>(&q, Some(vars)).await {
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
                        is_lobby_owner: user_id.as_deref() == Some(l.owner_user_id.as_str()),
                    }));
                    return;
                }
            }
        }
        let overlay_this_lobby = playing().as_ref().and_then(|p| p.return_lobby_id.as_deref()) == Some(l.id.as_str());
        if overlay_this_lobby && l.status != "in_game" {
            playing.set(None);
        }
        if l.status == "finished" {
            if let Some(gid) = l.game_instance_id.clone() {
                if results_shown_for_game().as_deref() != Some(gid.as_str()) {
                    results_shown_for_game.set(Some(gid.clone()));
                    results_initial_game.set(Some(gid.clone()));
                    results_open.set(true);
                }
                if user_id.as_deref() == Some(l.owner_user_id.as_str())
                    && reopened_for_game().as_deref() != Some(gid.as_str())
                {
                    reopened_for_game.set(Some(gid.clone()));
                    let lid = l.id.clone();
                    spawn(async move {
                        let q = "mutation R($id: ID!) { reopenLobbyAfterGame(lobbyId: $id) }";
                        let vars = serde_json::json!({ "id": lid });
                        let _ = graphql_exec::<Value>(q, Some(vars)).await;
                    });
                }
            }
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
                {
                    let lobby_id_results = l.id.clone();
                    let game_type_results = l.game_type.clone();
                    let is_owner = uid.as_deref() == Some(l.owner_user_id.as_str());
                    let lobby_finished = l.status == "finished";
                    let lid_play_again = l.id.clone();
                    rsx! {
                LobbyRoomBody {
                    lobby_for_cols: l.clone(),
                    gt_list: gt_list.clone(),
                    uid: uid.clone(),
                    on_detail_updated: EventHandler::new(move |updated: LobbyDetail| {
                        detail_for_body.set(Some(updated));
                    }),
                    on_open_history: EventHandler::new(move |_| {
                        results_initial_game.set(None);
                        results_open.set(true);
                    }),
                }
                LobbyResultsModal {
                    open: results_open(),
                    on_close: EventHandler::new(move |_| results_open.set(false)),
                    lobby_id: lobby_id_results.clone(),
                    game_type: game_type_results.clone(),
                    is_owner,
                    lobby_finished,
                    initial_game_id: results_initial_game(),
                    on_play_again: EventHandler::new(move |_| {
                        let lid = lid_play_again.clone();
                        spawn(async move {
                            let q = "mutation R($id: ID!) { reopenLobbyAfterGame(lobbyId: $id) }";
                            let vars = serde_json::json!({ "id": lid });
                            let _ = graphql_exec::<Value>(q, Some(vars)).await;
                        });
                        results_open.set(false);
                    }),
                }
                    }
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
