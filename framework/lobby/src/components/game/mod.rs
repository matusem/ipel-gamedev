mod media_gallery;
mod steam_sections;
mod storefront_editor;

pub use media_gallery::MediaGallery;
pub use steam_sections::{scroll_to_section, SteamSection, SteamSectionNav, SECTION_IDS};
pub use storefront_editor::StorefrontEditor;

use crate::components::ui::Chip;
use crate::models::*;
use crate::stub::demo_images::cover_image_url;
use crate::LobbyRoute;
use dioxus::prelude::*;

fn resolve_cover(gt: &GameTypeInfo) -> String {
    gt.cover_image_url
        .clone()
        .or_else(|| cover_image_url(&gt.slug).map(str::to_string))
        .unwrap_or_default()
}

#[component]
pub fn GameCard(
    game: GameInfo,
    catalog: Vec<GameTypeInfo>,
    mut playing: Signal<Option<PlayOverlay>>,
) -> Element {
    let game_type = game.game_type.clone();
    let title = game_type_display_title(&catalog, &game.game_type);
    let desc = game_type_description(&catalog, &game.game_type);
    rsx! {
        div { class: "rounded-xl border border-outline-variant/40 bg-surface-container-low p-4 flex items-center justify-between flex-wrap gap-3",
            div { class: "min-w-0",
                p { class: "font-medium text-on-surface",
                    "{title}"
                    span { class: "text-outline text-mono-code font-mono-code ml-2", "{game.game_id}" }
                }
                if let Some(ref d) = desc {
                    p { class: "text-body-sm text-outline mt-1 line-clamp-2", "{d}" }
                }
                p { class: "text-label-caps font-label-caps text-outline mt-1 uppercase",
                    "Connected: {game.connected_players} / {game.player_identities.len()}"
                }
            }
            div { class: "flex flex-wrap gap-2",
                for identity in game.player_identities.clone() {
                    {
                        let gt = game_type.clone();
                        let gid = game.game_id.clone();
                        let pid = identity.clone();
                        rsx! {
                            button {
                                class: "btn-primary",
                                onclick: move |_| {
                                    playing.set(Some(PlayOverlay {
                                        game_type: gt.clone(),
                                        game_id: gid.clone(),
                                        player: pid.clone(),
                                        return_lobby_id: None,
                                        spectator: false,
                                        is_lobby_owner: false,
                                    }));
                                },
                                "Join as {pid}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn TrendingGameCard(gt: GameTypeInfo) -> Element {
    let nav = use_navigator();
    let slug = gt.slug.clone();
    let cover = resolve_cover(&gt);
    let active = gt.active_players;
    let active_dot = if active > 0 {
        "status-dot-online"
    } else {
        "status-dot-full"
    };
    let owner_hint = gt.owner_name.clone();
    rsx! {
        div {
            class: "trending-card group",
            onclick: move |_| {
                nav.push(LobbyRoute::GameDetail { name: slug.clone() });
            },
            div { class: "aspect-video relative overflow-hidden bg-surface-container-high",
                if !cover.is_empty() {
                    img {
                        class: "absolute inset-0 w-full h-full object-cover group-hover:scale-105 transition-transform duration-500",
                        src: "{cover}",
                        alt: "{gt.display_name}",
                    }
                    div { class: "absolute inset-0 bg-gradient-to-t from-background/90 via-background/20 to-transparent" }
                } else {
                    div { class: "absolute inset-0 flex items-center justify-center",
                        span { class: "font-manrope text-h2 text-primary-container/50",
                            "{gt.display_name}"
                        }
                    }
                }
                div { class: "absolute top-3 left-3 bg-surface-container-lowest/80 backdrop-blur px-2 py-1 rounded-md flex items-center gap-1.5 z-10",
                    span { class: "{active_dot}" }
                    span { class: "text-[10px] font-mono-code text-on-surface", "{active} Active" }
                }
            }
            div { class: "p-4 space-y-3",
                div { class: "flex justify-between items-start",
                    div { class: "min-w-0",
                        h3 { class: "font-manrope text-lg font-semibold text-on-surface", "{gt.display_name}" }
                        if let Some(ref owner) = owner_hint {
                            p { class: "text-xs text-outline truncate", "by {owner}" }
                        }
                    }
                    if gt.avg_session_mins > 0 {
                        span { class: "text-xs font-mono-code text-outline", "~{gt.avg_session_mins} min" }
                    }
                }
                div { class: "flex gap-2 flex-wrap",
                    for tag in gt.tags.clone() {
                        Chip { label: tag.to_string(), muted: true }
                    }
                }
            }
        }
    }
}

#[component]
pub fn GameTypeCatalogCard(gt: GameTypeInfo) -> Element {
    let nav = use_navigator();
    let desc = gt.description.trim();
    let about_url = game_type_about_url(&gt);
    let slug = gt.slug.clone();
    let cover = resolve_cover(&gt);
    let owner_hint = gt.owner_name.clone();
    rsx! {
        div {
            class: "game-catalog-card p-4 flex flex-col gap-3 cursor-pointer group",
            onclick: move |_| {
                nav.push(LobbyRoute::GameDetail { name: slug.clone() });
            },
            div { class: "aspect-video rounded-lg bg-surface-container-high relative overflow-hidden border border-outline-variant/30",
                if !cover.is_empty() {
                    img {
                        class: "absolute inset-0 w-full h-full object-cover group-hover:scale-105 transition-transform duration-500",
                        src: "{cover}",
                        alt: "{gt.display_name}",
                    }
                    div { class: "absolute inset-0 bg-gradient-to-t from-background/70 via-transparent to-transparent" }
                } else {
                    div { class: "absolute inset-0 flex items-center justify-center",
                        span { class: "font-manrope text-h2 text-primary-container/60", "{gt.display_name}" }
                    }
                }
            }
            div { class: "min-w-0 flex-1",
                p { class: "font-manrope font-semibold text-on-surface", "{gt.display_name}" }
                if let Some(ref owner) = owner_hint {
                    p { class: "text-xs text-outline", "by {owner}" }
                }
                p { class: "text-mono-code font-mono-code text-outline mt-0.5", "{gt.name} · v{gt.version}" }
                p { class: "text-label-caps font-label-caps text-secondary mt-1 uppercase", "{gt.min_players}–{gt.max_players} players" }
                if !desc.is_empty() {
                    p { class: "text-body-sm text-on-surface-variant mt-2 leading-relaxed line-clamp-2", "{desc}" }
                }
            }
            div { class: "flex items-center gap-2 flex-wrap",
                for tag in gt.tags.clone() {
                    Chip { label: tag.to_string(), muted: true }
                }
                if let Some(url) = about_url {
                    a {
                        class: "btn-secondary text-label-caps ml-auto",
                        href: "{url}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        onclick: |e| e.stop_propagation(),
                        "Info"
                    }
                }
            }
        }
    }
}
