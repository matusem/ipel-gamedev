use std::fs;
use std::path::PathBuf;

use __SHARED_TYPES_CRATE__::{Move, Player};
use ts_rs::TS;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../frontend/web/src/generated");
    fs::create_dir_all(&out)?;
    Player::export_to(&out)?;
    Move::export_to(&out)?;
    println!("Generated TS types at {}", out.display());
    Ok(())
}
