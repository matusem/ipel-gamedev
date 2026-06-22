//! Per-game type contract ("type link") — JSON Schema bundle served by the platform.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// WIT / platform contract version this schema bundle targets.
pub const CONTRACT_WIT_VERSION: &str = "game-core-v2";

/// Required JSON Schema definition slots every published game must export.
pub const CONTRACT_SLOTS: &[&str] = &[
    "Player",
    "Config",
    "Action",
    "State",
    "PlayerState",
    "PlayerEvent",
    "GameResult",
    "PlayerResult",
];

/// Backend-owned envelope for `contract.json` shipped with every published game.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameContract {
    pub game: String,
    pub version: String,
    pub wit_version: String,
    pub contract_hash: String,
    /// JSON Schema draft-07 document with a `definitions` map for each slot.
    pub schema: Value,
}

impl GameContract {
    /// Build a contract from a definitions-only schema bundle and metadata.
    pub fn from_definitions(
        game: impl Into<String>,
        version: impl Into<String>,
        wit_version: impl Into<String>,
        definitions: Map<String, Value>,
    ) -> Self {
        let mut schema = Map::new();
        schema.insert(
            "$schema".to_string(),
            Value::String("http://json-schema.org/draft-07/schema#".to_string()),
        );
        schema.insert(
            "title".to_string(),
            Value::String("GameTypeContract".to_string()),
        );
        schema.insert("definitions".to_string(), Value::Object(definitions));
        let schema = Value::Object(schema);
        let contract_hash = compute_contract_hash(&schema);
        Self {
            game: game.into(),
            version: version.into(),
            wit_version: wit_version.into(),
            contract_hash,
            schema,
        }
    }

    pub fn definitions(&self) -> Option<&Map<String, Value>> {
        self.schema.get("definitions")?.as_object()
    }

    /// Validate that all required slots are present in `definitions`.
    pub fn validate_slots(&self) -> Result<(), Vec<String>> {
        let mut missing = Vec::new();
        let defs = self.definitions().cloned().unwrap_or_default();
        for slot in CONTRACT_SLOTS {
            if !defs.contains_key(*slot) {
                missing.push((*slot).to_string());
            }
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}

/// Stable SHA-256 hex digest of the canonical `definitions` object (sorted keys).
pub fn compute_contract_hash(schema: &Value) -> String {
    let defs = schema
        .get("definitions")
        .cloned()
        .unwrap_or(Value::Object(Map::new()));
    let canonical = canonical_json(&defs);
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Recompute hash from an existing contract document (ignores stored `contract_hash`).
pub fn recompute_contract_hash(schema: &Value) -> String {
    compute_contract_hash(schema)
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap(),
                        canonical_json(&map[*k])
                    )
                })
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(arr) => {
            let inner: Vec<String> = arr.iter().map(canonical_json).collect();
            format!("[{}]", inner.join(","))
        }
        other => serde_json::to_string(other).unwrap_or_else(|_| "null".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_hash_is_stable() {
        let mut defs = Map::new();
        defs.insert(
            "Player".to_string(),
            serde_json::json!({ "type": "string" }),
        );
        defs.insert(
            "Action".to_string(),
            serde_json::json!({ "type": "array" }),
        );
        let c = GameContract::from_definitions("tic_tac_toe", "1.0.0", CONTRACT_WIT_VERSION, defs);
        let h1 = c.contract_hash.clone();
        let h2 = compute_contract_hash(&c.schema);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn validate_slots_reports_missing() {
        let mut defs = Map::new();
        defs.insert("Player".to_string(), serde_json::json!({ "type": "string" }));
        let c = GameContract::from_definitions("g", "0.1.0", CONTRACT_WIT_VERSION, defs);
        let err = c.validate_slots().unwrap_err();
        assert!(err.contains(&"Action".to_string()));
    }
}
