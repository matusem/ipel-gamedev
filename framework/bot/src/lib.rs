//! Bot decision trait — implement in bot logic crates.

pub trait Bot {
    type Settings: serde::de::DeserializeOwned + serde::Serialize + Default;
    type PlayerState: serde::de::DeserializeOwned + serde::Serialize;
    type Action: serde::de::DeserializeOwned + serde::Serialize;

    /// Default settings when none are configured.
    fn default_settings() -> Self::Settings {
        Self::Settings::default()
    }

    /// Return serialized error bytes if settings are invalid.
    fn validate_settings(_settings: &Self::Settings) -> Option<Vec<u8>> {
        None
    }

    /// JSON Schema (draft-07 subset) describing [`Self::Settings`].
    fn settings_schema_json() -> &'static str {
        r#"{"type":"object","additionalProperties":false}"#
    }

    /// Given bot settings and the current per-player view, return the next action or `None` to wait.
    fn decide(settings: &Self::Settings, view: &Self::PlayerState) -> Option<Self::Action>;
}
