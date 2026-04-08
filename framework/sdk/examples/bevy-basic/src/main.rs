use bevy::prelude::*;
use framework_sdk_bevy::FrameworkBevySdkPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FrameworkBevySdkPlugin {
            ws_url: "ws://localhost:8080/graphql".to_string(),
            bearer_token: "dev-token".to_string(),
        })
        .run();
}
