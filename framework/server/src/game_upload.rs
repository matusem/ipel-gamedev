use crate::component_db::ComponentDb;
use crate::game_registry::GameManifest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use uuid::Uuid;
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub required_index_html: bool,
    pub required_config_html: bool,
    pub required_result_html: bool,
    pub required_about_html: bool,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadValidationResult {
    pub manifest: GameManifest,
    pub report: ValidationReport,
    pub staged_dir: PathBuf,
}

fn diag(
    severity: &str,
    code: &str,
    message: impl Into<String>,
    path: Option<&str>,
    hint: Option<&str>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity: severity.to_string(),
        code: code.to_string(),
        message: message.into(),
        path: path.map(str::to_string),
        hint: hint.map(str::to_string),
    }
}

fn summarize(
    diagnostics: Vec<ValidationDiagnostic>,
    req_index: bool,
    req_config: bool,
    req_result: bool,
    req_about: bool,
) -> ValidationReport {
    let errors = diagnostics.iter().filter(|d| d.severity == "error").count();
    let warnings = diagnostics
        .iter()
        .filter(|d| d.severity == "warning")
        .count();
    let infos = diagnostics.iter().filter(|d| d.severity == "info").count();
    ValidationReport {
        ok: errors == 0,
        errors,
        warnings,
        infos,
        required_index_html: req_index,
        required_config_html: req_config,
        required_result_html: req_result,
        required_about_html: req_about,
        diagnostics,
    }
}

fn ensure_within_base(base: &Path, candidate: &Path) -> bool {
    let Ok(base_canon) = fs::canonicalize(base) else {
        return false;
    };
    let Ok(candidate_canon) = fs::canonicalize(candidate) else {
        return false;
    };
    candidate_canon.starts_with(base_canon)
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), String> {
    if !to.exists() {
        fs::create_dir_all(to).map_err(|e| format!("create dir '{}': {e}", to.display()))?;
    }
    let rd = fs::read_dir(from).map_err(|e| format!("read dir '{}': {e}", from.display()))?;
    for entry in rd {
        let entry = entry.map_err(|e| e.to_string())?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else if src.is_file() {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&src, &dst).map_err(|e| format!("copy '{}': {e}", src.display()))?;
        }
    }
    Ok(())
}

/// Returns `true` if `GAMES_DIR/{name}` exists and is a directory (published game).
pub fn live_game_folder_exists(games_dir: &Path, name: &str) -> bool {
    let p = games_dir.join(name.trim());
    p.is_dir()
}

/// Machine-safe folder name for published games (matches manifest validation).
pub fn validate_game_folder_name(name: &str) -> Result<(), &'static str> {
    let t = name.trim();
    if t.is_empty() {
        return Err("name must not be empty");
    }
    if t.len() > 64 {
        return Err("name must be at most 64 characters");
    }
    for ch in t.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            continue;
        }
        return Err("name may only contain letters, digits, underscore, and hyphen");
    }
    Ok(())
}

