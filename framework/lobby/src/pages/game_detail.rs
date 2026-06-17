use crate::api::{create_lobby_with_game, graphql_exec, graphql_post};
use crate::components::game::{MediaGallery, SteamSection, SteamSectionNav, StorefrontEditor};
use crate::components::ui::*;
use crate::models::{
    format_estimated_match_time, format_play_time, format_relative_time, AspectRatings,
    DeploymentRow, GameComment, GameReview, GameSession, GameStorefront, GameTypeInfo,
    LeaderboardEntry, PlayTimeEntry,
};
use crate::LobbyRoute;
use dioxus::prelude::*;
use serde::Deserialize;

#[component]
pub fn GameDetailPage(name: String) -> Element {
    let nav = use_navigator();
    let mut game_types: Signal<Vec<GameTypeInfo>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut error_msg = use_signal(|| None::<String>);
    let mut active_section = use_signal(|| 0usize);
    let mut section_open = use_signal(|| [true, true, true, true, true, true, true]);
    let mut about_full = use_signal(|| false);
    let mut reviews_all = use_signal(|| false);
    let mut comments_all = use_signal(|| false);
    let mut history_all = use_signal(|| false);
    let mut lb_tab = use_signal(|| 0usize);
    let mut creating = use_signal(|| false);
    let mut editor_open = use_signal(|| false);
    let mut review_body = use_signal(String::new);
    let mut comment_body = use_signal(String::new);
    let mut review_aspects = use_signal(|| [4.0f32, 4.0, 3.5, 4.0, 3.5]);
    let mut sessions: Signal<Vec<GameSession>> = use_signal(Vec::new);
    let mut leaderboard: Signal<Vec<LeaderboardEntry>> = use_signal(Vec::new);
    let mut playtime_lb: Signal<Vec<PlayTimeEntry>> = use_signal(Vec::new);
    let mut storefront: Signal<Option<GameStorefront>> = use_signal(|| None);
    let mut reviews: Signal<Vec<GameReview>> = use_signal(Vec::new);
    let mut comments: Signal<Vec<GameComment>> = use_signal(Vec::new);
    let mut published_versions: Signal<Vec<DeploymentRow>> = use_signal(Vec::new);
    let toast = use_toast();
    let mut reload_key = use_signal(|| 0u32);

    use_hook(move || {
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Data { game_types: Vec<GameTypeInfo> }
            let q = r#"query { gameTypes { name displayName version minPlayers maxPlayers description configUiPath aboutUiPath configSchemaJson activePlayers featured tags creatorDisplayName avgSessionMins coverImageUrl } }"#;
            match graphql_post::<Data>(q).await {
                Ok(d) => game_types.set(d.game_types),
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    let game_name_for_fetch = name.clone();
    use_effect(move || {
        let _tick = reload_key();
        let gt = game_name_for_fetch.clone();
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Sf { game_storefront: GameStorefront }
            if let Ok(s) = graphql_exec::<Sf>(
                r#"query($t: String!) { gameStorefront(gameType: $t) {
                    gameName shortTagline longDescription screenshots { id caption gradient imageUrl }
                    patchNotes { version date title body tags } tags avgSessionMins
                    aspectRatings { gameplay balance visuals social depth }
                    reviewCount canEdit updatedAt
                } }"#,
                Some(serde_json::json!({ "t": gt })),
            ).await {
                storefront.set(Some(s.game_storefront));
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Rv { game_reviews: Vec<GameReview> }
            if let Ok(r) = graphql_exec::<Rv>(
                "query($t: String!) { gameReviews(gameType: $t, limit: 20) { id displayName body aspects { gameplay balance visuals social depth } helpfulVotes createdAt } }",
                Some(serde_json::json!({ "t": gt })),
            ).await {
                reviews.set(r.game_reviews);
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Cm { game_comments: Vec<GameComment> }
            if let Ok(c) = graphql_exec::<Cm>(
                "query($t: String!) { gameComments(gameType: $t, limit: 30) { id displayName body createdAt } }",
                Some(serde_json::json!({ "t": gt })),
            ).await {
                comments.set(c.game_comments);
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Hist { finished_games_by_type: Vec<GameSession> }
            if let Ok(h) = graphql_exec::<Hist>(
                "query($t: String!) { finishedGamesByType(gameType: $t, limit: 20) { gameId gameType finishedAt winnerDisplayName participantCount durationSecs } }",
                Some(serde_json::json!({ "t": gt })),
            ).await {
                sessions.set(h.finished_games_by_type);
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Lb { game_leaderboard: Vec<LeaderboardEntry> }
            if let Ok(l) = graphql_exec::<Lb>(
                "query($t: String!) { gameLeaderboard(gameType: $t, limit: 10) { rank displayName totalScore wins winRatePct } }",
                Some(serde_json::json!({ "t": gt })),
            ).await {
                leaderboard.set(l.game_leaderboard);
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Pt { game_play_time_leaderboard: Vec<PlayTimeEntry> }
            if let Ok(p) = graphql_exec::<Pt>(
                "query($t: String!) { gamePlayTimeLeaderboard(gameType: $t, limit: 10) { rank displayName totalMins sessions } }",
                Some(serde_json::json!({ "t": gt })),
            ).await {
                playtime_lb.set(p.game_play_time_leaderboard);
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Ver { game_published_versions: Vec<DeploymentRow> }
            if let Ok(v) = graphql_exec::<Ver>(
                "query($t: String!) { gamePublishedVersions(gameType: $t) { id gameName displayName version status deployedAt } }",
                Some(serde_json::json!({ "t": gt })),
            ).await {
                published_versions.set(v.game_published_versions);
            }
        });
    });
    let gt = game_types().into_iter().find(|g| g.name == name);
    let sf = storefront();
    let creator_label = sf
        .as_ref()
        .and_then(|s| s.creator_display_name.clone())
        .or_else(|| gt.as_ref().and_then(|g| g.creator_display_name.clone()))
        .unwrap_or_else(|| "—".into());
    let active_players = gt.as_ref().map(|g| g.active_players).unwrap_or(0);
    let avg_mins = sf
        .as_ref()
        .map(|s| s.avg_session_mins)
        .or_else(|| gt.as_ref().map(|g| g.avg_session_mins))
        .unwrap_or(0);
    let aspects = sf.as_ref().map(|s| s.aspect_ratings.clone()).unwrap_or(AspectRatings {
        gameplay: 4.0, balance: 4.0, visuals: 3.5, social: 4.0, depth: 3.5,
    });
    let spider_axes = vec![
        SpiderAxis { label: "Gameplay", value: aspects.gameplay },
        SpiderAxis { label: "Balance", value: aspects.balance },
        SpiderAxis { label: "Visuals", value: aspects.visuals },
        SpiderAxis { label: "Social", value: aspects.social },
        SpiderAxis { label: "Depth", value: aspects.depth },
    ];

    rsx! {
        div { class: "page-stack",
            if let Some(err) = error_msg() {
                ErrorBanner { message: err }
            }
            if loading() {
                SkeletonHero {}
            } else if let Some(ref game) = gt {
                if let Some(ref store) = sf {
                    StorefrontEditor {
                        open: editor_open(),
                        game_type: name.clone(),
                        storefront: store.clone(),
                        on_close: move |_| editor_open.set(false),
                        on_saved: move |_| reload_key.set(reload_key() + 1),
                    }
                }

                div { class: "grid grid-cols-12 gap-6 lg:gap-8 items-start",
                    div { class: "col-span-12 lg:col-span-8 space-y-2",
                        if let Some(ref store) = sf {
                            MediaGallery { screenshots: store.screenshots.clone() }
                        } else {
                            section { class: "page-hero min-h-[200px]",
                                div { class: "absolute inset-0 bg-gradient-to-br from-primary-container/40 via-surface-container-low to-background z-0" }
                            }
                        }
                        div {
                            div { class: "flex flex-wrap gap-2 mb-2",
                                if let Some(ref store) = sf {
                                    for tag in store.tags.clone() {
                                        Chip { label: tag, muted: false }
                                    }
                                } else if let Some(ref game) = gt {
                                    for tag in game.tags.clone() {
                                        Chip { label: tag, muted: false }
                                    }
                                }
                            }
                            h1 { class: "font-manrope text-h1 text-3xl lg:text-4xl text-on-surface", "{game.display_name}" }
                            p { class: "font-mono-code text-outline mt-1", "{game.name} · v{game.version}" }
                            if let Some(ref store) = sf {
                                if let Some(ref tagline) = store.short_tagline {
                                    p { class: "text-body-lg text-primary mt-2", "{tagline}" }
                                }
                            }
                        }

                        SteamSectionNav {
                            active: active_section(),
                            on_select: move |i| active_section.set(i),
                        }

                        div { class: "space-y-0 pt-2",
                            {
                                let desc = sf.as_ref().map(|s| s.long_description.clone())
                                    .unwrap_or_else(|| game.description.clone());
                                let review_count = sf.as_ref().map(|s| s.review_count).unwrap_or(0);
                                let rev_list = reviews();
                                let rev_visible = if reviews_all() { rev_list.len() } else { rev_list.len().min(3) };
                                let com_list = comments();
                                let com_visible = if comments_all() { com_list.len() } else { com_list.len().min(5) };
                                let sess_list = sessions();
                                let sess_visible = if history_all() { sess_list.len() } else { sess_list.len().min(8) };
                                let version_list = published_versions();
                                rsx! {
                                    SteamSection {
                                        id: "section-about",
                                        title: "About this game",
                                        expanded: section_open()[0],
                                        on_toggle: move |_| {
                                            let mut o = section_open();
                                            o[0] = !o[0];
                                            section_open.set(o);
                                        },
                                        meta: rsx! {},
                                        div { class: "prose-game",
                                            p {
                                                class: if about_full() { "text-body-md text-on-surface-variant leading-relaxed whitespace-pre-wrap" } else { "steam-truncate-preview" },
                                                "{desc}"
                                            }
                                            if desc.len() > 280 && !about_full() {
                                                button {
                                                    class: "link-action mt-2 normal-case",
                                                    onclick: move |_| about_full.set(true),
                                                    "Read more"
                                                }
                                            }
                                            div { class: "mt-6 grid grid-cols-2 sm:grid-cols-4 gap-4",
                                                KpiCard { label: "Creator".to_string(), value: creator_label.clone(), icon: None, trend: None, trend_up: true }
                                                KpiCard { label: "Active".to_string(), value: active_players.to_string(), icon: None, trend: None, trend_up: active_players > 0 }
                                                KpiCard { label: "Reviews".to_string(), value: review_count.to_string(), icon: None, trend: None, trend_up: true }
                                                KpiCard { label: "Avg session".to_string(), value: format_estimated_match_time(avg_mins), icon: None, trend: None, trend_up: true }
                                            }
                                        }
                                    }

                                    SteamSection {
                                        id: "section-reviews",
                                        title: "Reviews",
                                        expanded: section_open()[1],
                                        meta: rsx! {
                                            p { class: "text-body-sm text-on-surface-variant mt-0.5", "{review_count} player reviews" }
                                        },
                                        on_toggle: move |_| {
                                            let mut o = section_open();
                                            o[1] = !o[1];
                                            section_open.set(o);
                                        },
                                        div { class: "space-y-4",
                                            details { class: "section-card group",
                                                summary { class: "cursor-pointer font-manrope font-semibold text-on-surface list-none flex items-center justify-between",
                                                    span { "Write a review" }
                                                    Icon { name: "edit", filled: false }
                                                }
                                                div { class: "mt-4 space-y-3",
                                                    textarea {
                                                        class: "input-field min-h-[80px]",
                                                        placeholder: "Share your experience…",
                                                        value: "{review_body}",
                                                        oninput: move |e| review_body.set(e.value()),
                                                    }
                                                    p { class: "text-label-caps text-outline uppercase text-[10px]", "Rate aspects (1–5)" }
                                                    div { class: "grid grid-cols-5 gap-2 text-center text-xs",
                                                        for (i, label) in ["Gameplay", "Balance", "Visuals", "Social", "Depth"].iter().enumerate() {
                                                            div {
                                                                p { class: "text-outline mb-1", "{label}" }
                                                                input {
                                                                    class: "input-field text-center py-1",
                                                                    r#type: "number",
                                                                    min: "1",
                                                                    max: "5",
                                                                    step: "0.5",
                                                                    value: "{review_aspects()[i]}",
                                                                    oninput: move |e| {
                                                                        let v: f32 = e.value().parse().unwrap_or(4.0);
                                                                        let mut a = review_aspects();
                                                                        a[i] = v;
                                                                        review_aspects.set(a);
                                                                    },
                                                                }
                                                            }
                                                        }
                                                    }
                                                    button {
                                                        class: "btn-primary",
                                                        onclick: {
                                                            let body = review_body;
                                                            let aspects = review_aspects;
                                                            let gt = name.clone();
                                                            let toast = toast;
                                                            let mut reload_key = reload_key;
                                                            move |_| {
                                                                let a = aspects();
                                                                let q = r#"mutation($t: String!, $b: String!, $g: Float!, $bal: Float!, $v: Float!, $so: Float!, $d: Float!) {
                                                                    submitGameReview(gameType: $t, body: $b, gameplay: $g, balance: $bal, visuals: $v, social: $so, depth: $d) { id }
                                                                }"#;
                                                                let vars = serde_json::json!({
                                                                    "t": gt, "b": body(), "g": a[0], "bal": a[1], "v": a[2], "so": a[3], "d": a[4]
                                                                });
                                                                spawn(async move {
                                                                    match graphql_exec::<serde_json::Value>(q, Some(vars)).await {
                                                                        Ok(_) => {
                                                                            push_toast(toast.show, "Review posted", ToastKind::Success);
                                                                            reload_key.set(reload_key() + 1);
                                                                        }
                                                                        Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                                    }
                                                                });
                                                            }
                                                        },
                                                        "Post review"
                                                    }
                                                }
                                            }
                                            for rev in rev_list.iter().take(rev_visible) {
                                                div { class: "review-card",
                                                    div { class: "flex items-start gap-3",
                                                        Avatar { seed: rev.display_name.clone(), size: AvatarSize::Md, image_url: None }
                                                        div { class: "flex-1 min-w-0",
                                                            div { class: "flex items-center justify-between gap-2",
                                                                p { class: "font-medium text-on-surface", "{rev.display_name}" }
                                                                span { class: "text-label-caps text-outline text-[10px]", "{format_relative_time(rev.created_at)}" }
                                                            }
                                                            p { class: "text-body-sm text-on-surface-variant mt-2 leading-relaxed", "{rev.body}" }
                                                            p { class: "text-xs text-outline mt-2", "{rev.helpful_votes} found helpful" }
                                                            if !rev.user_has_voted {
                                                                button {
                                                                    class: "btn-ghost text-xs mt-2",
                                                                    onclick: {
                                                                        let rid = rev.id.clone();
                                                                        let mut reload_key = reload_key;
                                                                        move |_| {
                                                                            let rid = rid.clone();
                                                                            spawn(async move {
                                                                                let q = "mutation($id: ID!) { markReviewHelpful(reviewId: $id) { id } }";
                                                                                let vars = serde_json::json!({ "id": rid });
                                                                                if graphql_exec::<serde_json::Value>(q, Some(vars)).await.is_ok() {
                                                                                    reload_key.set(reload_key() + 1);
                                                                                }
                                                                            });
                                                                        }
                                                                    },
                                                                    "Mark helpful"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            if rev_list.is_empty() {
                                                EmptyState { icon: "rate_review", title: "No reviews yet".to_string(), description: "Be the first to review this game.".to_string(), cta_label: None, on_cta: None }
                                            } else if rev_list.len() > 3 && !reviews_all() {
                                                button {
                                                    class: "btn-ghost text-sm",
                                                    onclick: move |_| reviews_all.set(true),
                                                    "View all {rev_list.len()} reviews"
                                                }
                                            }
                                        }
                                    }

                                    SteamSection {
                                        id: "section-discussions",
                                        title: "Discussions",
                                        expanded: section_open()[2],
                                        meta: rsx! {
                                            p { class: "text-body-sm text-on-surface-variant mt-0.5", "{com_list.len()} posts" }
                                        },
                                        on_toggle: move |_| {
                                            let mut o = section_open();
                                            o[2] = !o[2];
                                            section_open.set(o);
                                        },
                                        div { class: "section-card space-y-4",
                                            div { class: "flex gap-2",
                                                input {
                                                    class: "input-field flex-1",
                                                    placeholder: "Join the discussion…",
                                                    value: "{comment_body}",
                                                    oninput: move |e| comment_body.set(e.value()),
                                                }
                                                button {
                                                    class: "btn-primary shrink-0",
                                                    onclick: {
                                                        let body = comment_body;
                                                        let mut comment_body = comment_body;
                                                        let gt = name.clone();
                                                        let toast = toast;
                                                        let mut reload_key = reload_key;
                                                        move |_| {
                                                            let q = r#"mutation($t: String!, $b: String!) { submitGameComment(gameType: $t, body: $b) { id } }"#;
                                                            let vars = serde_json::json!({ "t": gt, "b": body() });
                                                            spawn(async move {
                                                                match graphql_exec::<serde_json::Value>(q, Some(vars)).await {
                                                                    Ok(_) => {
                                                                        push_toast(toast.show, "Comment posted", ToastKind::Success);
                                                                        comment_body.set(String::new());
                                                                        reload_key.set(reload_key() + 1);
                                                                    }
                                                                    Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                                }
                                                            });
                                                        }
                                                    },
                                                    "Post"
                                                }
                                            }
                                            for c in com_list.iter().take(com_visible) {
                                                div { class: "comment-row",
                                                    Avatar { seed: c.display_name.clone(), size: AvatarSize::Sm, image_url: None }
                                                    div { class: "min-w-0 flex-1",
                                                        p { class: "text-body-sm font-medium", "{c.display_name}" }
                                                        p { class: "text-body-sm text-on-surface-variant mt-0.5", "{c.body}" }
                                                        p { class: "text-[10px] text-outline mt-1", "{format_relative_time(c.created_at)}" }
                                                    }
                                                }
                                            }
                                            if com_list.is_empty() {
                                                p { class: "text-body-sm text-outline text-center py-6", "No discussions yet." }
                                            } else if com_list.len() > 5 && !comments_all() {
                                                button {
                                                    class: "btn-ghost text-sm w-full",
                                                    onclick: move |_| comments_all.set(true),
                                                    "View all {com_list.len()} posts"
                                                }
                                            }
                                        }
                                    }

                                    SteamSection {
                                        id: "section-patch-notes",
                                        title: "Patch notes",
                                        expanded: section_open()[3],
                                        meta: rsx! {
                                            if let Some(ref store) = sf {
                                                p { class: "text-body-sm text-on-surface-variant mt-0.5", "{store.patch_notes.len()} updates" }
                                            }
                                        },
                                        on_toggle: move |_| {
                                            let mut o = section_open();
                                            o[3] = !o[3];
                                            section_open.set(o);
                                        },
                                        if let Some(ref store) = sf {
                                            if store.patch_notes.is_empty() {
                                                EmptyState { icon: "history_edu", title: "No patch notes".to_string(), description: "Developer updates will appear here.".to_string(), cta_label: None, on_cta: None }
                                            } else {
                                                div { class: "space-y-2",
                                                    for (i, note) in store.patch_notes.iter().enumerate() {
                                                        details {
                                                            class: "patch-note-card group",
                                                            open: i == 0,
                                                            summary { class: "cursor-pointer list-none",
                                                                div { class: "flex flex-wrap items-center gap-2",
                                                                    span { class: "font-mono-code text-primary font-semibold", "v{note.version}" }
                                                                    span { class: "text-label-caps text-outline text-[10px]", "{note.date}" }
                                                                    for t in note.tags.clone() {
                                                                        Chip { label: t, muted: true }
                                                                    }
                                                                }
                                                                h4 { class: "font-manrope font-semibold text-on-surface mt-2", "{note.title}" }
                                                            }
                                                            p { class: "text-body-sm text-on-surface-variant mt-3 leading-relaxed", "{note.body}" }
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            EmptyState { icon: "history_edu", title: "No patch notes".to_string(), description: "Developer updates will appear here.".to_string(), cta_label: None, on_cta: None }
                                        }
                                    }

                                    SteamSection {
                                        id: "section-versions",
                                        title: "Versions",
                                        expanded: section_open()[4],
                                        meta: rsx! {
                                            p { class: "text-body-sm text-on-surface-variant mt-0.5", "{version_list.len()} published" }
                                        },
                                        on_toggle: move |_| {
                                            let mut o = section_open();
                                            o[4] = !o[4];
                                            section_open.set(o);
                                        },
                                        if version_list.is_empty() {
                                            EmptyState {
                                                icon: "deployed_code",
                                                title: "No version history".to_string(),
                                                description: "Published versions will appear here.".to_string(),
                                                cta_label: None,
                                                on_cta: None,
                                            }
                                        } else {
                                            div { class: "section-card overflow-x-auto p-0",
                                                table { class: "data-table",
                                                    thead { tr { th { "Version" } th { "Status" } th { "Published" } } }
                                                    tbody {
                                                        for row in version_list {
                                                            tr {
                                                                td { class: "font-mono-code text-primary", "v{row.version}" }
                                                                td {
                                                                    StatusBadge {
                                                                        label: row.status.clone(),
                                                                        variant: if row.status == "Live" { StatusVariant::Online } else { StatusVariant::Waiting },
                                                                    }
                                                                }
                                                                td { class: "text-outline", "{format_relative_time(row.deployed_at)}" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    SteamSection {
                                        id: "section-leaderboards",
                                        title: "Leaderboards",
                                        expanded: section_open()[5],
                                        meta: rsx! {
                                            p { class: "text-body-sm text-on-surface-variant mt-0.5", "Points and play time" }
                                        },
                                        on_toggle: move |_| {
                                            let mut o = section_open();
                                            o[5] = !o[5];
                                            section_open.set(o);
                                        },
                                        div { class: "space-y-4",
                                            SegmentedControl {
                                                options: vec!["Points", "Play time"],
                                                active: lb_tab(),
                                                on_select: move |i| lb_tab.set(i),
                                            }
                                            if lb_tab() == 0 {
                                                if leaderboard().is_empty() {
                                                    EmptyState { icon: "leaderboard", title: "No points data".to_string(), description: "Finish matches to rank up.".to_string(), cta_label: None, on_cta: None }
                                                } else {
                                                    div { class: "section-card overflow-x-auto p-0",
                                                        table { class: "data-table",
                                                            thead { tr { th { "#" } th { "Player" } th { "Score" } th { "Wins" } th { "Win %" } } }
                                                            tbody {
                                                                for row in leaderboard() {
                                                                    tr {
                                                                        td { class: "font-mono-code text-primary", "#{row.rank}" }
                                                                        td {
                                                                            div { class: "flex items-center gap-2",
                                                                                Avatar { seed: row.display_name.clone(), size: AvatarSize::Sm, image_url: None }
                                                                                "{row.display_name}"
                                                                            }
                                                                        }
                                                                        td { class: "font-mono-code", "{row.total_score}" }
                                                                        td { "{row.wins}" }
                                                                        td { "{row.win_rate_pct}%" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            } else if playtime_lb().is_empty() {
                                                EmptyState { icon: "schedule", title: "No play time data".to_string(), description: "Time rankings estimate from completed sessions.".to_string(), cta_label: None, on_cta: None }
                                            } else {
                                                div { class: "section-card overflow-x-auto p-0",
                                                    table { class: "data-table",
                                                        thead { tr { th { "#" } th { "Player" } th { "Time" } th { "Sessions" } } }
                                                        tbody {
                                                            for row in playtime_lb() {
                                                                tr {
                                                                    td { class: "font-mono-code text-primary", "#{row.rank}" }
                                                                    td {
                                                                        div { class: "flex items-center gap-2",
                                                                            Avatar { seed: row.display_name.clone(), size: AvatarSize::Sm, image_url: None }
                                                                            "{row.display_name}"
                                                                        }
                                                                    }
                                                                    td { class: "font-mono-code text-tertiary", "{format_play_time(row.total_mins)}" }
                                                                    td { "{row.sessions}" }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    SteamSection {
                                        id: "section-match-history",
                                        title: "Match history",
                                        expanded: section_open()[6],
                                        meta: rsx! {
                                            p { class: "text-body-sm text-on-surface-variant mt-0.5", "{sess_list.len()} finished games" }
                                        },
                                        on_toggle: move |_| {
                                            let mut o = section_open();
                                            o[6] = !o[6];
                                            section_open.set(o);
                                        },
                                        if sess_list.is_empty() {
                                            EmptyState { icon: "history", title: "No match history".to_string(), description: "Finished games appear here.".to_string(), cta_label: None, on_cta: None }
                                        } else {
                                            div { class: "space-y-3",
                                                div { class: "section-card overflow-x-auto p-0",
                                                    table { class: "data-table",
                                                        thead { tr { th { "Match" } th { "Winner" } th { "Players" } th { "When" } } }
                                                        tbody {
                                                            for row in sess_list.iter().take(sess_visible) {
                                                                tr {
                                                                    td { class: "font-mono-code", "#{row.game_id.chars().take(8).collect::<String>()}" }
                                                                    td {
                                                                        if let Some(ref w) = row.winner_display_name {
                                                                            span { class: "text-tertiary font-medium", "{w}" }
                                                                        } else { "Draw" }
                                                                    }
                                                                    td { "{row.participant_count}" }
                                                                    td { "{format_relative_time(row.finished_at)}" }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                if sess_list.len() > 8 && !history_all() {
                                                    button {
                                                        class: "btn-ghost text-sm",
                                                        onclick: move |_| history_all.set(true),
                                                        "View all {sess_list.len()} matches"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    aside { class: "col-span-12 lg:col-span-4 space-y-4 lg:sticky lg:top-24 lg:self-start",
                        div { class: "steam-sidebar-card sticky top-24",
                            SpiderChart { axes: spider_axes, size: 220 }
                            p { class: "text-center text-body-sm text-on-surface-variant",
                                if let Some(ref store) = sf {
                                    "{store.review_count} player reviews"
                                } else {
                                    "Player reviews"
                                }
                            }
                            div { class: "grid grid-cols-2 gap-3 text-body-sm",
                                div {
                                    p { class: "text-outline text-label-caps uppercase text-[10px]", "Players" }
                                    p { class: "font-mono-code text-on-surface", "{game.min_players}–{game.max_players}" }
                                }
                                div {
                                    p { class: "text-outline text-label-caps uppercase text-[10px]", "Avg session" }
                                    p { class: "font-mono-code text-on-surface",
                                        if avg_mins > 0 { "{avg_mins} min" } else { "—" }
                                    }
                                }
                            }
                            PrimaryButton {
                                label: if creating() { "Creating…".to_string() } else { "Create Lobby".to_string() },
                                disabled: creating(),
                                onclick: move |_| {
                                    creating.set(true);
                                    let toast = toast;
                                    let game_type = name.clone();
                                    spawn(async move {
                                        match create_lobby_with_game(Some(&game_type)).await {
                                            Ok(id) => {
                                                push_toast(toast.show, "Lobby created — game selected", ToastKind::Success);
                                                nav.push(LobbyRoute::Lobby { id });
                                            }
                                            Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                        }
                                        creating.set(false);
                                    });
                                },
                            }
                            if sf.as_ref().map(|s| s.can_edit).unwrap_or(false) {
                                button {
                                    class: "btn-secondary w-full",
                                    onclick: move |_| editor_open.set(true),
                                    Icon { name: "edit", filled: false }
                                    "Edit store page"
                                }
                            }
                        }

                        div { class: "steam-sidebar-card",
                            h3 { class: "font-manrope font-semibold text-on-surface text-sm", "Top players (points)" }
                            div { class: "space-y-2",
                                for row in leaderboard().iter().take(5) {
                                    div { class: "flex items-center gap-2",
                                        span { class: "font-mono-code text-primary w-5 text-xs", "#{row.rank}" }
                                        Avatar { seed: row.display_name.clone(), size: AvatarSize::Sm, image_url: None }
                                        span { class: "text-body-sm truncate flex-1", "{row.display_name}" }
                                        span { class: "font-mono-code text-tertiary text-xs", "{row.total_score}" }
                                    }
                                }
                                if leaderboard().is_empty() {
                                    p { class: "text-body-sm text-outline", "No rankings yet." }
                                }
                            }
                        }
                    }
                }
            } else {
                EmptyState {
                    icon: "search_off",
                    title: "Game not found".to_string(),
                    description: format!("No published game named {name}."),
                    cta_label: Some("Browse games".to_string()),
                    on_cta: Some(EventHandler::new(move |_| {
                        nav.push(LobbyRoute::GamesList {});
                    })),
                }
            }
        }
    }
}
