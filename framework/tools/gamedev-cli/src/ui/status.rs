//! Home dashboard status snapshot (built once per TUI session).

use std::path::Path;
use std::process::Command;

use crate::auth::{self, AuthSummary};
use crate::config;
use crate::project::{self, ProjectConfig, ProjectLayout};
use crate::version;

#[derive(Clone, Debug)]
pub struct HomeStatus {
    pub auth: Option<AuthSummary>,
    pub default_profile: String,
    pub project: Option<ProjectConfig>,
    pub layout: ProjectLayout,
    pub in_project: bool,
    pub cli_version: String,
    pub cargo_ok: bool,
    pub wasm_bindgen_ok: bool,
}

pub fn build_home_status() -> HomeStatus {
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let in_project = project::is_game_project(&cwd);
    let project = if in_project {
        project::load_config(&cwd).ok()
    } else {
        None
    };
    let layout = if in_project {
        project::detect_layout(&cwd)
    } else {
        ProjectLayout::Unknown
    };
    let default_profile = config::load_cli_config()
        .map(|c| c.default_profile)
        .unwrap_or_else(|_| config::DEFAULT_PROFILE_NAME.to_string());

    HomeStatus {
        auth: auth::current_auth_summary(),
        default_profile,
        project,
        layout,
        in_project,
        cli_version: version::cli_version().to_string(),
        cargo_ok: tool_ok("cargo", &["--version"]),
        wasm_bindgen_ok: tool_ok("wasm-bindgen", &["--version"]),
    }
}

fn tool_ok(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn layout_label(layout: ProjectLayout) -> &'static str {
    match layout {
        ProjectLayout::FlatRustBevy => "flat rust+bevy",
        ProjectLayout::NestedRust => "nested rust",
        ProjectLayout::NestedJava => "nested java",
        ProjectLayout::Unknown => "unknown",
    }
}
