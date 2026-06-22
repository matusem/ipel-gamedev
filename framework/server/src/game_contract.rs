//! Validate and load per-game type contracts (`contract.json`).

use std::path::{Path, PathBuf};

use upjs_gdd_shared_types::contract::GameContract;
use upjs_gdd_shared_types::{CONTRACT_WIT_VERSION, recompute_contract_hash};

pub fn validate_contract_json(raw: &str) -> Result<GameContract, String> {
    let contract: GameContract =
        serde_json::from_str(raw).map_err(|e| format!("contract.json parse error: {e}"))?;
    validate_contract(&contract)?;
    Ok(contract)
}

pub fn validate_contract(contract: &GameContract) -> Result<(), String> {
    if contract.wit_version != CONTRACT_WIT_VERSION {
        return Err(format!(
            "contract wit_version {:?} does not match platform {:?}",
            contract.wit_version, CONTRACT_WIT_VERSION
        ));
    }
    contract
        .validate_slots()
        .map_err(|missing| format!("contract missing required type slots: {}", missing.join(", ")))?;
    let expected = recompute_contract_hash(&contract.schema);
    if contract.contract_hash != expected {
        return Err(format!(
            "contract_hash mismatch: stored {:?}, computed {:?}",
            contract.contract_hash, expected
        ));
    }
    Ok(())
}

pub fn load_contract_from_dir(dir: &Path) -> Result<GameContract, String> {
    let path = dir.join("contract.json");
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    validate_contract_json(&raw)
}

pub fn contract_path_for_game(games_dir: &Path, slug: &str) -> PathBuf {
    games_dir.join(slug).join("contract.json")
}

pub fn load_contract_for_game(games_dir: &Path, slug: &str) -> Option<GameContract> {
    load_contract_from_dir(&games_dir.join(slug)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    #[test]
    fn rejects_hash_mismatch() {
        let mut defs = Map::new();
        defs.insert("Player".into(), serde_json::json!({"type":"string"}));
        defs.insert("Config".into(), serde_json::json!({"type":"object"}));
        defs.insert("Action".into(), serde_json::json!({"type":"object"}));
        defs.insert("State".into(), serde_json::json!({"type":"object"}));
        defs.insert("PlayerState".into(), serde_json::json!({"type":"object"}));
        defs.insert("PlayerEvent".into(), serde_json::json!({"type":"object"}));
        defs.insert("GameResult".into(), serde_json::json!({"type":"object"}));
        defs.insert("PlayerResult".into(), serde_json::json!({"type":"object"}));
        let mut c = GameContract::from_definitions("g", "1", CONTRACT_WIT_VERSION, defs);
        c.contract_hash = "bad".into();
        assert!(validate_contract(&c).is_err());
    }
}
