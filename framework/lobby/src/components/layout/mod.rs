use crate::api::{create_lobby_with_game, stored_user_id};
use crate::components::ui::{push_toast, Avatar, AvatarSize, Icon, SearchInput, ToastKind, use_toast};
use crate::api::graphql_post;
use crate::models::{format_relative_time, NotificationGql, PlatformStats};
use crate::stub::demo_mode;
use crate::LobbyRoute;
use dioxus::prelude::*;
use serde::Deserialize;

#[derive(Clone, Copy)]
pub struct SearchContext {
    pub query: Signal<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum NavTab {
    Discover,
    Games,
    Lobbies,
    Developer,
    Profile,
}

impl NavTab {
    fn from_route(route: &LobbyRoute) -> Self {
        match route {
            LobbyRoute::Home {} => NavTab::Discover,
            LobbyRoute::GamesList {} | LobbyRoute::GameDetail { .. } => NavTab::Games,
            LobbyRoute::LobbiesBrowser {}
            | LobbyRoute::Lobby { .. }
            | LobbyRoute::GameResult { .. } => NavTab::Lobbies,
            LobbyRoute::DeveloperUploads {} => NavTab::Developer,
            LobbyRoute::Profile {} | LobbyRoute::Settings {} => NavTab::Profile,
        }
    }
}

#[component]
pub fn AppShell(children: Element) -> Element {
    let nav = use_navigator();
    let route = use_route::<LobbyRoute>();
    let active = NavTab::from_route(&route);
    let mut creating = use_signal(|| false);
    let mut notif_open = use_signal(|| false);
    let mut search_query: Signal<String> = use_signal(String::new);
    use_context_provider(|| SearchContext { query: search_query });
    let toast = use_toast();
    let mut server_status = use_signal(|| "System Stable".to_string());
    let mut ping_ms = use_signal(|| 18u32);
    let mut notifications = use_signal(Vec::<NotificationGql>::new);
    let mut unread_count = use_signal(|| 0i32);

    let reload_notifications = move || {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct N { my_notifications: Vec<NotificationGql> }
            if let Ok(n) = graphql_post::<N>(
                "query { myNotifications(limit: 12) { id title body kind unread createdAt } }",
            )
            .await
            {
                let unread = n.my_notifications.iter().filter(|x| x.unread).count() as i32;
                notifications.set(n.my_notifications);
                unread_count.set(unread);
            }
        });
    };

    use_hook(move || {
        spawn(async move {
            let start = js_sys::Date::now();
            if gloo_net::http::Request::get("/health").send().await.is_ok() {
                ping_ms.set((js_sys::Date::now() - start).round() as u32);
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct S { platform_stats: PlatformStats }
            if let Ok(s) = graphql_post::<S>(
                "query { platformStats { activeLobbies publishedGameTypes finishedGames24h status } }",
            )
            .await
            {
                let label = if s.platform_stats.status == "ok" {
                    "System Stable"
                } else {
                    "Degraded"
                };
                server_status.set(label.to_string());
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct U { unread_notification_count: i32 }
            if stored_user_id().is_some() {
                if let Ok(u) = graphql_post::<U>("query { unreadNotificationCount }").await {
                    unread_count.set(u.unread_notification_count);
                }
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct N { my_notifications: Vec<NotificationGql> }
                if let Ok(n) = graphql_post::<N>(
                    "query { myNotifications(limit: 12) { id title body kind unread createdAt } }",
                )
                .await
                {
                    notifications.set(n.my_notifications);
                }
            }
        });
    });
    let unread = unread_count() as usize;
    let user_id = stored_user_id().unwrap_or_else(|| "Player".into());
    let show_fab = active == NavTab::Discover;
    let lobby_room = matches!(route, LobbyRoute::Lobby { .. });
    let demo_on = demo_mode::is_demo_mode();
    const HEADER_CTRL_H: &str = "h-9";
    const HEADER_ICON_BTN: &str =
        "inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-lg text-on-surface-variant hover:text-primary hover:bg-surface-container-low/80 transition-colors";
    let demo_header_class = if demo_on {
        "hidden sm:inline-flex items-center justify-center gap-1.5 px-3 h-9 shrink-0 rounded-lg text-label-caps font-label-caps uppercase text-xs border border-secondary-container/50 bg-secondary-container/20 text-secondary hover:bg-secondary-container/30 transition-colors"
    } else {
        "hidden sm:inline-flex items-center justify-center gap-1.5 px-3 h-9 shrink-0 rounded-lg text-label-caps font-label-caps uppercase text-xs border border-outline-variant/50 bg-surface-container-low text-on-surface-variant hover:text-primary hover:border-primary-container/40 transition-colors"
    };
    let demo_icon = if demo_on { "science" } else { "auto_awesome" };
    let create_lobby = move |_| {
        creating.set(true);
        let nav = nav;
        let toast = toast;
        spawn(async move {
            match create_lobby_with_game(None).await {
                Ok(id) => {
                    push_toast(toast.show, "Lobby created", ToastKind::Success);
                    nav.push(LobbyRoute::Lobby { id });
                }
                Err(e) => push_toast(toast.show, e, ToastKind::Error),
            }
            creating.set(false);
        });
    };

    rsx! {
        div { class: "min-h-screen bg-background text-on-surface",
            header {
                class: "sticky top-0 w-full z-50 flex items-center justify-between px-4 sm:px-6 h-16 bg-surface-container-lowest/90 backdrop-blur-md border-b border-outline-variant/40",
                div { class: "flex items-center gap-4 md:pl-64 min-w-0",
                    span { class: "text-lg sm:text-xl font-manrope font-bold tracking-tighter text-on-surface shrink-0",
                        "IPEL GameDev"
                    }
                }
                div { class: "flex items-center gap-2 shrink-0",
                    button {
                        class: "{demo_header_class}",
                        onclick: move |_| demo_mode::toggle_demo_mode_and_reload(),
                        Icon { name: demo_icon, filled: demo_on }
                        if demo_on { "Demo on" } else { "Demo" }
                    }
                    SearchInput {
                        placeholder: "Search…",
                        value: search_query(),
                        width_class: "w-28 sm:w-40 lg:w-48",
                        oninput: move |val| search_query.set(val),
                    }
                    div { class: "relative shrink-0",
                        button {
                            class: "{HEADER_ICON_BTN}",
                            aria_label: "Notifications",
                            onclick: move |_| {
                                let open = !notif_open();
                                notif_open.set(open);
                                if open {
                                    reload_notifications();
                                }
                            },
                            Icon { name: "notifications", filled: false }
                        }
                        if unread > 0 {
                            span { class: "absolute top-1 right-1 block h-2 w-2 rounded-full bg-primary-container ring-2 ring-surface-container-lowest" }
                        }
                        if notif_open() {
                            div { class: "absolute right-0 top-full mt-2 w-72 rounded-xl border border-outline-variant/40 bg-surface-container shadow-raised z-50",
                                div { class: "px-4 py-3 border-b border-outline-variant/30 flex justify-between items-center gap-2",
                                    p { class: "font-manrope font-semibold text-on-surface", "Notifications" }
                                    div { class: "flex items-center gap-2",
                                        if unread > 0 {
                                            button {
                                                class: "text-label-caps font-label-caps text-primary hover:text-on-surface",
                                                onclick: move |_| {
                                                    spawn(async move {
                                                        let _ = graphql_post::<serde_json::Value>(
                                                            "mutation { markAllNotificationsRead }"
                                                        ).await;
                                                        reload_notifications();
                                                    });
                                                },
                                                "Mark all read"
                                            }
                                        }
                                        button {
                                            class: "text-label-caps font-label-caps text-outline hover:text-on-surface",
                                            onclick: move |_| notif_open.set(false),
                                            "Close"
                                        }
                                    }
                                }
                                if notifications().is_empty() {
                                    p { class: "px-4 py-6 text-body-sm text-outline text-center", "No notifications yet." }
                                }
                                for item in notifications() {
                                    div {
                                        class: if item.unread {
                                            "px-4 py-3 border-b border-outline-variant/20 bg-primary-container/5"
                                        } else {
                                            "px-4 py-3 border-b border-outline-variant/20"
                                        },
                                        p { class: "text-body-sm font-medium text-on-surface", "{item.title}" }
                                        p { class: "text-body-sm text-on-surface-variant mt-0.5", "{item.body}" }
                                        p { class: "text-label-caps font-label-caps text-outline mt-1",
                                            "{format_relative_time(item.created_at)}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    button {
                        class: "{HEADER_ICON_BTN} hidden sm:inline-flex",
                        aria_label: "Settings",
                        onclick: move |_| { nav.push(LobbyRoute::Settings {}); },
                        Icon { name: "settings", filled: false }
                    }
                    button {
                        class: "{HEADER_ICON_BTN}",
                        onclick: move |_| { nav.push(LobbyRoute::Profile {}); },
                        title: "{user_id}",
                        Avatar { seed: user_id.clone(), size: AvatarSize::Sm, image_url: None }
                    }
                }
            }

            aside {
                class: "hidden md:flex flex-col fixed left-0 top-0 h-screen w-64 border-r border-outline-variant/40 bg-surface-container-lowest z-40 pt-16",
                div { class: "px-6 py-6",
                    div { class: "flex items-center gap-3",
                        div { class: "w-10 h-10 bg-primary-container flex items-center justify-center rounded-lg",
                            Icon { name: "terminal", filled: true }
                        }
                        div {
                            h2 { class: "text-lg font-black text-on-surface uppercase tracking-widest font-manrope leading-none", "IPEL" }
                            p { class: "text-[10px] text-tertiary font-mono-code mt-0.5",
                                "{server_status()} · {ping_ms()}ms"
                            }
                        }
                    }
                }
                nav { class: "flex-1 space-y-1 w-full",
                    SidebarLink { label: "Discover", icon: "explore", active: active == NavTab::Discover, onclick: move |_| { nav.push(LobbyRoute::Home {}); } }
                    SidebarLink { label: "Games", icon: "sports_esports", active: active == NavTab::Games, onclick: move |_| { nav.push(LobbyRoute::GamesList {}); } }
                    SidebarLink { label: "Lobbies", icon: "groups", active: active == NavTab::Lobbies, onclick: move |_| { nav.push(LobbyRoute::LobbiesBrowser {}); } }
                    SidebarLink { label: "Developer Hub", icon: "terminal", active: active == NavTab::Developer, onclick: move |_| { nav.push(LobbyRoute::DeveloperUploads {}); } }
                    SidebarLink { label: "Profile", icon: "account_circle", active: active == NavTab::Profile, onclick: move |_| { nav.push(LobbyRoute::Profile {}); } }
                }
                div { class: "px-4 pb-6 mt-auto space-y-3",
                    if demo_on {
                        div { class: "rounded-lg border border-secondary-container/40 bg-secondary-container/10 px-3 py-2.5",
                            div { class: "flex items-start gap-2",
                                span { class: "text-secondary mt-0.5 shrink-0",
                                    Icon { name: "science", filled: true }
                                }
                                div { class: "min-w-0 flex-1",
                                    p { class: "text-xs font-semibold text-secondary leading-tight", "Demo mode" }
                                    p { class: "text-[10px] text-on-surface-variant mt-0.5 leading-snug",
                                        "Synthetic lobbies, reviews, leaderboards. No backend."
                                    }
                                }
                                button {
                                    class: "text-[10px] font-label-caps font-label-caps uppercase text-outline hover:text-on-surface shrink-0 px-1 py-0.5",
                                    onclick: move |_| demo_mode::toggle_demo_mode_and_reload(),
                                    "Exit"
                                }
                            }
                        }
                    } else {
                        button {
                            class: "w-full btn-ghost py-2 rounded-lg text-label-caps font-label-caps uppercase text-[10px] border border-dashed border-outline-variant/50",
                            onclick: move |_| demo_mode::toggle_demo_mode_and_reload(),
                            Icon { name: "auto_awesome", filled: false }
                            "Load demo data"
                        }
                    }
                    button {
                        class: "w-full btn-primary btn-lg active:scale-95 transition-transform",
                        disabled: creating(),
                        onclick: create_lobby,
                        Icon { name: "rocket_launch", filled: true }
                        if creating() { "Creating…" } else { "Launch Game" }
                    }
                }
            }

            if lobby_room {
                main { class: "lobby-room-main",
                    {children}
                }
            } else {
                main { class: "md:ml-64 min-h-[calc(100vh-4rem)] p-4 sm:p-6 lg:p-10 pb-24 md:pb-10",
                    div { class: "max-w-container-max mx-auto",
                        {children}
                    }
                }
            }

            nav { class: "bottom-nav",
                BottomNavItem { label: "Discover", icon: "explore", active: active == NavTab::Discover, onclick: move |_| { nav.push(LobbyRoute::Home {}); } }
                BottomNavItem { label: "Lobbies", icon: "groups", active: active == NavTab::Lobbies, onclick: move |_| { nav.push(LobbyRoute::LobbiesBrowser {}); } }
                BottomNavItem { label: "Developer", icon: "terminal", active: active == NavTab::Developer, onclick: move |_| { nav.push(LobbyRoute::DeveloperUploads {}); } }
                BottomNavItem { label: "Profile", icon: "account_circle", active: active == NavTab::Profile, onclick: move |_| { nav.push(LobbyRoute::Profile {}); } }
            }

            if show_fab {
                button {
                    class: "fab",
                    aria_label: "Launch Game",
                    disabled: creating(),
                    onclick: create_lobby,
                    Icon { name: "add", filled: true }
                }
            }
        }
    }
}

#[component]
fn SidebarLink(label: &'static str, icon: &'static str, active: bool, onclick: EventHandler<()>) -> Element {
    let class = if active { "sidebar-link sidebar-link-active" } else { "sidebar-link" };
    rsx! {
        button {
            class: "{class}",
            onclick: move |_| onclick.call(()),
            Icon { name: icon, filled: active }
            span { "{label}" }
        }
    }
}

#[component]
fn BottomNavItem(label: &'static str, icon: &'static str, active: bool, onclick: EventHandler<()>) -> Element {
    let class = if active { "bottom-nav-item bottom-nav-item-active" } else { "bottom-nav-item" };
    rsx! {
        button {
            class: "{class}",
            onclick: move |_| onclick.call(()),
            Icon { name: icon, filled: active }
            span { "{label}" }
        }
    }
}
