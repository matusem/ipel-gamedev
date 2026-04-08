use bevy::prelude::*;
use shared_types::Player;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, startup_log)
        .run();
}

fn startup_log() {
    info!(
        "Tic-Tac-Toe (3x3) starter loaded; first player is {:?}",
        Player::Player1
    );
}
