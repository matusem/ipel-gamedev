//! Build game zip: Rust component, Bevy wasm-bindgen, frontend merge, validation.

mod java_gradle;
mod validate;

pub(crate) use java_gradle::ensure_java_for_gradle;

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::cli::{BackendKind, BuildArgs, FrontendKind};
use crate::pack::{copy_dir_recursive, create_zip};
use crate::project::{
    find_built_component_wasm, find_built_java_logic_wasm, load_config, read_package_name, resolve_bevy_dir,
    resolve_component_dir, resolve_java_backend_dir,
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
        BackendKind::Java => {
            ensure_java_for_gradle()?;
            let java_dir = resolve_java_backend_dir(&root);
            if !java_dir.join("settings.gradle.kts").is_file() {
                bail!(
                    "Java backend expected {} (or java/settings.gradle.kts) with Gradle settings",
                    java_dir.display()
                );
            }
            let gradlew = java_dir.join("gradlew.bat");
            let gradlew_unix = java_dir.join("gradlew");
            let mut cmd = if gradlew.is_file() {
                Command::new(gradlew)
            } else if gradlew_unix.is_file() {
                Command::new(gradlew_unix)
            } else {
                Command::new("gradle")
            };
            cmd.current_dir(&java_dir);
            let export_task = if java_dir.join("component").join("build.gradle.kts").is_file() {
                ":component:exportLogicWasm"
            } else {
                "exportLogicWasm"
            };
            let status = cmd
                .arg(export_task)
                .args(["--no-daemon", "-q"])
                .status()
                .context("failed to run Gradle for Java logic.wasm; install JDK 21+ and Gradle (see sdk/java/README.md)")?;
            if !status.success() {
                bail!("Java Gradle build failed (`:component:exportLogicWasm`)");
            }
            let built = find_built_java_logic_wasm(&java_dir)?;
            fs::copy(built, stage.path().join("logic.wasm"))
                .context("failed to copy Java logic.wasm into stage")?;
            eprintln!(
                "warning: Java/Fermyon TeaVM `logic.wasm` is a core Wasm module (WASI + TeaVM runtime \
                 imports), not a WebAssembly Component from `cargo component build`. Upload validation \
                 may reject it. Use `backend = \"rust\"` for deployable component zips, or see \
                 sdk/java/README.md (Deployment / server upload)."
            );
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

    let mut dist_ready = false;
    if npm_ok {
        let install_status = Command::new("npm").arg("install").current_dir(&web_dir).status();
        if install_status.as_ref().map(|s| !s.success()).unwrap_or(true) {
            println!("warning: npm install failed, falling back to static frontend/web merge");
        } else {
            let build_status = Command::new("npm").arg("run").arg("build").current_dir(&web_dir).status();
            if build_status.as_ref().map(|s| !s.success()).unwrap_or(true) {
                println!("warning: npm run build failed, falling back to static frontend/web merge");
            } else {
                let dist_dir = web_dir.join("dist");
                if dist_dir.is_dir() {
                    copy_dir_recursive(&dist_dir, &stage_root.join("client"))?;
                    dist_ready = true;
                } else {
                    println!(
                        "warning: frontend/web has no dist/ after build (plain-static script?), falling back to static merge"
                    );
                }
            }
        }
    } else {
        println!("warning: npm not available, merging frontend/web/src into packaged client/");
    }

    if !dist_ready {
        merge_frontend_web_static_into_staged_client(root, stage_root)?;
    }
    Ok(())
}

/// When there is no Vite `dist/` (no npm, failed install/build, or `echo` placeholder build), copy
/// `frontend/web/src/main.js` into the staged `client/main.js` and ensure `index.html` loads it via
/// `import … from "./main.js"` so packaging validation passes.
fn merge_frontend_web_static_into_staged_client(root: &Path, stage_root: &Path) -> Result<()> {
    let main_src = root.join("frontend/web/src/main.js");
    if !main_src.is_file() {
        println!(
            "warning: missing frontend/web/src/main.js; add it or install Node and run npm run build in frontend/web"
        );
        return Ok(());
    }
    let client_dir = stage_root.join("client");
    fs::copy(&main_src, client_dir.join("main.js"))
        .with_context(|| format!("copy {} to staged client/main.js", main_src.display()))?;
    ensure_client_index_imports_main_js(&client_dir)?;
    Ok(())
}

fn ensure_client_index_imports_main_js(client_dir: &Path) -> Result<()> {
    let index_path = client_dir.join("index.html");
    let html = fs::read_to_string(&index_path).context("read staged client/index.html")?;
    if html.contains("from \"./main.js\"") || html.contains("from './main.js'") {
        return Ok(());
    }
    let inject = "<script type=\"module\">import * as _ from \"./main.js\";</script>\n";
    let new_html = if let Some(pos) = html.rfind("</body>") {
        let mut s = html;
        s.insert_str(pos, inject);
        s
    } else {
        format!("{html}{inject}")
    };
    fs::write(&index_path, new_html).context("write staged client/index.html")?;
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
