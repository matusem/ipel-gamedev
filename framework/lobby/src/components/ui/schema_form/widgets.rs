use dioxus::prelude::*;
use serde_json::Value;

use super::model::FieldSpec;
use super::validate::FieldErrors;
use crate::components::ui::FieldLabel;

#[component]
pub fn FieldError(path: String, errors: FieldErrors) -> Element {
    if let Some(msg) = errors.get(&path) {
        rsx! {
            p { class: "text-body-sm text-error mt-1", "{msg}" }
        }
    } else {
        rsx! {}
    }
}

#[component]
pub fn TextField(
    path: String,
    label: String,
    value: String,
    errors: FieldErrors,
    on_change: EventHandler<(String, String)>,
) -> Element {
    let path_for_input = path.clone();
    rsx! {
        div { class: "space-y-1",
            FieldLabel { label }
            input {
                class: "input-field",
                value: "{value}",
                oninput: move |e| on_change.call((path_for_input.clone(), e.value())),
            }
            FieldError { path, errors }
        }
    }
}

#[component]
pub fn NumberField(
    path: String,
    label: String,
    value: String,
    is_integer: bool,
    errors: FieldErrors,
    on_change: EventHandler<(String, String)>,
) -> Element {
    let path_for_input = path.clone();
    rsx! {
        div { class: "space-y-1",
            FieldLabel { label }
            input {
                class: "input-field",
                r#type: "number",
                step: if is_integer { "1" } else { "any" },
                value: "{value}",
                oninput: move |e| on_change.call((path_for_input.clone(), e.value())),
            }
            FieldError { path, errors }
        }
    }
}

#[component]
pub fn CheckboxField(
    path: String,
    label: String,
    checked: bool,
    errors: FieldErrors,
    on_change: EventHandler<(String, bool)>,
) -> Element {
    let path_for_input = path.clone();
    rsx! {
        div { class: "space-y-1",
            label { class: "inline-flex items-center gap-2 cursor-pointer",
                input {
                    r#type: "checkbox",
                    class: "rounded border-outline-variant/40",
                    checked,
                    onchange: move |e| on_change.call((path_for_input.clone(), e.checked())),
                }
                span { class: "text-body-sm text-on-surface", "{label}" }
            }
            FieldError { path, errors }
        }
    }
}

#[component]
pub fn SelectField(
    path: String,
    label: String,
    value: String,
    options: Vec<String>,
    errors: FieldErrors,
    on_change: EventHandler<(String, String)>,
) -> Element {
    let path_for_input = path.clone();
    rsx! {
        div { class: "space-y-1",
            FieldLabel { label }
            select {
                class: "select-field",
                value: "{value}",
                onchange: move |e| on_change.call((path_for_input.clone(), e.value())),
                for opt in options.iter().cloned() {
                    option { value: "{opt}", "{opt}" }
                }
            }
            FieldError { path, errors }
        }
    }
}

#[component]
pub fn EnumSegmentedField(
    path: String,
    label: String,
    options: Vec<String>,
    active_value: String,
    errors: FieldErrors,
    on_change: EventHandler<(String, String)>,
) -> Element {
    rsx! {
        SelectField {
            path,
            label,
            value: active_value,
            options,
            errors,
            on_change,
        }
    }
}

