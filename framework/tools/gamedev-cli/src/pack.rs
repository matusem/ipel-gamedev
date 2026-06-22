//! Zip packaging and recursive directory copy.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(src_path, dst_path)?;
        }
    }
    Ok(())
}

pub fn create_zip(stage_root: &Path, out: &Path) -> Result<()> {
    let file = fs::File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for p in ["manifest.json", "logic.wasm", "contract.json"] {
        let path = stage_root.join(p);
        if path.is_file() {
            let bytes = fs::read(&path)?;
            zip.start_file(p, opts)?;
            zip.write_all(&bytes)?;
        }
    }
    zip.add_directory("client/", opts)?;
    zip_client_dir_recursive(stage_root, stage_root.join("client"), &mut zip, opts)?;
    zip.finish()?;
    Ok(())
}

pub fn create_bot_zip(stage_root: &Path, out: &Path) -> Result<()> {
    let file = fs::File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for p in ["manifest.json", "bot.wasm"] {
        let bytes = fs::read(stage_root.join(p))?;
        zip.start_file(p, opts)?;
        zip.write_all(&bytes)?;
    }
    zip.finish()?;
    Ok(())
}

fn zip_client_dir_recursive(
    stage_root: &Path,
    current: PathBuf,
    zip: &mut zip::ZipWriter<fs::File>,
    opts: zip::write::SimpleFileOptions,
) -> Result<()> {
    for entry in fs::read_dir(&current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            zip_client_dir_recursive(stage_root, path, zip, opts)?;
        } else {
            let rel = path
                .strip_prefix(stage_root)?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(&path)?;
            zip.start_file(rel, opts)?;
            zip.write_all(&bytes)?;
        }
    }
    Ok(())
}
