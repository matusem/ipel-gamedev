use crate::api::*;
use crate::components::game::TrendingGameCard;
use crate::components::ui::*;
use crate::models::*;
use crate::stub::demo_images::cover_image_url;
use crate::LobbyRoute;
use dioxus::prelude::*;
use serde::Deserialize;

#[component]
pub fn HomePage(playing: Signal<Option<PlayOverlay>>, mut error_msg: Signal<Option<String>>) -> Element {
    let _playing = playing;
    let nav = use_navigator();
    let game_types: Signal<Vec<GameTypeInfo>> = use_signal(Vec::new);
    let lobbies: Signal<Vec<LobbySummary>> = use_signal(Vec::new);
    let loading = use_signal(|| true);
    let platform_stats: Signal<Option<PlatformStats>> = use_signal(|| None);
    let activity: Signal<Vec<ActivityEventGql>> = use_signal(Vec::new);

    use_hook(move || {
        let mut game_types = game_types;
        let mut lobbies = lobbies;
        let mut error_msg = error_msg;
        let mut loading = loading;
        let mut platform_stats = platform_stats;
        let mut activity = activity;
        start_lobbies_subscription(lobbies, error_msg);
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Boot {
                game_types: Vec<GameTypeInfo>,
                lobbies: Vec<LobbySummary>,
            }
            let q = format!(
                "query {{ gameTypes {{ {} }} lobbies {{ id gameType status seatsFilled seatsTotal ownerDisplayName gameInstanceId createdAt }} }}",
                crate::models::GAME_TYPES_GQL_FIELDS
            );
            match graphql_post::<Boot>(q).await {
                Ok(data) => {
                    game_types.set(data.game_types);
                    lobbies.set(data.lobbies);
                }
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct S { platform_stats: PlatformStats }
            if let Ok(s) = graphql_post::<S>(
                "query { platformStats { activeLobbies publishedGameTypes finishedGames24h activeSessions status trends { label value deltaPct up } proTip } }",
            )
            .await
            {
                platform_stats.set(Some(s.platform_stats));
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A { activity_feed: Vec<ActivityEventGql> }
            if let Ok(a) = graphql_post::<A>(
                "query { activityFeed(limit: 12) { actor action target timestamp } }",
            )
            .await
            {
                activity.set(a.activity_feed);
            }
        });
    });

    let featured = game_types()
        .iter()
        .find(|g| g.featured)
        .cloned()
        .or_else(|| game_types().first().cloned());

    let trends = platform_stats()
        .map(|s| s.trends)
        .unwrap_or_default();
    let pro_tip = platform_stats()
        .map(|s| s.pro_tip)
        .unwrap_or_default();

    rsx! {
        div { class: "grid grid-cols-12 gap-8",
            if let Some(err) = error_msg() {
                div { class: "col-span-12", ErrorBanner { message: err } }
            }
            if loading() {
                div { class: "col-span-12 space-y-6",
                    SkeletonHero {}
                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-6",
                        SkeletonCard {}
                        SkeletonCard {}
                        SkeletonCard {}
                    }
                }
            } else {
                if let Some(feat) = featured {
                    {
                        let fname = feat.slug.clone();
                        let display = feat.display_name.clone();
                        let desc = feat.description.clone();
                        let long_desc = if desc.is_empty() {
                            feat.tags.join(" · ")
                        } else {
                            desc.clone()
                        };
                        let cover = feat.cover_image_url.clone()
                            .or_else(|| cover_image_url(&feat.slug).map(str::to_string))
                            .unwrap_or_default();
                        rsx! {
                            section { class: "col-span-12 page-hero group",
                                if !cover.is_empty() {
                                    img {
                                        class: "absolute inset-0 w-full h-full object-cover z-0 transition-transform duration-[2000ms] group-hover:scale-105",
                                        src: "{cover}",
                                        alt: "{display}",
                                    }
                                }
                                div { class: "absolute inset-0 bg-gradient-to-br from-primary-container/40 via-surface-container-low to-background z-0 mix-blend-multiply opacity-60" }
                                div { class: "absolute inset-0 bg-gradient-to-t from-background via-background/60 to-transparent z-10" }
                                div { class: "relative z-20 h-full flex flex-col justify-end p-8 lg:p-12 space-y-6 min-h-[320px] lg:min-h-[420px]",
                                    span { class: "inline-flex items-center gap-2 px-3 py-1 bg-primary-container/20 text-primary border border-primary-container/30 rounded-full font-label-caps text-xs w-fit",
                                        span { class: "status-dot-online animate-pulse" }
                                        "Featured Game"
                                    }
                                    h1 { class: "font-manrope text-h1 text-4xl lg:text-6xl text-on-surface", "{display}" }
                                    p { class: "font-body-lg text-on-surface-variant max-w-2xl", "{long_desc}" }
                                    div { class: "flex items-center gap-4 flex-wrap",
                                        button {
                                            class: "btn-primary btn-lg active:scale-95 transition-transform",
                                            onclick: move |_| { nav.push(LobbyRoute::GameDetail { name: fname.clone() }); },
                                            Icon { name: "play_arrow", filled: true }
                                            "Play Now"
                                        }
                                        button {
                                            class: "btn-secondary btn-lg",
                                            onclick: move |_| { nav.push(LobbyRoute::LobbiesBrowser {}); },
                                            "Browse Lobbies"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                section { class: "col-span-12 lg:col-span-9 space-y-8",
                    div { class: "flex items-center justify-between",
                        h2 { class: "font-manrope text-h2 text-on-surface", "Trending Games" }
                        LinkAction {
                            label: "View All".to_string(),
                            icon: Some("arrow_forward"),
                            onclick: move |_| { nav.push(LobbyRoute::GamesList {}); },
                        }
                    }
                    if game_types().is_empty() {
                        EmptyState {
                            icon: "sports_esports",
                            title: "No games yet".to_string(),
                            description: "Upload your first game in Developer Hub.".to_string(),
                            cta_label: Some("Developer Hub".to_string()),
                            on_cta: Some(EventHandler::new(move |_| {
                                nav.push(LobbyRoute::DeveloperUploads {});
                            })),
                        }
                    } else {
                        div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6",
                            for gt in game_types().iter().take(6) {
                                TrendingGameCard { gt: gt.clone() }
                            }
                        }
                    }
                    div { class: "grid grid-cols-1 md:grid-cols-2 gap-6",
                        QuickLinkCard {
                            title: "Lobby Browser".to_string(),
                            subtitle: format!("Join {} active lobbies currently running.", lobbies().len()),
                            icon: "dns",
                            accent: "text-primary",
                            onclick: move |_| { nav.push(LobbyRoute::LobbiesBrowser {}); },
                        }
                        QuickLinkCard {
                            title: "Developer Console".to_string(),
                            subtitle: "Upload WASM builds and manage game drafts.".to_string(),
                            icon: "monitoring",
                            accent: "text-tertiary",
                            onclick: move |_| { nav.push(LobbyRoute::DeveloperUploads {}); },
                        }
                    }
                }

                aside { class: "col-span-12 lg:col-span-3 space-y-6",
                    div { class: "grid gap-3",
                        KpiCard {
                            label: "Active lobbies".to_string(),
                            value: platform_stats().map(|s| s.active_lobbies.to_string()).unwrap_or_else(|| lobbies().len().to_string()),
                            icon: Some("groups"),
                            trend: trends.get(0).map(|t| t.delta_pct.clone()),
                            trend_up: trends.get(0).map(|t| t.up).unwrap_or(true),
                        }
                        KpiCard {
                            label: "Published games".to_string(),
                            value: platform_stats().map(|s| s.published_game_types.to_string()).unwrap_or_else(|| game_types().len().to_string()),
                            icon: Some("sports_esports"),
                            trend: trends.get(1).map(|t| t.delta_pct.clone()),
                            trend_up: trends.get(1).map(|t| t.up).unwrap_or(true),
                        }
                        KpiCard {
                            label: "Finished (24h)".to_string(),
                            value: platform_stats().map(|s| s.finished_games24h.to_string()).unwrap_or_else(|| "—".into()),
                            icon: Some("signal_cellular_alt"),
                            trend: trends.get(2).map(|t| t.delta_pct.clone()),
                            trend_up: trends.get(2).map(|t| t.up).unwrap_or(true),
                        }
                    }
                    div { class: "section-card",
                        h3 { class: "card-title-sm mb-4", "Live Pulse" }
                        if activity().is_empty() {
                            p { class: "text-body-sm text-outline", "No recent activity yet." }
                        } else {
                            ActivityFeed { events: activity() }
                        }
                    }
                    div { class: "section-card border-primary-container/20 bg-primary-container/5",
                        h3 { class: "font-manrope font-semibold text-primary text-sm mb-2", "Pro tip" }
                        p { class: "text-body-sm text-on-surface-variant",
                            if pro_tip.is_empty() { "Claim a seat and mark Ready before the host launches." } else { "{pro_tip}" }
                        }
                    }
                    div { class: "section-card",
                        h3 { class: "card-title-sm mb-3", "Recent Lobbies" }
                        if lobbies().is_empty() {
                            p { class: "text-body-sm text-outline", "No active lobbies." }
                        } else {
                            div { class: "space-y-2",
                                for lob in lobbies().iter().take(5) {
                                    {
                                        let lid = lob.id.clone();
                                        let title = game_type_display_title(&game_types(), &lob.game_type);
                                        rsx! {
                                            button {
                                                class: "w-full text-left rounded-lg border border-outline-variant/30 px-3 py-2 hover:bg-surface-container-high transition-colors",
                                                onclick: move |_| { nav.push(LobbyRoute::Lobby { id: lid.clone() }); },
                                                p { class: "text-body-sm font-medium text-on-surface truncate", "{title}" }
                                                p { class: "text-label-caps font-label-caps text-outline mt-0.5",
                                                    "{lob.seats_filled}/{lob.seats_total} · {lob.status}"
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
