use crate::api::{graphql_exec, graphql_post};
use crate::components::ui::{push_toast, ErrorBanner, LoadingState, ToastKind, use_toast};
use crate::models::{
    format_relative_time, AdminCommentRow, AdminDraftRow, AdminPlatformOverview, AdminReviewRow,
    AdminUserRow, LobbySummary,
};
use crate::LobbyRoute;
use dioxus::prelude::*;
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq)]
enum AdminTab {
    Overview,
    Users,
    Drafts,
    Lobbies,
    Moderation,
}

#[component]
pub fn AdminPage() -> Element {
    let nav = use_navigator();
    let mut is_superadmin = use_signal(|| None::<bool>);
    let mut tab = use_signal(|| AdminTab::Overview);
    let mut err = use_signal(|| None::<String>);
    let mut overview = use_signal(|| None::<AdminPlatformOverview>);
    let mut users = use_signal(Vec::<AdminUserRow>::new);
    let mut user_search = use_signal(String::new);
    let mut drafts = use_signal(Vec::<AdminDraftRow>::new);
    let mut lobbies = use_signal(Vec::<LobbySummary>::new);
    let mut reviews = use_signal(Vec::<AdminReviewRow>::new);
    let mut comments = use_signal(Vec::<AdminCommentRow>::new);
    let toast = use_toast();

    let fetch_overview = move || {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct W {
                admin_platform_overview: AdminPlatformOverview,
            }
            if let Ok(w) = graphql_post::<W>(
                "query { adminPlatformOverview { userCount draftCount activeLobbies publishedGames reviewCount commentCount } }",
            )
            .await
            {
                overview.set(Some(w.admin_platform_overview));
            }
        });
    };

    let fetch_users = move |search: String| {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct W {
                admin_users: Vec<AdminUserRow>,
            }
            let q = r#"query($s: String) { adminUsers(limit: 100, search: $s) { id displayName createdAt roles hasPassword } }"#;
            let vars = serde_json::json!({ "s": if search.trim().is_empty() { None::<String> } else { Some(search.trim().to_string()) } });
            match graphql_exec::<W>(q, Some(vars)).await {
                Ok(w) => users.set(w.admin_users),
                Err(e) => err.set(Some(e)),
            }
        });
    };

    let fetch_drafts = move || {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct W {
                admin_game_drafts: Vec<AdminDraftRow>,
            }
            if let Ok(w) = graphql_post::<W>(
                "query { adminGameDrafts(limit: 100) { id ownerUserId gameName displayName version status createdAt publishedAt } }",
            )
            .await
            {
                drafts.set(w.admin_game_drafts);
            }
        });
    };

    let fetch_lobbies = move || {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct W {
                admin_lobbies: Vec<LobbySummary>,
            }
            if let Ok(w) = graphql_post::<W>(
                "query { adminLobbies { id ownerDisplayName gameType status seatsFilled seatsTotal createdAt } }",
            )
            .await
            {
                lobbies.set(w.admin_lobbies);
            }
        });
    };

    let fetch_moderation = move || {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct R {
                admin_reviews: Vec<AdminReviewRow>,
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct C {
                admin_comments: Vec<AdminCommentRow>,
            }
            if let Ok(r) = graphql_post::<R>(
                "query { adminReviews(limit: 50) { id gameName displayName body helpfulVotes createdAt } }",
            )
            .await
            {
                reviews.set(r.admin_reviews);
            }
            if let Ok(c) = graphql_post::<C>(
                "query { adminComments(limit: 50) { id gameName displayName body createdAt } }",
            )
            .await
            {
                comments.set(c.admin_comments);
            }
        });
    };

    use_hook(move || {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct P {
                is_superadmin: bool,
            }
            match graphql_post::<P>("query { isSuperadmin }").await {
                Ok(p) => is_superadmin.set(Some(p.is_superadmin)),
                Err(e) => {
                    is_superadmin.set(Some(false));
                    err.set(Some(e));
                }
            }
        });
    });

    use_effect(move || {
        if is_superadmin() != Some(true) {
            return;
        }
        fetch_overview();
        fetch_users(String::new());
        fetch_drafts();
        fetch_lobbies();
        fetch_moderation();
    });

    let tab_class = |active: bool| {
        if active {
            "px-3 py-1.5 rounded-lg text-body-sm font-semibold bg-primary-container text-on-primary-container"
        } else {
            "px-3 py-1.5 rounded-lg text-body-sm text-on-surface-variant hover:bg-surface-container-low"
        }
    };

    rsx! {
        div { class: "page-stack",
            div { class: "lobby-command-bar",
                div { class: "lobby-command-center",
                    p { class: "lobby-command-kicker", "Platform" }
                    h1 { class: "lobby-command-title", "Admin Console" }
                    p { class: "text-body-sm text-on-surface-variant mt-1",
                        "Manage users, games, lobbies, and community content."
                    }
                }
            }

            if let Some(e) = err() {
                ErrorBanner { message: e }
            }

            if is_superadmin() == Some(false) {
                div { class: "section-card",
                    p { class: "text-on-surface font-medium", "Superadmin access required" }
                    p { class: "mt-2 text-body-sm text-on-surface-variant",
                        "Add your user id to SUPERADMIN_USER_IDS in the server environment, or ask an existing superadmin to grant the role."
                    }
                }
            } else if is_superadmin().is_none() {
                LoadingState {
                    title: "Checking permissions…".to_string(),
                    subtitle: "Verifying admin access".to_string(),
                }
            } else {
                div { class: "flex flex-wrap gap-2",
                    button {
                        class: tab_class(tab() == AdminTab::Overview),
                        onclick: move |_| tab.set(AdminTab::Overview),
                        "Overview"
                    }
                    button {
                        class: tab_class(tab() == AdminTab::Users),
                        onclick: move |_| tab.set(AdminTab::Users),
                        "Users"
                    }
                    button {
                        class: tab_class(tab() == AdminTab::Drafts),
                        onclick: move |_| tab.set(AdminTab::Drafts),
                        "Drafts"
                    }
                    button {
                        class: tab_class(tab() == AdminTab::Lobbies),
                        onclick: move |_| tab.set(AdminTab::Lobbies),
                        "Lobbies"
                    }
                    button {
                        class: tab_class(tab() == AdminTab::Moderation),
                        onclick: move |_| tab.set(AdminTab::Moderation),
                        "Moderation"
                    }
                }

                if tab() == AdminTab::Overview {
                    if let Some(o) = overview() {
                        div { class: "grid grid-cols-2 sm:grid-cols-3 gap-4",
                            div { class: "kpi-card",
                                p { class: "text-label-caps font-label-caps text-outline uppercase", "Users" }
                                p { class: "font-manrope text-2xl font-bold text-on-surface mt-1", "{o.user_count}" }
                            }
                            div { class: "kpi-card",
                                p { class: "text-label-caps font-label-caps text-outline uppercase", "Drafts" }
                                p { class: "font-manrope text-2xl font-bold text-on-surface mt-1", "{o.draft_count}" }
                            }
                            div { class: "kpi-card",
                                p { class: "text-label-caps font-label-caps text-outline uppercase", "Active lobbies" }
                                p { class: "font-manrope text-2xl font-bold text-on-surface mt-1", "{o.active_lobbies}" }
                            }
                            div { class: "kpi-card",
                                p { class: "text-label-caps font-label-caps text-outline uppercase", "Published games" }
                                p { class: "font-manrope text-2xl font-bold text-on-surface mt-1", "{o.published_games}" }
                            }
                            div { class: "kpi-card",
                                p { class: "text-label-caps font-label-caps text-outline uppercase", "Reviews" }
                                p { class: "font-manrope text-2xl font-bold text-on-surface mt-1", "{o.review_count}" }
                            }
                            div { class: "kpi-card",
                                p { class: "text-label-caps font-label-caps text-outline uppercase", "Comments" }
                                p { class: "font-manrope text-2xl font-bold text-on-surface mt-1", "{o.comment_count}" }
                            }
                        }
                    }
                }

                if tab() == AdminTab::Users {
                    div { class: "section-card space-y-4",
                        div { class: "flex flex-wrap gap-2",
                            input {
                                class: "input-field flex-1 min-w-[12rem]",
                                placeholder: "Search by name or id…",
                                value: "{user_search}",
                                oninput: move |e| user_search.set(e.value()),
                            }
                            button {
                                class: "btn-secondary",
                                onclick: move |_| fetch_users(user_search()),
                                "Search"
                            }
                        }
                        for u in users() {
                            div { class: "rounded-xl border border-outline-variant/30 p-4 space-y-2",
                                p { class: "font-medium text-on-surface", "{u.display_name}" }
                                p { class: "text-xs text-outline font-mono-code break-all", "{u.id}" }
                                p { class: "text-xs text-on-surface-variant",
                                    "Joined {format_relative_time(u.created_at)}"
                                    if u.has_password { " · password set" }
                                }
                                p { class: "text-body-sm text-on-surface-variant",
                                    "Roles: "
                                    if u.roles.is_empty() { "—" } else { "{u.roles.join(\", \")}" }
                                }
                                div { class: "flex flex-wrap gap-1.5",
                                    if !u.roles.iter().any(|r| r == "developer") {
                                        button {
                                            class: "btn-ghost btn-sm",
                                            onclick: {
                                                let id = u.id.clone();
                                                let toast = toast;
                                                move |_| {
                                                    let id = id.clone();
                                                    spawn(async move {
                                                        let q = format!(r#"mutation {{ adminGrantRole(userId: "{id}", role: "developer") }}"#);
                                                        match graphql_post::<serde_json::Value>(&q).await {
                                                            Ok(_) => {
                                                                push_toast(toast.show, "Developer role granted", ToastKind::Success);
                                                            }
                                                            Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                        }
                                                    });
                                                }
                                            },
                                            "+ Dev"
                                        }
                                    } else {
                                        button {
                                            class: "btn-ghost btn-sm text-error",
                                            onclick: {
                                                let id = u.id.clone();
                                                let toast = toast;
                                                move |_| {
                                                    let id = id.clone();
                                                    spawn(async move {
                                                        let q = format!(r#"mutation {{ adminRevokeRole(userId: "{id}", role: "developer") }}"#);
                                                        match graphql_post::<serde_json::Value>(&q).await {
                                                            Ok(_) => push_toast(toast.show, "Developer revoked", ToastKind::Success),
                                                            Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                        }
                                                    });
                                                }
                                            },
                                            "− Dev"
                                        }
                                    }
                                    if !u.roles.iter().any(|r| r == "superadmin") {
                                        button {
                                            class: "btn-ghost btn-sm",
                                            onclick: {
                                                let id = u.id.clone();
                                                let toast = toast;
                                                move |_| {
                                                    let id = id.clone();
                                                    spawn(async move {
                                                        let q = format!(r#"mutation {{ adminGrantRole(userId: "{id}", role: "superadmin") }}"#);
                                                        match graphql_post::<serde_json::Value>(&q).await {
                                                            Ok(_) => push_toast(toast.show, "Superadmin granted", ToastKind::Success),
                                                            Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                        }
                                                    });
                                                }
                                            },
                                            "+ Admin"
                                        }
                                    }
                                    button {
                                        class: "btn-ghost btn-sm",
                                        onclick: {
                                            let id = u.id.clone();
                                            let toast = toast;
                                            move |_| {
                                                let id = id.clone();
                                                spawn(async move {
                                                    let q = format!(r#"mutation {{ adminRevokeUserSessions(userId: "{id}") }}"#);
                                                    match graphql_post::<serde_json::Value>(&q).await {
                                                        Ok(_) => push_toast(toast.show, "Sessions revoked", ToastKind::Success),
                                                        Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                    }
                                                });
                                            }
                                        },
                                        "Revoke sessions"
                                    }
                                }
                            }
                        }
                    }
                }

                if tab() == AdminTab::Drafts {
                    div { class: "space-y-3",
                        for d in drafts() {
                            div { class: "section-card",
                                p { class: "font-medium", "{d.display_name}" }
                                p { class: "text-body-sm text-on-surface-variant", "{d.game_name} v{d.version} · {d.status}" }
                                p { class: "text-xs font-mono-code text-outline break-all", "Owner {d.owner_user_id}" }
                                div { class: "flex flex-wrap gap-1.5 mt-3",
                                    button {
                                        class: "btn-ghost btn-sm",
                                        onclick: {
                                            let name = d.game_name.clone();
                                            move |_| { nav.push(LobbyRoute::GameDetail { name: name.clone() }); }
                                        },
                                        "Store page"
                                    }
                                    if d.status == "ready" {
                                        button {
                                            class: "btn-primary btn-sm",
                                            onclick: {
                                                let id = d.id.clone();
                                                let toast = toast;
                                                move |_| {
                                                    let id = id.clone();
                                                    spawn(async move {
                                                        let q = format!(r#"mutation {{ adminPublishGameDraft(draftId: "{id}") {{ id }} }}"#);
                                                        match graphql_post::<serde_json::Value>(&q).await {
                                                            Ok(_) => push_toast(toast.show, "Published", ToastKind::Success),
                                                            Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                        }
                                                    });
                                                }
                                            },
                                            "Publish"
                                        }
                                    }
                                    if d.status == "published" {
                                        button {
                                            class: "btn-secondary btn-sm",
                                            onclick: {
                                                let id = d.id.clone();
                                                let toast = toast;
                                                move |_| {
                                                    let id = id.clone();
                                                    spawn(async move {
                                                        let q = format!(r#"mutation {{ adminUnpublishGameDraft(draftId: "{id}") {{ id }} }}"#);
                                                        match graphql_post::<serde_json::Value>(&q).await {
                                                            Ok(_) => push_toast(toast.show, "Unpublished", ToastKind::Success),
                                                            Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                        }
                                                    });
                                                }
                                            },
                                            "Take down"
                                        }
                                    }
                                    if d.status != "published" && d.status != "discarded" {
                                        button {
                                            class: "btn-ghost btn-sm text-error",
                                            onclick: {
                                                let id = d.id.clone();
                                                let toast = toast;
                                                move |_| {
                                                    let id = id.clone();
                                                    spawn(async move {
                                                        let q = format!(r#"mutation {{ adminDiscardGameDraft(draftId: "{id}") }}"#);
                                                        match graphql_post::<serde_json::Value>(&q).await {
                                                            Ok(_) => push_toast(toast.show, "Discarded", ToastKind::Success),
                                                            Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                        }
                                                    });
                                                }
                                            },
                                            "Discard"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if tab() == AdminTab::Lobbies {
                    div { class: "space-y-3",
                        for l in lobbies() {
                            div { class: "section-card",
                                p { class: "font-medium", "{l.owner_display_name}" }
                                p { class: "text-body-sm text-on-surface-variant",
                                    if l.game_type.is_empty() { "No game selected" } else { "{l.game_type}" }
                                    " · {l.status} · {l.seats_filled}/{l.seats_total} seats"
                                }
                                p { class: "text-xs font-mono-code text-outline break-all", "{l.id}" }
                                div { class: "flex flex-wrap gap-1.5 mt-3",
                                    button {
                                        class: "btn-ghost btn-sm",
                                        onclick: {
                                            let id = l.id.clone();
                                            move |_| { nav.push(LobbyRoute::Lobby { id: id.clone() }); }
                                        },
                                        "Open"
                                    }
                                    if l.status != "cancelled" && l.status != "in_game" {
                                        button {
                                            class: "btn-ghost btn-sm text-error",
                                            onclick: {
                                                let id = l.id.clone();
                                                let toast = toast;
                                                move |_| {
                                                    let id = id.clone();
                                                    spawn(async move {
                                                        let q = format!(r#"mutation {{ adminCancelLobby(lobbyId: "{id}") }}"#);
                                                        match graphql_post::<serde_json::Value>(&q).await {
                                                            Ok(_) => push_toast(toast.show, "Lobby cancelled", ToastKind::Success),
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
                    }
                }

                if tab() == AdminTab::Moderation {
                    div { class: "space-y-6",
                        div { class: "section-card space-y-3",
                            h2 { class: "card-title", "Reviews" }
                            if reviews().is_empty() {
                                p { class: "text-body-sm text-outline", "No reviews." }
                            }
                            for r in reviews() {
                                div { class: "comment-row",
                                    div { class: "flex-1 min-w-0",
                                        p { class: "text-body-sm font-medium", "{r.display_name} · {r.game_name}" }
                                        p { class: "text-body-sm text-on-surface-variant mt-1", "{r.body}" }
                                        p { class: "text-xs text-outline mt-1", "{format_relative_time(r.created_at)}" }
                                    }
                                    button {
                                        class: "btn-ghost btn-sm text-error shrink-0",
                                        onclick: {
                                            let id = r.id.clone();
                                            let toast = toast;
                                            move |_| {
                                                let id = id.clone();
                                                spawn(async move {
                                                    let q = format!(r#"mutation {{ adminDeleteReview(reviewId: "{id}") }}"#);
                                                    match graphql_post::<serde_json::Value>(&q).await {
                                                        Ok(_) => push_toast(toast.show, "Review deleted", ToastKind::Success),
                                                        Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                    }
                                                });
                                            }
                                        },
                                        "Delete"
                                    }
                                }
                            }
                        }
                        div { class: "section-card space-y-3",
                            h2 { class: "card-title", "Comments" }
                            if comments().is_empty() {
                                p { class: "text-body-sm text-outline", "No comments." }
                            }
                            for c in comments() {
                                div { class: "comment-row",
                                    div { class: "flex-1 min-w-0",
                                        p { class: "text-body-sm font-medium", "{c.display_name} · {c.game_name}" }
                                        p { class: "text-body-sm text-on-surface-variant mt-1", "{c.body}" }
                                        p { class: "text-xs text-outline mt-1", "{format_relative_time(c.created_at)}" }
                                    }
                                    button {
                                        class: "btn-ghost btn-sm text-error shrink-0",
                                        onclick: {
                                            let id = c.id.clone();
                                            let toast = toast;
                                            move |_| {
                                                let id = id.clone();
                                                spawn(async move {
                                                    let q = format!(r#"mutation {{ adminDeleteComment(commentId: "{id}") }}"#);
                                                    match graphql_post::<serde_json::Value>(&q).await {
                                                        Ok(_) => push_toast(toast.show, "Comment deleted", ToastKind::Success),
                                                        Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                    }
                                                });
                                            }
                                        },
                                        "Delete"
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
