//! `/game` WebSocket play protocol (browser WASM). GraphQL realtime lives in `realtime.rs`.

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PlayClientConfig {
    pub ws_url: String,
    pub game_id: String,
    pub player: String,
}

impl PlayClientConfig {
    /// Build from lobby iframe query params: `ws`, `id`, `player`.
    #[cfg(target_arch = "wasm32")]
    pub fn from_window_location() -> Option<Self> {
        let window = web_sys::window()?;
        let search = window.location().search().ok()?;
        let params: std::collections::HashMap<String, String> =
            serde_urlencoded::from_str(search.trim_start_matches('?')).ok()?;
        let ws = params.get("ws")?.clone();
        let id = params.get("id")?.clone();
        let player = params.get("player")?.clone();
        if ws.is_empty() || id.is_empty() || player.is_empty() {
            return None;
        }
        Some(Self {
            ws_url: ws,
            game_id: id,
            player,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_window_location() -> Option<Self> {
        None
    }

    pub fn socket_url(&self) -> String {
        format!(
            "{}?id={}&player={}",
            self.ws_url,
            urlencoding::encode(&self.game_id),
            urlencoding::encode(&self.player)
        )
    }
}

/// Callback-driven play client matching `@upjs-gdd/game-sdk` semantics.
pub struct PlayClient {
    #[cfg(target_arch = "wasm32")]
    ws: Option<web_sys::WebSocket>,
}

impl Default for PlayClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayClient {
    pub fn new() -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            ws: None,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn connect(
        &mut self,
        cfg: &PlayClientConfig,
        mut on_state: impl FnMut(Value) + 'static,
        mut on_event: impl FnMut(Value) + 'static,
        mut on_status: impl FnMut(&str) + 'static,
    ) {
        use wasm_bindgen::JsCast;
        let url = cfg.socket_url();
        let Ok(ws) = web_sys::WebSocket::new(&url) else {
            on_status("WebSocket new() failed");
            return;
        };
        self.ws = Some(ws.clone());

        let onopen = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            on_status("Connected");
        });
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        onopen.forget();

        let mut first = false;
        let onmsg = wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(
            move |e: web_sys::MessageEvent| {
                let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() else {
                    return;
                };
                let s = String::from(txt);
                let Ok(v) = serde_json::from_str::<Value>(&s) else {
                    return;
                };
                if !first {
                    first = true;
                    on_state(v);
                } else {
                    on_event(v);
                }
            },
        );
        ws.set_onmessage(Some(onmsg.as_ref().unchecked_ref()));
        onmsg.forget();

        let onerr = wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |_e: web_sys::Event| {
            on_status("WebSocket error");
        });
        ws.set_onerror(Some(onerr.as_ref().unchecked_ref()));
        onerr.forget();
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn connect(
        &mut self,
        _cfg: &PlayClientConfig,
        _on_state: impl FnMut(Value) + 'static,
        _on_event: impl FnMut(Value) + 'static,
        mut on_status: impl FnMut(&str) + 'static,
    ) {
        on_status("PlayClient requires wasm32 target");
    }

    #[cfg(target_arch = "wasm32")]
    pub fn send_action_json(&self, json: &str) -> Result<(), String> {
        let Some(ws) = &self.ws else {
            return Err("not connected".into());
        };
        ws.send_with_str(json).map_err(|e| format!("{e:?}"))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn send_action_json(&self, _json: &str) -> Result<(), String> {
        Err("not connected".into())
    }

    pub fn send_action_value(&self, action: &Value) -> Result<(), String> {
        let json = serde_json::to_string(action).map_err(|e| e.to_string())?;
        self.send_action_json(&json)
    }
}
