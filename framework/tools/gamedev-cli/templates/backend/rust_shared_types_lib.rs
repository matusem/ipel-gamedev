use serde::{Deserialize, Serialize};

#[cfg(feature = "typegen")]
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
pub enum Player {
    Player1,
    Player2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
pub enum Move {
    Place { index: u8 },
}
