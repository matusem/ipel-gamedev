//! JSON Schema subset validation for settings/config (mirrors lobby SchemaForm subset).

use serde_json::Value;
use std::collections::BTreeMap;

const MAX_ARRAY_ITEMS: usize = 32;

#[derive(Debug, Clone)]
pub enum FieldSpec {
    Fallback,
    OptionalObject { inner: Box<FieldSpec> },
    Object {
        properties: Vec<(String, FieldSpec)>,
        required: Vec<String>,
    },
    String {
        min_length: Option<u64>,
        max_length: Option<u64>,
    },
    Integer {
        minimum: Option<i64>,
        maximum: Option<i64>,
    },
    Number {
        minimum: Option<f64>,
        maximum: Option<f64>,
    },
    Boolean,
    Enum { options: Vec<String> },
    Array {
        item: Box<FieldSpec>,
        max_items: usize,
    },
}

pub fn validate_against_schema(schema: &Value, instance: &Value) -> Result<(), Vec<String>> {
    let spec = parse_schema(schema);
    if matches!(spec, FieldSpec::Fallback) {
        return Err(vec!["Schema uses unsupported constructs".into()]);
    }
    let mut errors = BTreeMap::new();
    validate_into(&spec, instance, "", &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.into_values().collect())
    }
}

fn parse_schema(schema: &Value) -> FieldSpec {
    if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
        let has_null = one_of.iter().any(|v| v.get("type") == Some(&Value::String("null".into())));
        let object_branch = one_of.iter().find(|v| v.get("type") == Some(&Value::String("object".into())));
        if has_null {
            if let Some(obj) = object_branch {
                return FieldSpec::OptionalObject {
                    inner: Box::new(parse_object_schema(obj)),
                };
            }
            return FieldSpec::Fallback;
        }
    }
    if schema.get("type") == Some(&Value::String("object".into())) {
        return parse_object_schema(schema);
    }
    parse_leaf(schema).unwrap_or(FieldSpec::Fallback)
}

fn parse_object_schema(schema: &Value) -> FieldSpec {
    let props = schema
        .get("properties")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let required: Vec<String> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let mut properties = Vec::new();
    for (key, sub) in props {
        let leaf = parse_leaf(&sub).unwrap_or_else(|| parse_schema(&sub));
        if matches!(leaf, FieldSpec::Fallback) {
            return FieldSpec::Fallback;
        }
        properties.push((key, leaf));
    }
    FieldSpec::Object {
        properties,
        required,
    }
}

fn parse_leaf(schema: &Value) -> Option<FieldSpec> {
    if let Some(enums) = schema.get("enum").and_then(|v| v.as_array()) {
        let options: Vec<String> = enums
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        if !options.is_empty() {
            return Some(FieldSpec::Enum { options });
        }
    }
    if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
        let options: Vec<String> = one_of
            .iter()
            .filter_map(|v| v.get("const").and_then(|c| c.as_str().map(str::to_string)))
            .collect();
        if !options.is_empty() {
            return Some(FieldSpec::Enum { options });
        }
    }
    match schema.get("type").and_then(|v| v.as_str()) {
        Some("string") => Some(FieldSpec::String {
            min_length: schema.get("minLength").and_then(|v| v.as_u64()),
            max_length: schema.get("maxLength").and_then(|v| v.as_u64()),
        }),
        Some("integer") => Some(FieldSpec::Integer {
            minimum: schema.get("minimum").and_then(|v| v.as_i64()),
            maximum: schema.get("maximum").and_then(|v| v.as_i64()),
        }),
        Some("number") => Some(FieldSpec::Number {
            minimum: schema.get("minimum").and_then(|v| v.as_f64()),
            maximum: schema.get("maximum").and_then(|v| v.as_f64()),
        }),
        Some("boolean") => Some(FieldSpec::Boolean),
        Some("array") => {
            let item_schema = schema.get("items")?;
            let item = parse_leaf(item_schema).unwrap_or_else(|| parse_schema(item_schema));
            if matches!(
                item,
                FieldSpec::Fallback
                    | FieldSpec::OptionalObject { .. }
                    | FieldSpec::Object { .. }
                    | FieldSpec::Array { .. }
            ) {
                return None;
            }
            Some(FieldSpec::Array {
                item: Box::new(item),
                max_items: MAX_ARRAY_ITEMS,
            })
        }
        _ => None,
    }
}

