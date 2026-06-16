//! Canonical game types — source of truth for codegen (TS, JSON Schema, Rust clients).

use serde::{Deserialize, Serialize};

#[cfg(feature = "typegen")]
use ts_rs::TS;
#[cfg(feature = "schemars")]
use schemars::JsonSchema;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub enum Player {
    Player1,
    Player2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct Config {
    pub side_length: u8,
    pub win_length: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            side_length: 3,
            win_length: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub enum Action {
    Place { row: u8, col: u8 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct State {
    pub board: Vec<Option<Player>>,
    pub current_player: Player,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct PlayerState {
    pub player: Player,
    pub view: State,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct PlayerEvent {
    pub player: Player,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub enum GameResult {
    Win { winner: Player },
    Draw,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct PlayerResult {
    pub player: Player,
    pub outcome: String,
    pub score: f64,
}
