use crate::api::{graphql_post, start_lobbies_subscription};
use crate::components::SearchContext;
use crate::components::ui::*;
use crate::models::*;
use crate::stub::{game_media, lobby_elapsed_stub};
use crate::LobbyRoute;
use dioxus::prelude::*;
use serde::Deserialize;

const PAGE_SIZE: usize = 10;

#[component]
pub fn LobbiesBrowserPage() -> Element {
    let nav = use_navigator();
    let search_ctx = use_context::<SearchContext>();
    let game_types: Signal<Vec<GameTypeInfo>> = use_signal(Vec::new);
    let lobbies: Signal<Vec<LobbySummary>> = use_signal(Vec::new);
    let loading = use_signal(|| true);
    let mut error_msg = use_signal(|| None::<String>);
    let mut segment = use_signal(|| 0usize);
    let mut page = use_signal(|| 0usize);
    let toast = use_toast();

    use_hook(move || {
        let mut game_types = game_types;
        let mut lobbies = lobbies;
        let mut error_msg = error_msg;
        let mut loading = loading;
        start_lobbies_subscription(lobbies, error_msg);
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Boot {
                game_types: Vec<GameTypeInfo>,
                lobbies: Vec<LobbySummary>,
            }
            let q = r#"query {
                gameTypes { name displayName version minPlayers maxPlayers description configUiPath aboutUiPath configSchemaJson }
                lobbies { id gameType status seatsFilled seatsTotal ownerDisplayName gameInstanceId createdAt }
            }"#;
            match graphql_post::<Boot>(q).await {
                Ok(data) => {
                    game_types.set(data.game_types);
                    lobbies.set(data.lobbies);
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    let query = (search_ctx.query)().to_lowercase();
    let filtered: Vec<LobbySummary> = lobbies()
        .into_iter()
        .filter(|l| {
            let title = game_type_display_title(&game_types(), &l.game_type).to_lowercase();
            let matches_search = query.is_empty()
                || title.contains(&query)
                || l.owner_display_name.to_lowercase().contains(&query)
                || l.game_type.to_lowercase().contains(&query);
            let s = l.status.to_lowercase();
            let matches_segment = match segment() {
                1 => s.contains("open") || s.contains("waiting") || s.contains("config"),
                2 => s.contains("in_game") || s.contains("playing"),
                3 => l.seats_total > 0 && l.seats_filled >= l.seats_total,
                _ => true,
            };
            matches_search && matches_segment
        })
        .collect();

    let total = filtered.len();
    let pg = page();
    let start = pg * PAGE_SIZE;
    let page_items: Vec<LobbySummary> = filtered.into_iter().skip(start).take(PAGE_SIZE).collect();
    let showing = page_items.len();

    rsx! {
        div { class: "page-stack",
            PageHeader {
                title: "Active Lobbies".to_string(),
                subtitle: Some("Join an open room or create one with Launch Game.".to_string()),
                badge: None,
                children: None,
            }
            SegmentedControl {
                options: vec!["All", "Open", "In Game", "Full"],
                active: segment(),
                on_select: move |i| { segment.set(i); page.set(0); },
            }
            if let Some(err) = error_msg() {
                ErrorBanner { message: err }
            }
            if loading() {
                div { class: "section-card p-0", SkeletonTableRows { count: 5 } }
            } else if page_items.is_empty() {
                EmptyState {
                    icon: "groups",
                    title: "No lobbies match".to_string(),
                    description: "Try different filters or create a new lobby.".to_string(),
                    cta_label: Some("Launch Game".to_string()),
                    on_cta: None,
                }
            } else {
                div { class: "hidden md:block section-card overflow-x-auto p-0",
                    table { class: "data-table",
                        thead {
                            tr {
                                th { "" }
                                th { "Lobby" }
                                th { "Game" }
                                th { "Status" }
                                th { "Seats" }
                                th { "Owner" }
                                th { "Elapsed" }
                                th { "Action" }
                            }
                        }
                        tbody {
                            for lob in page_items.clone() {
                                {
                                    let types = game_types();
                                    let title = game_type_display_title(&types, &lob.game_type);
                                    let lid = lob.id.clone();
                                    let media = game_media(&lob.game_type);
                                    let variant = status_variant_from_lobby(&lob.status, lob.seats_filled, lob.seats_total);
                                    let is_full = lob.seats_total > 0 && lob.seats_filled >= lob.seats_total;
                                    let in_game = lob.status.to_lowercase().contains("in_game");
                                    rsx! {
                                        tr {
                                            onclick: move |_| {
                                                if !is_full {
                                                    nav.push(LobbyRoute::Lobby { id: lid.clone() });
                                                }
                                            },
                                            td {
                                                div { class: "game-thumb bg-surface-container-high", "{media.icon_emoji}" }
                                            }
                                            td { class: "font-mono-code text-sm", "#{lob.id.chars().take(8).collect::<String>()}" }
                                            td { class: "font-medium", "{title}" }
                                            td { StatusBadge { label: lob.status.clone(), variant } }
                                            td { "{lob.seats_filled}/{lob.seats_total}" }
                                            td {
                                                div { class: "flex items-center gap-2",
                                                    Avatar { seed: lob.owner_display_name.clone(), size: AvatarSize::Sm, image_url: None }
                                                    span { class: "text-on-surface-variant", "{lob.owner_display_name}" }
                                                }
                                            }
                                            td { class: "font-mono-code text-outline", "{lobby_elapsed_stub(lob.created_at)}" }
                                            td {
                                                if is_full {
                                                    span { class: "text-outline text-body-sm", "Locked" }
                                                } else if in_game {
                                                    button {
                                                        class: "btn-ghost btn-sm",
                                                        onclick: move |e| {
                                                            e.stop_propagation();
                                                            push_toast(toast.show, "Spectate coming soon", ToastKind::Info);
                                                        },
                                                        "Spectate"
                                                    }
                                                } else {
                                                    span { class: "text-primary text-body-sm", "Join →" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Pagination {
                        showing,
                        total,
                        page: pg,
                        page_size: PAGE_SIZE,
                        on_page: move |p| page.set(p),
                    }
                }
                div { class: "md:hidden space-y-3",
                    for lob in page_items {
                        {
                            let types = game_types();
                            let title = game_type_display_title(&types, &lob.game_type);
                            let lid = lob.id.clone();
                            let media = game_media(&lob.game_type);
                            let variant = status_variant_from_lobby(&lob.status, lob.seats_filled, lob.seats_total);
                            rsx! {
                                button {
                                    class: "section-card w-full text-left flex items-center gap-4",
                                    onclick: move |_| { nav.push(LobbyRoute::Lobby { id: lid.clone() }); },
                                    div { class: "game-thumb", "{media.icon_emoji}" }
                                    div { class: "flex-1 min-w-0",
                                        p { class: "font-medium text-on-surface truncate", "{title}" }
                                        p { class: "text-body-sm text-outline", "{lob.owner_display_name}" }
                                        StatusBadge { label: lob.status.clone(), variant }
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
