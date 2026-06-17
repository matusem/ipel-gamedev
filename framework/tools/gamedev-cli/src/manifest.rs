//! Read and update the game project's local `manifest.json`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::reporter;

#[derive(Debug, Clone)]
pub struct ManifestFields {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
}

pub fn manifest_path(root: &Path) -> PathBuf {
    root.join("manifest.json")
}

fn read_manifest_value(root: &Path) -> Result<Value> {
    let path = manifest_path(root);
    if !path.is_file() {
        bail!("missing {} — run from a game project directory", path.display());
    }
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

pub fn read_fields(root: &Path) -> Result<ManifestFields> {
    let v = read_manifest_value(root)?;
    let obj = v
        .as_object()
        .context("manifest.json root must be a JSON object")?;
    Ok(ManifestFields {
        name: string_field(obj, "name")?,
        display_name: string_field(obj, "display_name")?,
        version: string_field(obj, "version")?,
        description: string_field(obj, "description")?,
    })
}

fn string_field(obj: &serde_json::Map<String, Value>, key: &str) -> Result<String> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .with_context(|| format!("manifest.json missing or invalid string field `{key}`"))
}

pub fn show_local(root: &Path) -> Result<()> {
    let path = manifest_path(root);
    let v = read_manifest_value(root)?;
    reporter::section(&format!("manifest.json ({})", path.display()));
    if let Some(obj) = v.as_object() {
        let rows: Vec<Vec<String>> = obj
            .iter()
            .map(|(k, val)| vec![k.clone(), val.to_string()])
            .collect();
        reporter::print_table(&["Key", "Value"], rows);
    } else {
        reporter::hint(&v.to_string());
    }
    reporter::status("manifest", "local manifest.json");
    Ok(())
}

pub fn edit_local(root: &Path, fields: &ManifestFields) -> Result<()> {
    let path = manifest_path(root);
    let mut v = read_manifest_value(root)?;
    let obj = v
        .as_object_mut()
        .context("manifest.json root must be a JSON object")?;
    obj.insert("name".into(), Value::String(fields.name.clone()));
    obj.insert(
        "display_name".into(),
        Value::String(fields.display_name.clone()),
    );
    obj.insert("version".into(), Value::String(fields.version.clone()));
    obj.insert(
        "description".into(),
        Value::String(fields.description.clone()),
    );
    fs::write(&path, serde_json::to_string_pretty(&v)?)
        .with_context(|| format!("write {}", path.display()))?;
    reporter::status("manifest", &format!("updated {}", path.display()));
    Ok(())
}
