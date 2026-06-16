use bevy::prelude::*;
use shared_types::Player;
use upjs_gdd_bevy::{FrameworkGamePlayPlugin, GamePlayStatus, GameStateReceived};

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(FrameworkGamePlayPlugin)
        .add_systems(Startup, startup_log)
        .add_systems(Update, (on_status, on_state))
        .run();
}

fn startup_log() {
    info!("Bevy play client — waiting for lobby connection ({:?})", Player::Player1);
}

fn on_status(mut reader: EventReader<GamePlayStatus>) {
    for GamePlayStatus(s) in reader.read() {
        info!("play: {s}");
    }
}

fn on_state(mut reader: EventReader<GameStateReceived>) {
    for GameStateReceived(v) in reader.read() {
        info!("state: {v}");
    }
}
