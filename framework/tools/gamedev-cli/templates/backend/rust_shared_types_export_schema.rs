use std::fs;
use std::path::PathBuf;

use __SHARED_TYPES_CRATE__::{
    Action, Config, GameResult, Player, PlayerEvent, PlayerResult, PlayerState, State,
};
use schemars::schema_for;
use upjs_gdd_shared_types::contract::GameContract;
use upjs_gdd_shared_types::CONTRACT_WIT_VERSION;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap_or(&manifest_dir);
    let manifest_path = project_root.join("manifest.json");
    let (game_name, version) = if manifest_path.is_file() {
        let raw = fs::read_to_string(&manifest_path)?;
        let v: serde_json::Value = serde_json::from_str(&raw)?;
        (
            v.get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("game")
                .to_string(),
            v.get("version")
                .and_then(|x| x.as_str())
                .unwrap_or("0.1.0")
                .to_string(),
        )
    } else {
        ("game".to_string(), "0.1.0".to_string())
    };

    let definitions = serde_json::Map::from_iter([
        ("Player".to_string(), serde_json::to_value(schema_for!(Player))?),
        ("Config".to_string(), serde_json::to_value(schema_for!(Config))?),
        ("Action".to_string(), serde_json::to_value(schema_for!(Action))?),
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
            serde_json::to_value(schema_for!(GameResult))?,
        ),
        (
            "PlayerResult".to_string(),
            serde_json::to_value(schema_for!(PlayerResult))?,
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

    let schema_dir = manifest_dir.join("schema");
    fs::create_dir_all(&schema_dir)?;
    let contract_json = serde_json::to_string_pretty(&contract)?;
    fs::write(schema_dir.join("contract.json"), &contract_json)?;
    fs::write(schema_dir.join("game-types.json"), &contract_json)?;

    let gen = project_root.join("generated").join("schema");
    fs::create_dir_all(&gen)?;
    fs::write(gen.join("contract.json"), &contract_json)?;
    fs::write(gen.join("game-types.json"), &contract_json)?;

    println!("Generated contract.json at {}", schema_dir.display());
    Ok(())
}
