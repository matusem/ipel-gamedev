use base64::{engine::general_purpose::STANDARD, Engine};
use crate::api::graphql_exec;
use crate::components::ui::use_confirm;
use crate::models::{GameDraftShort, manifest_description_from_json};
use dioxus::html::FileData;
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn DeveloperDraftRow(
    draft: GameDraftShort,
    mut err: Signal<Option<String>>,
    on_refresh: EventHandler<()>,
) -> Element {
    let desc0 = manifest_description_from_json(&draft.manifest_json);
    let mut publish_name = use_signal(|| draft.game_name.clone());
    let mut publish_display = use_signal(|| draft.display_name.clone());
    let mut publish_version = use_signal(|| draft.version.clone());
    let mut publish_desc = use_signal(|| desc0);
    let mut saving = use_signal(|| false);
    let confirm = use_confirm();

    use_effect({
        let d = draft.clone();
        move || {
            publish_name.set(d.game_name.clone());
            publish_display.set(d.display_name.clone());
            publish_version.set(d.version.clone());
            publish_desc.set(manifest_description_from_json(&d.manifest_json));
        }
    });

    let status = draft.status.clone();
    let id_pub = draft.id.clone();
    let id_unpub = draft.id.clone();
    let id_disc = draft.id.clone();
    let id_save = draft.id.clone();

    rsx! {
        div { class: "{draft_card_classes(&draft.status)}",
            div { class: "flex-1 min-w-0 space-y-3",
                div {
                    div { class: "flex flex-wrap items-center gap-2 mb-1",
                        span { class: "px-2.5 py-0.5 rounded-md text-[10px] font-bold uppercase tracking-wide {draft_status_style(&draft.status).0}",
                            "{draft.status}"
                        }
                        span { class: "text-on-surface font-semibold", "{draft.display_name}" }
                        span { class: "text-outline text-sm", "({draft.game_name})" }
                        if !draft.slug.is_empty() {
                            span { class: "text-outline text-xs font-mono-code", "slug: {draft.slug}" }
                        }
                        span { class: "text-primary text-sm font-mono-code", "v{draft.version}" }
                    }
                    p { class: "text-xs text-outline font-mono-code",
                        "created {draft.created_at}"
                        if draft.published_at.is_some() {
                            " · published {draft.published_at.unwrap_or(0)}"
                        }
                    }
                }
                if status == "ready" {
                    p { class: "text-xs text-on-surface-variant",
                        "Adjust manifest fields, then save before publishing. The live catalog slug is assigned by the server (shown above)."
                    }
                    div { class: "grid sm:grid-cols-2 gap-3",
                        label { class: "block space-y-1",
                            span { class: "text-label-caps font-label-caps text-outline uppercase", "manifest name" }
                            input {
                                class: "input-field font-mono-code",
                                value: "{publish_name()}",
                                oninput: move |e| publish_name.set(e.value()),
                            }
                        }
                        label { class: "block space-y-1",
                            span { class: "text-label-caps font-label-caps text-outline uppercase", "display name" }
                            input {
                                class: "input-field",
                                value: "{publish_display()}",
                                oninput: move |e| publish_display.set(e.value()),
                            }
                        }
                        label { class: "block space-y-1",
                            span { class: "text-label-caps font-label-caps text-outline uppercase", "version" }
                            input {
                                class: "input-field font-mono-code",
                                value: "{publish_version()}",
                                oninput: move |e| publish_version.set(e.value()),
                            }
                        }
                    }
                    label { class: "block space-y-1",
                        span { class: "text-label-caps font-label-caps text-outline uppercase", "description" }
                        textarea {
                            class: "input-field min-h-[4rem]",
                            value: "{publish_desc()}",
                            oninput: move |e| publish_desc.set(e.value()),
                        }
                    }
                    button {
                        class: "btn-secondary text-xs",
                        disabled: saving(),
                        onclick: move |_| {
                            saving.set(true);
                            let id = id_save.clone();
                            let n = publish_name();
                            let dn = publish_display();
                            let v = publish_version();
                            let d = publish_desc();
                            spawn(async move {
                                let q = r#"mutation U($id: ID!, $n: String!, $dn: String!, $v: String!, $d: String!) {
                                    updateGameDraftManifest(draftId: $id, name: $n, displayName: $dn, version: $v, description: $d) { id }
                                }"#;
                                let vars = serde_json::json!({
                                    "id": id,
                                    "n": n,
                                    "dn": dn,
                                    "v": v,
                                    "d": d,
                                });
                                match graphql_exec::<Value>(q, Some(vars)).await {
                                    Ok(_) => {
                                        err.set(None);
                                        on_refresh.call(());
                                    }
                                    Err(e) => err.set(Some(e)),
                                }
                                saving.set(false);
                            });
                        },
                        if saving() { "Saving…" } else { "Save manifest fields" }
                    }
                }
            }
            div { class: "flex flex-wrap gap-2 shrink-0",
                button {
                    class: "btn-primary text-xs",
                    disabled: draft.status != "ready",
                    onclick: move |_| {
                        let id2 = id_pub.clone();
                        spawn(async move {
                            let q = "mutation P($id: ID!) { publishGameDraft(draftId: $id) { id } }";
                            let vars = serde_json::json!({ "id": id2 });
                            match graphql_exec::<Value>(q, Some(vars)).await {
                                Ok(_) => {
                                    err.set(None);
                                    on_refresh.call(());
                                }
                                Err(e) => err.set(Some(e)),
                            }
                        });
                    },
                    "Publish"
                }
                button {
                    class: "btn-secondary text-xs",
                    disabled: draft.status != "published",
                    onclick: move |_| {
                        let id2 = id_unpub.clone();
                        let confirm = confirm;
                        spawn(async move {
                            if !confirm
                                .confirm(
                                    "Remove this game from the live lobby? Players will not see it until someone publishes again.",
                                )
                                .await
                            {
                                return;
                            }
                            let q = "mutation U($id: ID!) { unpublishGameDraft(draftId: $id) { id status } }";
                            let vars = serde_json::json!({ "id": id2 });
                            match graphql_exec::<Value>(q, Some(vars)).await {
                                Ok(_) => {
                                    err.set(None);
                                    on_refresh.call(());
                                }
                                Err(e) => err.set(Some(e)),
                            }
                        });
                    },
                    "Take down"
                }
                button {
                    class: "btn-danger btn-sm",
                    disabled: draft.status == "published",
                    onclick: move |_| {
                        let id2 = id_disc.clone();
                        spawn(async move {
                            let q = "mutation D($id: ID!) { discardGameDraft(draftId: $id) }";
                            let vars = serde_json::json!({ "id": id2 });
                            match graphql_exec::<Value>(q, Some(vars)).await {
                                Ok(_) => {
                                    err.set(None);
                                    on_refresh.call(());
                                }
                                Err(e) => err.set(Some(e)),
                            }
                        });
                    },
                    "Discard"
                }
            }
        }
    }
}

