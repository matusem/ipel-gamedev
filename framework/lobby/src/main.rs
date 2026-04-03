use dioxus::prelude::*;
use gloo_events::EventListener;
use js_sys::{Array, Object, Reflect};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;

/// Bundled CSS (no Tailwind CDN — avoids FOUC while the async script loads after first paint).
const LOBBY_STYLES: &str = include_str!("../assets/lobby.css");

const CONFIG_MSG_SOURCE: &str = "ipel-game-config";
const CONFIG_RESULT_SOURCE: &str = "ipel-game-config-result";
const CONFIG_SCHEMA_SOURCE: &str = "ipel-game-config-schema";

fn parse_iframe_config_message(data: &wasm_bindgen::JsValue) -> Option<(String, String)> {
    let s = js_sys::JSON::stringify(data).ok()?.as_string()?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    if v.get("source").and_then(|x| x.as_str()) != Some(CONFIG_MSG_SOURCE) {
        return None;
    }
    let game = v.get("game")?.as_str()?.to_string();
    let config_val = v.get("config")?;
    let config_str = if let Some(s) = config_val.as_str() {
        s.to_string()
    } else {
        config_val.to_string()
    };
    Some((game, config_str))
}

fn validate_config_json(config_str: &str, schema: Option<&Value>) -> Result<(), Vec<String>> {
    let Some(s) = schema else {
        return Ok(());
    };
    let instance: Value = serde_json::from_str(config_str.trim())
        .map_err(|e| vec![format!("Invalid JSON: {e}")])?;
    let validator =
        jsonschema::validator_for(s).map_err(|e| vec![format!("Invalid config schema: {e}")])?;
    let errs: Vec<String> = validator
        .iter_errors(&instance)
        .map(|e| format!("{}: {e}", e.instance_path().as_str()))
        .collect();
    if !errs.is_empty() {
        return Err(errs);
    }
    Ok(())
}

fn post_message_to_source(event: &web_sys::MessageEvent, origin: &str, payload: &JsValue) {
    let Some(src) = event.source() else {
        return;
    };
    let Ok(win) = JsValue::from(src).dyn_into::<web_sys::Window>() else {
        return;
    };
    let _ = win.post_message(payload, origin);
}

fn config_validation_reply(
    event: &web_sys::MessageEvent,
    origin: &str,
    game: &str,
    ok: bool,
    errors: &[String],
) {
    let obj = Object::new();
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("source"),
        &JsValue::from_str(CONFIG_RESULT_SOURCE),
    );
    let _ = Reflect::set(&obj, &JsValue::from_str("game"), &JsValue::from_str(game));
    let _ = Reflect::set(&obj, &JsValue::from_str("ok"), &JsValue::from_bool(ok));
    let arr = Array::new();
    for e in errors {
        arr.push(&JsValue::from_str(e));
    }
    let _ = Reflect::set(&obj, &JsValue::from_str("errors"), &JsValue::from(arr));
    post_message_to_source(event, origin, &JsValue::from(obj));
}

fn post_config_schema_to_window(win: &web_sys::Window, origin: &str, game: &str, schema: &Value) {
    let Ok(schema_js) = js_sys::JSON::parse(&serde_json::to_string(schema).unwrap_or_else(|_| "{}".to_string()))
    else {
        return;
    };
    let obj = Object::new();
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("source"),
        &JsValue::from_str(CONFIG_SCHEMA_SOURCE),
    );
    let _ = Reflect::set(&obj, &JsValue::from_str("game"), &JsValue::from_str(game));
    let _ = Reflect::set(&obj, &JsValue::from_str("schema"), &schema_js);
    let _ = win.post_message(&JsValue::from(obj), origin);
}

fn main() {
    dioxus::launch(App);
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct GameTypeInfo {
    name: String,
    display_name: String,
    version: String,
    min_players: u32,
    max_players: u32,
    description: String,
    #[serde(default)]
    config_ui_path: Option<String>,
    #[serde(default)]
    config_schema: Option<Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct GameInfo {
    game_id: String,
    game_type: String,
    player_identities: Vec<String>,
    connected_players: usize,
}

fn get_ws_base() -> String {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let protocol = location.protocol().unwrap_or_default();
    let host = location.host().unwrap_or_default();
    let ws_protocol = if protocol == "https:" { "wss:" } else { "ws:" };
    format!("{}//{}/game", ws_protocol, host)
}

async fn api_get<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    let resp = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|e| format!("{e}"))?;
    resp.json().await.map_err(|e| format!("{e}"))
}

async fn api_post_text(url: &str) -> Result<String, String> {
    let resp = gloo_net::http::Request::post(url)
        .send()
        .await
        .map_err(|e| format!("{e}"))?;
    resp.text().await.map_err(|e| format!("{e}"))
}

