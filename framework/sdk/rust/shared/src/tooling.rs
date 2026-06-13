use crate::api::SdkApiClient;
use anyhow::Result;
use base64::Engine as _;
use upjs_gdd_shared_types::UploadGameZipResponse;
use std::path::Path;

pub fn deploy_zip_file(
    client: &SdkApiClient,
    token: &str,
    zip_path: &Path,
) -> Result<UploadGameZipResponse> {
    let filename = zip_path
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("game.zip");
    let bytes = std::fs::read(zip_path)?;
    let zip_base64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    client.upload_game_zip_base64(token, filename, &zip_base64)
}
