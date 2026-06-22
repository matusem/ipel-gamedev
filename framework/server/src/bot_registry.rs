use crate::component_db::ComponentDb;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotManifest {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub game_slug: String,
    pub game_version: String,
    pub contract_hash: String,
    #[serde(default)]
    pub settings_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub default_settings: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct BotType {
    pub id: String,
    pub slug: String,
    pub manifest: BotManifest,
    pub bot_wasm_path: PathBuf,
}

#[derive(Clone)]
pub struct BotRegistry {
    bots: Vec<BotType>,
}

impl BotRegistry {
    pub fn load(bots_dir: &Path, component_db: &ComponentDb) -> Self {
        let bots = Self::scan(bots_dir, component_db);
        tracing::info!(count = bots.len(), "loaded bots");
        Self { bots }
    }

    fn scan(bots_dir: &Path, component_db: &ComponentDb) -> Vec<BotType> {
        let mut out = Vec::new();
        let entries = match std::fs::read_dir(bots_dir) {
            Ok(e) => e,
            Err(_) => return out,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let slug = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if slug.is_empty() || slug.starts_with('.') {
                continue;
            }
            let manifest_path = path.join("manifest.json");
            let wasm_path = path.join("bot.wasm");
            if !manifest_path.is_file() || !wasm_path.is_file() {
                continue;
            }
            let manifest: BotManifest = match std::fs::read_to_string(&manifest_path) {
                Ok(s) => match serde_json::from_str(&s) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(slug = %slug, error = %e, "invalid bot manifest");
                        continue;
                    }
                },
                Err(e) => {
                    tracing::warn!(slug = %slug, error = %e, "cannot read bot manifest");
                    continue;
                }
            };
            if let Ok(wasm_bytes) = std::fs::read(&wasm_path) {
                if let Err(e) = component_db.insert_bot_component(&slug, &wasm_bytes) {
                    tracing::warn!(slug = %slug, error = %e, "failed to load bot wasm");
                    continue;
                }
            } else {
                continue;
            }
            out.push(BotType {
                id: slug.clone(),
                slug,
                manifest,
                bot_wasm_path: wasm_path,
            });
        }
        out
    }

    pub fn reload(&mut self, bots_dir: &Path, component_db: &ComponentDb) {
        component_db.clear_bots();
        self.bots = Self::scan(bots_dir, component_db);
    }

    pub fn bots(&self) -> &[BotType] {
        &self.bots
    }

    pub fn get(&self, slug: &str) -> Option<&BotType> {
        self.bots.iter().find(|b| b.slug == slug)
    }

    pub fn compatible_with_game(
        &self,
        game_slug: &str,
        contract_hash: &str,
    ) -> Vec<&BotType> {
        self.bots
            .iter()
            .filter(|b| b.manifest.game_slug == game_slug && b.manifest.contract_hash == contract_hash)
            .collect()
    }
}
