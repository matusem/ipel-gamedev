//! Local auth token storage (`auth.json`).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct AuthEntry {
    pub server_url: String,
    pub token: String,
    pub user_id: String,
    pub expires_at: i64,
}

pub struct StoredToken {
    pub token: String,
}

pub fn auth_db_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir().unwrap_or(std::env::current_dir()?);
    Ok(base.join("gamedev-cli").join("auth.json"))
}

pub fn load_auth_store(path: &Path) -> Result<Vec<AuthEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

pub fn load_token(server_url: &str) -> Result<StoredToken> {
    let path = auth_db_path()?;
    let db = load_auth_store(&path)?;
    let tok = db
        .into_iter()
        .find(|e| e.server_url == server_url)
        .context("run login first")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    if tok.expires_at <= now {
        bail!("stored token expired, run login again");
    }
    Ok(StoredToken { token: tok.token })
}

/// Current logged-in user label for the TUI header (first non-expired entry).
pub fn current_user_label() -> Option<String> {
    let path = auth_db_path().ok()?;
    let db = load_auth_store(&path).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    db.into_iter()
        .find(|e| e.expires_at > now)
        .map(|e| e.user_id)
}
