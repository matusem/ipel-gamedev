use anyhow::Result;
#[cfg(feature = "typegen")]
use upjs_gdd_shared_types::{
    DraftLite, GameManifest, PublishToken, RealtimeEnvelope, UploadGameZipResponse, ValidationDiagnostic,
    ValidationReport,
};
#[cfg(feature = "typegen")]
use std::{fs, path::PathBuf};
#[cfg(feature = "typegen")]
use ts_rs::TS;

#[cfg(feature = "typegen")]
fn main() -> Result<()> {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../js/generated-types")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../js/generated-types"));
    fs::create_dir_all(&out)?;
    PublishToken::export_to(&out)?;
    ValidationDiagnostic::export_to(&out)?;
    ValidationReport::export_to(&out)?;
    DraftLite::export_to(&out)?;
    UploadGameZipResponse::export_to(&out)?;
    GameManifest::export_to(&out)?;
    RealtimeEnvelope::export_to(&out)?;
    println!("Generated TS definitions in {}", out.display());
    Ok(())
}

#[cfg(not(feature = "typegen"))]
fn main() -> Result<()> {
    Err(anyhow::anyhow!("enable `typegen` feature to run export_ts"))
}
