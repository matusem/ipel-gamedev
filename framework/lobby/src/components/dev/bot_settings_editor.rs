use crate::api::graphql_exec;
use crate::components::ui::{
    coerce_initial_value, default_value_for_spec, parse_schema, push_toast, use_toast, ErrorBanner,
    PrimaryButton, SchemaForm, ToastKind,
};
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn BotSettingsEditor(
    open: bool,
    on_close: EventHandler<()>,
    bot_id: String,
    bot_name: String,
    settings_schema_json: Option<String>,
    settings_json: Option<String>,
    on_saved: EventHandler<()>,
) -> Element {
    if !open {
        return rsx! {};
    }

    let toast = use_toast();
    let schema_val = settings_schema_json
        .as_ref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or(Value::Null);
    let spec = parse_schema(&schema_val);
    let init_spec = spec.clone();
    let init = settings_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or_else(|| default_value_for_spec(&init_spec));
    let mut form_value = use_signal(move || coerce_initial_value(&init_spec, &init));
    let mut raw_json = use_signal(|| settings_json.clone().unwrap_or_else(|| "{}".into()));
    let mut saving = use_signal(|| false);
    let mut err = use_signal(|| None::<String>);
    let has_schema = !matches!(spec, crate::components::ui::FieldSpec::Fallback);

    rsx! {
        div { class: "lobby-game-modal-layer",
            button {
                class: "lobby-game-modal-backdrop",
                onclick: move |_| on_close.call(()),
            }
            div { class: "lobby-config-modal max-w-lg w-full",
                div { class: "lobby-game-modal-head",
                    div {
                        p { class: "lobby-section-kicker", "BOT SETTINGS" }
                        h2 { class: "lobby-section-title", "{bot_name}" }
                    }
                    button {
                        class: "lobby-game-modal-close",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                div { class: "lobby-config-modal-body space-y-4",
                    if let Some(e) = err() {
                        ErrorBanner { message: e }
                    }
                    if has_schema {
                        SchemaForm {
                            schema: schema_val,
                            value: form_value,
                            read_only: false,
                            show_preview: true,
                        }
                    } else {
                        label { class: "field-label", "Settings JSON" }
                        textarea {
                            class: "input-field font-mono-code min-h-[10rem]",
                            value: "{raw_json()}",
                            oninput: move |e| raw_json.set(e.value()),
                        }
                    }
                    PrimaryButton {
                        label: if saving() { "Saving…".to_string() } else { "Save settings".to_string() },
                        disabled: saving(),
                        onclick: {
                            let bot_id = bot_id.clone();
                            let on_saved = on_saved;
                            let on_close = on_close;
                            move |_| {
                                let payload = if has_schema {
                                    serde_json::to_string(&form_value()).unwrap_or_else(|_| "{}".into())
                                } else {
                                    raw_json()
                                };
                                let bot_id = bot_id.clone();
                                let toast = toast;
                                let mut saving = saving;
                                let mut err = err;
                                let on_saved = on_saved;
                                let on_close = on_close;
                                spawn(async move {
                                    saving.set(true);
                                    err.set(None);
                                    let q = r#"mutation U($id: ID!, $s: String!) { updateBotSettings(botId: $id, settingsJson: $s) { id } }"#;
                                    let vars = serde_json::json!({ "id": bot_id, "s": payload });
                                    match graphql_exec::<Value>(q, Some(vars)).await {
                                        Ok(_) => {
                                            push_toast(toast.show, "Bot settings saved", ToastKind::Success);
                                            on_saved.call(());
                                            on_close.call(());
                                        }
                                        Err(e) => err.set(Some(e)),
                                    }
                                    saving.set(false);
                                });
                            }
                        },
                    }
                }
            }
        }
    }
}
