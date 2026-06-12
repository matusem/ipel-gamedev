use crate::component_db::ComponentDb;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameManifest {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub min_players: u32,
    pub max_players: u32,
    #[serde(default)]
    pub description: String,
    /// Relative path under `client/` for the create-config iframe (default checks `config.html`).
    #[serde(default)]
    pub config_entry: Option<String>,
    /// Relative path under `client/` for the post-game result iframe (default checks `result.html`).
    #[serde(default)]
    pub result_entry: Option<String>,
    /// Relative path under `client/` for the game info/rules screen (default checks `about.html`).
    #[serde(default)]
    pub about_entry: Option<String>,
    /// JSON Schema for the `config` string posted by the lobby config UI (JSON instance).
    #[serde(default)]
    pub config_schema: Option<Value>,
    /// When false, lobby hides the spectate action for in-game rooms.
    #[serde(default = "default_supports_spectators")]
    pub supports_spectators: bool,
}

fn default_supports_spectators() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct GameType {
    pub manifest: GameManifest,
    pub client_dir: PathBuf,
    /// Relative path under `client/` served at `/games/{name}/{path}` when present.
    pub config_ui_path: Option<String>,
    /// Relative path under `client/` for `/games/{name}/{path}` result screen iframe when present.
    pub result_ui_path: Option<String>,
    /// Relative path under `client/` for `/games/{name}/{path}` about/info screen when present.
    pub about_ui_path: Option<String>,
}

#[derive(Clone)]
pub struct GameRegistry {
    game_types: Vec<GameType>,
}

impl GameRegistry {
    pub fn load(games_dir: &Path, component_db: &ComponentDb) -> Self {
        let game_types = Self::scan_game_types(games_dir, component_db);
        tracing::info!(count = game_types.len(), "loaded game types");
        Self { game_types }
    }

    fn scan_game_types(games_dir: &Path, component_db: &ComponentDb) -> Vec<GameType> {
        let mut game_types = Vec::new();

        let entries = match std::fs::read_dir(games_dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(games_dir = %games_dir.display(), error = %e, "could not read games directory");
                return game_types;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("manifest.json");
            let logic_path = path.join("logic.wasm");
            let client_dir = path.join("client");

            let manifest: GameManifest = match std::fs::read_to_string(&manifest_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(path = %manifest_path.display(), error = %e, "invalid game manifest");
                        continue;
                    }
                },
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "missing game manifest");
                    continue;
                }
            };

            match std::fs::read(&logic_path) {
                Ok(wasm_bytes) => {
                    match component_db.insert_components_as_wasm_bytes(&manifest.name, &wasm_bytes) {
                        Ok(_) => tracing::info!(
                            display_name = %manifest.display_name,
                            game_type = %manifest.name,
                            "loaded game component"
                        ),
                        Err(e) => {
                            tracing::warn!(game_type = %manifest.name, error = %e, "failed to load game wasm");
                            continue;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "missing logic.wasm");
                    continue;
                }
            }

            let config_candidate = manifest
                .config_entry
                .clone()
                .unwrap_or_else(|| "config.html".to_string());
            let config_ui_path = if client_dir.join(&config_candidate).is_file() {
                Some(config_candidate)
            } else {
                None
            };

            let result_candidate = manifest
                .result_entry
                .clone()
                .unwrap_or_else(|| "result.html".to_string());
            let result_ui_path = if client_dir.join(&result_candidate).is_file() {
                Some(result_candidate)
            } else {
                None
            };

            let about_candidate = manifest
                .about_entry
                .clone()
                .unwrap_or_else(|| "about.html".to_string());
            let about_ui_path = if client_dir.join(&about_candidate).is_file() {
                Some(about_candidate)
            } else {
                None
            };

            game_types.push(GameType {
                manifest,
                client_dir,
                config_ui_path,
                result_ui_path,
                about_ui_path,
            });
        }

        game_types
    }

    pub fn reload(&mut self, games_dir: &Path, component_db: &ComponentDb) {
        self.game_types = Self::scan_game_types(games_dir, component_db);
        tracing::info!(count = self.game_types.len(), "reloaded game types");
    }

    pub fn game_types(&self) -> &[GameType] {
        &self.game_types
    }

    pub fn get_client_dir(&self, name: &str) -> Option<&Path> {
        self.game_types
            .iter()
            .find(|gt| gt.manifest.name == name)
            .map(|gt| gt.client_dir.as_path())
    }

}