pub async fn validate_and_stage_zip_bytes(
    zip_bytes: &[u8],
    component_db: &ComponentDb,
    drafts_root: &Path,
    games_dir_for_collision_check: Option<&Path>,
) -> Result<UploadValidationResult, String> {
    let mut diagnostics = Vec::new();
    if zip_bytes.is_empty() {
        diagnostics.push(diag(
            "error",
            "E_ZIP_EMPTY",
            "zip payload is empty",
            None,
            Some("Upload a .zip archive with game files."),
        ));
        return Err(serde_json::to_string(&summarize(diagnostics, false, false, false, false)).unwrap_or_else(|_| "invalid zip".to_string()));
    }

    let mut archive = ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| format!("Invalid zip archive: {e}"))?;
    let tmp = tempdir().map_err(|e| format!("create temp dir: {e}"))?;
    let extract_root = tmp.path().join("extract");
    fs::create_dir_all(&extract_root).map_err(|e| format!("create extract dir: {e}"))?;

    let mut total_unpacked: u64 = 0;
    let max_unpacked: u64 = std::env::var("UPLOAD_MAX_UNPACKED_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100 * 1024 * 1024);
    let max_entries: usize = std::env::var("UPLOAD_MAX_ENTRIES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5000);

    if archive.len() > max_entries {
        diagnostics.push(diag(
            "error",
            "E_ZIP_TOO_MANY_ENTRIES",
            format!("zip contains too many entries: {} > {}", archive.len(), max_entries),
            None,
            None,
        ));
    }

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("zip entry read failed: {e}"))?;
        let Some(rel) = file.enclosed_name() else {
            diagnostics.push(diag(
                "error",
                "E_ZIP_PATH_TRAVERSAL",
                format!("zip entry has invalid path: {}", file.name()),
                Some(file.name()),
                Some("Ensure archive paths are relative and do not contain '..'."),
            ));
            continue;
        };
        let out = extract_root.join(&rel);
        if file.is_dir() {
            fs::create_dir_all(&out).map_err(|e| format!("create dir '{}': {e}", out.display()))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create parent '{}': {e}", parent.display()))?;
        }
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|e| format!("read zip entry '{}': {e}", file.name()))?;
        total_unpacked = total_unpacked.saturating_add(buf.len() as u64);
        if total_unpacked > max_unpacked {
            diagnostics.push(diag(
                "error",
                "E_ZIP_TOO_LARGE",
                format!("unpacked size exceeds limit: {} bytes", max_unpacked),
                None,
                Some("Reduce assets size or split package."),
            ));
            break;
        }
        fs::write(&out, buf).map_err(|e| format!("write '{}': {e}", out.display()))?;
        if !ensure_within_base(&extract_root, &out) {
            diagnostics.push(diag(
                "error",
                "E_ZIP_PATH_ESCAPE",
                format!("entry escaped extraction root: {}", out.display()),
                Some(file.name()),
                None,
            ));
        }
    }

    let manifest_path = extract_root.join("manifest.json");
    let logic_path = extract_root.join("logic.wasm");
    let client_dir = extract_root.join("client");
    let index_path = client_dir.join("index.html");
    let config_path = client_dir.join("config.html");
    let result_path = client_dir.join("result.html");
    let about_path = client_dir.join("about.html");

    if !manifest_path.is_file() {
        diagnostics.push(diag(
            "error",
            "E_MANIFEST_MISSING",
            "manifest.json is required",
            Some("manifest.json"),
            None,
        ));
    }
    if !logic_path.is_file() {
        diagnostics.push(diag(
            "error",
            "E_LOGIC_WASM_MISSING",
            "logic.wasm is required",
            Some("logic.wasm"),
            None,
        ));
    }
    if !client_dir.is_dir() {
        diagnostics.push(diag(
            "error",
            "E_CLIENT_DIR_MISSING",
            "client/ directory is required",
            Some("client"),
            None,
        ));
    }
    let has_index = index_path.is_file();
    let has_config = config_path.is_file();
    let has_result = result_path.is_file();
    let has_about = about_path.is_file();
    if !has_index {
        diagnostics.push(diag(
            "error",
            "E_CLIENT_INDEX_MISSING",
            "client/index.html is required",
            Some("client/index.html"),
            None,
        ));
    }
    if !has_config {
        diagnostics.push(diag(
            "error",
            "E_CLIENT_CONFIG_MISSING",
            "client/config.html is required",
            Some("client/config.html"),
            None,
        ));
    }
    if !has_result {
        diagnostics.push(diag(
            "error",
            "E_CLIENT_RESULT_MISSING",
            "client/result.html is required",
            Some("client/result.html"),
            None,
        ));
    }
    if !has_about {
        diagnostics.push(diag(
            "error",
            "E_CLIENT_ABOUT_MISSING",
            "client/about.html is required",
            Some("client/about.html"),
            None,
        ));
    }

    let mut manifest: Option<GameManifest> = None;
    if manifest_path.is_file() {
        match fs::read_to_string(&manifest_path) {
            Ok(s) => match serde_json::from_str::<GameManifest>(&s) {
                Ok(m) => {
                    if let Err(msg) = validate_game_folder_name(&m.name) {
                        diagnostics.push(diag(
                            "error",
                            "E_MANIFEST_NAME_INVALID",
                            format!("manifest.name invalid: {msg}"),
                            Some("manifest.json"),
                            Some("Use letters, digits, underscore, and hyphen only (max 64 chars)."),
                        ));
                    }
                    if m.display_name.trim().is_empty() {
                        diagnostics.push(diag("error", "E_MANIFEST_DISPLAY_NAME_EMPTY", "manifest.display_name must be non-empty", Some("manifest.json"), None));
                    }
                    if m.version.trim().is_empty() {
                        diagnostics.push(diag("error", "E_MANIFEST_VERSION_MISSING", "manifest.version must be non-empty", Some("manifest.json"), None));
                    }
                    if m.min_players == 0 {
                        diagnostics.push(diag("error", "E_MANIFEST_MIN_PLAYERS_INVALID", "manifest.min_players must be >= 1", Some("manifest.json"), None));
                    }
                    if m.max_players < m.min_players {
                        diagnostics.push(diag("error", "E_MANIFEST_MAX_PLAYERS_INVALID", "manifest.max_players must be >= min_players", Some("manifest.json"), None));
                    }
                    diagnostics.extend(crate::platform_manifest::validate_built_with_from_manifest_json(
                        &manifest_path,
                    ));
                    manifest = Some(m);
                }
                Err(e) => diagnostics.push(diag(
                    "error",
                    "E_MANIFEST_INVALID_JSON",
                    format!("manifest.json parse error: {e}"),
                    Some("manifest.json"),
                    None,
                )),
            },
            Err(e) => diagnostics.push(diag(
                "error",
                "E_MANIFEST_READ_FAILED",
                format!("cannot read manifest.json: {e}"),
                Some("manifest.json"),
                None,
            )),
        }
    }

    if logic_path.is_file() {
        match fs::read(&logic_path) {
            Ok(bytes) => match component_db.validate_component_instantiable(&bytes).await {
                Ok(()) => diagnostics.push(diag(
                    "info",
                    "I_WASM_COMPONENT_VALID",
                    "logic.wasm is a valid component compatible with runtime",
                    Some("logic.wasm"),
                    None,
                )),
                Err(e) => diagnostics.push(diag(
                    "error",
                    "E_WASM_COMPONENT_INVALID",
                    e,
                    Some("logic.wasm"),
                    Some(
                        "The message field lists the step ([1/3] parse, [2/3] link WASI, [3/3] instantiate GameCore), \
                         file size, and (when applicable) core-module import samples. \
                         Fix: produce a WebAssembly Component with cargo component build --release. \
                         Java/TeaVM builds are core Wasm (often WASI); they need a separate component packaging step to upload here.",
                    ),
                )),
            },
            Err(e) => diagnostics.push(diag(
                "error",
                "E_LOGIC_WASM_READ_FAILED",
                format!("cannot read logic.wasm: {e}"),
                Some("logic.wasm"),
                None,
            )),
        }
    }

    let Some(manifest_ref) = manifest.as_ref() else {
        let report = summarize(diagnostics, has_index, has_config, has_result, has_about);
        if !report.ok {
            return Err(
                serde_json::to_string(&report).unwrap_or_else(|_| "{\"ok\":false,\"errors\":1}".to_string())
            );
        }
        return Err("{\"ok\":false,\"errors\":1,\"diagnostics\":[{\"code\":\"E_MANIFEST_MISSING\"}]}".to_string());
    };

    if let Some(gdir) = games_dir_for_collision_check {
        if live_game_folder_exists(gdir, &manifest_ref.name) {
            diagnostics.push(diag(
                "warning",
                "W_GAME_NAME_COLLISION",
                format!(
                    "a published game already exists at '{}/' — publishing this draft will replace it",
                    manifest_ref.name.trim()
                ),
                Some("manifest.json"),
                Some("Change manifest name before publishing if you want a separate game entry."),
            ));
        }
    }

    let report = summarize(diagnostics, has_index, has_config, has_result, has_about);
    if !report.ok {
        return Err(
            serde_json::to_string(&report).unwrap_or_else(|_| "{\"ok\":false,\"errors\":1}".to_string())
        );
    }
    let Some(manifest) = manifest else {
        return Err("{\"ok\":false,\"errors\":1,\"diagnostics\":[{\"code\":\"E_MANIFEST_MISSING\"}]}".to_string());
    };

    fs::create_dir_all(drafts_root)
        .map_err(|e| format!("create drafts root '{}': {e}", drafts_root.display()))?;
    let staged_dir = drafts_root.join(Uuid::new_v4().to_string());
    copy_dir_recursive(&extract_root, &staged_dir)?;
    Ok(UploadValidationResult {
        manifest,
        report,
        staged_dir,
    })
}

