//! Build game zip: Rust component, Bevy wasm-bindgen, frontend merge, validation.

mod validate;

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::cli::{BackendKind, BuildArgs, FrontendKind};
use crate::pack::{copy_dir_recursive, create_zip};
use crate::project::{
    find_built_component_wasm, load_config, read_package_name, resolve_bevy_dir, resolve_component_dir,
};

pub use validate::{
    REQUIRED_CLIENT_HTML, validate_logic_wasm_file, validate_staged_pack,
};

pub fn run(args: BuildArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    let cfg = load_config(&root)?;
    let stage = tempfile::tempdir()?;
    fs::copy(root.join("manifest.json"), stage.path().join("manifest.json"))?;
    match cfg.backend {
        BackendKind::Rust => {
            let cargo_ok = Command::new("cargo")
                .arg("--version")
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !cargo_ok {
                bail!("cargo is not available on PATH; install Rust toolchain first");
            }
            let component_dir = resolve_component_dir(&root);
            let status = Command::new("cargo")
                .arg("component")
                .arg("build")
                .arg("--release")
                .current_dir(&component_dir)
                .status()
                .context("failed to execute `cargo component build --release`")?;
            if !status.success() {
                bail!(
                    "component build failed; install cargo-component and ensure component crate is valid"
                );
            }
            let built_wasm = find_built_component_wasm(&root, &component_dir)?;
            fs::copy(built_wasm, stage.path().join("logic.wasm"))
                .context("failed to copy built component to logic.wasm")?;
        }
        _ => bail!("backend adapter not implemented yet"),
    }

    validate_logic_wasm_file(&stage.path().join("logic.wasm"))?;

    let client_src = root.join("client");
    if !client_src.is_dir() {
        bail!("missing client/ directory");
    }
    for f in REQUIRED_CLIENT_HTML {
        if !client_src.join(f).is_file() {
            bail!("missing required client/{f}");
        }
    }

    if matches!(cfg.frontend, FrontendKind::Bevy) {
        build_bevy_wasm_bindgen_client(&root, &client_src)?;
    }

    copy_dir_recursive(&client_src, &stage.path().join("client"))?;

    match cfg.frontend {
        FrontendKind::Js | FrontendKind::Ts => {
            merge_frontend_web_build_into_client(&root, stage.path())?;
        }
        FrontendKind::Bevy => merge_bevy_build_into_client(&root, stage.path())?,
        _ => {}
    }

    validate_staged_pack(&cfg, stage.path())?;

    let out = args.out.unwrap_or(root.join("dist/game.zip"));
    if let Some(p) = out.parent() {
        fs::create_dir_all(p)?;
    }
    create_zip(stage.path(), &out)?;
    println!("Built package: {}", out.display());
    Ok(())
}

fn build_bevy_wasm_bindgen_client(root: &Path, client_dir: &Path) -> Result<()> {
    let bevy_dir = resolve_bevy_dir(root)
        .context("frontend=bevy but no bevy/Cargo.toml (or frontend/bevy/Cargo.toml)")?;
    let pkg = read_package_name(&bevy_dir.join("Cargo.toml"))?;
    ensure_wasm_browser_tooling(root)?;

    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--target")
        .arg("wasm32-unknown-unknown")
        .arg("-p")
        .arg(&pkg)
        .current_dir(root)
        .status()
        .context("failed to spawn `cargo build` for Bevy wasm")?;
    if !status.success() {
        bail!("Bevy wasm build failed (`cargo build --release --target wasm32-unknown-unknown -p {pkg}`).");
    }

    let stem = pkg.replace('-', "_");
    let wasm_path = root
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{stem}.wasm"));
    if !wasm_path.is_file() {
        bail!(
            "expected wasm at {} — check Bevy package/binary name matches [package].name (hyphens become underscores in the file name)",
            wasm_path.display()
        );
    }

    let wg_ok = Command::new("wasm-bindgen")
        .arg(&wasm_path)
        .arg("--out-dir")
        .arg(client_dir)
        .arg("--target")
        .arg("web")
        .arg("--no-typescript")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !wg_ok {
        bail!(
            "wasm-bindgen failed or is not on PATH. Install with: cargo install wasm-bindgen-cli"
        );
    }

    Ok(())
}

fn ensure_wasm_browser_tooling(root: &Path) -> Result<()> {
    let cfg_path = root.join(".cargo/config.toml");
    if !cfg_path.is_file() {
        if let Some(parent) = cfg_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &cfg_path,
            include_str!("../../templates/misc/dot_cargo_config_wasm.toml"),
        )
        .with_context(|| format!("write {}", cfg_path.display()))?;
        println!(
            "note: wrote {} (needed for getrandom on wasm32-unknown-unknown)",
            cfg_path.display()
        );
    }
    let _ = Command::new("rustup")
        .args(["target", "add", "wasm32-unknown-unknown"])
        .status();
    Ok(())
}

fn merge_frontend_web_build_into_client(root: &Path, stage_root: &Path) -> Result<()> {
    let web_dir = root.join("frontend").join("web");
    if !web_dir.exists() {
        return Ok(());
    }

    let npm_ok = Command::new("npm")
        .arg("--version")
        .current_dir(&web_dir)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !npm_ok {
        println!("warning: npm not available, skipping frontend web build");
        return Ok(());
    }

    let install_status = Command::new("npm").arg("install").current_dir(&web_dir).status();
    if install_status.as_ref().map(|s| !s.success()).unwrap_or(true) {
        println!("warning: npm install failed, keeping static client html");
        return Ok(());
    }

    let build_status = Command::new("npm").arg("run").arg("build").current_dir(&web_dir).status();
    if build_status.as_ref().map(|s| !s.success()).unwrap_or(true) {
        println!("warning: npm build failed, keeping static client html");
        return Ok(());
    }

    let dist_dir = web_dir.join("dist");
    if !dist_dir.exists() {
        println!("warning: frontend dist/ missing after build, keeping static client html");
        return Ok(());
    }

    copy_dir_recursive(&dist_dir, &stage_root.join("client"))?;
    Ok(())
}

fn merge_bevy_build_into_client(root: &Path, stage_root: &Path) -> Result<()> {
    let candidates = [
        root.join("bevy").join("dist"),
        root.join("frontend").join("bevy").join("dist"),
    ];

    let Some(dist_dir) = candidates.iter().find(|d| d.exists()) else {
        println!("warning: bevy dist/ missing, keeping static client html");
        return Ok(());
    };

    copy_dir_recursive(dist_dir, &stage_root.join("client"))?;
    Ok(())
}
