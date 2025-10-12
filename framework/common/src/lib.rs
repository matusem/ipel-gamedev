use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub type SerializedBuffer = Vec<u8>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum SerializationFormat {
    #[default]
    Json,
    Rmp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingInput<T: ProcessingTransaction> {
    pub format: SerializationFormat,
    pub payload: T::Input,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingError {
    SerializationError(String),
    DeserializationError(String),
    ProcessingError(String),
    GameCoreError(SerializedBuffer),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingOutput<T: ProcessingTransaction>(
    pub Result<<T as ProcessingTransaction>::Output, ProcessingError>,
);

pub trait ProcessingTransaction {
    type Input: Debug + Clone + Serialize + for<'de> Deserialize<'de>;
    type Output: Debug + Clone + Serialize + for<'de> Deserialize<'de>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Game {
    pub state: SerializedBuffer,
    pub player_states: SerializedBuffer,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TryInitInput {
    pub config: SerializedBuffer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TryInit;
impl ProcessingTransaction for TryInit {
    type Input = TryInitInput;
    type Output = Game;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TryTakeAction;
impl ProcessingTransaction for TryTakeAction {
    type Input = TryTakeActionInput;
    type Output = TryTakeActionOutput;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TryTakeActionInput {
    pub game: Game,
    pub player: SerializedBuffer,
    pub action: SerializedBuffer,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TryTakeActionOutput {
    pub game: Game,
    pub player_events: Vec<(SerializedBuffer, SerializedBuffer)>,
}
