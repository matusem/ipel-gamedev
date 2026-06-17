use crate::api::{graphql_exec, graphql_post, FRIENDS_PAGE_QUERY};
use crate::components::ui::*;
use crate::models::{format_relative_time, FriendGql, FriendRequestGql, LobbySummary, UserSearchResultGql};
use crate::LobbyRoute;
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::json;

#[component]
pub fn FriendsPage(initial_tab: Option<String>) -> Element {
    let nav = use_navigator();
    let toast = use_toast();
    let mut tab = use_signal(move || match initial_tab.as_deref() {
        Some("requests") => 1usize,
        Some("find") => 2,
        _ => 0,
    });
    let mut friends = use_signal(Vec::<FriendGql>::new);
    let mut incoming = use_signal(Vec::<FriendRequestGql>::new);
    let mut outgoing = use_signal(Vec::<FriendRequestGql>::new);
    let mut pending_count = use_signal(|| 0i32);
    let mut search_query = use_signal(String::new);
    let mut search_results = use_signal(Vec::<UserSearchResultGql>::new);
    let mut searching = use_signal(|| false);
    let mut my_lobbies = use_signal(Vec::<LobbySummary>::new);
    let mut loading = use_signal(|| true);

    let reload = move || {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct F {
                my_friends: Vec<FriendGql>,
                pending_friend_requests: Vec<FriendRequestGql>,
                sent_friend_requests: Vec<FriendRequestGql>,
                pending_friend_request_count: i32,
                lobbies: Vec<LobbySummary>,
            }
            let q = FRIENDS_PAGE_QUERY;
            if let Ok(data) = graphql_post::<F>(q).await {
                friends.set(data.my_friends);
                incoming.set(data.pending_friend_requests);
                outgoing.set(data.sent_friend_requests);
                pending_count.set(data.pending_friend_request_count);
                my_lobbies.set(data.lobbies);
            }
            loading.set(false);
        });
    };

    use_hook(move || {
        reload();
    });

    let run_search = move |_| {
        let q = search_query().trim().to_string();
        if q.len() < 2 {
            search_results.set(Vec::new());
            return;
        }
        searching.set(true);
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct S {
                search_users: Vec<UserSearchResultGql>,
            }
            let query = r#"query S($q: String!) { searchUsers(query: $q, limit: 20) { id displayName avatarUrl friendshipStatus } }"#;
            let vars = json!({ "q": q });
            if let Ok(s) = graphql_exec::<S>(query, Some(vars)).await {
                search_results.set(s.search_users);
            }
            searching.set(false);
        });
    };

    let invite_lobby_id = my_lobbies()
        .into_iter()
        .find(|l| l.status == "waiting" || l.status == "configuring")
        .map(|l| l.id);

    rsx! {
        div { class: "page-stack max-w-3xl",
            PageHeader {
                title: "Friends".to_string(),
                subtitle: Some("Manage friends, requests, and invites.".to_string()),
                badge: None,
                children: None,
            }

            div {
                role: "tablist",
                class: "flex gap-1 border-b border-outline-variant/40 mb-6 overflow-x-auto",
                button {
                    role: "tab",
                    class: if tab() == 0 { "tab-btn tab-btn-active" } else { "tab-btn" },
                    onclick: move |_| tab.set(0),
                    "Friends"
                }
                button {
                    role: "tab",
                    class: if tab() == 1 { "tab-btn tab-btn-active" } else { "tab-btn" },
                    onclick: move |_| tab.set(1),
                    if pending_count() > 0 {
                        "Requests ({pending_count()})"
                    } else {
                        "Requests"
                    }
                }
                button {
                    role: "tab",
                    class: if tab() == 2 { "tab-btn tab-btn-active" } else { "tab-btn" },
                    onclick: move |_| tab.set(2),
                    "Find"
                }
            }

            if loading() {
                SkeletonTableRows { count: 4 }
            } else {
                match tab() {
                    0 => rsx! {
                        div { class: "section-card space-y-3",
                            if friends().is_empty() {
                                EmptyState {
                                    icon: "group",
                                    title: "No friends yet".to_string(),
                                    description: "Search for players on the Find tab or accept incoming requests.".to_string(),
                                    cta_label: None,
                                    on_cta: None,
                                }
                            }
                            for f in friends() {
                                {
                                    let invite_lid = invite_lobby_id.clone();
                                    rsx! {
                                        div {
                                            key: "{f.user_id}",
                                            class: "flex items-center gap-3 py-2 border-b border-outline-variant/20 last:border-0",
                                            Avatar {
                                                seed: f.user_id.clone(),
                                                size: AvatarSize::Md,
                                                image_url: f.avatar_url.clone(),
                                            }
                                            div { class: "flex-1 min-w-0",
                                                div { class: "flex items-center gap-2",
                                                    p { class: "font-medium text-on-surface truncate", "{f.display_name}" }
                                                    if f.online {
                                                        span { class: "status-dot-online shrink-0", title: "Online" }
                                                    }
                                                }
                                                p { class: "text-body-sm text-outline",
                                                    "Friends since {format_relative_time(f.since)}"
                                                }
                                            }
                                            div { class: "flex gap-2 shrink-0",
                                                if let Some(lid) = invite_lid.clone() {
                                                    button {
                                                        class: "btn-ghost btn-sm",
                                                        onclick: {
                                                            let fid = f.user_id.clone();
                                                            let toast = toast;
                                                            move |_| {
                                                                let fid = fid.clone();
                                                                let lid = lid.clone();
                                                                spawn(async move {
                                                                    let q = "mutation I($f: ID!, $l: ID!) { inviteFriendToLobby(friendUserId: $f, lobbyId: $l) }";
                                                                    let vars = json!({ "f": fid, "l": lid });
                                                                    match graphql_exec::<serde_json::Value>(q, Some(vars)).await {
                                                                        Ok(_) => push_toast(toast.show, "Invite sent", ToastKind::Success),
                                                                        Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                                    }
                                                                });
                                                            }
                                                        },
                                                        "Invite"
                                                    }
                                                }
                                                button {
                                                    class: "btn-ghost btn-sm text-error",
                                                    onclick: {
                                                        let fid = f.user_id.clone();
                                                        let toast = toast;
                                                        move |_| {
                                                            let fid = fid.clone();
                                                            spawn(async move {
                                                                let q = "mutation R($id: ID!) { removeFriend(userId: $id) }";
                                                                let vars = json!({ "id": fid });
                                                                match graphql_exec::<serde_json::Value>(q, Some(vars)).await {
                                                                    Ok(_) => {
                                                                        push_toast(toast.show, "Friend removed", ToastKind::Info);
                                                                        reload();
                                                                    }
                                                                    Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                                }
                                                            });
                                                        }
                                                    },
                                                    "Remove"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    1 => rsx! {
                        div { class: "space-y-6",
                            section { class: "section-card space-y-3",
                                h2 { class: "font-manrope text-h2 text-lg", "Incoming requests" }
                                if incoming().is_empty() {
                                    p { class: "text-body-sm text-outline", "No incoming friend requests." }
                                }
                                for r in incoming() {
                                    div {
                                        key: "in-{r.user_id}",
                                        class: "flex items-center gap-3 py-2 border-b border-outline-variant/20 last:border-0",
                                        Avatar {
                                            seed: r.user_id.clone(),
                                            size: AvatarSize::Md,
                                            image_url: r.avatar_url.clone(),
                                        }
                                        div { class: "flex-1 min-w-0",
                                            p { class: "font-medium text-on-surface", "{r.display_name}" }
                                            p { class: "text-body-sm text-outline", "{format_relative_time(r.created_at)}" }
                                        }
                                        div { class: "flex gap-2",
                                            button {
                                                class: "btn-primary btn-sm",
                                                onclick: {
                                                    let uid = r.user_id.clone();
                                                    let toast = toast;
                                                    move |_| {
                                                        let uid = uid.clone();
                                                        spawn(async move {
                                                            let q = "mutation A($id: ID!) { acceptFriendRequest(userId: $id) }";
                                                            let vars = json!({ "id": uid });
                                                            match graphql_exec::<serde_json::Value>(q, Some(vars)).await {
                                                                Ok(_) => {
                                                                    push_toast(toast.show, "Friend request accepted", ToastKind::Success);
                                                                    reload();
                                                                }
                                                                Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                            }
                                                        });
                                                    }
                                                },
                                                "Accept"
                                            }
                                            button {
                                                class: "btn-ghost btn-sm",
                                                onclick: {
                                                    let uid = r.user_id.clone();
                                                    let toast = toast;
                                                    move |_| {
                                                        let uid = uid.clone();
                                                        spawn(async move {
                                                            let q = "mutation D($id: ID!) { declineFriendRequest(userId: $id) }";
                                                            let vars = json!({ "id": uid });
                                                            match graphql_exec::<serde_json::Value>(q, Some(vars)).await {
                                                                Ok(_) => {
                                                                    push_toast(toast.show, "Request declined", ToastKind::Info);
                                                                    reload();
                                                                }
                                                                Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                            }
                                                        });
                                                    }
                                                },
                                                "Decline"
                                            }
                                        }
                                    }
                                }
                            }
                            section { class: "section-card space-y-3",
                                h2 { class: "font-manrope text-h2 text-lg", "Sent requests" }
                                if outgoing().is_empty() {
                                    p { class: "text-body-sm text-outline", "No outgoing requests." }
                                }
                                for r in outgoing() {
                                    div {
                                        key: "out-{r.user_id}",
                                        class: "flex items-center gap-3 py-2 border-b border-outline-variant/20 last:border-0",
                                        Avatar {
                                            seed: r.user_id.clone(),
                                            size: AvatarSize::Md,
                                            image_url: r.avatar_url.clone(),
                                        }
                                        div { class: "flex-1 min-w-0",
                                            p { class: "font-medium text-on-surface", "{r.display_name}" }
                                            p { class: "text-body-sm text-outline",
                                                "Pending · {format_relative_time(r.created_at)}"
                                            }
                                        }
                                        button {
                                            class: "btn-ghost btn-sm",
                                            onclick: {
                                                let uid = r.user_id.clone();
                                                let toast = toast;
                                                move |_| {
                                                    let uid = uid.clone();
                                                    spawn(async move {
                                                        let q = "mutation C($id: ID!) { cancelFriendRequest(userId: $id) }";
                                                        let vars = json!({ "id": uid });
                                                        match graphql_exec::<serde_json::Value>(q, Some(vars)).await {
                                                            Ok(_) => {
                                                                push_toast(toast.show, "Request cancelled", ToastKind::Info);
                                                                reload();
                                                            }
                                                            Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                        }
                                                    });
                                                }
                                            },
                                            "Cancel"
                                        }
                                    }
                                }
                            }
                        }
                    },
                    _ => rsx! {
                        div { class: "section-card space-y-4",
                            div { class: "flex gap-2",
                                SearchInput {
                                    placeholder: "Search by display name…",
                                    value: search_query(),
                                    width_class: "flex-1",
                                    oninput: move |v| search_query.set(v),
                                }
                                button {
                                    class: "btn-primary shrink-0",
                                    disabled: searching(),
                                    onclick: run_search,
                                    if searching() { "Searching…" } else { "Search" }
                                }
                            }
                            if search_results().is_empty() && !searching() {
                                p { class: "text-body-sm text-outline", "Enter at least 2 characters to search." }
                            }
                            for u in search_results() {
                                div {
                                    key: "{u.id}",
                                    class: "flex items-center gap-3 py-2 border-b border-outline-variant/20 last:border-0",
                                    Avatar {
                                        seed: u.id.clone(),
                                        size: AvatarSize::Md,
                                        image_url: u.avatar_url.clone(),
                                    }
                                    div { class: "flex-1 min-w-0",
                                        p { class: "font-medium text-on-surface", "{u.display_name}" }
                                        if let Some(ref st) = u.friendship_status {
                                            p { class: "text-body-sm text-outline capitalize", "{st}" }
                                        }
                                    }
                                    match u.friendship_status.as_deref() {
                                        Some("accepted") => rsx! {
                                            span { class: "text-body-sm text-outline", "Friends" }
                                        },
                                        Some("pending") => rsx! {
                                            span { class: "text-body-sm text-outline", "Pending" }
                                        },
                                        Some("blocked") => rsx! {
                                            span { class: "text-body-sm text-error", "Blocked" }
                                        },
                                        _ => rsx! {
                                            button {
                                                class: "btn-primary btn-sm",
                                                onclick: {
                                                    let uid = u.id.clone();
                                                    let toast = toast;
                                                    move |_| {
                                                        let uid = uid.clone();
                                                        spawn(async move {
                                                            let q = "mutation S($id: ID!) { sendFriendRequest(userId: $id) }";
                                                            let vars = json!({ "id": uid });
                                                            match graphql_exec::<serde_json::Value>(q, Some(vars)).await {
                                                                Ok(_) => {
                                                                    push_toast(toast.show, "Friend request sent", ToastKind::Success);
                                                                    reload();
                                                                }
                                                                Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                            }
                                                        });
                                                    }
                                                },
                                                "Add friend"
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    },
                }
            }

            button {
                class: "btn-ghost text-sm",
                onclick: move |_| { nav.push(LobbyRoute::Profile {}); },
                "Back to profile"
            }
        }
    }
}
