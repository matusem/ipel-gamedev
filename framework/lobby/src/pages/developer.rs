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
use std::collections::BTreeMap;
use std::rc::Rc;

fn site_origin() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .filter(|o| !o.is_empty())
        .unwrap_or_else(|| "http://localhost:8080".to_string())
}

fn install_windows_cmd(origin: &str) -> String {
    format!("irm {origin}/tools/gamedev-cli/install.ps1 | iex")
}

fn install_unix_cmd(origin: &str) -> String {
    format!("curl -fsSL {origin}/tools/gamedev-cli/install.sh | bash")
}

fn doctor_cmd(origin: &str) -> String {
    format!("gamedev doctor --platform {origin}")
}

fn quick_start_commands(origin: &str) -> Vec<(&'static str, String, &'static str)> {
    vec![
        ("Build package", "gamedev build".to_string(), "Compile your game package"),
        ("Validate package", "gamedev validate".to_string(), "Check package before upload"),
        (
            "Deploy to server",
            format!("gamedev deploy --server {origin}/graphql"),
            "Publish to this platform",
        ),
        ("Init new game", "gamedev init".to_string(), "Scaffold a new project"),
    ]
}

fn spawn_copy_command(text: String, toast: ToastContext) {
    spawn(async move {
        let ok = if let Some(win) = web_sys::window() {
            let clipboard = win.navigator().clipboard();
            wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&text))
                .await
                .is_ok()
        } else {
            false
        };
        if ok {
            push_toast(toast.show, "Copied command", ToastKind::Success);
        } else {
            push_toast(toast.show, "Could not copy to clipboard", ToastKind::Error);
        }
    });
}

#[component]
fn CliCommandCard(label: String, hint: String, command: String) -> Element {
    let toast = use_toast();
    rsx! {
        div { class: "command-card",
            div { class: "command-card-header",
                p { class: "text-label-caps font-label-caps text-outline uppercase", "{label}" }
                p { class: "text-body-sm text-on-surface-variant", "{hint}" }
            }
            div { class: "command-card-body",
                code { class: "command-line", "{command}" }
                button {
                    class: "btn-ghost shrink-0",
                    title: "Copy command",
                    onclick: {
                        let command = command.clone();
                        move |_| spawn_copy_command(command.clone(), toast)
                    },
                    Icon { name: "content_copy", filled: false }
                    "Copy"
                }
            }
        }
    }
}

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
                let q = "query { myGameDrafts { id slug gameName displayName version status manifestJson createdAt publishedAt } }";
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
    let origin = site_origin();
    let manifest_url = format!("{origin}/tools/gamedev-cli/manifest.json");

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
                                    struct UploadResp {
                                        report: UploadReport,
                                        publish_warning: Option<String>,
                                    }
                                    let q = "mutation Upload($f: String!, $z: String!) { uploadGameZip(filename: $f, zipBase64: $z) { publishWarning report { ok errors warnings infos requiredIndexHtml requiredConfigHtml requiredResultHtml requiredAboutHtml diagnostics { severity code message path hint } } } }";
                                    let vars = serde_json::json!({ "f": n, "z": payload.trim() });
                                    match graphql_exec::<R>(q, Some(vars)).await {
                                        Ok(v) => {
                                            let upload = v.upload_game_zip;
                                            let report_ok = upload.report.ok;
                                            report.set(Some(upload.report));
                                            report_open.set(true);
                                            if let Some(warn) = upload.publish_warning {
                                                push_toast(toast.show, warn, ToastKind::Error);
                                            } else if report_ok {
                                                push_toast(toast.show, "Uploaded and published", ToastKind::Success);
                                            } else {
                                                push_toast(toast.show, "Validation complete", ToastKind::Info);
                                            }
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
                    div { class: "flex items-center gap-3 mb-4",
                        Icon { name: "download", filled: false }
                        div {
                            h2 { class: "card-title text-lg", "Download CLI" }
                            p { class: "text-body-sm text-on-surface-variant",
                                "Install the platform-matching gamedev-cli for your OS."
                            }
                        }
                    }
                    p { class: "text-body-sm text-on-surface-variant mb-4",
                        "Manifest: "
                        span { class: "font-mono-code text-on-surface break-all", "{manifest_url}" }
                    }
                    div { class: "space-y-3",
                        CliCommandCard {
                            label: "Windows".to_string(),
                            hint: "PowerShell — installs the CLI for this platform".to_string(),
                            command: install_windows_cmd(&origin),
                        }
                        CliCommandCard {
                            label: "macOS / Linux".to_string(),
                            hint: "Bash — installs the CLI for this platform".to_string(),
                            command: install_unix_cmd(&origin),
                        }
                        CliCommandCard {
                            label: "Verify install".to_string(),
                            hint: "Run after install to confirm CLI matches this site".to_string(),
                            command: doctor_cmd(&origin),
                        }
                    }
                }

                section { class: "section-card",
                    div { class: "flex items-center gap-3 mb-4",
                        Icon { name: "terminal", filled: false }
                        div {
                            h2 { class: "card-title text-lg", "CLI quick-start" }
                            p { class: "text-body-sm text-on-surface-variant",
                                "Common commands once gamedev-cli is installed."
                            }
                        }
                    }
                    div { class: "space-y-3",
                        for (label, cmd, hint) in quick_start_commands(&origin) {
                            CliCommandCard {
                                key: "{label}",
                                label: label.to_string(),
                                hint: hint.to_string(),
                                command: cmd,
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
                        {
                            let grouped: BTreeMap<String, Vec<GameDraftShort>> = drafts()
                                .into_iter()
                                .fold(BTreeMap::new(), |mut acc, d| {
                                    acc.entry(d.game_name.clone()).or_default().push(d);
                                    acc
                                });
                            rsx! {
                                div { class: "space-y-4",
                                    for (game_name, mut versions) in grouped {
                                        {
                                            versions.sort_by(|a, b| {
                                                b.created_at
                                                    .cmp(&a.created_at)
                                                    .then_with(|| b.version.cmp(&a.version))
                                            });
                                            let display = versions
                                                .first()
                                                .map(|d| d.display_name.clone())
                                                .unwrap_or_else(|| game_name.clone());
                                            rsx! {
                                                div { class: "section-card space-y-3",
                                                    key: "{game_name}",
                                                    div { class: "flex flex-wrap items-baseline justify-between gap-2 border-b border-outline-variant/30 pb-3",
                                                        div {
                                                            h3 { class: "font-manrope font-semibold text-on-surface", "{display}" }
                                                            p { class: "font-mono-code text-body-sm text-outline mt-0.5", "{game_name}" }
                                                        }
                                                        p { class: "text-label-caps text-outline uppercase text-[10px]", "{versions.len()} version(s)" }
                                                    }
                                                    div { class: "space-y-3 pl-1",
                                                        for d in versions {
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
                        }
                    }
                }
            }
        }
    }
}
