use crate::api::{graphql_post, stored_user_id};
use crate::components::ui::*;
use crate::models::{ActivityEventGql, UserProfile};
use crate::stub::badges_stub;
use crate::LobbyRoute;
use dioxus::prelude::*;
use serde::Deserialize;

#[component]
pub fn ProfilePage() -> Element {
    let nav = use_navigator();
    let user_id = stored_user_id().unwrap_or_else(|| "guest".into());
    let mut profile: Signal<Option<UserProfile>> = use_signal(|| None);
    let mut activity: Signal<Vec<ActivityEventGql>> = use_signal(Vec::new);

    use_hook(move || {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct P { my_profile: Option<UserProfile> }
            if let Ok(p) = graphql_post::<P>(
                "query { myProfile { displayName createdAt matchesPlayed gamesPublished wins repScore } }",
            )
            .await
            {
                profile.set(p.my_profile);
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A { activity_feed: Vec<ActivityEventGql> }
            if let Ok(a) = graphql_post::<A>(
                "query { activityFeed(limit: 6) { actor action target timestamp } }",
            )
            .await
            {
                activity.set(a.activity_feed);
            }
        });
    });

    let display_name = profile().as_ref().map(|x| x.display_name.clone()).unwrap_or_else(|| user_id.clone());
    let rep_score = profile().map(|x| x.rep_score).unwrap_or(0);
    let matches_played = profile().map(|x| x.matches_played).unwrap_or(0);
    let matches_target = 200u32;
    let upvotes = profile().map(|x| x.wins).unwrap_or(0);
    let games_published = profile().map(|x| x.games_published).unwrap_or(0);
    let rank_progress = (rep_score % 100).min(99) as u8;

    rsx! {
        div { class: "space-y-8",
            section { class: "section-card flex flex-col sm:flex-row gap-6 items-start",
                Avatar { seed: user_id.clone(), size: AvatarSize::Xl, image_url: None }
                div { class: "flex-1 min-w-0",
                    div { class: "flex items-center gap-2 flex-wrap",
                        h1 { class: "font-manrope text-h1 text-2xl text-on-surface", "{display_name}" }
                        if matches_played > 10 {
                            span { class: "inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-tertiary-container/20 text-tertiary text-label-caps font-label-caps uppercase border border-tertiary-container/30",
                                Icon { name: "verified", filled: true }
                                "Verified"
                            }
                        }
                    }
                    p { class: "font-mono-code text-outline mt-1 truncate", "{user_id}" }
                    div { class: "mt-4 inline-flex items-center gap-2 px-3 py-1 rounded-full bg-primary-container/15 border border-primary-container/30",
                        Icon { name: "military_tech", filled: true }
                        span { class: "text-label-caps font-label-caps text-primary", "Member" }
                        span { class: "font-mono-code text-on-surface-variant", "· Rep {rep_score}" }
                    }
                    div { class: "mt-4 h-2 rounded-full bg-surface-container-high overflow-hidden max-w-xs",
                        div { class: "h-full bg-primary-container rounded-full", style: "width: {rank_progress}%" }
                    }
                    p { class: "text-body-sm text-outline mt-1", "{rank_progress}% to next rank" }
                }
                GhostButton {
                    label: "Settings".to_string(),
                    onclick: move |_| { nav.push(LobbyRoute::Settings {}); },
                }
            }

            div { class: "grid gap-4 sm:grid-cols-2 lg:grid-cols-4",
                KpiCard { label: "Matches played".to_string(), value: format!("{matches_played}/{matches_target}"), icon: Some("sports_esports"), trend: None, trend_up: true }
                KpiCard { label: "Wins".to_string(), value: upvotes.to_string(), icon: Some("thumb_up"), trend: None, trend_up: true }
                KpiCard { label: "Rep score".to_string(), value: rep_score.to_string(), icon: Some("signal_cellular_alt"), trend: None, trend_up: true }
                KpiCard { label: "Games published".to_string(), value: games_published.to_string(), icon: Some("deployed_code"), trend: None, trend_up: true }
            }

            section { class: "section-card",
                h2 { class: "font-manrope text-h2 text-xl mb-4", "Earned badges" }
                p { class: "text-label-caps font-label-caps text-outline uppercase mb-4", "Preview — achievements API coming soon" }
                div { class: "grid grid-cols-2 sm:grid-cols-3 gap-3",
                    for badge in badges_stub() {
                        div {
                            class: if badge.locked { "badge-tile badge-tile-locked" } else { "badge-tile hover:border-primary-container/50" },
                            if badge.locked {
                                Icon { name: "lock", filled: false }
                            } else {
                                Icon { name: "military_tech", filled: true }
                            }
                            p { class: "font-manrope font-semibold text-on-surface mt-2 text-sm", "{badge.label}" }
                            p { class: "text-label-caps font-label-caps text-outline uppercase text-[10px]", "{badge.tier}" }
                        }
                    }
                }
            }

            section { class: "section-card",
                h2 { class: "font-manrope text-h2 text-xl mb-4", "Recent activity" }
                if activity().is_empty() {
                    p { class: "text-body-sm text-outline", "No activity yet." }
                } else {
                    ActivityFeed { events: activity() }
                }
            }

            div { class: "section-card flex flex-wrap items-center justify-between gap-4",
                div {
                    p { class: "text-body-sm text-on-surface-variant", "Games published" }
                    p { class: "font-mono-code text-2xl text-tertiary mt-1", "{games_published}" }
                }
                PrimaryButton {
                    label: "Open Developer Hub".to_string(),
                    disabled: false,
                    onclick: move |_| { nav.push(LobbyRoute::DeveloperUploads {}); },
                }
            }
        }
    }
}
