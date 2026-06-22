//! GraphQL client (blocking).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;

fn format_graphql_errors(errs: &serde_json::Value) -> String {
    let Some(arr) = errs.as_array() else {
        return errs.to_string();
    };
    let messages: Vec<String> = arr
        .iter()
        .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
        .map(str::to_string)
        .collect();
    if messages.is_empty() {
        errs.to_string()
    } else {
        messages.join("; ")
    }
}

fn bail_graphql(context: &str, errs: &serde_json::Value) -> anyhow::Error {
    let msg = format_graphql_errors(errs);
    if msg.contains("invalid or expired bearer") {
        return anyhow::anyhow!("{context}: token is invalid or expired");
    }
    if msg.contains("developer permission required") {
        return anyhow::anyhow!(
            "{context}: developer access required - sign in with a developer account or ask an admin to grant the developer role (Settings -> user id for admins)"
        );
    }
    if msg.contains("login required") {
        return anyhow::anyhow!("{context}: not authenticated");
    }
    anyhow::anyhow!("{context}: {msg}")
}

#[derive(Debug, Deserialize)]
pub struct UploadResp {
    pub data: Option<UploadData>,
    pub errors: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UploadData {
    #[serde(rename = "uploadGameZip")]
    pub upload_game_zip: UploadGameZip,
}

#[derive(Debug, Deserialize)]
pub struct UploadGameZip {
    #[serde(rename = "uploadId")]
    pub upload_id: String,
    pub draft: Option<DraftLite>,
    pub report: ValidationReport,
}

#[derive(Debug, Deserialize)]
pub struct DraftLite {
    pub id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(rename = "gameName")]
    pub game_name: String,
    pub version: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: i32,
    pub warnings: i32,
    pub infos: i32,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

#[derive(Debug, Deserialize)]
pub struct ValidationDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
}

pub struct PublishTokenResp {
    pub token: String,
    pub user_id: String,
    pub expires_at: i64,
}

pub struct AuthSessionResp {
    pub token: String,
    pub user_id: String,
    pub expires_at: i64,
}

pub fn gql_login_with_password(
    server_url: &str,
    display_name: &str,
    password: &str,
) -> Result<AuthSessionResp> {
    let q = r#"mutation($n: String!, $p: String!) { loginWithPassword(displayName: $n, password: $p) { sessionToken expiresAt user { id displayName createdAt } } }"#;
    let body = gql_raw_anonymous(server_url, q, json!({ "n": display_name, "p": password }))?;
    let v: serde_json::Value = serde_json::from_str(&body)?;
    if let Some(errs) = v.get("errors") {
        return Err(bail_graphql("login failed", errs));
    }
    let t = &v["data"]["loginWithPassword"];
    let user_id = t["user"]["id"].as_str().unwrap_or_default().to_string();
    let expires_at = t["expiresAt"].as_i64().unwrap_or_else(|| {
        let created = t["user"]["createdAt"].as_i64().unwrap_or(0);
        created + 30 * 24 * 60 * 60
    });
    Ok(AuthSessionResp {
        token: t["sessionToken"].as_str().unwrap_or_default().to_string(),
        user_id,
        expires_at,
    })
}

/// Validate a publish token from the lobby and return metadata for local storage.
pub fn store_publish_token(
    server_url: &str,
    publish_token: &str,
    expires_at: Option<i64>,
) -> Result<PublishTokenResp> {
    let q = r#"query { myProfile { displayName } }"#;
    let body = gql_raw(server_url, publish_token, q, json!({}))?;
    let v: serde_json::Value = serde_json::from_str(&body)?;
    if let Some(errs) = v.get("errors") {
        return Err(bail_graphql("publish token rejected", errs));
    }
    let display_name = v
        .pointer("/data/myProfile/displayName")
        .and_then(|n| n.as_str())
        .unwrap_or("user");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    Ok(PublishTokenResp {
        token: publish_token.to_string(),
        user_id: display_name.to_string(),
        expires_at: expires_at.unwrap_or(now + 7 * 24 * 60 * 60),
    })
}

