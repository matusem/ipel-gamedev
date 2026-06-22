use serde_json::Value;

/// Parsed field spec for the supported JSON Schema subset.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldSpec {
    Fallback,
    OptionalObject {
        inner: Box<FieldSpec>,
    },
    Object {
        properties: Vec<(String, FieldSpec)>,
        required: Vec<String>,
    },
    String {
        title: Option<String>,
        description: Option<String>,
        min_length: Option<u64>,
        max_length: Option<u64>,
        default: Option<String>,
    },
    Integer {
        title: Option<String>,
        description: Option<String>,
        minimum: Option<i64>,
        maximum: Option<i64>,
        default: Option<i64>,
    },
    Number {
        title: Option<String>,
        description: Option<String>,
        minimum: Option<f64>,
        maximum: Option<f64>,
        default: Option<f64>,
    },
    Boolean {
        title: Option<String>,
        description: Option<String>,
        default: Option<bool>,
    },
    Enum {
        title: Option<String>,
        description: Option<String>,
        options: Vec<String>,
        default: Option<String>,
    },
    Array {
        title: Option<String>,
        description: Option<String>,
        item: Box<FieldSpec>,
        max_items: usize,
    },
}

const MAX_ARRAY_ITEMS: usize = 32;

pub fn parse_schema(schema: &Value) -> FieldSpec {
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
        if options.is_empty() {
            return None;
        }
        return Some(FieldSpec::Enum {
            title: title_of(schema),
            description: description_of(schema),
            options,
            default: schema.get("default").and_then(|v| v.as_str().map(str::to_string)),
        });
    }
    if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
        let options: Vec<String> = one_of
            .iter()
            .filter_map(|v| v.get("const").and_then(|c| c.as_str().map(str::to_string)))
            .collect();
        if !options.is_empty() {
            return Some(FieldSpec::Enum {
                title: title_of(schema),
                description: description_of(schema),
                options,
                default: schema.get("default").and_then(|v| v.as_str().map(str::to_string)),
            });
        }
    }
    match schema.get("type").and_then(|v| v.as_str()) {
        Some("string") => Some(FieldSpec::String {
            title: title_of(schema),
            description: description_of(schema),
            min_length: schema.get("minLength").and_then(|v| v.as_u64()),
            max_length: schema.get("maxLength").and_then(|v| v.as_u64()),
            default: schema.get("default").and_then(|v| v.as_str().map(str::to_string)),
        }),
        Some("integer") => Some(FieldSpec::Integer {
            title: title_of(schema),
            description: description_of(schema),
            minimum: schema.get("minimum").and_then(|v| v.as_i64()),
            maximum: schema.get("maximum").and_then(|v| v.as_i64()),
            default: schema.get("default").and_then(|v| v.as_i64()),
        }),
        Some("number") => Some(FieldSpec::Number {
            title: title_of(schema),
            description: description_of(schema),
            minimum: schema.get("minimum").and_then(|v| v.as_f64()),
            maximum: schema.get("maximum").and_then(|v| v.as_f64()),
            default: schema.get("default").and_then(|v| v.as_f64()),
        }),
        Some("boolean") => Some(FieldSpec::Boolean {
            title: title_of(schema),
            description: description_of(schema),
            default: schema.get("default").and_then(|v| v.as_bool()),
        }),
        Some("array") => {
            let item_schema = schema.get("items")?;
            let item = parse_leaf(item_schema).unwrap_or_else(|| parse_schema(item_schema));
            if matches!(item, FieldSpec::Fallback | FieldSpec::OptionalObject { .. } | FieldSpec::Object { .. } | FieldSpec::Array { .. }) {
                return None;
            }
            Some(FieldSpec::Array {
                title: title_of(schema),
                description: description_of(schema),
                item: Box::new(item),
                max_items: MAX_ARRAY_ITEMS,
            })
        }
        _ => None,
    }
}

fn title_of(schema: &Value) -> Option<String> {
    schema.get("title").and_then(|v| v.as_str().map(str::to_string))
}

fn description_of(schema: &Value) -> Option<String> {
    schema
        .get("description")
        .and_then(|v| v.as_str().map(str::to_string))
}

pub fn default_value_for_spec(spec: &FieldSpec) -> Value {
    match spec {
        FieldSpec::Fallback => Value::Null,
        FieldSpec::OptionalObject { inner } => default_value_for_spec(inner),
        FieldSpec::Object { properties, .. } => {
            let mut map = serde_json::Map::new();
            for (k, p) in properties {
                map.insert(k.clone(), default_value_for_spec(p));
            }
            Value::Object(map)
        }
        FieldSpec::String { default, .. } => {
            Value::String(default.clone().unwrap_or_default())
        }
        FieldSpec::Integer { default, .. } => {
            Value::Number(default.unwrap_or(0).into())
        }
        FieldSpec::Number { default, .. } => serde_json::Number::from_f64(default.unwrap_or(0.0))
            .map(Value::Number)
            .unwrap_or(Value::Null),
        FieldSpec::Boolean { default, .. } => Value::Bool(default.unwrap_or(false)),
        FieldSpec::Enum { default, options, .. } => Value::String(
            default
                .clone()
                .or_else(|| options.first().cloned())
                .unwrap_or_default(),
        ),
        FieldSpec::Array { .. } => Value::Array(vec![]),
    }
}

pub fn coerce_initial_value(spec: &FieldSpec, raw: &Value) -> Value {
    if raw.is_null() {
        return default_value_for_spec(spec);
    }
    if matches!(spec, FieldSpec::OptionalObject { .. }) && raw.is_null() {
        return Value::Null;
    }
    raw.clone()
}
