//! Project config (`gamedev.toml`) and filesystem layout helpers.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::cli::{BackendKind, FrontendKind};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub backend: BackendKind,
    pub frontend: FrontendKind,
}

pub fn load_config(root: &Path) -> Result<ProjectConfig> {
    let s = fs::read_to_string(root.join("gamedev.toml")).context("missing gamedev.toml")?;
    Ok(toml::from_str(&s)?)
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

/// Directory containing `sdk/java/game/settings.gradle.kts` (framework repo root containing the Java SDK).
pub fn find_framework_root(from: &Path) -> Option<PathBuf> {
    let mut dir = Some(from);
    while let Some(current) = dir {
        if current.join("sdk/java/game/settings.gradle.kts").is_file() {
            return Some(current.to_path_buf());
        }
        dir = current.parent();
    }
    None
}

pub fn resolve_java_backend_dir(root: &Path) -> PathBuf {
    let nested = root.join("backend").join("java");
    if nested.join("settings.gradle.kts").is_file() {
        return nested;
    }
    root.join("java")
}

pub fn find_built_java_logic_wasm(java_backend: &Path) -> Result<PathBuf> {
    let component = if java_backend.join("component").join("build.gradle.kts").is_file() {
        java_backend.join("component")
    } else {
        java_backend.to_path_buf()
    };
    let candidates = [
        component.join("build").join("out").join("logic.wasm"),
        component.join("build").join("generated").join("teavm").join("wasm-gc").join("logic.wasm"),
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
        "no logic.wasm found under {}; run Gradle `exportLogicWasm` in {}",
        component.display(),
        java_backend.display()
    );
}

pub fn find_built_component_wasm(root: &Path, component_dir: &Path) -> Result<PathBuf> {
    let out_dirs = [
        root.join("target").join("wasm32-wasip1").join("release"),
        component_dir.join("target").join("wasm32-wasip1").join("release"),
    ];

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
    let s = fs::read_to_string(cargo_toml)
        .with_context(|| format!("read {}", cargo_toml.display()))?;
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
            let Some(inner) = val
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
            else {
                continue;
            };
            if !inner.is_empty() && !inner.contains('"') {
                return Ok(inner.to_string());
            }
        }
    }
    bail!("{}: missing [package].name", cargo_toml.display())
}
