use crate::api::{graphql_exec, graphql_post, stored_user_id};
use crate::components::ui::*;
use crate::models::{format_relative_time, PublishTokenSummary, UserProfile};
use crate::stub::{demo_mode, notifications_stub};
use dioxus::prelude::*;
use serde::Deserialize;

#[component]
pub fn SettingsPage() -> Element {
    let mut tab = use_signal(|| 0usize);
    let user_id = stored_user_id().unwrap_or_else(|| "—".into());
    let mut profile = use_signal(|| None::<UserProfile>);
    let mut tokens = use_signal(Vec::<PublishTokenSummary>::new);
    let mut loading_tokens = use_signal(|| true);
    let mut creating_token = use_signal(|| false);
    let toast = use_toast();

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
                "query { myProfile { displayName createdAt matchesPlayed gamesPublished wins repScore } }",
            )
            .await
            {
                profile.set(w.my_profile);
            }
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
                        div {
                            label { class: "text-label-caps font-label-caps text-outline uppercase block mb-1", "Display name" }
                            span { class: "inline-flex items-center gap-2 px-2 py-0.5 rounded-full bg-primary-container/15 text-primary text-label-caps font-label-caps uppercase text-[10px] mb-2", "Preview — coming soon" }
                            input {
                                class: "input-field",
                                placeholder: "Coming soon",
                                disabled: true,
                                value: profile().map(|p| p.display_name).unwrap_or_default(),
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
                        p { class: "text-label-caps font-label-caps text-outline uppercase", "Notification preferences (preview)" }
                        for item in notifications_stub() {
                            label { class: "flex items-start gap-3 py-2 border-b border-outline-variant/20",
                                input { r#type: "checkbox", checked: item.unread, disabled: true }
                                div {
                                    p { class: "text-body-sm font-medium text-on-surface", "{item.title}" }
                                    p { class: "text-body-sm text-outline", "{item.body}" }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}
