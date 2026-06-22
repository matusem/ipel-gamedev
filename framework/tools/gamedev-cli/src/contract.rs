//! Emit `contract.json` into the build stage.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::codegen::find_shared_types_crate;
use crate::reporter::{self, LoggedCommand};

pub fn emit_contract_to_stage(root: &Path, stage: &Path) -> Result<()> {
    let shared_types = match find_shared_types_crate(root) {
        Ok(p) => p,
        Err(_) => {
            reporter::warn(
                "contract",
                "no shared-types crate — skipping contract.json (add backend/rust/shared-types)",
            );
            return Ok(());
        }
    };

    let status = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg(shared_types.join("Cargo.toml"))
        .arg("--features")
        .arg("schemars")
        .arg("--bin")
        .arg("export_schema")
        .current_dir(root)
        .status_logged()
        .context("run export_schema")?;
    if !status.success() {
        bail!("contract export failed (cargo run --features schemars --bin export_schema)");
    }

    let candidates = [
        shared_types.join("schema").join("contract.json"),
        root.join("generated").join("schema").join("contract.json"),
    ];
    let src = candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .with_context(|| "contract.json not found after export_schema")?;
    fs::copy(&src, stage.join("contract.json"))
        .with_context(|| format!("copy contract to {}", stage.display()))?;
    reporter::status("contract", &format!("Emitted {}", src.display()));
    Ok(())
}

pub fn fetch_contract_from_server(
    server_url: &str,
    game_slug: &str,
) -> Result<(serde_json::Value, PathBuf)> {
    let base = server_url.trim_end_matches('/');
    let url = format!("{base}/games/{game_slug}/contract.json");
    let body = reqwest::blocking::get(&url)
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("fetch contract from {url}"))?
        .text()?;
    let v: serde_json::Value = serde_json::from_str(&body).context("parse contract.json")?;
    let tmp = tempfile::tempdir()?;
    let path = tmp.path().join("contract.json");
    fs::write(&path, serde_json::to_string_pretty(&v)?)?;
    // Leak temp dir path by copying to caller-owned location — caller should copy immediately
    let out = std::env::temp_dir().join(format!("gamedev-contract-{game_slug}.json"));
    fs::copy(&path, &out)?;
    Ok((v, out))
}
