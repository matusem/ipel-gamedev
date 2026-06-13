use bevy::prelude::*;
use upjs_gdd_rust_shared::realtime::{RealtimeClient, RealtimeConfig};

#[derive(Resource)]
pub struct RealtimeResource {
    pub client: RealtimeClient,
}

pub struct FrameworkBevySdkPlugin {
    pub ws_url: String,
    pub bearer_token: String,
}

impl Plugin for FrameworkBevySdkPlugin {
    fn build(&self, app: &mut App) {
        let cfg = RealtimeConfig {
            ws_url: self.ws_url.clone(),
            bearer_token: self.bearer_token.clone(),
            ..RealtimeConfig::default()
        };
        app.insert_resource(RealtimeResource {
            client: RealtimeClient::new(cfg),
        });
    }
}
