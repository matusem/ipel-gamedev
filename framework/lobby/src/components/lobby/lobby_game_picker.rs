use crate::api::{graphql_error_message, lobby_mutation_needs_force, set_lobby_game_type};
use crate::components::ui::{push_toast, use_toast, Icon, ToastKind};
use crate::models::{
    game_type_cover_url, game_type_display_title, GameTypeInfo, LobbyDetail,
};
use crate::stub::game_media;
use dioxus::prelude::*;

#[component]
pub fn LobbyActiveGame(
    selected_game_type: String,
    gt_list: Vec<GameTypeInfo>,
    is_owner: bool,
    on_open_games: EventHandler<()>,
    show_config: bool,
    on_open_config: EventHandler<()>,
    show_rules: bool,
    on_open_rules: EventHandler<()>,
) -> Element {
    let no_game = selected_game_type.trim().is_empty();
    let title = game_type_display_title(&gt_list, &selected_game_type);
    let media = game_media(if no_game { "" } else { &selected_game_type });
    let cover = if no_game {
        None
    } else {
        gt_list
            .iter()
            .find(|g| g.name == selected_game_type)
            .and_then(game_type_cover_url)
    };

    rsx! {
        div { class: "lobby-game-bar",
            div { class: "lobby-game-bar-active",
                div { class: "lobby-game-bar-art bg-gradient-to-br {media.accent_gradient}",
                    if let Some(ref url) = cover {
                        img { class: "lobby-game-bar-cover", src: "{url}", alt: "" }
                    } else if no_game {
                        Icon { name: "sports_esports", filled: false }
                    } else {
                        span { class: "lobby-game-bar-emoji", "{media.icon_emoji}" }
                    }
                }
                div { class: "lobby-game-bar-text min-w-0",
                    p { class: "lobby-section-kicker", "Active game" }
                    h2 { class: "lobby-game-bar-title truncate",
                        if no_game { "No game selected" } else { "{title}" }
                    }
                }
            }
            div { class: "lobby-game-bar-actions",
                button {
                    class: "lobby-game-bar-action",
                    disabled: !is_owner,
                    title: if is_owner { "Change game" } else { "Only the host can change the game" },
                    onclick: move |_| on_open_games.call(()),
                    span { class: "lobby-game-bar-action-icon",
                        Icon { name: "grid_view", filled: false }
                    }
                    span { class: "lobby-game-bar-action-label", "Change game" }
                }
                if show_config {
                    button {
                        class: "lobby-game-bar-action",
                        title: "Match settings",
                        onclick: move |_| on_open_config.call(()),
                        span { class: "lobby-game-bar-action-icon",
                            Icon { name: "settings", filled: false }
                        }
                        span { class: "lobby-game-bar-action-label", "Game config" }
                    }
                } else {
                    div { class: "lobby-game-bar-action lobby-game-bar-action-disabled",
                        span { class: "lobby-game-bar-action-icon",
                            Icon { name: "settings", filled: false }
                        }
                        span { class: "lobby-game-bar-action-label", "Game config" }
                    }
                }
                if show_rules {
                    button {
                        class: "lobby-game-bar-action",
                        title: "View game rules",
                        onclick: move |_| on_open_rules.call(()),
                        span { class: "lobby-game-bar-action-icon",
                            Icon { name: "menu_book", filled: false }
                        }
                        span { class: "lobby-game-bar-action-label", "Game rules" }
                    }
                } else {
                    div { class: "lobby-game-bar-action lobby-game-bar-action-disabled",
                        span { class: "lobby-game-bar-action-icon",
                            Icon { name: "menu_book", filled: false }
                        }
                        span { class: "lobby-game-bar-action-label", "Game rules" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn LobbyGameRulesModal(
    open: bool,
    on_close: EventHandler<()>,
    title: String,
    about_url: String,
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
                class: "lobby-game-rules-modal",
                role: "dialog",
                aria_modal: "true",
                div { class: "lobby-game-modal-head",
                    div {
                        p { class: "lobby-section-kicker", "GAME RULES" }
                        h2 { class: "lobby-section-title", "{title}" }
                    }
                    button {
                        class: "lobby-game-modal-close",
                        title: "Close",
                        onclick: move |_| on_close.call(()),
                        Icon { name: "close", filled: false }
                    }
                }
                div { class: "lobby-game-rules-modal-body",
                    iframe {
                        class: "lobby-game-rules-frame",
                        src: "{about_url}",
                        title: "Game rules for {title}",
                    }
                }
            }
        }
    }
}

#[component]
pub fn LobbyGameModal(
    open: bool,
    on_close: EventHandler<()>,
    lobby_id: String,
    selected_game_type: String,
    gt_list: Vec<GameTypeInfo>,
    is_owner: bool,
    on_detail_updated: EventHandler<LobbyDetail>,
) -> Element {
    if !open {
        return rsx! {};
    }

    let toast = use_toast();
    let no_game_yet = selected_game_type.trim().is_empty();

    rsx! {
        div { class: "lobby-game-modal-layer",
            button {
                class: "lobby-game-modal-backdrop",
                onclick: move |_| on_close.call(()),
            }
            div {
                class: "lobby-game-modal",
                role: "dialog",
                aria_modal: "true",
                div { class: "lobby-game-modal-head",
                    div {
                        p { class: "lobby-section-kicker", "GAME LIBRARY" }
                        h2 { class: "lobby-section-title", "Select mode" }
                    }
                    button {
                        class: "lobby-game-modal-close",
                        onclick: move |_| on_close.call(()),
                        Icon { name: "close", filled: false }
                    }
                }
                if !is_owner && no_game_yet {
                    p { class: "lobby-game-modal-wait",
                        Icon { name: "hourglass_top", filled: false }
                        "Waiting for the host to pick a game…"
                    }
                } else {
                    div { class: "lobby-game-modal-list",
                        for gt in gt_list.iter().cloned() {
                            {
                                let active = !no_game_yet && gt.name == selected_game_type;
                                let media = game_media(&gt.name);
                                let cover = game_type_cover_url(&gt);
                                let lid = lobby_id.clone();
                                let gtn = gt.name.clone();
                                let desc = gt.description.trim();
                                let on_close = on_close;
                                let on_detail_updated = on_detail_updated;
                                let card_class = if active {
                                    "lobby-game-modal-item lobby-game-modal-item-active"
                                } else {
                                    "lobby-game-modal-item"
                                };
                                rsx! {
                                    button {
                                        class: "{card_class}",
                                        disabled: !is_owner,
                                        onclick: move |_| {
                                            if !is_owner { return; }
                                            let lid = lid.clone();
                                            let gtn = gtn.clone();
                                            let toast = toast;
                                            let on_close = on_close;
                                            let on_detail_updated = on_detail_updated;
                                            spawn(async move {
                                                match set_lobby_game_type(&lid, &gtn, false).await {
                                                    Ok(updated) => {
                                                        on_detail_updated.call(updated);
                                                        on_close.call(());
                                                        push_toast(toast.show, "Game selected", ToastKind::Success);
                                                    }
                                                    Err(e) if lobby_mutation_needs_force(&e) => {
                                                        let force = web_sys::window()
                                                            .map(|w| {
                                                                w.confirm_with_message(
                                                                    "Changing mode resets claimed seats. Continue?",
                                                                )
                                                                .unwrap_or(false)
                                                            })
                                                            .unwrap_or(false);
                                                        if force {
                                                            match set_lobby_game_type(&lid, &gtn, true).await {
                                                                Ok(updated) => {
                                                                    on_detail_updated.call(updated);
                                                                    on_close.call(());
                                                                    push_toast(
                                                                        toast.show,
                                                                        "Game changed",
                                                                        ToastKind::Success,
                                                                    );
                                                                }
                                                                Err(e2) => push_toast(
                                                                    toast.show,
                                                                    graphql_error_message(&e2),
                                                                    ToastKind::Error,
                                                                ),
                                                            }
                                                        }
                                                    }
                                                    Err(e) => push_toast(
                                                        toast.show,
                                                        graphql_error_message(&e),
                                                        ToastKind::Error,
                                                    ),
                                                }
                                            });
                                        },
                                        div { class: "lobby-game-modal-thumb bg-gradient-to-br {media.accent_gradient}",
                                            if let Some(ref url) = cover {
                                                img { class: "lobby-game-modal-cover", src: "{url}", alt: "" }
                                            } else {
                                                span { class: "text-2xl", "{media.icon_emoji}" }
                                            }
                                        }
                                        div { class: "lobby-game-modal-meta min-w-0",
                                            p { class: "lobby-game-modal-name", "{gt.display_name}" }
                                            if !desc.is_empty() {
                                                p { class: "lobby-game-modal-desc", "{desc}" }
                                            }
                                            span { class: "lobby-game-modal-players",
                                                "{gt.min_players}–{gt.max_players} players"
                                            }
                                        }
                                        if active {
                                            Icon { name: "check_circle", filled: true }
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
