use dioxus::prelude::*;
use framework_sdk_rust_shared::realtime::{RealtimeClient, RealtimeConfig};

#[derive(Clone)]
pub struct RealtimeController {
    pub client: Signal<RealtimeClient>,
}

pub fn use_realtime_controller(ws_url: String, bearer_token: String) -> RealtimeController {
    let cfg = RealtimeConfig {
        ws_url,
        bearer_token,
        ..RealtimeConfig::default()
    };
    let client = use_signal(|| RealtimeClient::new(cfg));
    RealtimeController { client }
}
