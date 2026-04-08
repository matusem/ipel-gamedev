use serde::{Deserialize, Serialize};

#[cfg(feature = "typegen")]
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
pub struct PublishToken {
    pub token: String,
    pub user_id: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
pub struct ValidationDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: i32,
    pub warnings: i32,
    pub infos: i32,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
pub struct DraftLite {
    pub id: String,
    pub game_name: String,
    pub version: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
pub struct UploadGameZipResponse {
    pub upload_id: String,
    pub draft: Option<DraftLite>,
    pub report: ValidationReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
pub struct GameManifest {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(TS))]
#[cfg_attr(feature = "typegen", ts(export))]
pub struct RealtimeEnvelope {
    pub channel: String,
    pub event: String,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_response_serializes() {
        let payload = UploadGameZipResponse {
            upload_id: "u1".to_string(),
            draft: Some(DraftLite {
                id: "d1".to_string(),
                game_name: "game".to_string(),
                version: "0.1.0".to_string(),
                status: "DRAFT".to_string(),
            }),
            report: ValidationReport {
                ok: true,
                errors: 0,
                warnings: 0,
                infos: 1,
                diagnostics: vec![],
            },
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        assert!(json.contains("upload_id"));
    }
}
