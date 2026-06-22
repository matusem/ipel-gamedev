use serde_json::Value;
use std::collections::BTreeMap;

use super::model::FieldSpec;

pub type FieldErrors = BTreeMap<String, String>;

pub fn validate_value(spec: &FieldSpec, value: &Value, path: &str) -> FieldErrors {
    let mut errors = FieldErrors::new();
    validate_into(spec, value, path, &mut errors);
    errors
}

fn validate_into(spec: &FieldSpec, value: &Value, path: &str, errors: &mut FieldErrors) {
    match spec {
        FieldSpec::Fallback => {}
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
                    errors.insert(path.to_string(), "Expected an object".into());
                    return;
                }
            };
            for key in required {
                if !obj.contains_key(key) || obj.get(key).is_some_and(|v| v.is_null()) {
                    let p = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    errors.insert(p, "Required field".into());
                }
            }
            for (key, sub) in properties {
                if let Some(v) = obj.get(key) {
                    let p = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    validate_into(sub, v, &p, errors);
                }
            }
        }
        FieldSpec::String {
            min_length,
            max_length,
            ..
        } => {
            let s = match value.as_str() {
                Some(s) => s,
                None => {
                    errors.insert(path.to_string(), "Expected a string".into());
                    return;
                }
            };
            if let Some(min) = min_length {
                if (s.len() as u64) < *min {
                    errors.insert(path.to_string(), format!("Minimum length is {min}"));
                }
            }
            if let Some(max) = max_length {
                if (s.len() as u64) > *max {
                    errors.insert(path.to_string(), format!("Maximum length is {max}"));
                }
            }
        }
        FieldSpec::Integer {
            minimum, maximum, ..
        } => {
            let n = match value.as_i64() {
                Some(n) => n,
                None => {
                    errors.insert(path.to_string(), "Expected an integer".into());
                    return;
                }
            };
            if let Some(min) = minimum {
                if n < *min {
                    errors.insert(path.to_string(), format!("Minimum is {min}"));
                }
            }
            if let Some(max) = maximum {
                if n > *max {
                    errors.insert(path.to_string(), format!("Maximum is {max}"));
                }
            }
        }
        FieldSpec::Number {
            minimum, maximum, ..
        } => {
            let n = match value.as_f64() {
                Some(n) => n,
                None => {
                    errors.insert(path.to_string(), "Expected a number".into());
                    return;
                }
            };
            if let Some(min) = minimum {
                if n < *min {
                    errors.insert(path.to_string(), format!("Minimum is {min}"));
                }
            }
            if let Some(max) = maximum {
                if n > *max {
                    errors.insert(path.to_string(), format!("Maximum is {max}"));
                }
            }
        }
        FieldSpec::Boolean { .. } => {
            if !value.is_boolean() {
                errors.insert(path.to_string(), "Expected a boolean".into());
            }
        }
        FieldSpec::Enum { options, .. } => {
            let s = match value.as_str() {
                Some(s) => s,
                None => {
                    errors.insert(path.to_string(), "Expected a string".into());
                    return;
                }
            };
            if !options.iter().any(|o| o == s) {
                errors.insert(path.to_string(), "Invalid option".into());
            }
        }
        FieldSpec::Array {
            item,
            max_items,
            ..
        } => {
            let arr = match value.as_array() {
                Some(a) => a,
                None => {
                    errors.insert(path.to_string(), "Expected an array".into());
                    return;
                }
            };
            if arr.len() > *max_items {
                errors.insert(
                    path.to_string(),
                    format!("Maximum {max_items} items allowed"),
                );
            }
            for (i, v) in arr.iter().enumerate() {
                let p = format!("{path}[{i}]");
                validate_into(item, v, &p, errors);
            }
        }
    }
}

pub fn first_error_message(errors: &FieldErrors) -> Option<String> {
    errors.values().next().cloned()
}

pub fn is_valid(errors: &FieldErrors) -> bool {
    errors.is_empty()
}
