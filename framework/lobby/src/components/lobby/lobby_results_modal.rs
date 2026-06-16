use crate::api::graphql_exec;
use crate::components::ui::{Avatar, AvatarSize, EmptyState, ErrorBanner, Icon, JsonConsole, LoadingState};
use crate::models::{
    compute_lobby_standings, format_match_points, format_relative_time, game_result_summary,
    loaded_game_result_from_row, parse_match_player_scores, GameResultRow, LoadedGameResult,
};
use dioxus::prelude::*;
use serde::Deserialize;

#[component]
pub fn LobbyResultsModal(
    open: bool,
    on_close: EventHandler<()>,
    lobby_id: String,
    game_type: String,
    is_owner: bool,
    lobby_finished: bool,
    initial_game_id: Option<String>,
    on_play_again: EventHandler<()>,
) -> Element {
    if !open {
        return rsx! {};
    }

    let mut history: Signal<Vec<GameResultRow>> = use_signal(Vec::new);
    let mut loading: Signal<bool> = use_signal(|| true);
    let mut err: Signal<Option<String>> = use_signal(|| None);
    let mut detail_open: Signal<bool> = use_signal(|| false);
    let mut detail_game_id: Signal<Option<String>> = use_signal(|| None);

    let lid_fetch = lobby_id.clone();
    let initial = initial_game_id.clone();

    use_effect(move || {
        if !open {
            return;
        }
        let lid = lid_fetch.clone();
        let initial = initial.clone();
        let mut history = history;
        let mut loading = loading;
        let mut err = err;
        let mut detail_open = detail_open;
        let mut detail_game_id = detail_game_id;
        spawn(async move {
            loading.set(true);
            err.set(None);
            let q = r#"query H($id: ID!, $lim: Int) {
                finishedGamesByLobby(lobbyId: $id, limit: $lim) {
                    gameId gameType lobbyId finishedAt resultJson playerScoresJson seatsSnapshotJson resultUiPath
                }
            }"#;
            let vars = serde_json::json!({ "id": lid, "lim": 50 });
            #[derive(Deserialize)]
            struct Wrap {
                #[serde(rename = "finishedGamesByLobby")]
                finished_games_by_lobby: Vec<GameResultRow>,
            }
            match graphql_exec::<Wrap>(q, Some(vars)).await {
                Ok(w) => {
                    history.set(w.finished_games_by_lobby);
                    if let Some(gid) = initial {
                        detail_game_id.set(Some(gid));
                        detail_open.set(true);
                    }
                }
                Err(e) => err.set(Some(e)),
            }
            loading.set(false);
        });
    });

    let history_rows = history();
    let standings = compute_lobby_standings(&history_rows);
    let on_close_backdrop = on_close;

    rsx! {
        div { class: "lobby-game-modal-layer",
            button {
                class: "lobby-game-modal-backdrop",
                onclick: move |_| on_close_backdrop.call(()),
            }
            div {
                class: "lobby-results-modal",
                role: "dialog",
                aria_modal: "true",
                div { class: "lobby-game-modal-head",
                    div {
                        p { class: "lobby-section-kicker", "MATCH HISTORY" }
                        h2 { class: "lobby-section-title", "Results & points" }
                    }
                    button {
                        class: "lobby-game-modal-close",
                        title: "Close",
                        onclick: move |_| on_close.call(()),
                        Icon { name: "close", filled: false }
                    }
                }

                div { class: "lobby-results-modal-body",
                    if let Some(e) = err() {
                        ErrorBanner { message: e }
                    } else if loading() {
                        LoadingState {
                            title: "Loading match history…".to_string(),
                            subtitle: game_type.clone(),
                        }
                    } else if history_rows.is_empty() {
                        EmptyState {
                            icon: "emoji_events",
                            title: "No matches yet".to_string(),
                            description: "Finished games in this lobby will appear here.".to_string(),
                            cta_label: None,
                            on_cta: None,
                        }
                    } else {
                        div { class: "space-y-5",
                            if !standings.is_empty() {
                                div { class: "space-y-2",
                                    h3 { class: "font-manrope font-semibold text-on-surface text-sm", "Lobby standings" }
                                    div { class: "section-card overflow-x-auto p-0",
                                        table { class: "data-table",
                                            thead {
                                                tr {
                                                    th { "#" }
                                                    th { "Player" }
                                                    th { "Points" }
                                                    th { "Matches" }
                                                }
                                            }
                                            tbody {
                                                for (i, row) in standings.iter().enumerate() {
                                                    tr {
                                                        td { class: "font-mono-code text-primary", "#{i + 1}" }
                                                        td {
                                                            div { class: "flex items-center gap-2",
                                                                Avatar {
                                                                    seed: row.display_name.clone(),
                                                                    size: AvatarSize::Sm,
                                                                    image_url: None,
                                                                }
                                                                "{row.display_name}"
                                                            }
                                                        }
                                                        td { class: "font-mono-code text-tertiary", "{row.total_points}" }
                                                        td { "{row.matches_played}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            div { class: "space-y-2",
                                h3 { class: "font-manrope font-semibold text-on-surface text-sm", "Match history" }
                                div { class: "section-card overflow-x-auto p-0",
                                    table { class: "data-table",
                                        thead {
                                            tr {
                                                th { "When" }
                                                th { "Outcome" }
                                                th { "Points" }
                                                th { "" }
                                            }
                                        }
                                        tbody {
                                            for row in history_rows.iter().cloned() {
                                                {
                                                    let gid = row.game_id.clone();
                                                    let summary = game_result_summary(&row);
                                                    let when = format_relative_time(row.finished_at);
                                                    let points = format_match_points(&parse_match_player_scores(&row));
                                                    let mut detail_open = detail_open;
                                                    let mut detail_game_id = detail_game_id;
                                                    rsx! {
                                                        tr {
                                                            td { class: "text-on-surface-variant whitespace-nowrap", "{when}" }
                                                            td { class: "font-medium text-on-surface", "{summary}" }
                                                            td { class: "text-body-sm text-on-surface-variant", "{points}" }
                                                            td { class: "text-right whitespace-nowrap",
                                                                button {
                                                                    class: "link-action",
                                                                    onclick: move |e| {
                                                                        e.stop_propagation();
                                                                        detail_game_id.set(Some(gid.clone()));
                                                                        detail_open.set(true);
                                                                    },
                                                                    "Details"
                                                                    Icon { name: "open_in_new", filled: false }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if is_owner && lobby_finished {
                    div { class: "lobby-results-modal-footer",
                        button {
                            class: "btn-secondary w-full sm:w-auto",
                            onclick: move |_| on_play_again.call(()),
                            Icon { name: "replay", filled: false }
                            "Play again"
                        }
                    }
                }
            }
        }

        if detail_open() {
            LobbyGameDetailModal {
                game_id: detail_game_id().unwrap_or_default(),
                history: history_rows.clone(),
                on_close: EventHandler::new(move |_| detail_open.set(false)),
            }
        }
    }
}

#[component]
fn LobbyGameDetailModal(
    game_id: String,
    history: Vec<GameResultRow>,
    on_close: EventHandler<()>,
) -> Element {
    let mut loaded: Signal<Option<LoadedGameResult>> = use_signal(|| None);
    let mut loading: Signal<bool> = use_signal(|| true);
    let mut err: Signal<Option<String>> = use_signal(|| None);

    let gid_fetch = game_id.clone();
    let cached = history.into_iter().find(|r| r.game_id == gid_fetch);

    use_effect(move || {
        let gid_fetch = gid_fetch.clone();
        let cached = cached.clone();
        let mut loaded = loaded;
        let mut loading = loading;
        let mut err = err;
        spawn(async move {
            loading.set(true);
            err.set(None);
            if let Some(row) = cached {
                loaded.set(Some(loaded_game_result_from_row(row)));
                loading.set(false);
                return;
            }
            load_detail(gid_fetch, loaded, loading, err).await;
        });
    });

    let on_close_backdrop = on_close;

    rsx! {
        div { class: "lobby-results-detail-layer",
            button {
                class: "lobby-game-modal-backdrop",
                onclick: move |_| on_close_backdrop.call(()),
            }
            div {
                class: "lobby-results-modal",
                role: "dialog",
                aria_modal: "true",
                div { class: "lobby-game-modal-head",
                    div {
                        p { class: "lobby-section-kicker", "MATCH DETAIL" }
                        h2 { class: "lobby-section-title",
                            "Game #{game_id.chars().take(8).collect::<String>()}"
                        }
                    }
                    button {
                        class: "lobby-game-modal-close",
                        title: "Close",
                        onclick: move |_| on_close.call(()),
                        Icon { name: "close", filled: false }
                    }
                }
                div { class: "lobby-results-modal-body",
                    if let Some(e) = err() {
                        ErrorBanner { message: e }
                    } else if loading() {
                        LoadingState {
                            title: "Loading result…".to_string(),
                            subtitle: game_id.clone(),
                        }
                    } else if let Some(ld) = loaded() {
                        LobbyResultDetail { loaded: ld }
                    } else {
                        EmptyState {
                            icon: "search_off",
                            title: "Result not found".to_string(),
                            description: "This match may have been removed.".to_string(),
                            cta_label: None,
                            on_cta: None,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LobbyResultDetail(loaded: LoadedGameResult) -> Element {
    let match_scores = parse_match_player_scores(&loaded.row);
    rsx! {
        div { class: "space-y-4",
            div { class: "flex flex-wrap gap-4 text-body-sm",
                span { class: "text-on-surface-variant",
                    "Game: "
                    span { class: "text-on-surface font-medium", "{loaded.row.game_type}" }
                }
                span { class: "text-on-surface-variant",
                    "Finished: "
                    span { class: "font-mono-code", "{format_relative_time(loaded.row.finished_at)}" }
                }
            }
            if !match_scores.is_empty() {
                div { class: "section-card overflow-x-auto p-0",
                    table { class: "data-table",
                        thead {
                            tr {
                                th { "Player" }
                                th { "Points" }
                            }
                        }
                        tbody {
                            for score in match_scores {
                                tr {
                                    td {
                                        div { class: "flex items-center gap-2",
                                            Avatar {
                                                seed: score.display_name.clone(),
                                                size: AvatarSize::Sm,
                                                image_url: None,
                                            }
                                            "{score.display_name}"
                                        }
                                    }
                                    td { class: "font-mono-code text-tertiary", "+{score.points}" }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(src) = loaded.iframe_src.clone() {
                iframe { class: "config-iframe min-h-[20rem]", src: src }
            } else {
                p { class: "text-body-sm text-secondary rounded-lg border border-secondary-container/30 bg-secondary-container/10 px-3 py-2",
                    "This game type has no client/result.html — raw payload below."
                }
            }
            details { class: "section-card",
                summary { class: "cursor-pointer text-primary font-medium", "Raw JSON" }
                div { class: "mt-4 space-y-4",
                    h3 { class: "font-manrope font-semibold", "Scores" }
                    JsonConsole { content: loaded.row.player_scores_json.clone(), max_height: None }
                    h3 { class: "font-manrope font-semibold", "Outcome" }
                    JsonConsole { content: loaded.row.result_json.clone(), max_height: None }
                    h3 { class: "font-manrope font-semibold", "Seats" }
                    JsonConsole { content: loaded.row.seats_snapshot_json.clone(), max_height: None }
                }
            }
        }
    }
}

async fn load_detail(
    game_id: String,
    mut loaded: Signal<Option<LoadedGameResult>>,
    mut loading: Signal<bool>,
    mut err: Signal<Option<String>>,
) {
    let q = r#"query G($id: ID!) { finishedGame(gameId: $id) { gameId gameType lobbyId finishedAt resultJson playerScoresJson seatsSnapshotJson resultUiPath } }"#;
    let vars = serde_json::json!({ "id": game_id });
    #[derive(Deserialize)]
    struct Wrap {
        #[serde(rename = "finishedGame")]
        finished_game: Option<GameResultRow>,
    }
    match graphql_exec::<Wrap>(q, Some(vars)).await {
        Ok(w) => {
            if let Some(r) = w.finished_game {
                loaded.set(Some(loaded_game_result_from_row(r)));
            } else {
                loaded.set(None);
            }
        }
        Err(e) => err.set(Some(e)),
    }
    loading.set(false);
}
