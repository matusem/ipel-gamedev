//! Generate typed client bindings and JSON Schema from canonical game types.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::cli::{BackendKind, CodegenArgs};
use crate::project::{
    find_framework_root, load_config, resolve_java_backend_dir,
};
use crate::reporter::{self, LoggedCommand};

pub fn run(args: CodegenArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    let cfg = load_config(&root)?;
    reporter::section("Codegen");
    match cfg.backend {
        BackendKind::Rust => codegen_rust_backend(&root)?,
        BackendKind::Java => codegen_java_backend(&root)?,
        _ => bail!("codegen: backend {:?} not supported", cfg.backend),
    }
    reporter::status("codegen", "finished - see generated/ and frontend/web/src/generated/");
    Ok(())
}

fn generated_dir(root: &Path) -> PathBuf {
    root.join("generated")
}

fn ts_out_dir(root: &Path) -> PathBuf {
    root.join("frontend").join("web").join("src").join("generated")
}

fn codegen_rust_backend(root: &Path) -> Result<()> {
    let shared_types = find_shared_types_crate(root)?;
    let out_schema = generated_dir(root);
    fs::create_dir_all(&out_schema)?;
    let ts_out = ts_out_dir(root);
    fs::create_dir_all(&ts_out)?;

    let status = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg(shared_types.join("Cargo.toml"))
        .arg("--features")
        .arg("typegen")
        .arg("--bin")
        .arg("export_ts")
        .current_dir(root)
        .status_logged()
        .context("run export_ts for shared-types")?;
    if !status.success() {
        bail!("Rust type export failed (cargo run --features typegen --bin export_ts)");
    }

    let status = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg(shared_types.join("Cargo.toml"))
        .arg("--features")
        .arg("schemars")
        .arg("--bin")
        .arg("export_schema")
        .current_dir(root)
        .status_logged()
        .context("run export_schema for shared-types")?;
    if !status.success() {
        reporter::warn(
            "json-schema",
            "export_schema bin missing or failed - ensure shared-types has schemars feature",
        );
    } else {
        let schema_src = shared_types.join("schema");
        if schema_src.is_dir() {
            copy_dir(&schema_src, &out_schema.join("schema"))?;
        }
    }

    reporter::status("rust", &format!("TS -> {}", ts_out.display()));
    Ok(())
}

fn codegen_java_backend(root: &Path) -> Result<()> {
    let java_dir = resolve_java_backend_dir(root);
    let game_dir = java_dir.join("game");
    if !game_dir.is_dir() {
        bail!("Java game module missing at {}", game_dir.display());
    }
    let out_schema = generated_dir(root);
    fs::create_dir_all(&out_schema)?;

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
    let status = cmd
        .arg(":game:exportJsonSchema")
        .args(["--no-daemon", "-q"])
        .status_logged()
        .context("run Gradle :game:exportJsonSchema")?;
    if status.success() {
        let schema_src = game_dir.join("build").join("schema");
        if schema_src.is_dir() {
            copy_dir(&schema_src, &out_schema.join("schema"))?;
            codegen_ts_from_schema(&out_schema.join("schema"), &ts_out_dir(root))?;
        }
        reporter::status("java", &format!("schema -> {}", out_schema.display()));
        Ok(())
    } else {
        reporter::warn(
            "java-schema",
            "Gradle exportJsonSchema failed - add task or define types in game module",
        );
        Ok(())
    }
}

pub fn find_shared_types_crate(root: &Path) -> Result<PathBuf> {
    let nested = root.join("backend").join("rust").join("shared-types");
    if nested.join("Cargo.toml").is_file() {
        return Ok(nested);
    }
    let flat = root.join("shared-types");
    if flat.join("Cargo.toml").is_file() {
        return Ok(flat);
    }
    if let Some(fw) = find_framework_root(root) {
        let p = fw.join("tools/gamedev-cli/templates/backend");
        let _ = p;
    }
    bail!("shared-types crate not found (expected backend/rust/shared-types/)");
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let dest = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

/// Minimal JSON Schema -> TypeScript interface generator (properties only).
fn codegen_ts_from_schema(schema_dir: &Path, ts_out: &Path) -> Result<()> {
    fs::create_dir_all(ts_out)?;
    let index = schema_dir.join("game-types.json");
    if !index.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&index)?;
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    let mut out = String::from("/** Generated from JSON Schema - do not edit. */\n\n");
    if let Some(defs) = v.get("definitions").and_then(|d| d.as_object()) {
        for (name, def) in defs {
            out.push_str(&schema_def_to_ts(name, def));
            out.push('\n');
        }
    } else if let Some(props) = v.get("properties") {
        out.push_str(&format!("export interface GameTypes {{\n"));
        for (k, ty) in props.as_object().unwrap_or(&serde_json::Map::new()) {
            out.push_str(&format!("  {}: {};\n", k, json_type_to_ts(ty)));
        }
        out.push_str("}\n");
    }
    fs::write(ts_out.join("game-types.ts"), out)?;
    Ok(())
}

fn schema_def_to_ts(name: &str, def: &serde_json::Value) -> String {
    let mut s = format!("export interface {name} {{\n");
    if let Some(props) = def.get("properties").and_then(|p| p.as_object()) {
        for (k, ty) in props {
            s.push_str(&format!("  {}: {};\n", k, json_type_to_ts(ty)));
        }
    }
    s.push_str("}\n");
    s
}

fn json_type_to_ts(v: &serde_json::Value) -> String {
    match v.get("type").and_then(|t| t.as_str()) {
        Some("string") => "string".to_string(),
        Some("integer") | Some("number") => "number".to_string(),
        Some("boolean") => "boolean".to_string(),
        Some("array") => {
            let inner = v
                .get("items")
                .map(json_type_to_ts)
                .unwrap_or_else(|| "unknown".to_string());
            format!("{inner}[]")
        }
        Some("object") => "Record<string, unknown>".to_string(),
        _ => "unknown".to_string(),
    }
}