pub fn write_manifest_to_staged_dir(staged_dir: &Path, manifest: &GameManifest) -> Result<(), String> {
    let path = staged_dir.join("manifest.json");
    let s = serde_json::to_string_pretty(manifest).map_err(|e| format!("serialize manifest: {e}"))?;
    fs::write(&path, s).map_err(|e| format!("write '{}': {e}", path.display()))
}

/// Removes `GAMES_DIR/{game_name}` if present (live published game folder).
pub fn remove_published_game_dir(games_dir: &Path, game_name: &str) -> Result<(), String> {
    validate_game_folder_name(game_name)?;
    let live = games_dir.join(game_name.trim());
    if live.is_dir() {
        fs::remove_dir_all(&live)
            .map_err(|e| format!("remove live game dir '{}': {e}", live.display()))?;
    }
    Ok(())
}

pub fn publish_staged_game(staged_dir: &Path, games_dir: &Path, game_name: &str) -> Result<PathBuf, String> {
    let live_dir = games_dir.join(game_name);
    let tmp_live = games_dir.join(format!(".tmp_publish_{}", Uuid::new_v4()));
    if tmp_live.exists() {
        fs::remove_dir_all(&tmp_live).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(games_dir).map_err(|e| format!("create games dir '{}': {e}", games_dir.display()))?;
    copy_dir_recursive(staged_dir, &tmp_live)?;
    if live_dir.exists() {
        fs::remove_dir_all(&live_dir).map_err(|e| format!("remove old game dir '{}': {e}", live_dir.display()))?;
    }
    fs::rename(&tmp_live, &live_dir).map_err(|e| format!("activate game dir '{}': {e}", live_dir.display()))?;
    Ok(live_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_counts_diagnostics() {
        let report = summarize(
            vec![
                diag("error", "E1", "a", None, None),
                diag("warning", "W1", "b", None, None),
                diag("info", "I1", "c", None, None),
            ],
            true,
            false,
            true,
            false,
        );
        assert!(!report.ok);
        assert_eq!(report.errors, 1);
        assert_eq!(report.warnings, 1);
        assert_eq!(report.infos, 1);
        assert!(report.required_index_html);
        assert!(!report.required_config_html);
        assert!(report.required_result_html);
        assert!(!report.required_about_html);
    }
}
