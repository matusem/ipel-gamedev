use crate::api::graphql_exec;
use crate::components::ui::*;
use crate::models::{GameResultRow, LoadedGameResult};
use crate::LobbyRoute;
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::Value;

#[component]
pub fn GameResultPage(game_id: String) -> Element {
    let nav = use_navigator();
    let mut loaded: Signal<Option<LoadedGameResult>> = use_signal(|| None);
    let mut err: Signal<Option<String>> = use_signal(|| None);
    let mut done: Signal<bool> = use_signal(|| false);
    let gid_fetch = game_id.clone();

    use_hook(move || {
        let gid_fetch = gid_fetch.clone();
        let mut loaded = loaded;
        let mut err = err;
        let mut done = done;
        spawn(async move {
            let q = r#"query G($id: ID!) { finishedGame(gameId: $id) { gameId gameType lobbyId finishedAt resultJson playerScoresJson seatsSnapshotJson resultUiPath } }"#;
            let vars = serde_json::json!({ "id": gid_fetch });
            #[derive(Deserialize)]
            struct Wrap {
                #[serde(rename = "finishedGame")]
                finished_game: Option<GameResultRow>,
            }
            match graphql_exec::<Wrap>(q, Some(vars)).await {
                Ok(w) => {
                    if let Some(r) = w.finished_game {
                        let iframe_src = r.result_ui_path.as_ref().and_then(|path| {
                            let result_v: Value = serde_json::from_str(&r.result_json).unwrap_or(Value::Null);
                            let scores_v: Value = serde_json::from_str(&r.player_scores_json).unwrap_or(Value::Null);
                            let seats_v: Value = serde_json::from_str(&r.seats_snapshot_json).unwrap_or(Value::Null);
                            let payload = serde_json::json!({
                                "gameId": &r.game_id,
                                "gameType": &r.game_type,
                                "finishedAt": r.finished_at,
                                "lobbyId": &r.lobby_id,
                                "result": result_v,
                                "scores": scores_v,
                                "seats": seats_v,
                            });
                            let payload_str = payload.to_string();
                            let enc = urlencoding::encode(&payload_str);
                            Some(format!("/games/{}/{}?payload={}", r.game_type, path, enc))
                        });
                        loaded.set(Some(LoadedGameResult { row: r, iframe_src }));
                    } else {
                        loaded.set(None);
                    }
                }
                Err(e) => err.set(Some(e)),
            }
            done.set(true);
        });
    });

    rsx! {
        div { class: "space-y-6",
            PageHeader {
                title: "Game result".to_string(),
                subtitle: Some(game_id.clone()),
                badge: None,
                children: Some(rsx! {
                    GhostButton {
                        label: "← Home".to_string(),
                        onclick: move |_| { nav.push(LobbyRoute::Home {}); },
                    }
                }),
            }
            if let Some(e) = err() {
                ErrorBanner { message: e }
            } else if !done() {
                LoadingState {
                    title: "Loading result…".to_string(),
                    subtitle: game_id.clone(),
                }
            } else if let Some(ld) = loaded() {
                div { class: "section-card space-y-4",
                    div { class: "flex flex-wrap gap-4 text-body-sm",
                        span { class: "text-on-surface-variant",
                            "Game: "
                            span { class: "text-on-surface font-medium", "{ld.row.game_type}" }
                        }
                        span { class: "text-on-surface-variant",
                            "Finished: "
                            span { class: "font-mono-code", "UNIX {ld.row.finished_at}" }
                        }
                    }
                    if let Some(lid) = ld.row.lobby_id.clone() {
                        button {
                            class: "btn-ghost text-sm",
                            onclick: move |_| { nav.push(LobbyRoute::Lobby { id: lid.clone() }); },
                            "View lobby"
                        }
                    }
                    if let Some(src) = ld.iframe_src.clone() {
                        iframe { class: "config-iframe min-h-[32rem]", src: src }
                    } else {
                        p { class: "text-body-sm text-secondary rounded-lg border border-secondary-container/30 bg-secondary-container/10 px-3 py-2",
                            "This game type has no client/result.html — raw payload below."
                        }
                    }
                    details { class: "section-card",
                        summary { class: "cursor-pointer text-primary font-medium", "Raw JSON" }
                        div { class: "mt-4 space-y-4",
                            h3 { class: "font-manrope font-semibold", "Scores" }
                            JsonConsole { content: ld.row.player_scores_json.clone(), max_height: None }
                            h3 { class: "font-manrope font-semibold", "Outcome" }
                            JsonConsole { content: ld.row.result_json.clone(), max_height: None }
                            h3 { class: "font-manrope font-semibold", "Seats" }
                            JsonConsole { content: ld.row.seats_snapshot_json.clone(), max_height: None }
                        }
                    }
                }
            } else {
                EmptyState {
                    icon: "sports_esports",
                    title: "No result found".to_string(),
                    description: "Wrong id or the game has not finished yet.".to_string(),
                    cta_label: Some("Go home".to_string()),
                    on_cta: Some(EventHandler::new(move |_| {
                        nav.push(LobbyRoute::Home {});
                    })),
                }
            }
        }
    }
}
