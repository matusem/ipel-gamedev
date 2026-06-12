use crate::api::*;
use crate::components::lobby::{
    LobbyActiveGame, LobbyFloatingChat, LobbyGameModal, LobbyGameRulesModal, LobbyPlayerCard,
};
use crate::components::ui::{push_toast, status_variant_from_lobby, Icon, JsonConsole, StatusBadge, use_toast, ToastKind};
use crate::models::*;
use crate::stub::game_media;
use crate::LobbyRoute;
use dioxus::prelude::*;
use gloo_events::EventListener;
use gloo_timers::future::TimeoutFuture;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
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
pub fn LobbyConfigPanel(
    lobby_id: String,
    game_type: String,
    iframe_src: String,
    schema_json: Option<String>,
    read_only: bool,
    server_config_json: Option<String>,
) -> Element {
    let toast = use_toast();
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
            class: if read_only {
                "lobby-config-iframe config-iframe pointer-events-none opacity-90"
            } else {
                "lobby-config-iframe config-iframe"
            },
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
        details { class: "lobby-config-preview mt-3",
            summary { class: "lobby-config-preview-toggle",
                Icon { name: "data_object", filled: false }
                "Live config preview"
            }
            div { class: "mt-2",
                JsonConsole { content: preview(), max_height: Some("max-h-40") }
            }
        }
        if !read_only {
            button {
                class: "btn-primary lobby-config-apply mt-4 w-full sm:w-auto",
                onclick: move |_| {
                    let cfg = draft_for_apply.borrow().clone();
                    let lid = lobby_id_apply.clone();
                    let toast = toast;
                    spawn(async move {
                        let q = "mutation U($id: ID!, $c: String!, $f: Boolean!) { updateLobbyConfig(lobbyId: $id, configJson: $c, force: $f) { id } }";
                        let vars = serde_json::json!({ "id": lid, "c": cfg, "f": false });
                        let r = graphql_exec::<Value>(q, Some(vars)).await;
                        if r.is_ok() {
                            push_toast(toast.show, "Configuration applied", ToastKind::Success);
                        } else if let Err(_) = r {
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
                                if graphql_exec::<Value>(q, Some(vars2)).await.is_ok() {
                                    push_toast(toast.show, "Configuration applied (seats reset)", ToastKind::Success);
                                }
                            }
                        }
                    });
                },
                "Apply configuration"
            }
        } else {
            p { class: "text-xs text-amber-200/80 mt-3 rounded-lg border border-amber-500/25 bg-amber-950/30 px-3 py-2",
                "Read-only: only the lobby owner can change configuration."
            }
        }
    }
}

