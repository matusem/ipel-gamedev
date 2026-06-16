use crate::components::ui::{Avatar, AvatarSize, Icon, StatusBadge, StatusVariant};
use crate::models::{game_type_cover_url, game_type_display_title, GameTypeInfo};
use crate::stub::game_media;
use crate::LobbyRoute;
use dioxus::prelude::*;

#[component]
pub fn LobbyRoomHeader(
    owner_user_id: String,
    owner_display_name: String,
    game_type: String,
    status: String,
    status_variant: StatusVariant,
    gt_list: Vec<GameTypeInfo>,
    is_owner: bool,
    show_config: bool,
    show_rules: bool,
    on_open_history: EventHandler<()>,
    on_open_games: EventHandler<()>,
    on_open_config: EventHandler<()>,
    on_open_rules: EventHandler<()>,
) -> Element {
    let nav = use_navigator();
    let no_game = game_type.trim().is_empty();
    let title = game_type_display_title(&gt_list, &game_type);
    let media = game_media(if no_game { "" } else { &game_type });
    let cover = if no_game {
        None
    } else {
        gt_list
            .iter()
            .find(|g| g.name == game_type)
            .and_then(game_type_cover_url)
    };

    rsx! {
        header { class: "lobby-room-header",
            div { class: "lobby-room-header-nav",
                button {
                    class: "lobby-command-back",
                    onclick: move |_| { nav.push(LobbyRoute::LobbiesBrowser {}); },
                    Icon { name: "arrow_back", filled: false }
                    "Lobbies"
                }
            }

            div { class: "lobby-room-header-divider" }

            div { class: "lobby-room-header-game",
                div { class: "lobby-room-header-art bg-gradient-to-br {media.accent_gradient}",
                    if let Some(ref url) = cover {
                        img { class: "lobby-room-header-cover", src: "{url}", alt: "" }
                    } else if no_game {
                        Icon { name: "sports_esports", filled: false }
                    } else {
                        span { class: "lobby-room-header-emoji", "{media.icon_emoji}" }
                    }
                }
                div { class: "lobby-room-header-game-text min-w-0",
                    p { class: "lobby-section-kicker", "Active game" }
                    h2 { class: "lobby-room-header-title truncate",
                        if no_game { "No game selected" } else { "{title}" }
                    }
                }
            }

            div { class: "lobby-room-header-right",
                div { class: "lobby-room-header-actions",
                    button {
                        class: "lobby-room-header-action",
                        disabled: !is_owner,
                        title: if is_owner { "Change game" } else { "Only the host can change the game" },
                        onclick: move |_| on_open_games.call(()),
                        span { class: "lobby-room-header-action-icon",
                            Icon { name: "grid_view", filled: false }
                        }
                        span { class: "lobby-room-header-action-label", "Change" }
                    }
                    if show_config {
                        button {
                            class: "lobby-room-header-action",
                            title: "Match settings",
                            onclick: move |_| on_open_config.call(()),
                            span { class: "lobby-room-header-action-icon",
                                Icon { name: "settings", filled: false }
                            }
                            span { class: "lobby-room-header-action-label", "Config" }
                        }
                    } else {
                        div { class: "lobby-room-header-action lobby-room-header-action-disabled",
                            span { class: "lobby-room-header-action-icon",
                                Icon { name: "settings", filled: false }
                            }
                            span { class: "lobby-room-header-action-label", "Config" }
                        }
                    }
                    if show_rules {
                        button {
                            class: "lobby-room-header-action",
                            title: "View game rules",
                            onclick: move |_| on_open_rules.call(()),
                            span { class: "lobby-room-header-action-icon",
                                Icon { name: "menu_book", filled: false }
                            }
                            span { class: "lobby-room-header-action-label", "Rules" }
                        }
                    } else {
                        div { class: "lobby-room-header-action lobby-room-header-action-disabled",
                            span { class: "lobby-room-header-action-icon",
                                Icon { name: "menu_book", filled: false }
                            }
                            span { class: "lobby-room-header-action-label", "Rules" }
                        }
                    }
                    button {
                        class: "lobby-room-header-action",
                        title: "Match history",
                        onclick: move |_| on_open_history.call(()),
                        span { class: "lobby-room-header-action-icon",
                            Icon { name: "history", filled: false }
                        }
                        span { class: "lobby-room-header-action-label", "History" }
                    }
                }

                div { class: "lobby-room-header-divider" }

                div { class: "lobby-room-header-meta",
                    div { class: "lobby-room-header-host",
                        Avatar {
                            seed: owner_user_id.clone(),
                            size: AvatarSize::Sm,
                            image_url: None,
                        }
                        div { class: "min-w-0 hidden lg:block",
                            p { class: "lobby-room-header-host-kicker", "Host" }
                            p { class: "lobby-room-header-host-name truncate", "{owner_display_name}" }
                        }
                    }
                    StatusBadge { label: status.clone(), variant: status_variant }
                }
            }
        }
    }
}
