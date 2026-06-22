//! Shared WebAssembly host for `Bot` implementations (wit `game-bot` world).

#![allow(warnings)]

pub mod bindings;

pub use bindings::{BotError, Buffer, SerializationFormat};

use bindings::Guest;
use bot::Bot;
use std::marker::PhantomData;

mod serialization;
use serialization::{get_deserialize as de, get_serialize as se};

pub struct MyBotHost<BotT: Bot> {
    pub _marker: PhantomData<BotT>,
}

impl<BotT: Bot> Guest for MyBotHost<BotT> {
    fn default_settings(format: SerializationFormat) -> Result<Buffer, BotError> {
        let settings = BotT::default_settings();
        se(format)(&settings).map_err(BotError::Serialize)
    }

    fn validate_settings(
        format: SerializationFormat,
        settings: Buffer,
    ) -> Result<Option<Buffer>, BotError> {
        let parsed: BotT::Settings = de(format)(&settings).map_err(BotError::Deserialize)?;
        Ok(BotT::validate_settings(&parsed))
    }

    fn settings_schema() -> String {
        BotT::settings_schema_json().to_string()
    }

    fn decide(
        format: SerializationFormat,
        settings: Buffer,
        player_state: Buffer,
    ) -> Result<Option<Buffer>, BotError> {
        let settings: BotT::Settings = de(format)(&settings).map_err(BotError::Deserialize)?;
        if let Some(err) = BotT::validate_settings(&settings) {
            return Err(BotError::BotCore(err));
        }

        let view: BotT::PlayerState = de(format)(&player_state).map_err(BotError::Deserialize)?;

        match BotT::decide(&settings, &view) {
            Some(action) => {
                let buf = se(format)(&action).map_err(BotError::Serialize)?;
                Ok(Some(buf))
            }
            None => Ok(None),
        }
    }
}

#[macro_export]
macro_rules! export_game_bot {
    ($ty:ident) => {
        $crate::__export_world_game_bot_cabi!($ty with_types_in $crate::bindings);
    };
}
