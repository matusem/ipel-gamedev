//! End-to-end smoke: `init` → `test` → `build` for Rust backend + Dioxus frontend.
//!
//! Run manually or in CI:
//! ```text
//! set GAMEDEV_CLI_SMOKE=1
//! cargo test -p gamedev-cli smoke_rust_dioxus -- --ignored --nocapture
//! ```

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use gamedev_cli::build::validate_staged_pack;
use gamedev_cli::cli::{BackendKind, BuildArgs, FrontendKind, InitArgs, TestArgs};
use gamedev_cli::commands::{run_build, run_init, run_test};
use gamedev_cli::project::load_config;

const GAME_NAME: &str = "smoke_dioxus";

struct ChdirGuard {
    previous: PathBuf,
}

impl ChdirGuard {
    fn new(dir: &Path) -> Result<Self> {
        let previous = std::env::current_dir().context("read current dir")?;
        std::env::set_current_dir(dir).with_context(|| format!("cd {}", dir.display()))?;
        Ok(Self { previous })
    }
}

impl Drop for ChdirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous);
    }
}

fn framework_root() -> Result<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .context("locate framework root from tools/gamedev-cli")
}

fn command_ok(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn assert_prerequisites() -> Result<()> {
    if !command_ok("cargo", &["--version"]) {
        bail!("cargo is not on PATH");
    }
    if !command_ok("cargo", &["component", "--version"]) {
        bail!("cargo-component is not on PATH (`cargo install cargo-component`)");
    }
    if !command_ok("wasm-bindgen", &["--version"]) {
        bail!("wasm-bindgen-cli is not on PATH (`cargo install wasm-bindgen-cli`)");
    }
    let target_installed = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("wasm32-unknown-unknown"))
        .unwrap_or(false);
    if !target_installed {
        bail!("rustup target wasm32-unknown-unknown is not installed");
    }
    Ok(())
}

fn extract_zip_to_dir(zip_path: &Path, out_dir: &Path) -> Result<()> {
    fs::create_dir_all(out_dir)?;
    let file = fs::File::open(zip_path).with_context(|| format!("open {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out_path = out_dir.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

fn assert_wasm_magic(path: &Path) -> Result<()> {
    let mut f = fs::File::open(path)?;
    let mut magic = [0u8; 4];
    f.read_exact(&mut magic)?;
    if &magic != b"\0asm" {
        bail!("{} is not a wasm file", path.display());
    }
    Ok(())
}

#[test]
#[ignore = "e2e smoke (set GAMEDEV_CLI_SMOKE=1, then: cargo test -p gamedev-cli smoke_rust_dioxus -- --ignored --nocapture)"]
fn smoke_rust_dioxus_init_test_build() -> Result<()> {
    if std::env::var("GAMEDEV_CLI_SMOKE").ok().as_deref() != Some("1") {
        eprintln!("skip smoke_rust_dioxus: set GAMEDEV_CLI_SMOKE=1 to run");
        return Ok(());
    }

    assert_prerequisites()?;

    let fw = framework_root()?;
    let smoke_parent = fw.join("target").join("smoke");
    fs::create_dir_all(&smoke_parent)?;
    let workspace = tempfile::tempdir_in(&smoke_parent).context("create smoke temp dir")?;
    let game_root = workspace.path().join(GAME_NAME);

    let _cwd = ChdirGuard::new(workspace.path())?;
    run_init(InitArgs {
        name: Some(GAME_NAME.to_string()),
        backend: Some(BackendKind::Rust),
        frontend: Some(FrontendKind::Dioxus),
        js_template: None,
    })?;

    assert!(
        game_root.join("gamedev.toml").is_file(),
        "gamedev.toml missing after init"
    );
    assert!(
        game_root.join("frontend/dioxus/Cargo.toml").is_file(),
        "dioxus frontend missing after init"
    );

    run_test(TestArgs {
        project_dir: Some(game_root.clone()),
    })?;

    run_build(BuildArgs {
        project_dir: Some(game_root.clone()),
        out: None,
        strict: false,
    })?;

    let zip_path = game_root.join("dist/game.zip");
    assert!(zip_path.is_file(), "dist/game.zip missing after build");

    let cfg = load_config(&game_root)?;
    let stage = tempfile::tempdir().context("stage dir for zip validation")?;
    extract_zip_to_dir(&zip_path, stage.path())?;
    validate_staged_pack(&cfg, stage.path())?;
    assert_wasm_magic(&stage.path().join("logic.wasm"))?;

    let client = stage.path().join("client");
    let mut has_bindgen_pair = false;
    for entry in fs::read_dir(&client)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with("_bg.wasm") {
            let stem = name.strip_suffix("_bg.wasm").unwrap();
            assert!(
                client.join(format!("{stem}.js")).is_file(),
                "missing wasm-bindgen js for {name}"
            );
            has_bindgen_pair = true;
        }
    }
    assert!(has_bindgen_pair, "client/ missing wasm-bindgen artifacts");

    eprintln!("smoke_rust_dioxus OK: {}", zip_path.display());
    Ok(())
}