#[component]
pub fn LobbyConfigModal(
    open: bool,
    on_close: EventHandler<()>,
    title: String,
    lobby_id: String,
    game_type: String,
    iframe_src: Option<String>,
    schema_json: Option<String>,
    read_only: bool,
    server_config_json: Option<String>,
    config_panel_key: String,
    show_spawn_defaults: bool,
    lobby_id_spawn: String,
) -> Element {
    if !open {
        return rsx! {};
    }

    rsx! {
        div { class: "lobby-game-modal-layer",
            button {
                class: "lobby-game-modal-backdrop",
                onclick: move |_| on_close.call(()),
            }
            div {
                class: "lobby-config-modal",
                role: "dialog",
                aria_modal: "true",
                div { class: "lobby-game-modal-head",
                    div {
                        p { class: "lobby-section-kicker", "MATCH SETTINGS" }
                        h2 { class: "lobby-section-title", "{title}" }
                    }
                    button {
                        class: "lobby-game-modal-close",
                        onclick: move |_| on_close.call(()),
                        Icon { name: "close", filled: false }
                    }
                }
                div { class: "lobby-config-modal-body",
                    if let Some(src) = iframe_src {
                        div { class: "lobby-config-shell",
                            LobbyConfigPanel {
                                key: "{config_panel_key}",
                                lobby_id: lobby_id.clone(),
                                game_type: game_type.clone(),
                                iframe_src: src,
                                schema_json: schema_json.clone(),
                                read_only,
                                server_config_json: server_config_json.clone(),
                            }
                        }
                    } else if show_spawn_defaults {
                        div { class: "lobby-panel-empty py-6",
                            p { "This game uses default rules — spawn seats to continue." }
                            button {
                                class: "btn-primary mt-3",
                                onclick: move |_| {
                                    let lid = lobby_id_spawn.clone();
                                    spawn(async move {
                                        let q = "mutation U($id: ID!, $c: String!, $f: Boolean!) { updateLobbyConfig(lobbyId: $id, configJson: $c, force: $f) { id } }";
                                        let vars = serde_json::json!({ "id": lid, "c": "null", "f": false });
                                        let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                    });
                                },
                                "Spawn seats with defaults"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LobbySquadFooter(
    lobby_id: String,
    ready_count: usize,
    claimed: usize,
    total: usize,
    my_user_id: Option<String>,
    seats: Vec<LobbySeat>,
    in_staging: bool,
) -> Element {
    if !in_staging || total == 0 {
        return rsx! {};
    }

    let toast = use_toast();
    let my_seat = my_user_id.as_ref().and_then(|u| {
        seats
            .iter()
            .find(|s| s.claimed_by_user_id.as_deref() == Some(u.as_str()))
    });
    let user_in_seat = my_seat.is_some();
    let my_seat_ready = my_seat.is_some_and(|s| s.ready);
    let lobby_id_ready = lobby_id.clone();
    let lobby_id_leave = lobby_id.clone();

    let ready_denominator = if claimed > 0 { claimed } else { total };

    rsx! {
        footer { class: "lobby-squad-footer",
            span { class: "lobby-squad-footer-stat",
                Icon { name: "check_circle", filled: true }
                "{ready_count} / {ready_denominator} ready"
            }
            div { class: "lobby-squad-footer-actions",
                if user_in_seat {
                    button {
                        class: if my_seat_ready {
                            "lobby-squad-footer-btn lobby-squad-footer-ready-on"
                        } else {
                            "lobby-squad-footer-btn"
                        },
                        onclick: move |_| {
                            let lid = lobby_id_ready.clone();
                            let next_ready = !my_seat_ready;
                            let toast = toast;
                            spawn(async move {
                                let q = "mutation R($id: ID!, $r: Boolean!) { setLobbySeatReady(lobbyId: $id, ready: $r) { id } }";
                                let vars = serde_json::json!({ "id": lid, "r": next_ready });
                                match graphql_exec::<Value>(q, Some(vars)).await {
                                    Ok(_) => {
                                        let msg = if next_ready { "Ready" } else { "Not ready" };
                                        push_toast(toast.show, msg, ToastKind::Success);
                                    }
                                    Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                }
                            });
                        },
                        if my_seat_ready {
                            Icon { name: "remove_circle", filled: false }
                            "Not ready"
                        } else {
                            Icon { name: "bolt", filled: false }
                            "Ready up"
                        }
                    }
                    button {
                        class: "lobby-squad-footer-btn lobby-squad-footer-leave",
                        onclick: move |_| {
                            let lid = lobby_id_leave.clone();
                            let toast = toast;
                            spawn(async move {
                                let q = "mutation L($id: ID!) { leaveLobby(lobbyId: $id) }";
                                let vars = serde_json::json!({ "id": lid });
                                match graphql_exec::<Value>(q, Some(vars)).await {
                                    Ok(_) => push_toast(toast.show, "Left seat", ToastKind::Success),
                                    Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                }
                            });
                        },
                        Icon { name: "event_seat", filled: false }
                        "Leave seat"
                    }
                }
            }
            span { class: "lobby-squad-footer-stat",
                Icon { name: "groups", filled: false }
                "{claimed} / {total} filled"
            }
        }
    }
}

#[component]
pub fn LobbyRoomBody(
    lobby_for_cols: LobbyDetail,
    gt_list: Vec<GameTypeInfo>,
    uid: Option<String>,
    on_detail_updated: EventHandler<LobbyDetail>,
) -> Element {
    let nav = use_navigator();
    let is_owner = uid.as_deref() == Some(lobby_for_cols.owner_user_id.as_str());
    let lobby_id_start_bar = lobby_for_cols.id.clone();
    let lobby_id_cancel_bar = lobby_for_cols.id.clone();
    let lobby_id_chat_panel = lobby_for_cols.id.clone();
    let no_game_yet = lobby_for_cols.game_type.trim().is_empty();
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
    let ready_count = lobby_for_cols
        .seats
        .iter()
        .filter(|s| s.claimed_by_user_id.is_some() && s.ready)
        .count();
    let all_ready = total > 0 && claimed == total && ready_count == claimed;
    let can_start = is_owner
        && total > 0
        && claimed == total
        && all_ready
        && (lobby_for_cols.status == "waiting" || lobby_for_cols.status == "configuring");
    let in_staging = lobby_for_cols.status == "waiting" || lobby_for_cols.status == "configuring";
    let lobby_id_default_config = lobby_for_cols.id.clone();
    let lobby_finished = lobby_for_cols.status == "finished";
    let game_id_for_results_btn = lobby_for_cols.game_instance_id.clone();
    let lobby_id_reopen_finished = lobby_for_cols.id.clone();

    let toast = use_toast();
    let game_name = lobby_for_cols.game_type.clone();
    let media = game_media(&game_name);
    let status_variant = status_variant_from_lobby(
        &lobby_for_cols.status,
        claimed as i32,
        total as i32,
    );
    let mut games_open = use_signal(|| false);
    let mut config_open = use_signal(|| false);
    let mut rules_open = use_signal(|| false);
    let has_config = !no_game_yet && (iframe_src.is_some() || is_owner);
    let about_url = selected_gt.as_ref().and_then(game_type_about_url);
    let show_rules = !no_game_yet && about_url.is_some();
    let config_title = game_type_display_title(&gt_list, &game_name);
    let rules_title = config_title.clone();
    let est_match = format_estimated_match_time(
        selected_gt.as_ref().map(|g| g.avg_session_mins).unwrap_or(0),
    );
    let my_seat_position = uid.as_ref().and_then(|u| {
        lobby_for_cols
            .seats
            .iter()
            .position(|s| s.claimed_by_user_id.as_deref() == Some(u.as_str()))
    });
    let (seats_before_me, my_roster_seat, seats_after_me) = if let Some(pos) = my_seat_position {
        let seats = &lobby_for_cols.seats;
        (
            seats[..pos].to_vec(),
            seats.get(pos).cloned(),
            seats[pos.saturating_add(1)..].to_vec(),
        )
    } else {
        (Vec::new(), None, Vec::new())
    };

    rsx! {
        div { class: "lobby-room-body",
        div { class: "lobby-stage",
            div { class: "lobby-stage-bg" }

            if lobby_finished {
                div { class: "lobby-finished-banner mx-auto max-w-3xl w-full",
                    Icon { name: "emoji_events", filled: true }
                    div { class: "min-w-0 flex-1",
                        p { class: "font-manrope font-semibold text-on-surface", "Match complete" }
                    }
                    div { class: "flex flex-wrap gap-2",
                        if let Some(g) = game_id_for_results_btn.clone() {
                            button {
                                class: "btn-primary",
                                onclick: move |_| { nav.push(LobbyRoute::GameResult { id: g.clone() }); },
                                "Results"
                            }
                        }
                        if is_owner {
                            button {
                                class: "btn-secondary",
                                onclick: move |_| {
                                    let lid = lobby_id_reopen_finished.clone();
                                    spawn(async move {
                                        let q = "mutation R($id: ID!) { reopenLobbyAfterGame(lobbyId: $id) }";
                                        let vars = serde_json::json!({ "id": lid });
                                        let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                    });
                                },
                                "Play again"
                            }
                        }
                    }
                }
            }

            LobbyGameModal {
                open: games_open(),
                on_close: EventHandler::new(move |_| games_open.set(false)),
                lobby_id: lobby_for_cols.id.clone(),
                selected_game_type: lobby_for_cols.game_type.clone(),
                gt_list: gt_list.clone(),
                is_owner,
                on_detail_updated,
            }

            LobbyConfigModal {
                open: config_open(),
                on_close: EventHandler::new(move |_| config_open.set(false)),
                title: config_title.clone(),
                lobby_id: lobby_for_cols.id.clone(),
                game_type: lobby_for_cols.game_type.clone(),
                iframe_src: iframe_src.clone(),
                schema_json: schema_json.clone(),
                read_only,
                server_config_json: lobby_for_cols.config_json.clone(),
                config_panel_key: config_panel_key.clone(),
                show_spawn_defaults: is_owner && lobby_for_cols.seats.is_empty(),
                lobby_id_spawn: lobby_id_default_config.clone(),
            }

            if let Some(rules_about_url) = about_url.clone() {
                LobbyGameRulesModal {
                    open: rules_open(),
                    on_close: EventHandler::new(move |_| rules_open.set(false)),
                    title: rules_title.clone(),
                    about_url: rules_about_url,
                }
            }

            div { class: "lobby-stage-main",
                div { class: "lobby-panel-top",
                    LobbyActiveGame {
                        selected_game_type: lobby_for_cols.game_type.clone(),
                        gt_list: gt_list.clone(),
                        is_owner,
                        on_open_games: EventHandler::new(move |_| games_open.set(true)),
                        show_config: has_config,
                        on_open_config: EventHandler::new(move |_| config_open.set(true)),
                        show_rules,
                        on_open_rules: EventHandler::new(move |_| rules_open.set(true)),
                    }
                }

                div { class: "lobby-panel-middle",
                    if !no_game_yet {
                        div { class: "lobby-squad-ambient bg-gradient-to-b {media.accent_gradient}" }
                    }

                    if no_game_yet {
                        div { class: "lobby-roster-empty",
                            Icon { name: "groups", filled: false }
                            p { "Pick a game above — your squad forms here." }
                            if is_owner {
                                button {
                                    class: "btn-secondary mt-2",
                                    onclick: move |_| games_open.set(true),
                                    "Select game"
                                }
                            }
                        }
                    } else if total == 0 {
                        div { class: "lobby-roster-empty",
                            Icon { name: "tune", filled: false }
                            p { "Open game config to spawn player slots." }
                            if has_config {
                                button {
                                    class: "btn-secondary mt-2",
                                    onclick: move |_| config_open.set(true),
                                    "Open game config"
                                }
                            }
                        }
                    } else if let Some(me_seat) = my_roster_seat.clone() {
                        div { class: "lobby-roster-centered",
                            div { class: "lobby-roster-side lobby-roster-side-left",
                                for seat in seats_before_me.clone() {
                                    LobbyPlayerCard {
                                        key: "{seat.seat_index}",
                                        seat: seat,
                                        lobby_id: lobby_for_cols.id.clone(),
                                        owner_user_id: lobby_for_cols.owner_user_id.clone(),
                                        my_user_id: uid.clone(),
                                        viewer_is_owner: is_owner,
                                        in_staging,
                                        on_detail_updated,
                                    }
                                }
                            }
                            div { class: "lobby-roster-center-seat",
                                LobbyPlayerCard {
                                    key: "{me_seat.seat_index}",
                                    seat: me_seat,
                                    lobby_id: lobby_for_cols.id.clone(),
                                    owner_user_id: lobby_for_cols.owner_user_id.clone(),
                                    my_user_id: uid.clone(),
                                    viewer_is_owner: is_owner,
                                    in_staging,
                                    on_detail_updated,
                                }
                            }
                            div { class: "lobby-roster-side lobby-roster-side-right",
                                for seat in seats_after_me.clone() {
                                    LobbyPlayerCard {
                                        key: "{seat.seat_index}",
                                        seat: seat,
                                        lobby_id: lobby_for_cols.id.clone(),
                                        owner_user_id: lobby_for_cols.owner_user_id.clone(),
                                        my_user_id: uid.clone(),
                                        viewer_is_owner: is_owner,
                                        in_staging,
                                        on_detail_updated,
                                    }
                                }
                            }
                        }
                    } else {
                        div { class: "lobby-roster-hscroll",
                            for seat in lobby_for_cols.seats.clone() {
                                LobbyPlayerCard {
                                    key: "{seat.seat_index}",
                                    seat: seat,
                                    lobby_id: lobby_for_cols.id.clone(),
                                    owner_user_id: lobby_for_cols.owner_user_id.clone(),
                                    my_user_id: uid.clone(),
                                    viewer_is_owner: is_owner,
                                    in_staging,
                                    on_detail_updated,
                                }
                            }
                        }
                    }
                }

                LobbySquadFooter {
                    lobby_id: lobby_for_cols.id.clone(),
                    ready_count,
                    claimed,
                    total,
                    my_user_id: uid.clone(),
                    seats: lobby_for_cols.seats.clone(),
                    in_staging,
                }
            }

            LobbyFloatingChat {
                lobby_id: lobby_id_chat_panel,
                messages: lobby_for_cols.messages.clone(),
                my_user_id: uid.clone(),
            }
        }

        if in_staging {
            div { class: "lobby-bottom-bar",
                div { class: "flex items-center gap-3",
                    StatusBadge { label: lobby_for_cols.status.clone(), variant: status_variant }
                    span { class: "text-body-sm text-on-surface-variant hidden sm:inline",
                        "Est. {est_match}"
                    }
                }
                div { class: "flex flex-wrap items-center gap-2",
                    if is_owner {
                        button {
                            class: "btn-ghost text-error border-error/40 text-sm",
                            onclick: move |_| {
                                let lid = lobby_id_cancel_bar.clone();
                                let nav = nav;
                                spawn(async move {
                                    let q = "mutation C($id: ID!) { cancelLobby(lobbyId: $id) }";
                                    let vars = serde_json::json!({ "id": lid });
                                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                    let home = LobbyRoute::Home {};
                                    nav.push(home);
                                });
                            },
                            "Disband"
                        }
                        button {
                            class: "btn-primary shadow-[0_0_20px_rgba(79,70,229,0.4)]",
                            disabled: !can_start,
                            onclick: move |_| {
                                let lid = lobby_id_start_bar.clone();
                                let toast = toast;
                                spawn(async move {
                                    let q = "mutation St($id: ID!) { startLobby(lobbyId: $id) }";
                                    let vars = serde_json::json!({ "id": lid });
                                    match graphql_exec::<Value>(q, Some(vars)).await {
                                        Ok(_) => push_toast(toast.show, "Match started", ToastKind::Success),
                                        Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                    }
                                });
                            },
                            "START MATCH"
                        }
                    }
                }
            }
        }
        }
    }
}
