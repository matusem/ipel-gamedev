//! Bot logic — implement [`bot::Bot`] using generated types.

use bot::Bot;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Per-player view from the game type contract.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlayerState(pub Value);

/// Action sent to the game server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action(pub Value);

/// Bot tuning parameters (edit schema in `settings_schema_json`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub aggression: f64,
}

pub struct __BOT_LOGIC_NAME__;

impl Bot for __BOT_LOGIC_NAME__ {
    type Settings = Settings;
    type PlayerState = PlayerState;
    type Action = Action;

    fn settings_schema_json() -> &'static str {
        r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "aggression": {
      "type": "number",
      "title": "Aggression",
      "description": "0 = passive, 1 = aggressive",
      "minimum": 0,
      "maximum": 1,
      "default": 0.5
    }
  }
}"#
    }

    fn decide(_settings: &Settings, view: &PlayerState) -> Option<Action> {
        let _ = view;
        // TODO: implement your bot strategy
        None
    }
}
