//! Self-update from platform-hosted CLI manifest.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

use crate::platform::{current_asset_key, fetch_cli_manifest, platform_base_from_graphql};
use crate::version;

pub fn run_update(base_url: &str, check_only: bool) -> Result<()> {
    let m = fetch_cli_manifest(base_url)?;
    let local = semver::Version::parse(version::cli_version())?;
    let latest = semver::Version::parse(&m.version)?;
    let min = semver::Version::parse(&m.min_supported)?;

    if local >= latest {
        println!("gamedev-cli {local} is up to date (platform release {latest})");
        return Ok(());
    }

    if check_only {
        if local < min {
            bail!("CLI {local} is below minimum supported {min}");
        }
        bail!("CLI {local} is outdated; latest is {latest}");
    }

    let key = current_asset_key();
    let asset = m
        .assets
        .get(key)
        .with_context(|| format!("no download asset for platform key {key}"))?;

    let download_url = if asset.url.starts_with("http") {
        asset.url.clone()
    } else {
        format!("{}{}", base_url.trim_end_matches('/'), asset.url)
    };

    println!("Downloading gamedev-cli {latest} from {download_url} ...");
    let bytes = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?
        .get(&download_url)
        .send()?
        .error_for_status()?
        .bytes()?;

    let hash = hex_sha256(&bytes);
    if hash != asset.sha256.to_lowercase() {
        bail!("checksum mismatch: expected {}, got {hash}", asset.sha256);
    }

    let exe = current_exe()?;
    let tmp = exe.with_extension("new");
    extract_binary(&bytes, &tmp)?;
    replace_executable(&exe, &tmp)?;
    println!("Updated to gamedev-cli {latest}");
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn current_exe() -> Result<PathBuf> {
    std::env::current_exe().context("resolve current executable path")
}

fn extract_binary(archive_bytes: &[u8], out: &Path) -> Result<()> {
    if archive_bytes.starts_with(b"PK") {
        let cursor = std::io::Cursor::new(archive_bytes);
        let mut zip = zip::ZipArchive::new(cursor)?;
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            let name = file.name().to_string();
            if name.ends_with(".exe") || name == "gamedev" || name.ends_with("/gamedev") {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                fs::write(out, buf)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = fs::metadata(out)?.permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(out, perms)?;
                }
                return Ok(());
            }
        }
        bail!("zip archive did not contain gamedev binary");
    }

    // Assume raw gzip tarball with single `gamedev` member — use external tar on unix
    #[cfg(unix)]
    {
        let tmp = tempfile::tempdir()?;
        let tgz = tmp.path().join("dl.tar.gz");
        fs::write(&tgz, archive_bytes)?;
        let status = std::process::Command::new("tar")
            .args(["-xzf"])
            .arg(&tgz)
            .arg("-C")
            .arg(tmp.path())
            .status()?;
        if !status.success() {
            bail!("failed to extract tar.gz");
        }
        let bin = tmp.path().join("gamedev");
        if !bin.is_file() {
            bail!("tar.gz did not contain gamedev");
        }
        fs::copy(&bin, out)?;
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(out)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(out, perms)?;
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        fs::write(out, archive_bytes)?;
        Ok(())
    }
}

#[cfg(unix)]
fn replace_executable(exe: &Path, tmp: &Path) -> Result<()> {
    if let Err(e) = fs::rename(tmp, exe) {
        fs::copy(tmp, exe)
            .with_context(|| format!("copy {} -> {}: {e}", tmp.display(), exe.display()))?;
        fs::remove_file(tmp)?;
    }
    Ok(())
}

#[cfg(windows)]
fn replace_executable(exe: &Path, tmp: &Path) -> Result<()> {
    let backup = exe.with_extension("old.exe");
    let _ = fs::remove_file(&backup);
    if exe.exists() {
        fs::rename(exe, &backup)?;
    }
    fs::rename(tmp, exe)?;
    let _ = fs::remove_file(&backup);
    Ok(())
}

pub fn base_from_server_url(server_url: &str) -> String {
    platform_base_from_graphql(server_url)
}
