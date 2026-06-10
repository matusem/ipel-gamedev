use crate::api::*;
use crate::api::config_bridge::*;
use crate::components::lobby::LobbyChatPanel;
use crate::components::ui::{push_toast, status_variant_from_lobby, Avatar, AvatarSize, JsonConsole, StatusBadge, use_toast, ToastKind};
use crate::models::*;
use crate::stub::{estimated_match_time_stub, game_media};
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
            class: if read_only { "config-iframe pointer-events-none opacity-90" } else { "config-iframe" },
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
        p { class: "text-body-sm text-on-surface-variant mt-2",
            "Preview JSON (from the config panel). Saving is separate — use Apply below."
        }
        div { class: "mt-2",
            JsonConsole { content: preview(), max_height: Some("max-h-48") }
        }
        if !read_only {
            button {
                class: "btn-primary mt-3",
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
pub fn LobbyRoomBody(
    lobby_for_cols: LobbyDetail,
    gt_list: Vec<GameTypeInfo>,
    uid: Option<String>,
) -> Element {
    let nav = use_navigator();
    let is_owner = uid.as_deref() == Some(lobby_for_cols.owner_user_id.as_str());
    let lobby_id_start = lobby_for_cols.id.clone();
    let lobby_id_cancel = lobby_for_cols.id.clone();
    let lobby_id_start_bar = lobby_for_cols.id.clone();
    let lobby_id_cancel_bar = lobby_for_cols.id.clone();
    let lobby_id_chat_panel = lobby_for_cols.id.clone();
    let lobby_id_mark_ready = lobby_for_cols.id.clone();
    let lobby_id_mark_unready = lobby_for_cols.id.clone();
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
    let user_in_seat = uid.as_ref().is_some_and(|u| {
        lobby_for_cols
            .seats
            .iter()
            .any(|s| s.claimed_by_user_id.as_deref() == Some(u.as_str()))
    });
    let my_seat_ready = uid
        .as_ref()
        .and_then(|u| {
            lobby_for_cols
                .seats
                .iter()
                .find(|s| s.claimed_by_user_id.as_deref() == Some(u.as_str()))
        })
        .map(|s| s.ready)
        .unwrap_or(false);
    let lobby_id_default_config = lobby_for_cols.id.clone();
    let lobby_id_leave = lobby_for_cols.id.clone();
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

    rsx! {
        div { class: "pb-24",
        if !no_game_yet {
            div { class: "mb-6 page-hero min-h-[120px] rounded-2xl overflow-hidden",
                div { class: "absolute inset-0 bg-gradient-to-r {media.accent_gradient} z-0" }
                div { class: "relative z-10 p-5 flex items-center gap-4",
                    div { class: "game-thumb text-2xl", "{media.icon_emoji}" }
                    div {
                        p { class: "text-label-caps font-label-caps text-outline uppercase", "Current game" }
                        p { class: "font-manrope text-xl font-semibold text-on-surface",
                            "{game_type_display_title(&gt_list, &game_name)}"
                        }
                    }
                }
            }
        }
        if lobby_finished {
            div { class: "mb-6 section-card border-secondary-container/40 bg-secondary-container/10",
                p { class: "text-on-surface font-semibold mb-3", "This match is over." }
                div { class: "flex flex-wrap gap-2",
                    if let Some(g) = game_id_for_results_btn.clone() {
                        button {
                            class: "btn-primary",
                            onclick: move |_| {
                                nav.push(LobbyRoute::GameResult { id: g.clone() });
                            },
                            "View results"
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
                            "Play again (reset lobby)"
                        }
                    }
                }
            }
        }
        div { class: "lobby-room-grid",
            div { class: "lobby-col lobby-col-types",
                h3 { class: "text-label-caps font-label-caps text-secondary uppercase mb-3", "Game type" }
                if no_game_yet {
                    p { class: "text-body-sm text-on-surface-variant mb-3 leading-relaxed",
                        if is_owner {
                            "Choose a game below. Configuration and player seats appear after you select one."
                        } else {
                            "Waiting for the lobby owner to choose a game."
                        }
                    }
                }
                div { class: "lobby-type-list",
                    for gt in gt_list.iter().cloned() {
                        {
                            let active = !no_game_yet && gt.name == lobby_for_cols.game_type;
                            let desc = gt.description.trim();
                            let about_url = game_type_about_url(&gt);
                            let lid_set = lobby_for_cols.id.clone();
                            let gtn = gt.name.clone();
                            rsx! {
                                div { class: "space-y-2",
                                    button {
                                        class: if active { "lobby-type-btn active" } else { "lobby-type-btn" },
                                        disabled: !is_owner,
                                        onclick: move |_| {
                                            if !is_owner { return; }
                                            let lid = lid_set.clone();
                                            let gtn = gtn.clone();
                                            spawn(async move {
                                                let q = "mutation S($id: ID!, $t: String!, $f: Boolean!) { setLobbyGameType(lobbyId: $id, gameType: $t, force: $f) { id } }";
                                                let vars = serde_json::json!({ "id": lid, "t": gtn, "f": false });
                                                let r = graphql_exec::<Value>(q, Some(vars)).await;
                                                if r.is_err() {
                                                    let force = web_sys::window()
                                                        .map(|w| {
                                                            w.confirm_with_message(
                                                                "Changing type resets seats if claimed. Continue?",
                                                            )
                                                            .unwrap_or(false)
                                                        })
                                                        .unwrap_or(false);
                                                    if force {
                                                        let vars = serde_json::json!({ "id": lid, "t": gtn, "f": true });
                                                        let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                                    }
                                                }
                                            });
                                        },
                                        span { class: "font-medium", "{gt.display_name}" }
                                        if !desc.is_empty() {
                                            span { class: "mt-1 text-body-sm text-on-surface-variant leading-snug line-clamp-4", "{desc}" }
                                        }
                                    }
                                    if let Some(url) = about_url {
                                        a {
                                            class: "btn-ghost text-[11px] py-1 px-2",
                                            href: "{url}",
                                            target: "_blank",
                                            rel: "noopener noreferrer",
                                            "Open game info and rules"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "lobby-col lobby-col-config",
                h3 { class: "text-label-caps font-label-caps text-secondary uppercase mb-3", "Configuration" }
                if no_game_yet {
                    p { class: "text-on-surface-variant text-body-sm leading-relaxed",
                        "Select a game in the first column. The config editor loads here when the game provides one."
                    }
                } else if let Some(src) = iframe_src {
                    LobbyConfigPanel {
                        key: "{config_panel_key}",
                        lobby_id: lobby_for_cols.id.clone(),
                        game_type: lobby_for_cols.game_type.clone(),
                        iframe_src: src,
                        schema_json: schema_json.clone(),
                        read_only,
                        server_config_json: lobby_for_cols.config_json.clone(),
                    }
                } else {
                    p { class: "text-on-surface-variant text-body-sm mb-2",
                        "This game has no config UI. Initialize seats with default config, or pick another type that has a config editor."
                    }
                    if is_owner && lobby_for_cols.seats.is_empty() {
                        button {
                            class: "btn-primary",
                            onclick: move |_| {
                                let lid = lobby_id_default_config.clone();
                                spawn(async move {
                                    let q = "mutation U($id: ID!, $c: String!, $f: Boolean!) { updateLobbyConfig(lobbyId: $id, configJson: $c, force: $f) { id } }";
                                    let vars = serde_json::json!({ "id": lid, "c": "null", "f": false });
                                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                });
                            },
                            "Use default config (create seats)"
                        }
                    }
                }
            }
            div { class: "lobby-col lobby-col-seats",
                h3 { class: "text-label-caps font-label-caps text-secondary uppercase mb-3", "Players" }
                p { class: "text-body-sm text-on-surface-variant mb-3",
                    "{claimed}/{total} seats taken"
                    if total > 0 && claimed > 0 {
                        " · "
                        "{ready_count}/{claimed} ready"
                    }
                }
                if in_staging && total > 0 {
                    p { class: "text-body-sm text-outline mb-3 leading-relaxed",
                        "Take a free seat, then mark Ready. The host can start only when every seat is filled and everyone is ready."
                    }
                }
                div { class: "space-y-2 mb-4",
                    for seat in lobby_for_cols.seats.clone() {
                        {
                            let lid_join = lobby_for_cols.id.clone();
                            let idx = seat.seat_index;
                            let taken = seat.claimed_by_user_id.is_some();
                            let label = seat
                                .claimed_display_name
                                .clone()
                                .unwrap_or_else(|| "free".into());
                            let seat_ready = seat.ready;
                            let seat_class = if taken { "seat-card" } else { "seat-card seat-card-open" };
                            let avatar_seed = seat.claimed_by_user_id.clone().unwrap_or_else(|| seat.player_identity.clone());
                            rsx! {
                                div { class: "{seat_class}",
                                    Avatar { seed: avatar_seed, size: AvatarSize::Sm, image_url: None }
                                    div { class: "min-w-0 flex-1",
                                        span { class: "font-mono-code text-xs text-outline", "{seat.player_identity}" }
                                        if taken {
                                            p { class: "text-body-sm text-on-surface font-medium", "{label}" }
                                            if seat_ready {
                                                span { class: "text-tertiary text-xs font-label-caps font-label-caps uppercase", "Ready" }
                                            } else {
                                                span { class: "text-secondary text-xs font-label-caps font-label-caps uppercase", "Not ready" }
                                            }
                                        } else {
                                            button {
                                                class: "btn-primary text-xs py-1 px-2",
                                                onclick: move |_| {
                                                    let lid = lid_join.clone();
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
                                                "Take seat"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if user_in_seat && in_staging {
                    div { class: "mb-4 section-card py-3 px-3",
                        p { class: "text-label-caps font-label-caps text-outline uppercase mb-2", "Your readiness" }
                        div { class: "flex flex-wrap gap-2",
                            button {
                                class: "btn-primary text-xs",
                                disabled: my_seat_ready,
                                onclick: move |_| {
                                    let lid = lobby_id_mark_ready.clone();
                                    spawn(async move {
                                        let q = "mutation R($id: ID!, $r: Boolean!) { setLobbySeatReady(lobbyId: $id, ready: $r) { id } }";
                                        let vars = serde_json::json!({ "id": lid, "r": true });
                                        let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                    });
                                },
                                "Ready"
                            }
                            button {
                                class: "btn-ghost text-xs",
                                disabled: !my_seat_ready,
                                onclick: move |_| {
                                    let lid = lobby_id_mark_unready.clone();
                                    spawn(async move {
                                        let q = "mutation R($id: ID!, $r: Boolean!) { setLobbySeatReady(lobbyId: $id, ready: $r) { id } }";
                                        let vars = serde_json::json!({ "id": lid, "r": false });
                                        let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                    });
                                },
                                "Not ready"
                            }
                        }
                    }
                }
                div { class: "flex flex-wrap gap-2 hidden md:flex",
                    if is_owner && in_staging {
                        button {
                            class: "btn-primary",
                            disabled: !can_start,
                            onclick: move |_| {
                                let lid = lobby_id_start.clone();
                                let toast = toast;
                                spawn(async move {
                                    let q = "mutation St($id: ID!) { startLobby(lobbyId: $id) }";
                                    let vars = serde_json::json!({ "id": lid });
                                    match graphql_exec::<Value>(q, Some(vars)).await {
                                        Ok(_) => push_toast(toast.show, "Game started", ToastKind::Success),
                                        Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                    }
                                });
                            },
                            "Start game"
                        }
                        button {
                            class: "btn-ghost text-error border-error/40",
                            onclick: move |_| {
                                let lid = lobby_id_cancel.clone();
                                let nav = nav;
                                spawn(async move {
                                    let q = "mutation C($id: ID!) { cancelLobby(lobbyId: $id) }";
                                    let vars = serde_json::json!({ "id": lid });
                                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                    nav.push(LobbyRoute::Home {});
                                });
                            },
                            "Cancel lobby"
                        }
                    }
                    if user_in_seat && in_staging {
                        button {
                            class: "btn-ghost",
                            onclick: move |_| {
                                let lid = lobby_id_leave.clone();
                                spawn(async move {
                                    let q = "mutation Lv($id: ID!) { leaveLobby(lobbyId: $id) }";
                                    let vars = serde_json::json!({ "id": lid });
                                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                });
                            },
                            "Leave seat"
                        }
                    }
                }
                LobbyChatPanel {
                    lobby_id: lobby_id_chat_panel,
                    messages: lobby_for_cols.messages.clone(),
                    my_user_id: uid.clone(),
                }
            }
        }

        if in_staging {
            div { class: "lobby-bottom-bar",
                div { class: "flex items-center gap-3",
                    StatusBadge { label: lobby_for_cols.status.clone(), variant: status_variant }
                    span { class: "text-body-sm text-on-surface-variant hidden sm:inline",
                        "Est. {estimated_match_time_stub()}"
                    }
                }
                div { class: "flex gap-2",
                    if is_owner {
                        button {
                            class: "btn-ghost text-error border-error/40 hidden sm:inline-flex",
                            onclick: move |_| {
                                let lid = lobby_id_cancel_bar.clone();
                                let nav = nav;
                                spawn(async move {
                                    let q = "mutation C($id: ID!) { cancelLobby(lobbyId: $id) }";
                                    let vars = serde_json::json!({ "id": lid });
                                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                                    nav.push(LobbyRoute::Home {});
                                });
                            },
                            "Cancel"
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
