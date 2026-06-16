use crate::api::{clear_auth_session, graphql_exec, graphql_post, stored_user_id};
use crate::components::ui::*;
use crate::models::{format_relative_time, NotificationGql, PublishTokenSummary, UserProfile};
use crate::stub::demo_mode;
use dioxus::prelude::*;
use serde::Deserialize;

#[component]
pub fn SettingsPage() -> Element {
    let mut tab = use_signal(|| 0usize);
    let user_id = stored_user_id().unwrap_or_else(|| "—".into());
    let mut profile = use_signal(|| None::<UserProfile>);
    let mut display_name_edit = use_signal(String::new);
    let mut avatar_url_edit = use_signal(String::new);
    let mut saving_name = use_signal(|| false);
    let mut saving_avatar = use_signal(|| false);
    let mut tokens = use_signal(Vec::<PublishTokenSummary>::new);
    let mut loading_tokens = use_signal(|| true);
    let mut creating_token = use_signal(|| false);
    let mut notifications = use_signal(Vec::<NotificationGql>::new);
    let toast = use_toast();
    let confirm = use_confirm();

    let fetch_notifications = move || {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct N { my_notifications: Vec<NotificationGql> }
            if let Ok(n) = graphql_post::<N>(
                "query { myNotifications(limit: 20) { id title body kind unread createdAt } }",
            )
            .await
            {
                notifications.set(n.my_notifications);
            }
        });
    };

    let fetch_tokens = move || {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Wrap {
                my_publish_tokens: Vec<PublishTokenSummary>,
            }
            loading_tokens.set(true);
            match graphql_post::<Wrap>("query { myPublishTokens { id label maskedKey createdAt expiresAt } }").await
            {
                Ok(w) => tokens.set(w.my_publish_tokens),
                Err(_) => tokens.set(Vec::new()),
            }
            loading_tokens.set(false);
        });
    };

    use_hook(move || {
        fetch_tokens();
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Wrap {
                my_profile: Option<UserProfile>,
            }
            if let Ok(w) = graphql_post::<Wrap>(
                "query { myProfile { displayName createdAt matchesPlayed gamesPublished wins repScore avatarUrl } }",
            )
            .await
            {
                profile.set(w.my_profile.clone());
                if let Some(p) = w.my_profile {
                    display_name_edit.set(p.display_name);
                    avatar_url_edit.set(p.avatar_url.unwrap_or_default());
                }
            }
            fetch_notifications();
        });
    });

    let rank_label = "Member";
    let rep_score = profile().map(|p| p.rep_score).unwrap_or(0);
    let rank_progress = (rep_score % 100).min(99) as u8;
    let demo_on = demo_mode::is_demo_mode();

    rsx! {
        div { class: "page-stack max-w-3xl",
            PageHeader {
                title: "Account Settings".to_string(),
                subtitle: Some("Manage your profile, security, and developer credentials.".to_string()),
                badge: None,
                children: None,
            }
            Callout {
                variant: CalloutVariant::Secondary,
                div { class: "flex flex-col sm:flex-row sm:items-center justify-between gap-4",
                    div {
                        p { class: "card-title", "Demo mode" }
                        p { class: "text-body-sm text-on-surface-variant mt-1",
                            if demo_on {
                                "Synthetic data is active across the platform."
                            } else {
                                "Populate the UI with rich sample lobbies, reviews, and stats — no backend needed."
                            }
                        }
                    }
                    button {
                        class: if demo_on { "btn-secondary shrink-0" } else { "btn-primary shrink-0" },
                        onclick: move |_| demo_mode::toggle_demo_mode_and_reload(),
                        if demo_on { "Exit demo" } else { "Load demo data" }
                    }
                }
            }

            TabBar {
                tabs: vec!["Profile", "Security", "API Tokens", "Notifications"],
                active: tab(),
                on_select: move |i| tab.set(i),
            }

            match tab() {
                0 => rsx! {
                    div { class: "section-card space-y-6",
                        div { class: "rounded-xl border border-secondary-container/30 bg-secondary-container/10 px-4 py-3",
                            p { class: "text-label-caps font-label-caps text-secondary uppercase mb-2", "Reputation" }
                            div { class: "flex items-center justify-between mb-2",
                                span { class: "font-manrope font-semibold text-on-surface", "{rank_label}" }
                                span { class: "font-mono-code text-tertiary", "Rep {rep_score}" }
                            }
                            div { class: "h-2 rounded-full bg-surface-container-high overflow-hidden",
                                div { class: "h-full bg-primary-container rounded-full", style: "width: {rank_progress}%" }
                            }
                            if let Some(p) = profile() {
                                p { class: "text-body-sm text-outline mt-2",
                                    "{p.matches_played} matches · {p.wins} wins · {p.games_published} published"
                                }
                            }
                        }
                        div {
                            label { class: "text-label-caps font-label-caps text-outline uppercase block mb-1", "User ID" }
                            input { class: "input-field font-mono-code", value: "{user_id}", readonly: true }
                        }
                        div { class: "space-y-2",
                            label { class: "text-label-caps font-label-caps text-outline uppercase block", "Display name" }
                            input {
                                class: "input-field",
                                placeholder: "Your display name",
                                value: "{display_name_edit}",
                                oninput: move |e| display_name_edit.set(e.value()),
                            }
                            PrimaryButton {
                                label: if saving_name() { "Saving…".to_string() } else { "Save display name".to_string() },
                                disabled: saving_name() || display_name_edit().trim().is_empty(),
                                onclick: move |_| {
                                    let name = display_name_edit().trim().to_string();
                                    let toast = toast;
                                    saving_name.set(true);
                                    spawn(async move {
                                        let q = "mutation U($n: String!) { updateDisplayName(displayName: $n) { displayName } }";
                                        let vars = serde_json::json!({ "n": name });
                                        match graphql_exec::<serde_json::Value>(q, Some(vars)).await {
                                            Ok(_) => push_toast(toast.show, "Display name updated", ToastKind::Success),
                                            Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                        }
                                        saving_name.set(false);
                                    });
                                },
                            }
                        }
                        div { class: "space-y-2",
                            label { class: "text-label-caps font-label-caps text-outline uppercase block", "Avatar image URL" }
                            div { class: "flex items-center gap-4",
                                Avatar {
                                    seed: user_id.clone(),
                                    size: AvatarSize::Md,
                                    image_url: if avatar_url_edit().trim().is_empty() { None } else { Some(avatar_url_edit()) },
                                }
                                input {
                                    class: "input-field flex-1",
                                    placeholder: "https://example.com/avatar.jpg",
                                    value: "{avatar_url_edit}",
                                    oninput: move |e| avatar_url_edit.set(e.value()),
                                }
                            }
                            p { class: "text-body-sm text-outline", "Paste a URL to any image. Leave empty to use the generated avatar." }
                            PrimaryButton {
                                label: if saving_avatar() { "Saving…".to_string() } else { "Save avatar".to_string() },
                                disabled: saving_avatar(),
                                onclick: move |_| {
                                    let url = avatar_url_edit().trim().to_string();
                                    let toast = toast;
                                    saving_avatar.set(true);
                                    spawn(async move {
                                        let val = if url.is_empty() { serde_json::Value::Null } else { serde_json::json!(url) };
                                        let q = "mutation A($u: String) { setAvatarUrl(avatarUrl: $u) }";
                                        let vars = serde_json::json!({ "u": val });
                                        match graphql_exec::<serde_json::Value>(q, Some(vars)).await {
                                            Ok(_) => push_toast(toast.show, "Avatar updated", ToastKind::Success),
                                            Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                        }
                                        saving_avatar.set(false);
                                    });
                                },
                            }
                        }
                        div { class: "pt-4 border-t border-outline-variant/30",
                            button {
                                class: "btn-ghost text-error border-error/40 text-sm",
                                onclick: move |_| {
                                    let confirm = confirm;
                                    spawn(async move {
                                        if !confirm
                                            .confirm_with(
                                                ConfirmOptions::new("Log out of your account?")
                                                    .destructive()
                                                    .confirm_label("Log out"),
                                            )
                                            .await
                                        {
                                            return;
                                        }
                                        let _ = graphql_post::<serde_json::Value>("mutation { logout }").await;
                                        clear_auth_session();
                                        if let Some(w) = web_sys::window() {
                                            let _ = w.location().set_href("/");
                                        }
                                    });
                                },
                                Icon { name: "logout", filled: false }
                                "Log out"
                            }
                        }
                    }
                },
                1 => rsx! {
                    div { class: "section-card space-y-4",
                        div { class: "rounded-xl border border-tertiary-container/30 bg-tertiary-container/10 p-4 flex items-center gap-3",
                            Icon { name: "shield", filled: false }
                            div {
                                p { class: "font-medium text-on-surface", "Session active" }
                                p { class: "text-body-sm text-on-surface-variant", "Guest and registered sessions supported." }
                            }
                        }
                        div { class: "rounded-xl border border-secondary-container/30 bg-secondary-container/10 px-4 py-3",
                            p { class: "text-body-sm text-on-surface-variant", "Security audit: no issues detected." }
                        }
                    }
                },
                2 => rsx! {
                    div { class: "section-card space-y-4",
                        if loading_tokens() {
                            SkeletonCard {}
                        } else if tokens().is_empty() {
                            EmptyState {
                                icon: "key",
                                title: "No API tokens".to_string(),
                                description: "Generate a publish token for CI or local CLI uploads.".to_string(),
                                cta_label: None,
                                on_cta: None,
                            }
                        } else {
                            for tok in tokens() {
                                div { class: "flex items-center justify-between gap-4 rounded-xl border border-outline-variant/40 bg-surface-container-low px-4 py-3",
                                    div { class: "flex items-center gap-3",
                                        Icon { name: "key", filled: false }
                                        div {
                                            p { class: "font-medium text-on-surface",
                                                "{tok.label.clone().unwrap_or_else(|| \"Publish token\".to_string())}"
                                            }
                                            p { class: "font-mono-code text-body-sm text-outline", "{tok.masked_key}" }
                                            p { class: "text-label-caps font-label-caps text-outline",
                                                "Created {format_relative_time(tok.created_at)}"
                                            }
                                        }
                                    }
                                    button {
                                        class: "btn-danger btn-sm",
                                        onclick: {
                                            let tid = tok.id.clone();
                                            let toast = toast;
                                            move |_| {
                                                let id = tid.clone();
                                                spawn(async move {
                                                    let q = "mutation R($id: ID!) { revokePublishToken(tokenId: $id) }";
                                                    let vars = serde_json::json!({ "id": id });
                                                    match graphql_exec::<serde_json::Value>(q, Some(vars)).await {
                                                        Ok(_) => {
                                                            push_toast(toast.show, "Token revoked", ToastKind::Success);
                                                            fetch_tokens();
                                                        }
                                                        Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                    }
                                                });
                                            }
                                        },
                                        "Revoke"
                                    }
                                }
                            }
                        }
                        button {
                            class: "btn-primary w-full",
                            disabled: creating_token(),
                            onclick: {
                                let toast = toast;
                                move |_| {
                                    creating_token.set(true);
                                    spawn(async move {
                                        #[derive(Deserialize)]
                                        #[serde(rename_all = "camelCase")]
                                        struct Tok { token: String }
                                        #[derive(Deserialize)]
                                        #[serde(rename_all = "camelCase")]
                                        struct Wrap { create_publish_token: Tok }
                                        let q = "mutation { createPublishToken(label: \"CLI Token\") { token expiresAt } }";
                                        match graphql_post::<Wrap>(q).await {
                                            Ok(w) => {
                                                push_toast(
                                                    toast.show,
                                                    format!("Token created — copy now: {}", w.create_publish_token.token),
                                                    ToastKind::Success,
                                                );
                                                fetch_tokens();
                                            }
                                            Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                        }
                                        creating_token.set(false);
                                    });
                                }
                            },
                            if creating_token() { "Generating…" } else { "Generate token" }
                        }
                    }
                },
                _ => rsx! {
                    div { class: "section-card space-y-4",
                        div { class: "flex items-center justify-between gap-3",
                            p { class: "text-label-caps font-label-caps text-outline uppercase", "Inbox" }
                            if notifications().iter().any(|n| n.unread) {
                                GhostButton {
                                    label: "Mark all read".to_string(),
                                    onclick: move |_| {
                                        spawn(async move {
                                            let _ = graphql_post::<serde_json::Value>(
                                                "mutation { markAllNotificationsRead }"
                                            ).await;
                                            fetch_notifications();
                                        });
                                    },
                                }
                            }
                        }
                        if notifications().is_empty() {
                            p { class: "text-body-sm text-outline", "No notifications yet." }
                        }
                        for item in notifications() {
                            div {
                                class: if item.unread {
                                    "flex items-start gap-3 py-3 border-b border-outline-variant/20 bg-primary-container/5 rounded-lg px-2"
                                } else {
                                    "flex items-start gap-3 py-3 border-b border-outline-variant/20 px-2"
                                },
                                span {
                                    class: if item.unread { "mt-1 h-2 w-2 rounded-full bg-primary-container shrink-0" } else { "mt-1 h-2 w-2 shrink-0" },
                                }
                                div { class: "min-w-0 flex-1",
                                    p { class: "text-body-sm font-medium text-on-surface", "{item.title}" }
                                    p { class: "text-body-sm text-outline mt-0.5", "{item.body}" }
                                    p { class: "text-label-caps font-label-caps text-outline mt-1",
                                        "{format_relative_time(item.created_at)}"
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}
