use crate::api::graphql_exec;
use crate::components::ui::{push_toast, SlideOver, use_toast, ToastKind};
use crate::models::GameStorefront;
use dioxus::prelude::*;
use serde::Deserialize;

#[component]
pub fn StorefrontEditor(
    open: bool,
    game_type: String,
    storefront: GameStorefront,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let toast = use_toast();
    let mut tagline = use_signal(|| storefront.short_tagline.clone().unwrap_or_default());
    let mut description = use_signal(|| storefront.long_description.clone());
    let mut tags = use_signal(|| storefront.tags.join(", "));
    let mut avg_mins = use_signal(|| storefront.avg_session_mins.to_string());
    let mut saving = use_signal(|| false);

    rsx! {
        SlideOver {
            open,
            title: "Edit store page".to_string(),
            on_close,
            div { class: "space-y-4",
                p { class: "text-body-sm text-on-surface-variant",
                    "Steam-style page content for players. Changes are live immediately."
                }
                div {
                    label { class: "text-label-caps font-label-caps text-outline uppercase block mb-1", "Tagline" }
                    input {
                        class: "input-field",
                        value: "{tagline}",
                        oninput: move |e| tagline.set(e.value()),
                    }
                }
                div {
                    label { class: "text-label-caps font-label-caps text-outline uppercase block mb-1", "About (long description)" }
                    textarea {
                        class: "input-field min-h-[120px]",
                        value: "{description}",
                        oninput: move |e| description.set(e.value()),
                    }
                }
                div {
                    label { class: "text-label-caps font-label-caps text-outline uppercase block mb-1", "Tags (comma-separated)" }
                    input {
                        class: "input-field",
                        value: "{tags}",
                        oninput: move |e| tags.set(e.value()),
                    }
                }
                div {
                    label { class: "text-label-caps font-label-caps text-outline uppercase block mb-1", "Avg session (minutes)" }
                    input {
                        class: "input-field",
                        value: "{avg_mins}",
                        oninput: move |e| avg_mins.set(e.value()),
                    }
                }
                p { class: "text-body-sm text-outline",
                    "Screenshots and patch notes: edit via JSON in Developer Hub (advanced) or contact support."
                }
                button {
                    class: "btn-primary w-full",
                    disabled: saving(),
                    onclick: {
                        let gt = game_type.clone();
                        let shots = serde_json::to_string(&storefront.screenshots).unwrap_or_else(|_| "[]".into());
                        let patches = serde_json::to_string(&storefront.patch_notes).unwrap_or_else(|_| "[]".into());
                        move |_| {
                            saving.set(true);
                            let tagline = tagline();
                            let description = description();
                            let tags_str = tags();
                            let mins: i32 = avg_mins().parse().unwrap_or(10);
                            let tags_json: Vec<String> = tags_str
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            let tags_json = serde_json::to_string(&tags_json).unwrap_or_else(|_| "[]".into());
                            let toast = toast;
                            let gt = gt.clone();
                            let shots = shots.clone();
                            let patches = patches.clone();
                            spawn(async move {
                                let q = r#"mutation U($t: String!, $tag: String, $desc: String!, $shots: String!, $patches: String!, $tags: String!, $mins: Int) {
                                    updateGameStorefront(gameType: $t, shortTagline: $tag, longDescription: $desc, screenshotsJson: $shots, patchNotesJson: $patches, tagsJson: $tags, avgSessionMins: $mins)
                                }"#;
                                let vars = serde_json::json!({
                                    "t": gt,
                                    "tag": if tagline.is_empty() { None } else { Some(tagline) },
                                    "desc": description,
                                    "shots": shots,
                                    "patches": patches,
                                    "tags": tags_json,
                                    "mins": mins,
                                });
                                #[derive(Deserialize)]
                                #[serde(rename_all = "camelCase")]
                                struct W { update_game_storefront: bool }
                                match graphql_exec::<W>(q, Some(vars)).await {
                                    Ok(_) => {
                                        push_toast(toast.show, "Store page saved", ToastKind::Success);
                                        on_saved.call(());
                                        on_close.call(());
                                    }
                                    Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                }
                                saving.set(false);
                            });
                        }
                    },
                    if saving() { "Saving…" } else { "Save changes" }
                }
            }
        }
    }
}
