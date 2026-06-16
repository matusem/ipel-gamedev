use bevy::prelude::*;

mod play;

pub use play::{
    FrameworkGamePlayPlugin, GameEventReceived, GamePlayResource, GamePlayStatus,
    GameStateReceived, send_play_action,
};

#[derive(Resource)]
pub struct RealtimeResource {
    pub client: upjs_gdd_rust_shared::realtime::RealtimeClient,
}

pub struct FrameworkBevySdkPlugin {
    pub ws_url: String,
    pub bearer_token: String,
}

impl Plugin for FrameworkBevySdkPlugin {
    fn build(&self, app: &mut App) {
        let cfg = upjs_gdd_rust_shared::realtime::RealtimeConfig {
            ws_url: self.ws_url.clone(),
            bearer_token: self.bearer_token.clone(),
            ..upjs_gdd_rust_shared::realtime::RealtimeConfig::default()
        };
        app.insert_resource(RealtimeResource {
            client: upjs_gdd_rust_shared::realtime::RealtimeClient::new(cfg),
        });
    }
}
