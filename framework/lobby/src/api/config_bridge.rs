use crate::models::{
    CONFIG_MSG_SOURCE, CONFIG_RESULT_SOURCE, CONFIG_SCHEMA_SOURCE, CONFIG_STATE_SOURCE,
};
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};

pub fn parse_iframe_config_message(data: &wasm_bindgen::JsValue) -> Option<(String, String)> {
    let s = js_sys::JSON::stringify(data).ok()?.as_string()?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    if v.get("source").and_then(|x| x.as_str()) != Some(CONFIG_MSG_SOURCE) {
        return None;
    }
    let game = v.get("game")?.as_str()?.to_string();
    let config_val = v.get("config")?;
    let config_str = if let Some(s) = config_val.as_str() {
        s.to_string()
    } else {
        config_val.to_string()
    };
    Some((game, config_str))
}

pub fn post_message_to_source(event: &web_sys::MessageEvent, origin: &str, payload: &JsValue) {
    let Some(src) = event.source() else {
        return;
    };
    let Ok(win) = JsValue::from(src).dyn_into::<web_sys::Window>() else {
        return;
    };
    let _ = win.post_message(payload, origin);
}

pub fn config_validation_reply(
    event: &web_sys::MessageEvent,
    origin: &str,
    game: &str,
    ok: bool,
    errors: &[String],
) {
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &obj,
        &JsValue::from_str("source"),
        &JsValue::from_str(CONFIG_RESULT_SOURCE),
    );
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("game"), &JsValue::from_str(game));
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("ok"), &JsValue::from_bool(ok));
    let arr = js_sys::Array::new();
    for e in errors {
        arr.push(&JsValue::from_str(e));
    }
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("errors"), &JsValue::from(arr));
    post_message_to_source(event, origin, &JsValue::from(obj));
}

pub fn post_config_schema_to_window(win: &web_sys::Window, origin: &str, game: &str, schema: &Value) {
    let Ok(schema_js) =
        js_sys::JSON::parse(&serde_json::to_string(schema).unwrap_or_else(|_| "{}".to_string()))
    else {
        return;
    };
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &obj,
        &JsValue::from_str("source"),
        &JsValue::from_str(CONFIG_SCHEMA_SOURCE),
    );
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("game"), &JsValue::from_str(game));
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("schema"), &schema_js);
    let _ = win.post_message(&JsValue::from(obj), origin);
}

pub fn post_config_state_to_window(
    win: &web_sys::Window,
    origin: &str,
    game: &str,
    config_json: &str,
) {
    let trimmed = config_json.trim();
    let config_js = if trimmed.is_empty() {
        JsValue::NULL
    } else {
        js_sys::JSON::parse(trimmed).unwrap_or(JsValue::NULL)
    };
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &obj,
        &JsValue::from_str("source"),
        &JsValue::from_str(CONFIG_STATE_SOURCE),
    );
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("game"), &JsValue::from_str(game));
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("config"), &config_js);
    let _ = win.post_message(&JsValue::from(obj), origin);
}
