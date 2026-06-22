mod model;
mod validate;
mod widgets;

use dioxus::prelude::*;
use serde_json::Value;

pub use model::{coerce_initial_value, default_value_for_spec, parse_schema, FieldSpec};
pub use validate::{first_error_message, is_valid, validate_value, FieldErrors};
pub use widgets::FieldRenderer;

/// JSON Schema-driven form for the supported subset.
#[component]
pub fn SchemaForm(
    schema: Value,
    value: Signal<Value>,
    #[props(default)]
    read_only: bool,
    #[props(default)]
    show_preview: bool,
) -> Element {
    let spec = parse_schema(&schema);
    if matches!(spec, FieldSpec::Fallback) {
        return rsx! {
            div { class: "space-y-2",
                p { class: "text-body-sm text-on-surface-variant",
                    "This schema uses unsupported constructs. Edit raw JSON below."
                }
                textarea {
                    class: "input-field font-mono-code min-h-[12rem]",
                    readonly: read_only,
                    value: "{serde_json::to_string_pretty(&value()).unwrap_or_else(|_| \"null\".into())}",
                    oninput: move |e| {
                        if let Ok(v) = serde_json::from_str::<Value>(&e.value()) {
                            value.set(v);
                        }
                    },
                }
            }
        };
    }

    let spec_for_errors = spec.clone();
    let errors = use_memo(move || validate_value(&spec_for_errors, &value(), ""));

    rsx! {
        div { class: "schema-form space-y-4",
            if !read_only {
                FieldRenderer {
                    path: String::new(),
                    spec: spec.clone(),
                    value: value(),
                    errors: errors(),
                    on_change: EventHandler::new(move |(path, new_val): (String, Value)| {
                        let mut root = value();
                        set_at_path(&mut root, &path, new_val);
                        value.set(root);
                    }),
                }
            } else {
                crate::components::ui::JsonConsole {
                    content: serde_json::to_string_pretty(&value()).unwrap_or_else(|_| "null".into()),
                    max_height: Some("16rem"),
                }
            }
            if show_preview {
                details { class: "mt-2",
                    summary { class: "text-label-caps font-label-caps text-outline uppercase cursor-pointer",
                        "JSON preview"
                    }
                    crate::components::ui::JsonConsole {
                        content: serde_json::to_string_pretty(&value()).unwrap_or_else(|_| "null".into()),
                        max_height: Some("12rem"),
                    }
                }
            }
            if !errors().is_empty() {
                div { class: "rounded-lg border border-error/40 bg-error-container/20 px-3 py-2",
                    p { class: "text-body-sm text-error",
                        "{first_error_message(&errors()).unwrap_or_else(|| \"Invalid settings\".into())}"
                    }
                }
            }
        }
    }
}

fn set_at_path(root: &mut Value, path: &str, new_val: Value) {
    if path.is_empty() {
        *root = new_val;
        return;
    }
    if let Some((prefix, idx)) = split_array_path(path) {
        if let Value::Object(map) = root {
            let entry = map
                .entry(prefix.to_string())
                .or_insert(Value::Array(vec![]));
            if let Value::Array(arr) = entry {
                while arr.len() <= idx {
                    arr.push(Value::Null);
                }
                if path == format!("{prefix}[{idx}]") {
                    arr[idx] = new_val;
                } else {
                    let suffix = &path[prefix.len() + idx.to_string().len() + 2..];
                    set_at_path(&mut arr[idx], suffix.trim_start_matches('.'), new_val);
                }
            }
        }
        return;
    }
    let mut parts = path.split('.');
    let first = parts.next().unwrap_or("");
    if let Some(rest) = parts.next() {
        let remaining = if let Some(more) = path.find(&format!(".{rest}")) {
            &path[more + 1..]
        } else {
            rest
        };
        if let Value::Object(map) = root {
            let entry = map
                .entry(first.to_string())
                .or_insert(Value::Object(serde_json::Map::new()));
            set_at_path(entry, remaining, new_val);
        }
    } else if let Value::Object(map) = root {
        map.insert(first.to_string(), new_val);
    }
}

fn split_array_path(path: &str) -> Option<(&str, usize)> {
    let bracket = path.find('[')?;
    let end = path.find(']')?;
    let prefix = &path[..bracket];
    let idx: usize = path[bracket + 1..end].parse().ok()?;
    Some((prefix, idx))
}
