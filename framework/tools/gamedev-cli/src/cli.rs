//! Clap definitions and shared CLI enums (single source of truth for flags and `gamedev.toml` kinds).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

pub use crate::defaults::DEFAULT_GRAPHQL_URL;

#[derive(Parser)]
#[command(
    name = "gamedev-cli",
    version,
    about = "UPJS GDD Platform developer CLI"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Init(InitArgs),
    Build(BuildArgs),
    Login(LoginArgs),
    Logout(LogoutArgs),
    Deploy(DeployArgs),
    Drafts(DraftsArgs),
    Manifest(ManifestArgs),
    Test(TestArgs),
    Doctor(DoctorArgs),
    Validate(ValidateArgs),
    /// Generate typed client bindings and JSON Schema from game types
    Codegen(CodegenArgs),
    /// Download and install the platform-matching CLI release
    Update(UpdateArgs),
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
    /// Server profile from `config.toml` (`local`, `prod`). Overrides default server URL when set.
    #[arg(long)]
    pub profile: Option<String>,
    /// Deprecated: raw user UUID for publish-token bootstrap. Prefer `--display-name` + `--password` or `--publish-token`.
    #[arg(long, hide = true)]
    pub user_id: Option<String>,
    #[arg(long)]
    pub display_name: Option<String>,
    #[arg(long)]
    pub password: Option<String>,
    /// Publish token from lobby settings (alternative to password login).
    #[arg(long)]
    pub publish_token: Option<String>,
    /// Log in via browser loopback (default when no other credentials are given).
    #[arg(long)]
    pub web: bool,
}

#[derive(Args)]
pub struct LogoutArgs {
    #[arg(long, default_value = DEFAULT_GRAPHQL_URL)]
    pub server_url: String,
    /// Server profile from `config.toml` (`local`, `prod`).
    #[arg(long)]
    pub profile: Option<String>,
    /// Remove credentials for every stored server.
    #[arg(long)]
    pub all: bool,
}

#[derive(Args)]
pub struct DeployArgs {
    #[arg(long)]
    pub project_dir: Option<PathBuf>,
    #[arg(long, default_value = DEFAULT_GRAPHQL_URL)]
    pub server_url: String,
    #[arg(long)]
    pub profile: Option<String>,
    /// Publish the draft after a successful upload (default: upload only).
    #[arg(long)]
    pub publish: bool,
    /// Deprecated: use default upload-only behavior or `--publish`.
    #[arg(long, hide = true)]
    pub auto_publish: bool,
    /// Deprecated: upload-only is already the default.
    #[arg(long, hide = true)]
    pub draft_only: bool,
}

#[derive(Args)]
pub struct DraftsArgs {
    #[command(subcommand)]
    pub command: DraftsSubcommands,
    #[arg(long, default_value = DEFAULT_GRAPHQL_URL)]
    pub server_url: String,
    #[arg(long)]
    pub profile: Option<String>,
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
    #[arg(long)]
    pub profile: Option<String>,
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
    /// Platform base URL (e.g. https://gamedev.jinxwashere.com) - checks CLI/SDK versions against production
    #[arg(long)]
    pub platform: Option<String>,
    /// Verify scaffold template coverage for all backend x frontend combinations
    #[arg(long)]
    pub matrix: bool,
}

#[derive(Args)]
pub struct CodegenArgs {
    #[arg(long)]
    pub project_dir: Option<PathBuf>,
}

use crate::defaults::DEFAULT_PLATFORM_BASE;

#[derive(Args)]
pub struct UpdateArgs {
    /// GraphQL or platform base URL (default: production for packaged releases, localhost for dev builds)
    #[arg(long, default_value = DEFAULT_PLATFORM_BASE)]
    pub platform: String,
    /// Exit with error if an update is available (CI-friendly)
    #[arg(long)]
    pub check: bool,
}

#[derive(Args)]
pub struct ValidateArgs {
    #[arg(long)]
    pub project_dir: Option<PathBuf>,
    /// Path to logic.wasm (default: dist/game.zip -> logic.wasm or backend build output).
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