pub fn gql_create_publish_token(server_url: &str, user_id: &str) -> Result<PublishTokenResp> {
    let q = r#"mutation($ttlDays: Int!) { createPublishToken(ttlDays: $ttlDays) { token userId expiresAt } }"#;
    let body = gql_raw(server_url, user_id, q, json!({ "ttlDays": 7 }))?;
    let v: serde_json::Value = serde_json::from_str(&body)?;
    let t = &v["data"]["createPublishToken"];
    Ok(PublishTokenResp {
        token: t["token"].as_str().unwrap_or_default().to_string(),
        user_id: t["userId"].as_str().unwrap_or(user_id).to_string(),
        expires_at: t["expiresAt"].as_i64().unwrap_or(0),
    })
}

pub fn gql_upload_game_zip(server_url: &str, token: &str, zip: &Path) -> Result<UploadGameZip> {
    let bytes = fs::read(zip)?;
    let q = r#"mutation($filename: String!, $zipBase64: String!) {
      uploadGameZip(filename: $filename, zipBase64: $zipBase64) {
        uploadId
        report { ok errors warnings infos diagnostics { severity code message } }
        draft { id slug gameName version status }
      }
    }"#;
    let raw = gql_raw(
        server_url,
        token,
        q,
        json!({
            "filename": zip.file_name().unwrap_or_default().to_string_lossy(),
            "zipBase64": base64::engine::general_purpose::STANDARD.encode(bytes)
        }),
    )?;
    let parsed: UploadResp = serde_json::from_str(&raw)?;
    if let Some(errs) = parsed.errors {
        bail!("graphql errors: {errs}");
    }
    Ok(parsed.data.context("missing data")?.upload_game_zip)
}

#[derive(Debug, Deserialize)]
pub struct BotUploadResp {
    pub data: Option<BotUploadData>,
    pub errors: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct BotUploadData {
    #[serde(rename = "uploadBotZip")]
    pub upload_bot_zip: BotUploadResult,
}

#[derive(Debug, Deserialize)]
pub struct BotUploadResult {
    #[serde(rename = "botId")]
    pub bot_id: String,
    pub slug: String,
    pub report: ValidationReport,
}

pub fn gql_upload_bot_zip(
    server_url: &str,
    token: &str,
    zip: &Path,
) -> Result<BotUploadResult> {
    let bytes = fs::read(zip)?;
    let q = r#"mutation($filename: String!, $zipBase64: String!) {
      uploadBotZip(filename: $filename, zipBase64: $zipBase64) {
        botId slug
        report { ok errors warnings infos diagnostics { severity code message } }
      }
    }"#;
    let raw = gql_raw(
        server_url,
        token,
        q,
        json!({
            "filename": zip.file_name().unwrap_or_default().to_string_lossy(),
            "zipBase64": base64::engine::general_purpose::STANDARD.encode(bytes)
        }),
    )?;
    let parsed: BotUploadResp = serde_json::from_str(&raw)?;
    if let Some(errs) = parsed.errors {
        bail!("graphql errors: {errs}");
    }
    Ok(parsed.data.context("missing data")?.upload_bot_zip)
}

pub fn gql_simple_mutation(
    server_url: &str,
    token: &str,
    field: &str,
    draft_id: &str,
) -> Result<()> {
    let q = format!(
        "mutation($draftId: ID!) {{ {}(draftId: $draftId) {{ id status }} }}",
        field
    );
    let _ = gql_raw(server_url, token, &q, json!({"draftId": draft_id}))?;
    Ok(())
}

pub fn gql_raw(
    server_url: &str,
    bearer: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<String> {
    let client = Client::new();
    let res = client
        .post(server_url)
        .header("Authorization", format!("Bearer {}", bearer))
        .json(&json!({ "query": query, "variables": variables }))
        .send()?
        .text()?;
    Ok(res)
}

pub fn gql_raw_anonymous(
    server_url: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<String> {
    let client = Client::new();
    let res = client
        .post(server_url)
        .json(&json!({ "query": query, "variables": variables }))
        .send()?
        .text()?;
    Ok(res)
}
