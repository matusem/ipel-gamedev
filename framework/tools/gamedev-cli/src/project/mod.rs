//! Project config (`gamedev.toml`) and filesystem layout helpers.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::cli::{BackendKind, FrontendKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectLayout {
    FlatRustBevy,
    NestedRust,
    NestedJava,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub backend: BackendKind,
    pub frontend: FrontendKind,
}

pub fn is_game_project(root: &Path) -> bool {
    root.join("gamedev.toml").is_file()
}

pub fn load_config(root: &Path) -> Result<ProjectConfig> {
    let s = fs::read_to_string(root.join("gamedev.toml")).context("missing gamedev.toml")?;
    Ok(toml::from_str(&s)?)
}

pub fn detect_layout(root: &Path) -> ProjectLayout {
    if root.join("logic").join("Cargo.toml").is_file()
        && root.join("component").join("Cargo.toml").is_file()
        && root.join("bevy").join("Cargo.toml").is_file()
    {
        return ProjectLayout::FlatRustBevy;
    }
    if root
        .join("backend")
        .join("rust")
        .join("logic")
        .join("Cargo.toml")
        .is_file()
    {
        return ProjectLayout::NestedRust;
    }
    if root
        .join("backend")
        .join("java")
        .join("settings.gradle.kts")
        .is_file()
        || root.join("java").join("settings.gradle.kts").is_file()
    {
        return ProjectLayout::NestedJava;
    }
    ProjectLayout::Unknown
}

/// Directory to run `cargo test` from (workspace root for flat Bevy layout).
pub fn resolve_test_dir(root: &Path) -> PathBuf {
    if root.join("tests").join("Cargo.toml").is_file() && root.join("Cargo.toml").is_file() {
        return root.to_path_buf();
    }
    resolve_logic_dir(root)
}

pub fn resolve_component_dir(root: &Path) -> PathBuf {
    let flat = root.join("component");
    if flat.exists() {
        return flat;
    }
    root.join("backend").join("rust").join("component")
}

pub fn resolve_logic_dir(root: &Path) -> PathBuf {
    let flat = root.join("logic");
    if flat.exists() {
        return flat;
    }
    root.join("backend").join("rust").join("logic")
}

pub fn resolve_bevy_dir(root: &Path) -> Option<PathBuf> {
    let flat = root.join("bevy");
    if flat.join("Cargo.toml").is_file() {
        return Some(flat);
    }
    let nested = root.join("frontend").join("bevy");
    if nested.join("Cargo.toml").is_file() {
        return Some(nested);
    }
    None
}

pub fn resolve_dioxus_dir(root: &Path) -> Option<PathBuf> {
    let nested = root.join("frontend").join("dioxus");
    if nested.join("Cargo.toml").is_file() {
        return Some(nested);
    }
    None
}

/// Directory containing `sdk/java/game/settings.gradle.kts` (framework repo root containing the Java SDK).
pub fn find_framework_root(from: &Path) -> Option<PathBuf> {
    let mut dir = Some(from);
    while let Some(current) = dir {
        if current.join("sdk/java/game/settings.gradle.kts").is_file()
            || current.join("sdk/rust/shared-types/Cargo.toml").is_file()
        {
            return Some(current.to_path_buf());
        }
        dir = current.parent();
    }
    None
}

pub fn find_upjs_gdd_js(from: &Path) -> Option<PathBuf> {
    find_framework_root(from).map(|fw| fw.join("sdk/js"))
}

pub fn find_upjs_gdd_rust_crate(from: &Path, crate_dir: &str) -> Option<PathBuf> {
    find_framework_root(from).map(|fw| fw.join("sdk/rust").join(crate_dir))
}

pub fn relative_path_from(from: &Path, to: &Path) -> Option<String> {
    pathdiff::diff_paths(to, from).map(|p| p.to_string_lossy().replace('\\', "/"))
}

pub fn resolve_java_backend_dir(root: &Path) -> PathBuf {
    let nested = root.join("backend").join("java");
    if nested.join("settings.gradle.kts").is_file() {
        return nested;
    }
    root.join("java")
}

pub fn find_built_java_logic_wasm(java_backend: &Path) -> Result<PathBuf> {
    let component = if java_backend
        .join("component")
        .join("build.gradle.kts")
        .is_file()
    {
        java_backend.join("component")
    } else {
        java_backend.to_path_buf()
    };
    let candidates = [
        component.join("build").join("out").join("logic.wasm"),
        component
            .join("build")
            .join("generated")
            .join("teavm")
            .join("wasm-gc")
            .join("logic.wasm"),
        component
            .join("target")
            .join("generated")
            .join("wasm")
            .join("teavm-wasm")
            .join("classes.wasm"),
    ];
    for p in candidates {
        if p.is_file() {
            return Ok(p);
        }
    }
    bail!(
        "no logic.wasm found under {}; run Gradle `exportLogicComponent` in {}",
        component.display(),
        java_backend.display()
    );
}