fn validate_into(spec: &FieldSpec, value: &Value, path: &str, errors: &mut BTreeMap<String, String>) {
    match spec {
        FieldSpec::Fallback => {
            errors.insert(path.to_string(), "Unsupported schema".into());
        }
        FieldSpec::OptionalObject { inner } => {
            if value.is_null() {
                return;
            }
            validate_into(inner, value, path, errors);
        }
        FieldSpec::Object {
            properties,
            required,
        } => {
            let obj = match value.as_object() {
                Some(o) => o,
                None => {
                    errors.insert(path_label(path), "Expected an object".into());
                    return;
                }
            };
            for key in required {
                if !obj.contains_key(key) || obj.get(key).is_some_and(|v| v.is_null()) {
                    errors.insert(path_label(&join_path(path, key)), "Required field".into());
                }
            }
            for (key, sub) in properties {
                if let Some(v) = obj.get(key) {
                    validate_into(sub, v, &join_path(path, key), errors);
                }
            }
        }
        FieldSpec::String {
            min_length,
            max_length,
        } => {
            let s = match value.as_str() {
                Some(s) => s,
                None => {
                    errors.insert(path_label(path), "Expected a string".into());
                    return;
                }
            };
            if let Some(min) = min_length {
                if (s.len() as u64) < *min {
                    errors.insert(path_label(path), format!("Minimum length is {min}"));
                }
            }
            if let Some(max) = max_length {
                if (s.len() as u64) > *max {
                    errors.insert(path_label(path), format!("Maximum length is {max}"));
                }
            }
        }
        FieldSpec::Integer { minimum, maximum } => {
            let n = match value.as_i64() {
                Some(n) => n,
                None => {
                    errors.insert(path_label(path), "Expected an integer".into());
                    return;
                }
            };
            if let Some(min) = minimum {
                if n < *min {
                    errors.insert(path_label(path), format!("Minimum is {min}"));
                }
            }
            if let Some(max) = maximum {
                if n > *max {
                    errors.insert(path_label(path), format!("Maximum is {max}"));
                }
            }
        }
        FieldSpec::Number { minimum, maximum } => {
            let n = match value.as_f64() {
                Some(n) => n,
                None => {
                    errors.insert(path_label(path), "Expected a number".into());
                    return;
                }
            };
            if let Some(min) = minimum {
                if n < *min {
                    errors.insert(path_label(path), format!("Minimum is {min}"));
                }
            }
            if let Some(max) = maximum {
                if n > *max {
                    errors.insert(path_label(path), format!("Maximum is {max}"));
                }
            }
        }
        FieldSpec::Boolean => {
            if !value.is_boolean() {
                errors.insert(path_label(path), "Expected a boolean".into());
            }
        }
        FieldSpec::Enum { options } => {
            let s = match value.as_str() {
                Some(s) => s,
                None => {
                    errors.insert(path_label(path), "Expected a string".into());
                    return;
                }
            };
            if !options.iter().any(|o| o == s) {
                errors.insert(path_label(path), "Invalid option".into());
            }
        }
        FieldSpec::Array { item, max_items } => {
            let arr = match value.as_array() {
                Some(a) => a,
                None => {
                    errors.insert(path_label(path), "Expected an array".into());
                    return;
                }
            };
            if arr.len() > *max_items {
                errors.insert(
                    path_label(path),
                    format!("Maximum {max_items} items allowed"),
                );
            }
            for (i, v) in arr.iter().enumerate() {
                validate_into(item, v, &format!("{path}[{i}]"), errors);
            }
        }
    }
}

fn path_label(path: &str) -> String {
    if path.is_empty() {
        "(root)".into()
    } else {
        path.to_string()
    }
}

fn join_path(base: &str, key: &str) -> String {
    if base.is_empty() {
        key.to_string()
    } else {
        format!("{base}.{key}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_tic_tac_toe_style_config() {
        let schema = json!({
            "oneOf": [
                { "type": "null" },
                {
                    "type": "object",
                    "required": ["side_length", "win_length"],
                    "properties": {
                        "side_length": { "type": "integer", "minimum": 2, "maximum": 20 },
                        "win_length": { "type": "integer", "minimum": 2, "maximum": 20 }
                    },
                    "additionalProperties": false
                }
            ]
        });
        let ok = json!({ "side_length": 3, "win_length": 3 });
        assert!(validate_against_schema(&schema, &ok).is_ok());
        let bad = json!({ "side_length": 1, "win_length": 3 });
        assert!(validate_against_schema(&schema, &bad).is_err());
    }
}
