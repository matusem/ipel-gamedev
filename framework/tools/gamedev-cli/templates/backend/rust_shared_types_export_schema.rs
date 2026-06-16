use std::fs;
use std::path::PathBuf;

use __SHARED_TYPES_CRATE__::{
    Action, Config, GameResult, Player, PlayerEvent, PlayerResult, PlayerState, State,
};
use schemars::schema_for;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema");
    fs::create_dir_all(&schema_dir)?;
    let bundle = serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "GameTypes",
        "definitions": {
            "Player": schema_for!(Player),
            "Config": schema_for!(Config),
            "Action": schema_for!(Action),
            "State": schema_for!(State),
            "PlayerState": schema_for!(PlayerState),
            "PlayerEvent": schema_for!(PlayerEvent),
            "GameResult": schema_for!(GameResult),
            "PlayerResult": schema_for!(PlayerResult),
        }
    });
    fs::write(
        schema_dir.join("game-types.json"),
        serde_json::to_string_pretty(&bundle)?,
    )?;
    let gen = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../generated/schema");
    fs::create_dir_all(&gen)?;
    fs::write(
        gen.join("game-types.json"),
        serde_json::to_string_pretty(&bundle)?,
    )?;
    println!("Generated JSON Schema at {}", schema_dir.display());
    Ok(())
}
