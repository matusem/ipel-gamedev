//! Bot settings validation and effective-settings resolution.

use crate::bot_core;
use crate::component_db::ComponentDb;
use crate::settings_validation;
use serde_json::Value;

pub async fn bot_settings_schema_wasm(
    component_db: &ComponentDb,
    bot_slug: &str,
) -> Result<String, String> {
    let (bot, mut store) = component_db.create_game_bot(bot_slug).await?;
    bot.call_settings_schema(&mut store)
        .await
        .map_err(|e| format!("wasm settings-schema: {e}"))
}

pub async fn bot_default_settings_wasm(
    component_db: &ComponentDb,
    bot_slug: &str,
) -> Result<Vec<u8>, String> {
    let (bot, mut store) = component_db.create_game_bot(bot_slug).await?;
    let result = bot
        .call_default_settings(&mut store, bot_core::SerializationFormat::Json)
        .await
        .map_err(|e| format!("wasm default-settings: {e}"))?;
    match result {
        Ok(buf) => Ok(buf),
        Err(e) => Err(format!("bot default-settings error: {e:?}")),
    }
}

pub async fn bot_validate_settings_wasm(
    component_db: &ComponentDb,
    bot_slug: &str,
    settings: &[u8],
) -> Result<(), String> {
    let (bot, mut store) = component_db.create_game_bot(bot_slug).await?;
    let result = bot
        .call_validate_settings(
            &mut store,
            bot_core::SerializationFormat::Json,
            &settings.to_vec(),
        )
        .await
        .map_err(|e| format!("wasm validate-settings: {e}"))?;
    match result {
        Ok(None) => Ok(()),
        Ok(Some(err)) => Err(String::from_utf8_lossy(&err).into_owned()),
        Err(e) => Err(bot_error_message(&e)),
    }
}

fn bot_error_message(e: &bot_core::BotError) -> String {
    match e {
        bot_core::BotError::Deserialize(s) => format!("deserialize: {s}"),
        bot_core::BotError::Serialize(s) => format!("serialize: {s}"),
        bot_core::BotError::Processing(s) => format!("processing: {s}"),
        bot_core::BotError::BotCore(buf) => String::from_utf8_lossy(buf).into_owned(),
    }
}

pub fn validate_settings_json(schema_json: &str, settings_json: &str) -> Result<(), String> {
    let schema: Value =
        serde_json::from_str(schema_json).map_err(|e| format!("invalid settings schema: {e}"))?;
    let instance: Value = serde_json::from_str(settings_json)
        .map_err(|e| format!("invalid settings json: {e}"))?;
    settings_validation::validate_against_schema(&schema, &instance)
        .map_err(|errs| errs.join("; "))
}

pub async fn validate_bot_settings(
    component_db: &ComponentDb,
    category: &str,
    bot_slug: Option<&str>,
    schema_json: Option<&str>,
    settings_json: &str,
) -> Result<(), String> {
    if let Some(schema) = schema_json.filter(|s| !s.trim().is_empty()) {
        validate_settings_json(schema, settings_json)?;
    }
    if category == "published" {
        let slug = bot_slug.ok_or_else(|| "bot slug required".to_string())?;
        bot_validate_settings_wasm(component_db, slug, settings_json.as_bytes()).await?;
    }
    Ok(())
}

/// Seat override -> bot registry default -> WASM core default (published only).
pub async fn resolve_effective_settings_json(
    component_db: &ComponentDb,
    category: &str,
    bot_slug: Option<&str>,
    seat_override: Option<&str>,
    bot_default: Option<&str>,
) -> Result<String, String> {
    if let Some(s) = seat_override.filter(|x| !x.trim().is_empty()) {
        return Ok(s.to_string());
    }
    if let Some(s) = bot_default.filter(|x| !x.trim().is_empty()) {
        return Ok(s.to_string());
    }
    if category == "published" {
        let slug = bot_slug.ok_or_else(|| "bot slug required".to_string())?;
        let bytes = bot_default_settings_wasm(component_db, slug).await?;
        return String::from_utf8(bytes).map_err(|e| format!("invalid utf8 default settings: {e}"));
    }
    Ok("null".to_string())
}

pub async fn resolve_effective_settings_bytes(
    component_db: &ComponentDb,
    category: &str,
    bot_slug: Option<&str>,
    seat_override: Option<&str>,
    bot_default: Option<&str>,
) -> Result<Vec<u8>, String> {
    let json = resolve_effective_settings_json(
        component_db,
        category,
        bot_slug,
        seat_override,
        bot_default,
    )
    .await?;
    Ok(json.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_validation_rejects_bad_enum() {
        let schema = r#"{"type":"object","properties":{"mode":{"type":"string","enum":["a","b"]}}}"#;
        let bad = r#"{"mode":"c"}"#;
        assert!(validate_settings_json(schema, bad).is_err());
    }
}
