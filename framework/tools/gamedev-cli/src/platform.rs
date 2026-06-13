//! Platform manifest fetch and compatibility checks.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::version;

#[derive(Debug, Clone, Deserialize)]
pub struct CliAsset {
    pub url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CliRelease {
    pub version: String,
    pub min_supported: String,
    #[serde(default)]
    pub assets: HashMap<String, CliAsset>,
}

#[derive(Debug, Deserialize)]
pub struct PlatformManifest {
    pub framework_version: String,
    pub wit_version: String,
    pub cli: CliRelease,
    #[serde(default)]
    pub sdk_versions: HashMap<String, String>,
}

pub fn platform_base_from_graphql(server_url: &str) -> String {
    let u = server_url.trim_end_matches('/');
    if let Some(i) = u.find("/graphql") {
        return u[..i].to_string();
    }
    u.to_string()
}

pub fn fetch_platform_manifest(base: &str) -> Result<PlatformManifest> {
    let url = format!("{}/platform/manifest.json", base.trim_end_matches('/'));
    let body = Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()?
        .get(&url)
        .send()
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("platform manifest HTTP error from {url}"))?
        .text()?;
    serde_json::from_str(&body).context("parse platform manifest JSON")
}

pub fn fetch_cli_manifest(base: &str) -> Result<CliRelease> {
    let url = format!("{}/tools/gamedev-cli/manifest.json", base.trim_end_matches('/'));
    let body = Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()?
        .get(&url)
        .send()
        .with_context(|| format!("GET {url}"))?
        .error_for_status()?
        .text()?;
    serde_json::from_str(&body).context("parse CLI manifest JSON")
}

pub fn check_local_cli_against_manifest(m: &CliRelease) -> Result<()> {
    let local = semver::Version::parse(version::cli_version())?;
    let min = semver::Version::parse(&m.min_supported)?;
    let latest = semver::Version::parse(&m.version)?;
    if local < min {
        bail!(
            "CLI {} is below platform minimum {} — download {} from {}/tools/gamedev-cli/manifest.json",
            local,
            min,
            latest,
            "platform"
        );
    }
    if local < latest {
        eprintln!(
            "warning: CLI {local} is older than platform release {latest}; run `gamedev-cli update`"
        );
    }
    Ok(())
}

pub fn check_local_toolchain_against_platform(m: &PlatformManifest) -> Result<()> {
    check_local_cli_against_manifest(&m.cli)?;
    if version::FRAMEWORK_VERSION != m.framework_version {
        bail!(
            "local framework tooling targets {} but platform runs {} — update gamedev-cli and SDKs",
            version::FRAMEWORK_VERSION,
            m.framework_version
        );
    }
    if version::WIT_VERSION != m.wit_version {
        bail!(
            "local WIT contract {} does not match platform {}",
            version::WIT_VERSION,
            m.wit_version
        );
    }
    Ok(())
}

pub fn current_asset_key() -> &'static str {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "windows-x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "linux-aarch64"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "macos-x86_64"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos-aarch64"
    } else {
        "linux-x86_64"
    }
}
