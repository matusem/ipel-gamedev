use std::fs;
use std::path::PathBuf;

use __SHARED_TYPES_CRATE__::{
    Action, Config, GameResult, Player, PlayerEvent, PlayerResult, PlayerState, State,
};
use ts_rs::TS;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../frontend/web/src/generated");
    fs::create_dir_all(&out)?;
    for export in [
        Player::export_to(&out),
        Config::export_to(&out),
        Action::export_to(&out),
        State::export_to(&out),
        PlayerState::export_to(&out),
        PlayerEvent::export_to(&out),
        GameResult::export_to(&out),
        PlayerResult::export_to(&out),
    ] {
        export?;
    }
    println!("Generated TS types at {}", out.display());
    Ok(())
}
