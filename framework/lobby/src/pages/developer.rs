use crate::api::*;
use crate::components::dev::{
    DeveloperDraftRow, spawn_read_zip_file, upload_diag_badge_class, upload_diag_panel_class,
    upload_file_check_class,
};
use crate::components::ui::*;
use crate::models::{format_relative_time, DeploymentRow, GameDraftShort, KpiTrend, PlatformStats, UploadReport};
use dioxus::events::{DragData, FormData};
use dioxus::html::HasFileData;
use dioxus::prelude::*;
use gloo_events::{EventListener, EventListenerOptions};
use serde::Deserialize;
use std::rc::Rc;

const CLI_COMMANDS: &[(&str, &str)] = &[
    ("Build package", "gamedev build"),
    ("Validate package", "gamedev validate"),
    ("Deploy to server", "gamedev deploy --server http://127.0.0.1:8081/graphql"),
    ("Init new game", "gamedev init"),
];

#[component]
pub fn DeveloperUploadsPage() -> Element {
    let mut is_dev = use_signal(|| None::<bool>);
    let mut err = use_signal(|| None::<String>);
    let mut filename = use_signal(|| "game.zip".to_string());
    let mut zip_base64 = use_signal(String::new);
    let file_status = use_signal(|| "No file selected".to_string());
    let mut uploading = use_signal(|| false);
    let mut report = use_signal(|| None::<UploadReport>);
    let mut report_open = use_signal(|| false);
    let mut drafts = use_signal(Vec::<GameDraftShort>::new);
    let mut zip_drag_over = use_signal(|| false);
    let deployments: Signal<Vec<DeploymentRow>> = use_signal(Vec::new);
    let platform_stats: Signal<Option<PlatformStats>> = use_signal(|| None);
    let toast = use_toast();

    let _global_file_drag_guard: Rc<(EventListener, EventListener)> = use_hook(move || {
        let win = web_sys::window().expect("window");
        let doc = win.document().expect("document");
        let opts = EventListenerOptions::enable_prevent_default();
        let drag_over = EventListener::new_with_options(&doc, "dragover", opts.clone(), |e: &web_sys::Event| {
            e.prevent_default();
        });
        let drop_doc = EventListener::new_with_options(&doc, "drop", opts, |e: &web_sys::Event| {
            e.prevent_default();
        });
        Rc::new((drag_over, drop_doc))
    });

    let refresh_drafts = {
        let mut drafts = drafts;
        let mut err = err;
        move || {
            spawn(async move {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Wrap { my_game_drafts: Vec<GameDraftShort> }
                let q = "query { myGameDrafts { id gameName displayName version status manifestJson createdAt publishedAt } }";
                match graphql_post::<Wrap>(q).await {
                    Ok(v) => drafts.set(v.my_game_drafts),
                    Err(e) => err.set(Some(e)),
                }
            });
        }
    };

    use_hook(move || {
        let mut is_dev = is_dev;
        let mut err = err;
        let mut deployments = deployments;
        let mut platform_stats = platform_stats;
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct P { is_developer: bool }
            match graphql_post::<P>("query { isDeveloper }").await {
                Ok(v) => is_dev.set(Some(v.is_developer)),
                Err(e) => {
                    is_dev.set(Some(false));
                    err.set(Some(e));
                }
            }
        });
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct D { published_deployments: Vec<DeploymentRow> }
            if let Ok(d) = graphql_post::<D>(
                "query { publishedDeployments(limit: 20) { id gameName displayName version status deployedAt } }",
            )
            .await
            {
                deployments.set(d.published_deployments);
            }
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct S { platform_stats: PlatformStats }
            if let Ok(s) = graphql_post::<S>(
                "query { platformStats { activeLobbies publishedGameTypes finishedGames24h status trends { label value deltaPct up } } }",
            )
            .await
            {
                platform_stats.set(Some(s.platform_stats));
            }
        });
        refresh_drafts();
    });

    let trends: Vec<KpiTrend> = platform_stats()
        .map(|s| s.trends)
        .unwrap_or_default();
    let api_badge = platform_stats()
        .map(|s| if s.status == "ok" { "API: Operational" } else { "API: Degraded" })
        .unwrap_or("API: —");

    rsx! {
        div { class: "page-stack",
            PageHeader {
                title: "Developer Command Center".to_string(),
                subtitle: Some("Package, validate, and publish WASM game builds.".to_string()),
                badge: Some(api_badge.to_string()),
                children: None,
            }

            if let Some(e) = err() {
                ErrorBanner { message: e }
            }

            if is_dev() == Some(false) {
                div { class: "rounded-xl border border-secondary-container/40 bg-secondary-container/10 px-5 py-4",
                    p { class: "text-on-surface font-medium", "Developer access required" }
                    p { class: "mt-2 text-body-sm text-on-surface-variant",
                        "Grant the developer role, or set OPEN_DEVELOPER_UPLOADS=true for open uploads."
                    }
                }
            } else if is_dev().is_none() {
                div { class: "grid gap-4 sm:grid-cols-3",
                    SkeletonCard {}
                    SkeletonCard {}
                    SkeletonCard {}
                }
            } else {
                div { class: "grid gap-4 sm:grid-cols-3",
                    KpiCard {
                        label: "Active lobbies".to_string(),
                        value: platform_stats().map(|s| s.active_lobbies.to_string()).unwrap_or_else(|| "—".into()),
                        icon: Some("groups"),
                        trend: trends.get(0).map(|t| t.delta_pct.clone()),
                        trend_up: trends.get(0).map(|t| t.up).unwrap_or(true),
                    }
                    KpiCard {
                        label: "Published games".to_string(),
                        value: platform_stats().map(|s| s.published_game_types.to_string()).unwrap_or_else(|| "—".into()),
                        icon: Some("deployed_code"),
                        trend: trends.get(1).map(|t| t.delta_pct.clone()),
                        trend_up: trends.get(1).map(|t| t.up).unwrap_or(true),
                    }
                    KpiCard {
                        label: "Finished (24h)".to_string(),
                        value: platform_stats().map(|s| s.finished_games24h.to_string()).unwrap_or_else(|| "—".into()),
                        icon: Some("monitoring"),
                        trend: trends.get(2).map(|t| t.delta_pct.clone()),
                        trend_up: trends.get(2).map(|t| t.up).unwrap_or(true),
                    }
                }

                section { class: "section-card",
                    div { class: "flex items-center gap-3 mb-4",
                        Icon { name: "upload_file", filled: false }
                        div {
                            h2 { class: "card-title text-lg", "Upload game zip" }
                            p { class: "text-body-sm text-on-surface-variant", "Drag & drop or browse — then validate on the server." }
                        }
                    }
                    div {
                        class: if zip_drag_over() { "dropzone dropzone-active relative" } else { "dropzone relative" },
                        div { class: "pointer-events-none py-6",
                            Icon { name: "folder_zip", filled: false }
                            p { class: "text-on-surface font-medium mt-3", "Drop your .zip here" }
                            p { class: "text-body-sm text-outline mt-1", "Release package at repo root" }
                            span { class: "btn-primary btn-sm mt-4 pointer-events-none", "Browse files…" }
                        }
                        input {
                            id: "dev-zip-upload",
                            class: "absolute inset-0 z-10 h-full w-full cursor-pointer opacity-0",
                            r#type: "file",
                            accept: ".zip,application/zip,application/x-zip-compressed",
                            ondragenter: move |evt: Event<DragData>| { evt.prevent_default(); zip_drag_over.set(true); },
                            ondragleave: move |evt: Event<DragData>| { evt.prevent_default(); zip_drag_over.set(false); },
                            ondragover: move |evt: Event<DragData>| {
                                evt.prevent_default();
                                evt.data().data_transfer().set_drop_effect("copy");
                            },
                            ondrop: move |evt: Event<DragData>| {
                                evt.prevent_default();
                                evt.stop_propagation();
                                zip_drag_over.set(false);
                                if let Some(fd) = evt.data().files().into_iter().next() {
                                    spawn_read_zip_file(fd, zip_base64, filename, file_status, err);
                                }
                            },
                            onchange: move |evt: Event<FormData>| {
                                zip_drag_over.set(false);
                                if let Some(fd) = evt.data().files().into_iter().next() {
                                    spawn_read_zip_file(fd, zip_base64, filename, file_status, err);
                                }
                            },
                        }
                    }
                    div { class: "mt-4 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3",
                        p { class: "text-body-sm text-on-surface-variant flex items-center gap-2",
                            span { class: if zip_base64().trim().is_empty() { "status-dot-full" } else { "status-dot-online" } }
                            span { class: "font-mono-code", "{file_status()}" }
                        }
                        PrimaryButton {
                            label: if uploading() { "Validating…".to_string() } else { "Run validation".to_string() },
                            disabled: uploading() || zip_base64().trim().is_empty(),
                            onclick: move |_| {
                                let n = filename();
                                let payload = zip_base64();
                                let toast = toast;
                                uploading.set(true);
                                err.set(None);
                                spawn(async move {
                                    #[derive(Deserialize)]
                                    #[serde(rename_all = "camelCase")]
                                    struct R { upload_game_zip: UploadResp }
                                    #[derive(Deserialize)]
                                    #[serde(rename_all = "camelCase")]
                                    struct UploadResp { report: UploadReport }
                                    let q = "mutation Upload($f: String!, $z: String!) { uploadGameZip(filename: $f, zipBase64: $z) { report { ok errors warnings infos requiredIndexHtml requiredConfigHtml requiredResultHtml requiredAboutHtml diagnostics { severity code message path hint } } } }";
                                    let vars = serde_json::json!({ "f": n, "z": payload.trim() });
                                    match graphql_exec::<R>(q, Some(vars)).await {
                                        Ok(v) => {
                                            report.set(Some(v.upload_game_zip.report));
                                            report_open.set(true);
                                            push_toast(toast.show, "Validation complete", ToastKind::Info);
                                        }
                                        Err(e) => {
                                            err.set(Some(e.clone()));
                                            push_toast(toast.show, e, ToastKind::Error);
                                        }
                                    }
                                    refresh_drafts();
                                    uploading.set(false);
                                });
                            },
                        }
                    }
                }

                SlideOver {
                    open: report_open() && report().is_some(),
                    title: "Validation report".to_string(),
                    on_close: move |_| report_open.set(false),
                    footer: rsx! {
                        GhostButton { label: "Close".to_string(), onclick: move |_| report_open.set(false) }
                    },
                    {match report() {
                        Some(rep) => rsx! {
                        div { class: "space-y-6",
                            div { class: "flex items-center gap-3",
                                span { class: if rep.ok { "text-tertiary text-2xl" } else { "text-error text-2xl" },
                                    if rep.ok { "✓" } else { "✕" }
                                }
                                div {
                                    p { class: "font-manrope font-semibold text-on-surface",
                                        if rep.ok { "Package accepted" } else { "Validation failed" }
                                    }
                                    p { class: "text-body-sm text-on-surface-variant",
                                        if rep.ok { "Draft created if applicable." } else { "Fix errors and upload again." }
                                    }
                                }
                            }
                            div { class: "grid grid-cols-3 gap-3",
                                div { class: "kpi-card text-center py-3",
                                    p { class: "text-2xl font-bold text-error", "{rep.errors}" }
                                    p { class: "text-label-caps font-label-caps text-outline uppercase", "Errors" }
                                }
                                div { class: "kpi-card text-center py-3",
                                    p { class: "text-2xl font-bold text-secondary", "{rep.warnings}" }
                                    p { class: "text-label-caps font-label-caps text-outline uppercase", "Warnings" }
                                }
                                div { class: "kpi-card text-center py-3",
                                    p { class: "text-2xl font-bold text-primary", "{rep.infos}" }
                                    p { class: "text-label-caps font-label-caps text-outline uppercase", "Infos" }
                                }
                            }
                            div {
                                p { class: "text-label-caps font-label-caps text-outline uppercase mb-3", "Required client files" }
                                div { class: "grid sm:grid-cols-2 gap-2",
                                    div { class: upload_file_check_class(rep.required_index_html),
                                        span { if rep.required_index_html { "✓" } else { "✕" } }
                                        span { class: "font-mono-code text-body-sm", "client/index.html" }
                                    }
                                    div { class: upload_file_check_class(rep.required_config_html),
                                        span { if rep.required_config_html { "✓" } else { "✕" } }
                                        span { class: "font-mono-code text-body-sm", "client/config.html" }
                                    }
                                    div { class: upload_file_check_class(rep.required_result_html),
                                        span { if rep.required_result_html { "✓" } else { "✕" } }
                                        span { class: "font-mono-code text-body-sm", "client/result.html" }
                                    }
                                    div { class: upload_file_check_class(rep.required_about_html),
                                        span { if rep.required_about_html { "✓" } else { "✕" } }
                                        span { class: "font-mono-code text-body-sm", "client/about.html" }
                                    }
                                }
                            }
                            div {
                                p { class: "text-label-caps font-label-caps text-outline uppercase mb-3", "Diagnostics" }
                                div { class: "space-y-2 max-h-64 overflow-y-auto",
                                    for d in rep.diagnostics {
                                        div { class: "pl-1 {upload_diag_panel_class(&d.severity)}",
                                            div { class: "p-3",
                                                span { class: "px-2 py-0.5 rounded text-[10px] font-bold uppercase {upload_diag_badge_class(&d.severity)}", "{d.severity}" }
                                                p { class: "text-body-sm text-on-surface mt-2", "{d.message}" }
                                                if let Some(ref pth) = d.path {
                                                    p { class: "mt-1 text-xs font-mono-code text-outline", "{pth}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        },
                        None => rsx! {},
                    }}
                }

                section { class: "section-card",
                    h2 { class: "card-title text-lg mb-4", "CLI quick-start" }
                    div { class: "space-y-3",
                        for (label, cmd) in CLI_COMMANDS {
                            div { class: "flex items-center justify-between gap-3 rounded-lg border border-outline-variant/30 bg-surface-container-low px-4 py-3",
                                div {
                                    p { class: "text-label-caps font-label-caps text-outline uppercase", "{label}" }
                                    p { class: "font-mono-code text-body-sm text-on-surface mt-1", "{cmd}" }
                                }
                                GhostButton {
                                    label: "Copy".to_string(),
                                    onclick: {
                                        let cmd = cmd.to_string();
                                        move |_| {
                                            let cmd = cmd.clone();
                                            spawn(async move {
                                                if let Some(win) = web_sys::window() {
                                                    let clipboard = win.navigator().clipboard();
                                                    let _ = wasm_bindgen_futures::JsFuture::from(
                                                        clipboard.write_text(&cmd),
                                                    )
                                                    .await;
                                                }
                                            });
                                        }
                                    },
                                }
                            }
                        }
                    }
                }

                section { class: "section-card overflow-x-auto p-0",
                    h2 { class: "card-title text-lg p-5 pb-0", "Deployments" }
                    table { class: "data-table",
                        thead {
                            tr {
                                th { "ID" }
                                th { "Game" }
                                th { "Version" }
                                th { "Status" }
                                th { "Deployed" }
                            }
                        }
                        tbody {
                            if deployments().is_empty() {
                                tr {
                                    td { colspan: "5", class: "text-center text-outline py-6", "No published deployments yet" }
                                }
                            } else {
                                for row in deployments() {
                                    tr {
                                        td { class: "font-mono-code", "#{row.id.chars().take(8).collect::<String>()}" }
                                        td { "{row.display_name}" }
                                        td { class: "font-mono-code", "{row.version}" }
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

                section { class: "section-card",
                    div { class: "flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 mb-6",
                        SectionHeader {
                            title: "My drafts".to_string(),
                            subtitle: Some("Publish to go live in the lobby game list.".to_string()),
                        }
                        GhostButton { label: "Refresh list".to_string(), onclick: move |_| refresh_drafts() }
                    }
                    if drafts().is_empty() {
                        EmptyState {
                            icon: "deployed_code",
                            title: "No drafts yet".to_string(),
                            description: "Validate a zip upload to create your first draft.".to_string(),
                            cta_label: None,
                            on_cta: None,
                        }
                    } else {
                        div { class: "space-y-3",
                            for d in drafts() {
                                DeveloperDraftRow {
                                    key: "{d.id}",
                                    draft: d.clone(),
                                    err,
                                    on_refresh: move |_| refresh_drafts(),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
