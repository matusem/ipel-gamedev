use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde_json::json;
use upjs_gdd_shared_types::{GameManifest, PublishToken, UploadGameZipResponse};

#[derive(Clone)]
pub struct SdkApiClient {
    pub server_url: String,
    client: Client,
}

impl SdkApiClient {
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            client: Client::new(),
        }
    }

    pub fn create_publish_token(&self, user_id: &str, ttl_days: i32) -> Result<PublishToken> {
        let query = r#"mutation($ttlDays: Int!) { createPublishToken(ttlDays: $ttlDays) { token userId expiresAt } }"#;
        let body = self.gql_raw(user_id, query, json!({ "ttlDays": ttl_days }))?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        let t = &v["data"]["createPublishToken"];
        Ok(PublishToken {
            token: t["token"].as_str().unwrap_or_default().to_string(),
            user_id: t["userId"].as_str().unwrap_or_default().to_string(),
            expires_at: t["expiresAt"].as_i64().unwrap_or_default(),
        })
    }

    pub fn upload_game_zip_base64(
        &self,
        token: &str,
        filename: &str,
        zip_base64: &str,
    ) -> Result<UploadGameZipResponse> {
        let query = r#"mutation($filename: String!, $zipBase64: String!) {
          uploadGameZip(filename: $filename, zipBase64: $zipBase64) {
            uploadId
            report { ok errors warnings infos diagnostics { severity code message } }
            draft { id gameName version status }
          }
        }"#;
        let body = self.gql_raw(
            token,
            query,
            json!({"filename": filename, "zipBase64": zip_base64}),
        )?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        let upload = &v["data"]["uploadGameZip"];
        Ok(UploadGameZipResponse {
            upload_id: upload["uploadId"].as_str().unwrap_or_default().to_string(),
            draft: upload["draft"]
                .as_object()
                .map(|d| upjs_gdd_shared_types::DraftLite {
                    id: d
                        .get("id")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    game_name: d
                        .get("gameName")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    version: d
                        .get("version")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    status: d
                        .get("status")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                }),
            report: serde_json::from_value(upload["report"].clone())
                .context("invalid report payload")?,
        })
    }

    pub fn update_draft_manifest(
        &self,
        token: &str,
        draft_id: &str,
        manifest: &GameManifest,
    ) -> Result<serde_json::Value> {
        let query = r#"mutation($draftId: ID!, $name: String!, $displayName: String!, $version: String!, $description: String!) {
          updateGameDraftManifest(draftId: $draftId, name: $name, displayName: $displayName, version: $version, description: $description) { id gameName version status }
        }"#;
        let body = self.gql_raw(
            token,
            query,
            json!({
                "draftId": draft_id,
                "name": manifest.name,
                "displayName": manifest.display_name,
                "version": manifest.version,
                "description": manifest.description
            }),
        )?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        Ok(v["data"]["updateGameDraftManifest"].clone())
    }

    pub fn list_my_drafts(&self, token: &str) -> Result<Vec<upjs_gdd_shared_types::DraftLite>> {
        let query = r#"query { myGameDrafts { id gameName version status } }"#;
        let body = self.gql_raw(token, query, json!({}))?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        let drafts = v["data"]["myGameDrafts"]
            .as_array()
            .context("missing myGameDrafts")?;
        let mut out = Vec::with_capacity(drafts.len());
        for d in drafts {
            out.push(upjs_gdd_shared_types::DraftLite {
                id: d["id"].as_str().unwrap_or_default().to_string(),
                game_name: d["gameName"].as_str().unwrap_or_default().to_string(),
                version: d["version"].as_str().unwrap_or_default().to_string(),
                status: d["status"].as_str().unwrap_or_default().to_string(),
            });
        }
        Ok(out)
    }

    fn gql_raw(&self, bearer: &str, query: &str, variables: serde_json::Value) -> Result<String> {
        let body = self
            .client
            .post(&self.server_url)
            .header("Authorization", format!("Bearer {}", bearer))
            .json(&json!({ "query": query, "variables": variables }))
            .send()?
            .text()?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        if v.get("errors").is_some() {
            bail!("graphql error: {}", v["errors"]);
        }
        Ok(body)
    }
}
