//! GraphQL client (blocking).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;

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
    let q = r#"mutation($n: String!, $p: String!) { loginWithPassword(displayName: $n, password: $p) { sessionToken user { id createdAt } } }"#;
    let body = gql_raw_anonymous(server_url, q, json!({ "n": display_name, "p": password }))?;
    let v: serde_json::Value = serde_json::from_str(&body)?;
    if let Some(errs) = v.get("errors") {
        bail!("graphql errors: {errs}");
    }
    let t = &v["data"]["loginWithPassword"];
    let user_id = t["user"]["id"].as_str().unwrap_or_default().to_string();
    let created = t["user"]["createdAt"].as_i64().unwrap_or(0);
    Ok(AuthSessionResp {
        token: t["sessionToken"].as_str().unwrap_or_default().to_string(),
        user_id,
        expires_at: created + 30 * 24 * 60 * 60,
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
        draft { id gameName version status }
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