#[component]
fn App() -> Element {
    let mut playing: Signal<Option<(String, String, String)>> = use_signal(|| None);

    rsx! {
        document::Style {
            "{LOBBY_STYLES}"
        }

        div { class: "min-h-screen bg-gray-900 text-white",
            if let Some((game_type, game_id, player)) = playing() {
                GamePlayer {
                    game_type,
                    game_id,
                    player,
                    on_back: move |_| playing.set(None),
                }
            } else {
                Lobby {
                    on_play: move |args: (String, String, String)| {
                        playing.set(Some(args));
                    },
                }
            }
        }
    }
}

#[component]
fn Lobby(on_play: EventHandler<(String, String, String)>) -> Element {
    let mut game_types: Signal<Vec<GameTypeInfo>> = use_signal(Vec::new);
    let mut games: Signal<Vec<GameInfo>> = use_signal(Vec::new);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);
    let mut loading = use_signal(|| true);
    let iframe_configs: Signal<HashMap<String, String>> = use_signal(HashMap::new);

    use_hook(move || {
        let window = web_sys::window().expect("window");
        let expected_origin = window.location().origin().unwrap_or_default();
        let mut iframe_configs = iframe_configs;
        let game_types = game_types;
        let listener = EventListener::new(&window, "message", move |event| {
            let event: &web_sys::MessageEvent = event.dyn_ref().expect("MessageEvent");
            if event.origin() != expected_origin {
                return;
            }
            let Some((game, config_str)) = parse_iframe_config_message(&event.data()) else {
                return;
            };
            let types = game_types();
            let schema_opt = types
                .iter()
                .find(|g| g.name == game)
                .and_then(|g| g.config_schema.clone());
            match validate_config_json(&config_str, schema_opt.as_ref()) {
                Ok(()) => {
                    iframe_configs.write().insert(game.clone(), config_str);
                    config_validation_reply(event, &expected_origin, &game, true, &[]);
                }
                Err(errs) => {
                    config_validation_reply(event, &expected_origin, &game, false, &errs);
                }
            }
        });
        std::mem::forget(listener);
    });

    let refresh_games = move || {
        spawn(async move {
            match api_get::<Vec<GameInfo>>("/api/games").await {
                Ok(g) => games.set(g),
                Err(e) => error_msg.set(Some(e)),
            }
        });
    };

    use_effect(move || {
        spawn(async move {
            match api_get::<Vec<GameTypeInfo>>("/api/game_types").await {
                Ok(types) => game_types.set(types),
                Err(e) => error_msg.set(Some(e)),
            }
            match api_get::<Vec<GameInfo>>("/api/games").await {
                Ok(g) => games.set(g),
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    rsx! {
        div { class: "max-w-4xl mx-auto px-4 py-8",
            h1 { class: "text-4xl font-bold mb-8 text-center", "Game Server" }

            if let Some(err) = error_msg() {
                div { class: "bg-red-900/50 border border-red-500 text-red-200 px-4 py-3 rounded mb-6",
                    "{err}"
                }
            }

            if loading() {
                p { class: "text-center text-gray-400", "Loading..." }
            } else {
                section { class: "mb-10",
                    h2 { class: "text-2xl font-semibold mb-4", "Game Types" }
                    if game_types().is_empty() {
                        p { class: "text-gray-400", "No game types available." }
                    }
                    div { class: "grid gap-4 md:grid-cols-2",
                        for gt in game_types() {
                            GameTypeCard {
                                game_type: gt,
                                iframe_configs,
                                on_created: move |_| refresh_games(),
                            }
                        }
                    }
                }

                section {
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-2xl font-semibold", "Active Games" }
                        button {
                            class: "px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-sm",
                            onclick: move |_| refresh_games(),
                            "Refresh"
                        }
                    }
                    if games().is_empty() {
                        p { class: "text-gray-400", "No active games. Create one above!" }
                    }
                    div { class: "space-y-3",
                        for game in games() {
                            GameCard {
                                game,
                                on_play: on_play,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn GameTypeCard(
    game_type: GameTypeInfo,
    iframe_configs: Signal<HashMap<String, String>>,
    on_created: EventHandler,
) -> Element {
    let mut creating = use_signal(|| false);
    let mut config_input = use_signal(|| "null".to_string());

    let gt_name = game_type.name.clone();
    let config_schema_push = game_type.config_schema.clone();
    let config_path = game_type.config_ui_path.clone();
    let iframe_src = config_path
        .as_ref()
        .map(|p| format!("/games/{}/{}", game_type.name, p));

    let do_create = {
        let name = gt_name.clone();
        let use_iframe = config_path.is_some();
        move |_| {
            let name = name.clone();
            let config = if use_iframe {
                iframe_configs()
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| "null".to_string())
            } else {
                config_input()
            };
            creating.set(true);
            spawn(async move {
                let url = format!(
                    "/api/create_game?game={}&config={}",
                    urlencoding::encode(&name),
                    urlencoding::encode(&config),
                );
                match api_post_text(&url).await {
                    Ok(_) => on_created.call(()),
                    Err(e) => {
                        let _ = web_sys::window()
                            .unwrap()
                            .alert_with_message(&format!("Error: {e}"));
                    }
                };
                creating.set(false);
            });
        }
    };

    rsx! {
        div { class: "bg-gray-800 rounded-lg p-4 border border-gray-700",
            div { class: "flex justify-between items-start mb-2",
                h3 { class: "text-lg font-semibold", "{game_type.display_name}" }
                span { class: "text-xs text-gray-400 bg-gray-700 px-2 py-1 rounded",
                    "v{game_type.version}"
                }
            }
            p { class: "text-gray-400 text-sm mb-3", "{game_type.description}" }
            p { class: "text-xs text-gray-500 mb-3",
                "Players: {game_type.min_players}-{game_type.max_players}"
            }

            if let Some(src) = iframe_src.clone() {
                p { class: "text-xs text-gray-500 mb-2",
                    "Set options in the frame below, then click Create Game."
                }
                iframe {
                    class: "config-iframe mb-3",
                    src: "{src}",
                    title: "Game config",
                    onmounted: move |evt| {
                        let Some(schema) = config_schema_push.clone() else {
                            return;
                        };
                        let game = gt_name.clone();
                        let origin = web_sys::window()
                            .unwrap()
                            .location()
                            .origin()
                            .unwrap_or_default();
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
                        let _load_listener = EventListener::new(&iframe_el, "load", move |_| {
                            if let Some(w) = iframe_for_load.content_window() {
                                post_config_schema_to_window(&w, &origin_l, &game_l, &schema_l);
                            }
                        });
                        std::mem::forget(_load_listener);
                        if let Some(w) = iframe_el.content_window() {
                            post_config_schema_to_window(&w, &origin, &game, &schema);
                        }
                    }
                }
            }

            if config_path.is_none() {
                div { class: "flex gap-2",
                    input {
                        class: "flex-1 px-2 py-1 bg-gray-700 border border-gray-600 rounded text-sm text-white",
                        placeholder: "Config JSON",
                        value: "{config_input}",
                        oninput: move |e| config_input.set(e.value()),
                    }
                }
            }

            div { class: "flex gap-2 mt-2",
                button {
                    class: "px-4 py-1 bg-blue-600 hover:bg-blue-500 rounded text-sm font-medium disabled:opacity-50",
                    disabled: creating(),
                    onclick: do_create,
                    if creating() { "Creating..." } else { "Create Game" }
                }
            }
        }
    }
}

#[component]
fn GameCard(game: GameInfo, on_play: EventHandler<(String, String, String)>) -> Element {
    let game_type = game.game_type.clone();

    rsx! {
        div { class: "bg-gray-800 rounded-lg p-4 border border-gray-700 flex items-center justify-between",
            div {
                p { class: "font-medium",
                    "{game.game_type}"
                    span { class: "text-gray-500 text-sm ml-2", "{game.game_id}" }
                }
                p { class: "text-xs text-gray-400 mt-1",
                    "Connected: {game.connected_players} / {game.player_identities.len()}"
                }
            }
            div { class: "flex gap-2",
                for identity in &game.player_identities {
                    {
                        let gt = game_type.clone();
                        let gid = game.game_id.clone();
                        let pid = identity.clone();
                        rsx! {
                            button {
                                class: "px-3 py-1 bg-green-700 hover:bg-green-600 rounded text-sm",
                                onclick: move |_| {
                                    on_play.call((gt.clone(), gid.clone(), pid.clone()));
                                },
                                "Join as {identity}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn GamePlayer(
    game_type: String,
    game_id: String,
    player: String,
    on_back: EventHandler,
) -> Element {
    let ws_base = get_ws_base();
    let iframe_src = format!(
        "/games/{game_type}/?ws={ws_base}&id={game_id}&player={player}"
    );

    rsx! {
        div { class: "flex flex-col h-screen",
            div { class: "flex items-center gap-4 px-4 py-3 bg-gray-800 border-b border-gray-700",
                button {
                    class: "px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-sm",
                    onclick: move |_| on_back.call(()),
                    "Back to Lobby"
                }
                span { class: "text-gray-400 text-sm",
                    "Playing {game_type} as {player}"
                }
            }
            iframe {
                class: "flex-1 w-full border-0",
                src: "{iframe_src}",
            }
        }
    }
}
