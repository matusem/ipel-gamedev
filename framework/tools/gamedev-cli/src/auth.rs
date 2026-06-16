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

fn write_auth_store(path: &Path, db: &[AuthEntry]) -> Result<()> {
    if db.is_empty() {
        if path.exists() {
            fs::remove_file(path)?;
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(db)?)?;
    Ok(())
}

/// Remove credentials for a specific GraphQL server URL.
pub fn logout_server(server_url: &str) -> Result<bool> {
    let path = auth_db_path()?;
    let mut db = load_auth_store(&path)?;
    let before = db.len();
    db.retain(|e| e.server_url != server_url);
    if db.len() == before {
        return Ok(false);
    }
    write_auth_store(&path, &db)?;
    Ok(true)
}

/// Remove all stored credentials.
pub fn clear_all_auth() -> Result<()> {
    let path = auth_db_path()?;
    write_auth_store(&path, &[])?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct AuthSummary {
    pub user_id: String,
    pub server_url: String,
    pub expires_at: i64,
}

pub fn expires_in_human(expires_at: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let secs = expires_at.saturating_sub(now);
    if secs <= 0 {
        return "expired".to_string();
    }
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3600;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h")
    } else {
        let mins = (secs % 3600) / 60;
        format!("{mins}m")
    }
}

/// First non-expired auth entry for dashboard / header display.
pub fn current_auth_summary() -> Option<AuthSummary> {
    let path = auth_db_path().ok()?;
    let db = load_auth_store(&path).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    db.into_iter()
        .find(|e| e.expires_at > now)
        .map(|e| AuthSummary {
            user_id: e.user_id,
            server_url: e.server_url,
            expires_at: e.expires_at,
        })
}

/// Current logged-in user label for the TUI header (first non-expired entry).
pub fn current_user_label() -> Option<String> {
    current_auth_summary().map(|a| a.user_id)
}
