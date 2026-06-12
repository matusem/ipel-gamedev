//! Clap definitions and shared CLI enums (single source of truth for flags and `gamedev.toml` kinds).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

/// Default GraphQL endpoint (matches `#[arg(long, default_value = ...)]` on several commands).
pub const DEFAULT_GRAPHQL_URL: &str = "http://localhost:8080/graphql";

#[derive(Parser)]
#[command(name = "gamedev-cli")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Init(InitArgs),
    Build(BuildArgs),
    Login(LoginArgs),
    Deploy(DeployArgs),
    Drafts(DraftsArgs),
    Manifest(ManifestArgs),
    Test(TestArgs),
    Doctor(DoctorArgs),
    Validate(ValidateArgs),
}

#[derive(Args)]
pub struct InitArgs {
    pub name: Option<String>,
    #[arg(long, value_enum)]
    pub backend: Option<BackendKind>,
    #[arg(long, value_enum)]
    pub frontend: Option<FrontendKind>,
    #[arg(long, value_enum)]
    pub js_template: Option<JsTemplate>,
}

#[derive(Args)]
pub struct BuildArgs {
    #[arg(long)]
    pub project_dir: Option<PathBuf>,
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Fail the build instead of falling back when npm/frontend steps fail.
    #[arg(long)]
    pub strict: bool,
}

#[derive(Args)]
pub struct LoginArgs {
    #[arg(long, default_value = DEFAULT_GRAPHQL_URL)]
    pub server_url: String,
    /// Deprecated: raw user UUID for publish-token bootstrap. Prefer `--display-name` + `--password`.
    #[arg(long)]
    pub user_id: Option<String>,
    #[arg(long)]
    pub display_name: Option<String>,
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args)]
pub struct DeployArgs {
    #[arg(long)]
    pub project_dir: Option<PathBuf>,
    #[arg(long, default_value = DEFAULT_GRAPHQL_URL)]
    pub server_url: String,
    #[arg(long)]
    pub auto_publish: bool,
    #[arg(long)]
    pub draft_only: bool,
}

#[derive(Args)]
pub struct DraftsArgs {
    #[command(subcommand)]
    pub command: DraftsSubcommands,
    #[arg(long, default_value = DEFAULT_GRAPHQL_URL)]
    pub server_url: String,
}

#[derive(Subcommand)]
pub enum DraftsSubcommands {
    List,
    Publish { draft_id: String },
    Unpublish { draft_id: String },
    Discard { draft_id: String },
}

#[derive(Args)]
pub struct ManifestArgs {
    #[command(subcommand)]
    pub command: ManifestSubcommands,
    #[arg(long, default_value = DEFAULT_GRAPHQL_URL)]
    pub server_url: String,
}

#[derive(Subcommand)]
pub enum ManifestSubcommands {
    Show {
        draft_id: String,
    },
    Edit {
        draft_id: String,
        name: String,
        display_name: String,
        version: String,
        description: String,
    },
}

#[derive(Args)]
pub struct TestArgs {
    #[arg(long)]
    pub project_dir: Option<PathBuf>,
}

#[derive(Args)]
pub struct DoctorArgs {
    #[arg(long)]
    pub project_dir: Option<PathBuf>,
}

#[derive(Args)]
pub struct ValidateArgs {
    #[arg(long)]
    pub project_dir: Option<PathBuf>,
    /// Path to logic.wasm (default: dist/game.zip → logic.wasm or backend build output).
    #[arg(long)]
    pub logic_wasm: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    Rust,
    Java,
    Csharp,
    Cpp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum FrontendKind {
    Js,
    Ts,
    Bevy,
    Dioxus,
    Unity,
    Godot,
    Threejs,
}

impl BackendKind {
    pub fn is_implemented(self) -> bool {
        matches!(self, BackendKind::Rust | BackendKind::Java)
    }
}

impl FrontendKind {
    pub fn is_implemented(self) -> bool {
        matches!(
            self,
            FrontendKind::Js | FrontendKind::Ts | FrontendKind::Bevy | FrontendKind::Dioxus
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum JsTemplate {
    VanillaVite,
    PlainStatic,
}
