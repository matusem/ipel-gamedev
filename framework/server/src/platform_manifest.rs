//! Platform release manifest: version contract between deployed framework and developer tooling.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static MANIFEST: OnceLock<PlatformManifest> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliAsset {
    pub url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliRelease {
    pub version: String,
    pub min_supported: String,
    #[serde(default)]
    pub released_at: Option<String>,
    #[serde(default)]
    pub assets: HashMap<String, CliAsset>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformManifest {
    pub framework_version: String,
    pub wit_version: String,
    #[serde(default)]
    pub wasmtime_version: Option<String>,
    #[serde(default)]
    pub released_at: Option<String>,
    #[serde(default)]
    pub require_built_with_metadata: bool,
    pub cli: CliRelease,
    #[serde(default)]
    pub sdk_versions: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltWithMetadata {
    pub cli_version: String,
    pub framework_version: String,
    pub wit_version: String,
    #[serde(default)]
    pub sdk_versions: HashMap<String, String>,
}

pub fn manifest_dir() -> PathBuf {
    PathBuf::from(
        std::env::var("PLATFORM_MANIFEST_DIR").unwrap_or_else(|_| "./platform".into()),
    )
}

pub fn tools_dir() -> PathBuf {
    PathBuf::from(std::env::var("TOOLS_DIR").unwrap_or_else(|_| "./tools".into()))
}

pub fn load_manifest() -> &'static PlatformManifest {
    MANIFEST.get_or_init(|| {
        let path = manifest_dir().join("manifest.json");
        let raw = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "failed to read platform manifest at {}: {e}",
                path.display()
            )
        });
        serde_json::from_str(&raw).unwrap_or_else(|e| {
            panic!("invalid platform manifest JSON at {}: {e}", path.display())
        })
    })
}

pub fn cli_manifest_json() -> String {
    let tools_manifest = tools_dir().join("gamedev-cli").join("manifest.json");
    if tools_manifest.is_file() {
        if let Ok(s) = fs::read_to_string(&tools_manifest) {
            return s;
        }
    }
    let m = load_manifest();
    serde_json::json!({
        "version": m.cli.version,
        "min_supported": m.cli.min_supported,
        "released_at": m.released_at,
        "assets": m.cli.assets,
        "notes": m.cli.notes,
    })
    .to_string()
}

pub fn platform_manifest_json() -> String {
    serde_json::to_string(load_manifest()).expect("serialize platform manifest")
}

fn parse_semver(v: &str) -> Result<semver::Version, String> {
    semver::Version::parse(v.trim()).map_err(|e| format!("invalid semver '{v}': {e}"))
}

/// Returns `Ok(())` when `built_with` satisfies the deployed platform contract.
pub fn validate_built_with(built: &BuiltWithMetadata) -> Result<(), String> {
    validate_built_with_against(load_manifest(), built)
}

pub fn validate_built_with_against(
    m: &PlatformManifest,
    built: &BuiltWithMetadata,
) -> Result<(), String> {
    let require_meta = std::env::var("REQUIRE_BUILT_WITH_METADATA")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(m.require_built_with_metadata);

    if !require_meta {
        return Ok(());
    }

    let cli = parse_semver(&built.cli_version)?;
    let min_cli = parse_semver(&m.cli.min_supported)?;
    if cli < min_cli {
        return Err(format!(
            "CLI version {} is below platform minimum {} (download latest from /tools/gamedev-cli/manifest.json)",
            built.cli_version, m.cli.min_supported
        ));
    }

    if built.framework_version != m.framework_version {
        return Err(format!(
            "game built for framework {} but server runs {} — rebuild with matching gamedev-cli and SDK versions",
            built.framework_version, m.framework_version
        ));
    }

    if built.wit_version != m.wit_version {
        return Err(format!(
            "game WIT version {} does not match platform {} — update SDK/toolchain and rebuild",
            built.wit_version, m.wit_version
        ));
    }

    for (sdk, expected) in &m.sdk_versions {
        if let Some(actual) = built.sdk_versions.get(sdk) {
            if actual != expected {
                return Err(format!(
                    "SDK {sdk} version {actual} does not match platform requirement {expected}"
                ));
            }
        }
    }

    Ok(())
}

pub fn validate_built_with_from_manifest_json(manifest_path: &Path) -> Vec<crate::game_upload::ValidationDiagnostic> {
    use crate::game_upload::ValidationDiagnostic;

    let Ok(raw) = fs::read_to_string(manifest_path) else {
        return vec![];
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return vec![];
    };
    let Some(bw) = v.get("built_with") else {
        let m = load_manifest();
        if m.require_built_with_metadata {
            return vec![ValidationDiagnostic {
                severity: "error".to_string(),
                code: "E_BUILT_WITH_MISSING".to_string(),
                message: "manifest.json must include built_with metadata (rebuild with current gamedev-cli)".to_string(),
                path: Some("manifest.json".to_string()),
                hint: Some("Run `gamedev build` with the CLI version from /tools/gamedev-cli/manifest.json".to_string()),
            }];
        }
        return vec![];
    };
    let Ok(built) = serde_json::from_value::<BuiltWithMetadata>(bw.clone()) else {
        return vec![ValidationDiagnostic {
            severity: "error".to_string(),
            code: "E_BUILT_WITH_INVALID".to_string(),
            message: "manifest.json built_with field is invalid".to_string(),
            path: Some("manifest.json".to_string()),
            hint: None,
        }];
    };
    match validate_built_with(&built) {
        Ok(()) => vec![],
        Err(msg) => vec![ValidationDiagnostic {
            severity: "error".to_string(),
            code: "E_BUILT_WITH_INCOMPATIBLE".to_string(),
            message: msg,
            path: Some("manifest.json".to_string()),
            hint: Some("Run `gamedev-cli doctor --platform https://your-host/graphql`".to_string()),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_with_rejects_old_cli() {
        let m = PlatformManifest {
            framework_version: "0.1.0".into(),
            wit_version: "game-core-v1".into(),
            wasmtime_version: None,
            released_at: None,
            require_built_with_metadata: true,
            cli: CliRelease {
                version: "0.2.0".into(),
                min_supported: "0.2.0".into(),
                released_at: None,
                assets: HashMap::new(),
                notes: None,
            },
            sdk_versions: HashMap::new(),
        };
        let built = BuiltWithMetadata {
            cli_version: "0.1.0".into(),
            framework_version: "0.1.0".into(),
            wit_version: "game-core-v1".into(),
            sdk_versions: HashMap::new(),
        };
        assert!(validate_built_with_against(&m, &built).is_err());
    }
}
