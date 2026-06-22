use crate::bot_registry::BotManifest;
use crate::component_db::ComponentDb;
use crate::game_contract::load_contract_for_game;
use crate::game_upload::{copy_dir_recursive, diag, summarize, ValidationDiagnostic, ValidationReport};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use uuid::Uuid;
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotUploadValidationResult {
    pub manifest: BotManifest,
    pub report: ValidationReport,
    pub staged_dir: PathBuf,
    pub settings_schema_json: Option<String>,
    pub settings_json: Option<String>,
}

pub async fn validate_and_stage_bot_zip_bytes(
    zip_bytes: &[u8],
    component_db: &ComponentDb,
    games_dir: &Path,
    drafts_root: &Path,
) -> Result<BotUploadValidationResult, String> {
    let mut diagnostics: Vec<ValidationDiagnostic> = Vec::new();
    if zip_bytes.is_empty() {
        diagnostics.push(diag(
            "error",
            "E_ZIP_EMPTY",
            "zip payload is empty",
            None,
            None,
        ));
        return Err(serde_json::to_string(&summarize(diagnostics, false, false, false, false))
            .unwrap_or_else(|_| "invalid zip".to_string()));
    }

    let mut archive =
        ZipArchive::new(Cursor::new(zip_bytes)).map_err(|e| format!("Invalid zip archive: {e}"))?;
    let tmp = tempdir().map_err(|e| format!("create temp dir: {e}"))?;
    let extract_root = tmp.path().join("extract");
    fs::create_dir_all(&extract_root).map_err(|e| format!("create extract dir: {e}"))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("zip entry read failed: {e}"))?;
        let Some(rel) = file.enclosed_name() else {
            diagnostics.push(diag(
                "error",
                "E_ZIP_PATH_TRAVERSAL",
                format!("zip entry has invalid path: {}", file.name()),
                Some(file.name()),
                None,
            ));
            continue;
        };
        let out = extract_root.join(&rel);
        if file.is_dir() {
            fs::create_dir_all(&out).map_err(|e| format!("create dir: {e}"))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|e| format!("read zip entry: {e}"))?;
        fs::write(&out, buf).map_err(|e| format!("write '{}': {e}", out.display()))?;
    }

    let manifest_path = extract_root.join("manifest.json");
    let wasm_path = extract_root.join("bot.wasm");

    if !manifest_path.is_file() {
        diagnostics.push(diag(
            "error",
            "E_BOT_MANIFEST_MISSING",
            "manifest.json is required",
            Some("manifest.json"),
            None,
        ));
    }
    if !wasm_path.is_file() {
        diagnostics.push(diag(
            "error",
            "E_BOT_WASM_MISSING",
            "bot.wasm is required",
            Some("bot.wasm"),
            None,
        ));
    }

    let mut manifest: Option<BotManifest> = None;
    if manifest_path.is_file() {
        match fs::read_to_string(&manifest_path) {
            Ok(s) => match serde_json::from_str::<BotManifest>(&s) {
                Ok(m) => {
                    if m.name.trim().is_empty() {
                        diagnostics.push(diag(
                            "error",
                            "E_BOT_NAME_EMPTY",
                            "manifest.name must be non-empty",
                            Some("manifest.json"),
                            None,
                        ));
                    }
                    if m.display_name.trim().is_empty() {
                        diagnostics.push(diag(
                            "error",
                            "E_BOT_DISPLAY_NAME_EMPTY",
                            "manifest.display_name must be non-empty",
                            Some("manifest.json"),
                            None,
                        ));
                    }
                    if m.game_slug.trim().is_empty() {
                        diagnostics.push(diag(
                            "error",
                            "E_BOT_GAME_SLUG_EMPTY",
                            "manifest.game_slug must be non-empty",
                            Some("manifest.json"),
                            None,
                        ));
                    }
                    if m.contract_hash.trim().is_empty() {
                        diagnostics.push(diag(
                            "error",
                            "E_BOT_CONTRACT_HASH_EMPTY",
                            "manifest.contract_hash must be non-empty",
                            Some("manifest.json"),
                            None,
                        ));
                    }
                    manifest = Some(m);
                }
                Err(e) => diagnostics.push(diag(
                    "error",
                    "E_BOT_MANIFEST_INVALID",
                    format!("manifest.json parse error: {e}"),
                    Some("manifest.json"),
                    None,
                )),
            },
            Err(e) => diagnostics.push(diag(
                "error",
                "E_BOT_MANIFEST_READ",
                format!("cannot read manifest.json: {e}"),
                Some("manifest.json"),
                None,
            )),
        }
    }

    if wasm_path.is_file() {
        match fs::read(&wasm_path) {
            Ok(bytes) => match component_db.validate_bot_component_instantiable(&bytes).await {
                Ok(()) => diagnostics.push(diag(
                    "info",
                    "I_BOT_WASM_VALID",
                    "bot.wasm is a valid game-bot component",
                    Some("bot.wasm"),
                    None,
                )),
                Err(e) => diagnostics.push(diag(
                    "error",
                    "E_BOT_WASM_INVALID",
                    e,
                    Some("bot.wasm"),
                    Some("Build with cargo component build --release in the bot component crate."),
                )),
            },
            Err(e) => diagnostics.push(diag(
                "error",
                "E_BOT_WASM_READ",
                format!("cannot read bot.wasm: {e}"),
                Some("bot.wasm"),
                None,
            )),
        }
    }

    if let Some(ref m) = manifest {
        match load_contract_for_game(games_dir, &m.game_slug) {
            Some(contract) => {
                if contract.contract_hash != m.contract_hash {
                    diagnostics.push(diag(
                        "error",
                        "E_BOT_CONTRACT_MISMATCH",
                        format!(
                            "bot contract_hash {:?} does not match published game {:?}",
                            m.contract_hash, contract.contract_hash
                        ),
                        Some("manifest.json"),
                        Some("Rebuild the bot against the current game contract."),
                    ));
                } else {
                    diagnostics.push(diag(
                        "info",
                        "I_BOT_CONTRACT_OK",
                        "bot contract_hash matches published game",
                        Some("manifest.json"),
                        None,
                    ));
                }
            }
            None => diagnostics.push(diag(
                "error",
                "E_BOT_GAME_NOT_FOUND",
                format!("published game {:?} not found or has no contract.json", m.game_slug),
                Some("manifest.json"),
                None,
            )),
        }
    }

    let report = summarize(diagnostics, false, false, false, false);
    if !report.ok {
        return Err(serde_json::to_string(&report)
            .unwrap_or_else(|_| "{\"ok\":false}".to_string()));
    }
    let manifest = manifest.ok_or_else(|| "manifest missing".to_string())?;

    let mut settings_schema_json = manifest
        .settings_schema
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok());
    let mut settings_json = manifest
        .default_settings
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok());

    if wasm_path.is_file() {
        if let Ok(bytes) = fs::read(&wasm_path) {
            let tmp_slug = format!("_upload_validate_{}", Uuid::new_v4());
            if component_db.insert_bot_component(&tmp_slug, &bytes).is_ok() {
                if let Ok(schema) = crate::bot_service::bot_settings_schema_wasm(component_db, &tmp_slug).await {
                    settings_schema_json = Some(schema);
                }
                if let Ok(default_bytes) =
                    crate::bot_service::bot_default_settings_wasm(component_db, &tmp_slug).await
                {
                    settings_json = Some(String::from_utf8_lossy(&default_bytes).into_owned());
                }
                if let (Some(schema), Some(settings)) = (&settings_schema_json, &settings_json) {
                    if let Err(e) = crate::bot_service::validate_settings_json(schema, settings) {
                        return Err(format!("bot default settings invalid: {e}"));
                    }
                    let _ = crate::bot_service::bot_validate_settings_wasm(
                        component_db,
                        &tmp_slug,
                        settings.as_bytes(),
                    )
                    .await;
                }
            }
        }
    }

    fs::create_dir_all(drafts_root).map_err(|e| e.to_string())?;
    let staged_dir = drafts_root.join(format!("bot_{}", Uuid::new_v4()));
    copy_dir_recursive(&extract_root, &staged_dir)?;
    Ok(BotUploadValidationResult {
        manifest,
        report,
        staged_dir,
        settings_schema_json,
        settings_json,
    })
}

pub fn publish_staged_bot(staged_dir: &Path, bots_dir: &Path, slug: &str) -> Result<PathBuf, String> {
    let live_dir = bots_dir.join(slug);
    let tmp = bots_dir.join(format!(".tmp_bot_{}", Uuid::new_v4()));
    fs::create_dir_all(bots_dir).map_err(|e| e.to_string())?;
    copy_dir_recursive(staged_dir, &tmp)?;
    if live_dir.exists() {
        fs::remove_dir_all(&live_dir).map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, &live_dir).map_err(|e| e.to_string())?;
    Ok(live_dir)
}
