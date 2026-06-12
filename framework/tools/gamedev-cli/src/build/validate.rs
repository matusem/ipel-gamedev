use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::cli::FrontendKind;
use crate::project::ProjectConfig;

pub const REQUIRED_CLIENT_HTML: [&str; 4] =
    ["index.html", "config.html", "result.html", "about.html"];

/// Dry-run upload validation: wasm magic + WebAssembly Component metadata (requires `wasm-tools` on PATH).
pub fn validate_logic_component_file(path: &Path) -> Result<()> {
    validate_logic_wasm_file(path)?;
    let output = Command::new("wasm-tools")
        .args([
            "component",
            "wit",
            path.to_str()
                .ok_or_else(|| anyhow::anyhow!("logic.wasm path is not valid UTF-8"))?,
        ])
        .output()
        .with_context(|| "run wasm-tools component wit (install wasm-tools CLI)")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "logic.wasm failed component validation (wasm-tools component wit). \
             Build with cargo component or Java exportLogicComponent. {stderr}"
        );
    }
    Ok(())
}

pub fn validate_logic_wasm_file(path: &Path) -> Result<()> {
    let mut f = fs::File::open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let mut magic = [0u8; 4];
    f.read_exact(&mut magic)
        .context("logic.wasm too small or unreadable")?;
    if &magic != b"\0asm" {
        bail!(
            "logic.wasm does not look like WebAssembly (expected wasm magic). Rebuild the component crate."
        );
    }
    Ok(())
}

pub fn validate_staged_pack(cfg: &ProjectConfig, stage_root: &Path) -> Result<()> {
    let logic = stage_root.join("logic.wasm");
    validate_logic_wasm_file(&logic)?;

    let client = stage_root.join("client");
    if !client.is_dir() {
        bail!("internal error: staged client/ missing");
    }
    for f in REQUIRED_CLIENT_HTML {
        if !client.join(f).is_file() {
            bail!("packaging incomplete: client/{f} missing after staging");
        }
    }

    match cfg.frontend {
        FrontendKind::Bevy | FrontendKind::Dioxus => {
            validate_wasm_bindgen_client_artifacts(&client, cfg.frontend)?;
            validate_index_html_js_imports_resolve(&client, cfg.frontend)?;
        }
        FrontendKind::Js | FrontendKind::Ts => {
            validate_vite_or_static_client_has_js(&client)?;
        }
        FrontendKind::Unity | FrontendKind::Godot | FrontendKind::Threejs => {}
    }

    Ok(())
}

fn validate_wasm_bindgen_client_artifacts(client_dir: &Path, frontend: FrontendKind) -> Result<()> {
    let label = match frontend {
        FrontendKind::Bevy => "Bevy",
        FrontendKind::Dioxus => "Dioxus",
        _ => "wasm-bindgen",
    };
    let mut pairs = Vec::new();
    for entry in fs::read_dir(client_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy().into_owned();
        if !name.ends_with("_bg.wasm") {
            continue;
        }
        let stem = name
            .strip_suffix("_bg.wasm")
            .expect("suffix checked")
            .to_string();
        let js = client_dir.join(format!("{stem}.js"));
        if !js.is_file() {
            bail!(
                "{label} web pack invalid: {name} is present but {stem}.js is missing (wasm-bindgen output incomplete)"
            );
        }
        pairs.push((stem, name));
    }
    if pairs.is_empty() {
        bail!(
            "{label} frontend requires wasm-bindgen browser artifacts in client/ (expected at least one *_bg.wasm + matching *.js). \
             Install wasm-bindgen-cli (`cargo install wasm-bindgen-cli`) and ensure `gamedev-cli build` can run wasm-bindgen."
        );
    }
    Ok(())
}

fn extract_quoted_from_imports(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in html.split("from \"").skip(1) {
        let Some(end) = part.find('"') else {
            continue;
        };
        let path = part[..end].trim();
        if path.starts_with("./") {
            out.push(path.trim_start_matches("./").to_string());
        }
    }
    for part in html.split("from '").skip(1) {
        let Some(end) = part.find('\'') else {
            continue;
        };
        let path = part[..end].trim();
        if path.starts_with("./") {
            out.push(path.trim_start_matches("./").to_string());
        }
    }
    out
}

fn validate_index_html_js_imports_resolve(client_dir: &Path, frontend: FrontendKind) -> Result<()> {
    let label = match frontend {
        FrontendKind::Bevy => "Bevy",
        FrontendKind::Dioxus => "Dioxus",
        _ => "wasm-bindgen",
    };
    let html = fs::read_to_string(client_dir.join("index.html"))
        .context("read client/index.html")?;
    let refs = extract_quoted_from_imports(&html);
    let js_refs: Vec<_> = refs.into_iter().filter(|r| r.ends_with(".js")).collect();
    if js_refs.is_empty() {
        bail!(
            "client/index.html has no `from \"./…js\"` module import; {label} play UI must load the wasm-bindgen entry script."
        );
    }
    for r in &js_refs {
        let p = client_dir.join(r);
        if !p.is_file() {
            bail!(
                "client/index.html imports \"./{r}\" but that file is not in the packaged client/ (got {}?)",
                if p.exists() { "not a file" } else { "missing" }
            );
        }
    }
    Ok(())
}

fn validate_vite_or_static_client_has_js(client_dir: &Path) -> Result<()> {
    let html = fs::read_to_string(client_dir.join("index.html"))
        .context("read client/index.html")?;
    let refs = extract_quoted_from_imports(&html);
    let js_refs: Vec<_> = refs.into_iter().filter(|r| r.ends_with(".js")).collect();
    for r in &js_refs {
        let p = client_dir.join(r);
        if !p.is_file() {
            bail!(
                "client/index.html imports \"./{r}\" but that file is missing after the web build merge. \
                 Run npm build in frontend/web (or fix paths)."
            );
        }
    }
    if js_refs.is_empty() && !dir_contains_js_recursive(client_dir)? {
        bail!(
            "JS/TS frontend: client/ has no .js files under client/ and index.html does not import a local ./…js module. \
             Run npm run build in frontend/web so the bundle is merged into client/."
        );
    }
    Ok(())
}

fn dir_contains_js_recursive(dir: &Path) -> Result<bool> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if dir_contains_js_recursive(&path)? {
                return Ok(true);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("js") {
            return Ok(true);
        }
    }
    Ok(false)
}
