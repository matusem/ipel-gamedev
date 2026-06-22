use std::fs;
use std::path::PathBuf;

use schemars::schema_for;
use tic_tac_toe::{
    Config, GameOutcome, Player, PlayerEvent, PlayerOutcome, PlayerState, Position, State,
};
use upjs_gdd_shared_types::contract::GameContract;
use upjs_gdd_shared_types::CONTRACT_WIT_VERSION;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let game_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("game root");
    let manifest_path = game_root.join("manifest.json");
    let (game_name, version) = if manifest_path.is_file() {
        let raw = fs::read_to_string(&manifest_path)?;
        let v: serde_json::Value = serde_json::from_str(&raw)?;
        (
            v.get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("tic_tac_toe")
                .to_string(),
            v.get("version")
                .and_then(|x| x.as_str())
                .unwrap_or("1.0.0")
                .to_string(),
        )
    } else {
        ("tic_tac_toe".to_string(), "1.0.0".to_string())
    };

    let definitions = serde_json::Map::from_iter([
        ("Player".to_string(), serde_json::to_value(schema_for!(Player))?),
        ("Config".to_string(), serde_json::to_value(schema_for!(Config))?),
        ("Action".to_string(), serde_json::to_value(schema_for!(Position))?),
        ("State".to_string(), serde_json::to_value(schema_for!(State))?),
        (
            "PlayerState".to_string(),
            serde_json::to_value(schema_for!(PlayerState))?,
        ),
        (
            "PlayerEvent".to_string(),
            serde_json::to_value(schema_for!(PlayerEvent))?,
        ),
        (
            "GameResult".to_string(),
            serde_json::to_value(schema_for!(GameOutcome))?,
        ),
        (
            "PlayerResult".to_string(),
            serde_json::to_value(schema_for!(PlayerOutcome))?,
        ),
    ]);

    let contract = GameContract::from_definitions(
        game_name,
        version,
        CONTRACT_WIT_VERSION,
        definitions,
    );
    contract.validate_slots().map_err(|missing| {
        format!("contract missing required slots: {}", missing.join(", "))
    })?;

    let contract_json = serde_json::to_string_pretty(&contract)?;
    fs::write(game_root.join("contract.json"), &contract_json)?;

    println!(
        "Generated contract.json at {} (hash={})",
        game_root.join("contract.json").display(),
        contract.contract_hash
    );
    Ok(())
}