/// `cargo` for a scaffolded game workspace (do not inherit the framework's `CARGO_TARGET_DIR`).
pub fn game_cargo_command() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.env_remove("CARGO_TARGET_DIR");
    cmd
}

/// Candidate `target/` roots for a workspace build (`target/`, `CARGO_TARGET_DIR`, member crate).
pub fn cargo_target_roots(root: &Path, member_dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        let p = PathBuf::from(dir);
        roots.push(if p.is_absolute() { p } else { root.join(p) });
    }
    roots.push(root.join("target"));
    roots.push(member_dir.join("target"));
    roots.sort();
    roots.dedup();
    roots
}

pub fn find_built_component_wasm(root: &Path, component_dir: &Path) -> Result<PathBuf> {
    let mut out_dirs: Vec<PathBuf> = Vec::new();
    for base in cargo_target_roots(root, component_dir) {
        for triple in ["wasm32-wasip1", "wasm32-wasip2"] {
            out_dirs.push(base.join(triple).join("release"));
        }
    }

    let mut wasm_candidates: Vec<(PathBuf, SystemTime)> = Vec::new();
    for out_dir in out_dirs {
        if !out_dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&out_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("wasm") {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            wasm_candidates.push((path, modified));
        }
    }

    wasm_candidates.sort_by_key(|(_, modified)| *modified);
    let Some((latest, _)) = wasm_candidates.pop() else {
        bail!("no .wasm artifact produced by `cargo component build --release`");
    };
    Ok(latest)
}

/// Expected `logic.wasm` path for a Rust backend before packaging (used by validate).
pub fn resolve_rust_logic_wasm_path(root: &Path) -> PathBuf {
    let component_dir = resolve_component_dir(root);
    for base in cargo_target_roots(root, &component_dir) {
        for triple in ["wasm32-wasip1", "wasm32-wasip2"] {
            let p = base.join(triple).join("release").join("logic.wasm");
            if p.is_file() {
                return p;
            }
        }
    }
    component_dir
        .join("target")
        .join("wasm32-wasip1")
        .join("release")
        .join("logic.wasm")
}

/// Walk parents from `from` until `game/Cargo.toml` exists.
pub fn find_framework_game_crate(from: &Path) -> Option<PathBuf> {
    let mut dir = Some(from);
    while let Some(current) = dir {
        let candidate = current.join("game").join("Cargo.toml");
        if candidate.exists() {
            return Some(current.join("game"));
        }
        dir = current.parent();
    }
    None
}

/// Walk parents from `from` until `game-wasm-host/Cargo.toml` exists.
pub fn find_framework_game_wasm_host_crate(from: &Path) -> Option<PathBuf> {
    let mut dir = Some(from);
    while let Some(current) = dir {
        let candidate = current.join("game-wasm-host").join("Cargo.toml");
        if candidate.exists() {
            return Some(current.join("game-wasm-host"));
        }
        dir = current.parent();
    }
    None
}

/// Read `[package].name` without parsing the full manifest as `toml::Value`.
pub fn read_package_name(cargo_toml: &Path) -> Result<String> {
    let s =
        fs::read_to_string(cargo_toml).with_context(|| format!("read {}", cargo_toml.display()))?;
    let mut in_package = false;
    for raw_line in s.lines() {
        let line = raw_line
            .split_once('#')
            .map(|(before, _)| before)
            .unwrap_or(raw_line)
            .trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            if key.trim() != "name" {
                continue;
            }
            let val = val.trim();
            let Some(inner) = val.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
                continue;
            };
            if !inner.is_empty() && !inner.contains('"') {
                return Ok(inner.to_string());
            }
        }
    }
    bail!("{}: missing [package].name", cargo_toml.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detect_flat_rust_bevy_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        for d in ["logic", "component", "bevy"] {
            fs::create_dir_all(root.join(d).join("src")).unwrap();
            fs::write(root.join(d).join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        }
        assert_eq!(detect_layout(root), ProjectLayout::FlatRustBevy);
    }

    #[test]
    fn resolve_test_dir_prefers_workspace_when_tests_crate_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("logic/src")).unwrap();
        fs::create_dir_all(root.join("tests/src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"tests\"]\n",
        )
        .unwrap();
        fs::write(root.join("tests/Cargo.toml"), "[package]\nname = \"t\"\n").unwrap();
        assert_eq!(resolve_test_dir(root), root);
    }
}
