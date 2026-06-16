//! Persisted CLI configuration (server profiles).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::defaults::{DEFAULT_GRAPHQL_URL, LOCAL_GRAPHQL_URL, LOCAL_PLATFORM_BASE};

pub const PROFILE_LOCAL: &str = "local";
pub const PROFILE_PROD: &str = "prod";

pub use crate::defaults::{DEFAULT_PROFILE_NAME, PROD_GRAPHQL_URL, PROD_PLATFORM_BASE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEntry {
    pub graphql_url: String,
    #[serde(default)]
    pub platform_base: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    #[serde(default = "default_profile_name")]
    pub default_profile: String,
    #[serde(default = "default_profiles")]
    pub profiles: HashMap<String, ProfileEntry>,
}

fn default_profile_name() -> String {
    crate::defaults::DEFAULT_PROFILE_NAME.to_string()
}

fn default_profiles() -> HashMap<String, ProfileEntry> {
    HashMap::from([
        (
            PROFILE_LOCAL.to_string(),
            ProfileEntry {
                graphql_url: LOCAL_GRAPHQL_URL.to_string(),
                platform_base: Some(LOCAL_PLATFORM_BASE.to_string()),
            },
        ),
        (
            PROFILE_PROD.to_string(),
            ProfileEntry {
                graphql_url: PROD_GRAPHQL_URL.to_string(),
                platform_base: Some(PROD_PLATFORM_BASE.to_string()),
            },
        ),
    ])
}

pub fn config_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir().unwrap_or(std::env::current_dir()?);
    Ok(base.join("gamedev-cli").join("config.toml"))
}

pub fn load_cli_config() -> Result<CliConfig> {
    let path = config_path()?;
    if !path.is_file() {
        let cfg = CliConfig {
            default_profile: crate::defaults::DEFAULT_PROFILE_NAME.to_string(),
            profiles: default_profiles(),
        };
        save_cli_config(&cfg)?;
        return Ok(cfg);
    }
    let s = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut cfg: CliConfig = toml::from_str(&s)?;
    for (name, entry) in default_profiles() {
        cfg.profiles.entry(name).or_insert(entry);
    }
    Ok(cfg)
}

pub fn save_cli_config(cfg: &CliConfig) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, toml::to_string_pretty(cfg)?)?;
    Ok(())
}

/// Resolve GraphQL URL: explicit `--server-url` wins unless it is still the default and `--profile` is set.
pub fn resolve_graphql_url(profile: Option<&str>, server_url: &str) -> Result<String> {
    if profile.is_none() && server_url != DEFAULT_GRAPHQL_URL {
        return Ok(server_url.to_string());
    }
    let cfg = load_cli_config()?;
    let name = profile.unwrap_or(&cfg.default_profile);
    let entry = cfg
        .profiles
        .get(name)
        .with_context(|| format!("unknown profile '{name}' (available: {:?})", cfg.profiles.keys().collect::<Vec<_>>()))?;
    Ok(entry.graphql_url.clone())
}

pub fn resolve_platform_base(profile: Option<&str>, server_url: &str) -> Result<String> {
    let graphql = resolve_graphql_url(profile, server_url)?;
    if let Ok(cfg) = load_cli_config() {
        let name = profile.unwrap_or(&cfg.default_profile);
        if let Some(entry) = cfg.profiles.get(name) {
            if let Some(base) = &entry.platform_base {
                return Ok(base.clone());
            }
        }
    }
    Ok(crate::platform::platform_base_from_graphql(&graphql))
}