pub fn upload_diag_panel_class(severity: &str) -> &'static str {
    match severity {
        "error" => "border-l-4 border-l-error bg-error-container/20 border border-error/30 rounded-r-lg",
        "warning" => "border-l-4 border-l-secondary bg-secondary-container/15 border border-secondary-container/30 rounded-r-lg",
        "info" => "border-l-4 border-l-primary bg-primary-container/15 border border-primary-container/30 rounded-r-lg",
        _ => "border-l-4 border-l-outline bg-surface-container-high border border-outline-variant/40 rounded-r-lg",
    }
}

pub fn upload_diag_badge_class(severity: &str) -> &'static str {
    match severity {
        "error" => "bg-error-container text-on-error-container",
        "warning" => "bg-secondary-container text-on-secondary-container",
        "info" => "bg-primary-container text-on-primary-container",
        _ => "bg-surface-container-highest text-on-surface-variant",
    }
}

pub fn upload_file_check_class(ok: bool) -> &'static str {
    if ok {
        "flex items-center gap-2 rounded-lg border border-tertiary-container/50 bg-tertiary-container/15 px-3 py-2 text-body-sm text-tertiary"
    } else {
        "flex items-center gap-2 rounded-lg border border-error/40 bg-error-container/20 px-3 py-2 text-body-sm text-error"
    }
}

pub fn draft_status_style(status: &str) -> (&'static str, &'static str) {
    match status {
        "ready" => ("bg-primary-container text-on-primary-container", "border-l-primary-container"),
        "published" => ("bg-tertiary-container text-on-tertiary-container", "border-l-tertiary"),
        "discarded" => ("bg-surface-container-highest text-on-surface-variant", "border-l-outline"),
        _ => ("bg-surface-container-high text-on-surface", "border-l-outline-variant"),
    }
}

pub fn draft_card_classes(status: &str) -> String {
    let (_, border) = draft_status_style(status);
    format!(
        "rounded-xl border border-outline-variant/40 bg-surface-container-low p-4 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 shadow-raised border-l-4 {border}"
    )
}

pub fn spawn_read_zip_file(
    fd: FileData,
    mut zip_base64: Signal<String>,
    mut filename: Signal<String>,
    mut file_status: Signal<String>,
    mut err: Signal<Option<String>>,
) {
    spawn(async move {
        match fd.read_bytes().await {
            Ok(bytes) => {
                let name = fd.name();
                let b64 = STANDARD.encode(&bytes);
                let status = format!("{name} — {} bytes (ready)", bytes.len());
                filename.set(name);
                zip_base64.set(b64);
                file_status.set(status);
                err.set(None);
            }
            Err(e) => {
                zip_base64.set(String::new());
                file_status.set("No file selected".to_string());
                err.set(Some(format!("Failed to read zip: {e}")));
            }
        }
    });
}