#[component]
pub fn FieldRenderer(
    path: String,
    spec: FieldSpec,
    value: Value,
    errors: FieldErrors,
    on_change: EventHandler<(String, Value)>,
) -> Element {
    match spec {
        FieldSpec::Fallback => rsx! {
            p { class: "text-body-sm text-on-surface-variant",
                "Unsupported field type"
            }
        },
        FieldSpec::OptionalObject { inner } => rsx! {
            FieldRenderer { path, spec: *inner, value, errors, on_change }
        },
        FieldSpec::Object { properties, .. } => rsx! {
            div { class: "space-y-3",
                for (key, sub) in properties {
                    {
                        let field_path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{path}.{key}")
                        };
                        let field_value = value.get(&key).cloned().unwrap_or(Value::Null);
                        rsx! {
                            FieldRenderer {
                                path: field_path,
                                spec: sub,
                                value: field_value,
                                errors: errors.clone(),
                                on_change,
                            }
                        }
                    }
                }
            }
        },
        FieldSpec::String { title, description, .. } => {
            let label = title.unwrap_or_else(|| humanize_path(&path));
            let val = value.as_str().unwrap_or("").to_string();
            rsx! {
                div { class: "space-y-1",
                    TextField {
                        path: path.clone(),
                        label,
                        value: val,
                        errors: errors.clone(),
                        on_change: move |(p, s)| on_change.call((p, Value::String(s))),
                    }
                    if let Some(d) = description {
                        p { class: "text-body-sm text-on-surface-variant", "{d}" }
                    }
                }
            }
        }
        FieldSpec::Integer { title, description, .. } => {
            let label = title.unwrap_or_else(|| humanize_path(&path));
            let val = value.as_i64().map(|n| n.to_string()).unwrap_or_default();
            rsx! {
                div { class: "space-y-1",
                    NumberField {
                        path: path.clone(),
                        label,
                        value: val,
                        is_integer: true,
                        errors: errors.clone(),
                        on_change: move |(p, s): (String, String)| {
                            let v = s
                                .parse::<i64>()
                                .ok()
                                .map(|n| Value::Number(n.into()))
                                .unwrap_or(Value::Null);
                            on_change.call((p, v));
                        },
                    }
                    if let Some(d) = description {
                        p { class: "text-body-sm text-on-surface-variant", "{d}" }
                    }
                }
            }
        }
        FieldSpec::Number { title, description, .. } => {
            let label = title.unwrap_or_else(|| humanize_path(&path));
            let val = value.as_f64().map(|n| n.to_string()).unwrap_or_default();
            rsx! {
                div { class: "space-y-1",
                    NumberField {
                        path: path.clone(),
                        label,
                        value: val,
                        is_integer: false,
                        errors: errors.clone(),
                        on_change: move |(p, s): (String, String)| {
                            let v = s
                                .parse::<f64>()
                                .ok()
                                .and_then(serde_json::Number::from_f64)
                                .map(Value::Number)
                                .unwrap_or(Value::Null);
                            on_change.call((p, v));
                        },
                    }
                    if let Some(d) = description {
                        p { class: "text-body-sm text-on-surface-variant", "{d}" }
                    }
                }
            }
        }
        FieldSpec::Boolean { title, .. } => {
            let label = title.unwrap_or_else(|| humanize_path(&path));
            let checked = value.as_bool().unwrap_or(false);
            rsx! {
                CheckboxField {
                    path,
                    label,
                    checked,
                    errors,
                    on_change: move |(p, b)| on_change.call((p, Value::Bool(b))),
                }
            }
        }
        FieldSpec::Enum { title, options, .. } => {
            let label = title.unwrap_or_else(|| humanize_path(&path));
            let val = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| options.first().cloned().unwrap_or_default());
            rsx! {
                EnumSegmentedField {
                    path,
                    label,
                    options,
                    active_value: val,
                    errors,
                    on_change: move |(p, s)| on_change.call((p, Value::String(s))),
                }
            }
        }
        FieldSpec::Array {
            title,
            item,
            max_items,
            ..
        } => {
            let label = title.unwrap_or_else(|| humanize_path(&path));
            let items = value.as_array().cloned().unwrap_or_default();
            let item_spec = *item;
            rsx! {
                div { class: "space-y-2",
                    FieldLabel { label }
                    for (i, item_val) in items.iter().cloned().enumerate() {
                        {
                            let item_path = format!("{path}[{i}]");
                            let path_rm = path.clone();
                            rsx! {
                                div { class: "flex gap-2 items-start",
                                    div { class: "flex-1",
                                        FieldRenderer {
                                            path: item_path,
                                            spec: item_spec.clone(),
                                            value: item_val,
                                            errors: errors.clone(),
                                            on_change: EventHandler::new({
                                                let path_arr = path.clone();
                                                let items_snapshot = items.clone();
                                                move |(sub_path, sub_val): (String, Value)| {
                                                    if let Some(idx) = parse_array_index(&sub_path) {
                                                        let mut arr = items_snapshot.clone();
                                                        if idx < arr.len() {
                                                            arr[idx] = sub_val;
                                                            on_change.call((path_arr.clone(), Value::Array(arr)));
                                                        }
                                                    }
                                                }
                                            }),
                                        }
                                    }
                                    button {
                                        class: "btn-ghost btn-sm shrink-0 mt-6",
                                        r#type: "button",
                                        onclick: {
                                            let path_rm = path_rm.clone();
                                            let items_snapshot = items.clone();
                                            move |_| {
                                                let mut arr = items_snapshot.clone();
                                                arr.remove(i);
                                                on_change.call((path_rm.clone(), Value::Array(arr)));
                                            }
                                        },
                                        "Remove"
                                    }
                                }
                            }
                        }
                    }
                    if items.len() < max_items {
                        button {
                            class: "btn-ghost btn-sm",
                            r#type: "button",
                            onclick: {
                                let path_add = path.clone();
                                let items_snapshot = items.clone();
                                let default = super::model::default_value_for_spec(&item_spec);
                                move |_| {
                                    let mut arr = items_snapshot.clone();
                                    arr.push(default.clone());
                                    on_change.call((path_add.clone(), Value::Array(arr)));
                                }
                            },
                            "Add item"
                        }
                    }
                    FieldError { path, errors }
                }
            }
        }
    }
}

fn humanize_path(path: &str) -> String {
    let base = path.rsplit('.').next().unwrap_or(path);
    base.replace('_', " ")
}

fn parse_array_index(path: &str) -> Option<usize> {
    let start = path.rfind('[')?;
    let end = path.rfind(']')?;
    path[start + 1..end].parse().ok()
}
