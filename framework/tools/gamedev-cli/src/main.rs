use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use clap::{Args, Parser, Subcommand, ValueEnum};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

mod init_scaffold;
mod ui;

#[derive(Parser)]
#[command(name = "gamedev-cli")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Init(InitArgs),
    Build(BuildArgs),
    Login(LoginArgs),
    Deploy(DeployArgs),
    Drafts(DraftsArgs),
    Manifest(ManifestArgs),
    Test(TestArgs),
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
}

#[derive(Args)]
pub struct LoginArgs {
    #[arg(long, default_value = "http://localhost:8080/graphql")]
    pub server_url: String,
    #[arg(long)]
    pub user_id: String,
}

#[derive(Args)]
pub struct DeployArgs {
    #[arg(long)]
    pub project_dir: Option<PathBuf>,
    #[arg(long, default_value = "http://localhost:8080/graphql")]
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
    #[arg(long, default_value = "http://localhost:8080/graphql")]
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
    #[arg(long, default_value = "http://localhost:8080/graphql")]
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

#[derive(Debug, Clone, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    Rust,
    Java,
    Csharp,
    Cpp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ValueEnum)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum JsTemplate {
    VanillaVite,
    PlainStatic,
    ReactVite,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    name: String,
    backend: BackendKind,
    frontend: FrontendKind,
}

#[derive(Debug, Deserialize)]
struct UploadResp {
    data: Option<UploadData>,
    errors: Option<serde_json::Value>,
}
#[derive(Debug, Deserialize)]
struct UploadData {
    #[serde(rename = "uploadGameZip")]
    upload_game_zip: UploadGameZip,
}
#[derive(Debug, Deserialize)]
struct UploadGameZip {
    #[serde(rename = "uploadId")]
    upload_id: String,
    draft: Option<DraftLite>,
    report: ValidationReport,
}
#[derive(Debug, Deserialize)]
struct DraftLite {
    id: String,
    #[serde(rename = "gameName")]
    game_name: String,
    version: String,
    status: String,
}
#[derive(Debug, Deserialize)]
struct ValidationReport {
    ok: bool,
    errors: i32,
    warnings: i32,
    infos: i32,
    diagnostics: Vec<ValidationDiagnostic>,
}
#[derive(Debug, Deserialize)]
struct ValidationDiagnostic {
    severity: String,
    code: String,
    message: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(cmd) = cli.command {
        return match cmd {
            Commands::Init(args) => init_scaffold::cmd_init(args),
            Commands::Build(args) => cmd_build(args),
            Commands::Login(args) => cmd_login(args),
            Commands::Deploy(args) => cmd_deploy(args),
            Commands::Drafts(args) => cmd_drafts(args),
            Commands::Manifest(args) => cmd_manifest(args),
            Commands::Test(args) => cmd_test(args),
        };
    }
    loop {
        match ui::run_once(current_user_label())? {
            ui::UiCommand::Init(args) => init_scaffold::cmd_init(args)?,
            ui::UiCommand::Login(args) => cmd_login(args)?,
            ui::UiCommand::Build(args) => cmd_build(args)?,
            ui::UiCommand::Deploy(args) => cmd_deploy(args)?,
            ui::UiCommand::Drafts(args) => cmd_drafts(args)?,
            ui::UiCommand::Manifest(args) => cmd_manifest(args)?,
            ui::UiCommand::Test(args) => cmd_test(args)?,
            ui::UiCommand::ExitProgram => break,
        }
    }
    Ok(())
}

fn current_user_label() -> Option<String> {
    let path = auth_db_path().ok()?;
    let db = load_auth_store(&path).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    db.into_iter().find(|e| e.expires_at > now).map(|e| e.user_id)
}

fn load_config(root: &Path) -> Result<ProjectConfig> {
    let s = fs::read_to_string(root.join("gamedev.toml")).context("missing gamedev.toml")?;
    Ok(toml::from_str(&s)?)
}

fn cmd_build(args: BuildArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    let cfg = load_config(&root)?;
    let stage = tempfile::tempdir()?;
    fs::copy(root.join("manifest.json"), stage.path().join("manifest.json"))?;
    match cfg.backend {
        BackendKind::Rust => {
            let cargo_ok = Command::new("cargo")
                .arg("--version")
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !cargo_ok {
                bail!("cargo is not available on PATH; install Rust toolchain first");
            }
            let component_dir = resolve_component_dir(&root);
            let status = Command::new("cargo")
                .arg("component")
                .arg("build")
                .arg("--release")
                .current_dir(&component_dir)
                .status()
                .context("failed to execute `cargo component build --release`")?;
            if !status.success() {
                bail!(
                    "component build failed; install cargo-component and ensure component crate is valid"
                );
            }
            let built_wasm = find_built_component_wasm(&root, &component_dir)?;
            fs::copy(built_wasm, stage.path().join("logic.wasm"))
                .context("failed to copy built component to logic.wasm")?;
        }
        _ => bail!("backend adapter not implemented yet"),
    }

    validate_logic_wasm_file(&stage.path().join("logic.wasm"))?;

    let client_src = root.join("client");
    if !client_src.is_dir() {
        bail!("missing client/ directory");
    }
    for f in REQUIRED_CLIENT_HTML {
        if !client_src.join(f).is_file() {
            bail!("missing required client/{f}");
        }
    }

    if matches!(cfg.frontend, FrontendKind::Bevy) {
        build_bevy_wasm_bindgen_client(&root, &client_src)?;
    }

    copy_dir_recursive(&client_src, &stage.path().join("client"))?;

    match cfg.frontend {
        FrontendKind::Js | FrontendKind::Ts => {
            merge_frontend_web_build_into_client(&root, stage.path())?;
        }
        FrontendKind::Bevy => merge_bevy_build_into_client(&root, stage.path())?,
        _ => {}
    }

    validate_staged_pack(&cfg, stage.path())?;

    let out = args.out.unwrap_or(root.join("dist/game.zip"));
    if let Some(p) = out.parent() {
        fs::create_dir_all(p)?;
    }
    create_zip(stage.path(), &out)?;
    println!("Built package: {}", out.display());
    Ok(())
}

const REQUIRED_CLIENT_HTML: [&str; 4] =
    ["index.html", "config.html", "result.html", "about.html"];

fn validate_logic_wasm_file(path: &Path) -> Result<()> {
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

fn validate_staged_pack(cfg: &ProjectConfig, stage_root: &Path) -> Result<()> {
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
        FrontendKind::Bevy => {
            validate_wasm_bindgen_client_artifacts(&client)?;
            validate_index_html_js_imports_resolve(&client)?;
        }
        FrontendKind::Js | FrontendKind::Ts => {
            validate_vite_or_static_client_has_js(&client)?;
        }
        FrontendKind::Dioxus
        | FrontendKind::Unity
        | FrontendKind::Godot
        | FrontendKind::Threejs => {}
    }

    Ok(())
}

fn validate_wasm_bindgen_client_artifacts(client_dir: &Path) -> Result<()> {
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
                "Bevy web pack invalid: {name} is present but {stem}.js is missing (wasm-bindgen output incomplete)"
            );
        }
        pairs.push((stem, name));
    }
    if pairs.is_empty() {
        bail!(
            "Bevy frontend requires wasm-bindgen browser artifacts in client/ (expected at least one *_bg.wasm + matching *.js). \
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

fn validate_index_html_js_imports_resolve(client_dir: &Path) -> Result<()> {
    let html = fs::read_to_string(client_dir.join("index.html"))
        .context("read client/index.html")?;
    let refs = extract_quoted_from_imports(&html);
    let js_refs: Vec<_> = refs.into_iter().filter(|r| r.ends_with(".js")).collect();
    if js_refs.is_empty() {
        bail!(
            "client/index.html has no `from \"./…js\"` module import; Bevy play UI must load the wasm-bindgen entry script."
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

fn resolve_bevy_dir(root: &Path) -> Option<PathBuf> {
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

/// Read `[package].name` without parsing the full manifest as `toml::Value`.
/// Cargo accepts table headers like `[target.'cfg(...)'.dependencies]` that some `toml` crate
/// versions reject; scanning avoids that mismatch.
fn read_package_name(cargo_toml: &Path) -> Result<String> {
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

fn ensure_wasm_browser_tooling(root: &Path) -> Result<()> {
    let cfg_path = root.join(".cargo/config.toml");
    if !cfg_path.is_file() {
        if let Some(parent) = cfg_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &cfg_path,
            include_str!("../templates/misc/dot_cargo_config_wasm.toml"),
        )
        .with_context(|| format!("write {}", cfg_path.display()))?;
        println!(
            "note: wrote {} (needed for getrandom on wasm32-unknown-unknown)",
            cfg_path.display()
        );
    }
    let _ = Command::new("rustup")
        .args(["target", "add", "wasm32-unknown-unknown"])
        .status();
    Ok(())
}

fn build_bevy_wasm_bindgen_client(root: &Path, client_dir: &Path) -> Result<()> {
    let bevy_dir = resolve_bevy_dir(root).context("frontend=bevy but no bevy/Cargo.toml (or frontend/bevy/Cargo.toml)")?;
    let pkg = read_package_name(&bevy_dir.join("Cargo.toml"))?;
    ensure_wasm_browser_tooling(root)?;

    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--target")
        .arg("wasm32-unknown-unknown")
        .arg("-p")
        .arg(&pkg)
        .current_dir(root)
        .status()
        .context("failed to spawn `cargo build` for Bevy wasm")?;
    if !status.success() {
        bail!("Bevy wasm build failed (`cargo build --release --target wasm32-unknown-unknown -p {pkg}`).");
    }

    let stem = pkg.replace('-', "_");
    let wasm_path = root
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{stem}.wasm"));
    if !wasm_path.is_file() {
        bail!(
            "expected wasm at {} — check Bevy package/binary name matches [package].name (hyphens become underscores in the file name)",
            wasm_path.display()
        );
    }

    let wg_ok = Command::new("wasm-bindgen")
        .arg(&wasm_path)
        .arg("--out-dir")
        .arg(client_dir)
        .arg("--target")
        .arg("web")
        .arg("--no-typescript")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !wg_ok {
        bail!(
            "wasm-bindgen failed or is not on PATH. Install with: cargo install wasm-bindgen-cli"
        );
    }

    Ok(())
}

fn create_zip(stage_root: &Path, out: &Path) -> Result<()> {
    let file = fs::File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for p in ["manifest.json", "logic.wasm"] {
        let bytes = fs::read(stage_root.join(p))?;
        zip.start_file(p, opts)?;
        zip.write_all(&bytes)?;
    }
    zip.add_directory("client/", opts)?;
    zip_client_dir_recursive(stage_root, stage_root.join("client"), &mut zip, opts)?;
    zip.finish()?;
    Ok(())
}

fn merge_frontend_web_build_into_client(root: &Path, stage_root: &Path) -> Result<()> {
    let web_dir = root.join("frontend").join("web");
    if !web_dir.exists() {
        return Ok(());
    }

    let npm_ok = Command::new("npm")
        .arg("--version")
        .current_dir(&web_dir)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !npm_ok {
        println!("warning: npm not available, skipping frontend web build");
        return Ok(());
    }

    let install_status = Command::new("npm").arg("install").current_dir(&web_dir).status();
    if install_status.as_ref().map(|s| !s.success()).unwrap_or(true) {
        println!("warning: npm install failed, keeping static client html");
        return Ok(());
    }

    let build_status = Command::new("npm").arg("run").arg("build").current_dir(&web_dir).status();
    if build_status.as_ref().map(|s| !s.success()).unwrap_or(true) {
        println!("warning: npm build failed, keeping static client html");
        return Ok(());
    }

    let dist_dir = web_dir.join("dist");
    if !dist_dir.exists() {
        println!("warning: frontend dist/ missing after build, keeping static client html");
        return Ok(());
    }

    copy_dir_recursive(&dist_dir, &stage_root.join("client"))?;
    Ok(())
}

fn merge_bevy_build_into_client(root: &Path, stage_root: &Path) -> Result<()> {
    // Accept both flat (`bevy/dist`) and nested (`frontend/bevy/dist`) layouts.
    let candidates = [
        root.join("bevy").join("dist"),
        root.join("frontend").join("bevy").join("dist"),
    ];

    let Some(dist_dir) = candidates.iter().find(|d| d.exists()) else {
        println!("warning: bevy dist/ missing, keeping static client html");
        return Ok(());
    };

    copy_dir_recursive(dist_dir, &stage_root.join("client"))?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(src_path, dst_path)?;
        }
    }
    Ok(())
}

fn zip_client_dir_recursive(
    stage_root: &Path,
    current: PathBuf,
    zip: &mut zip::ZipWriter<fs::File>,
    opts: zip::write::SimpleFileOptions,
) -> Result<()> {
    for entry in fs::read_dir(&current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            zip_client_dir_recursive(stage_root, path, zip, opts)?;
        } else {
            let rel = path
                .strip_prefix(stage_root)?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(&path)?;
            zip.start_file(rel, opts)?;
            zip.write_all(&bytes)?;
        }
    }
    Ok(())
}

fn auth_db_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir().unwrap_or(std::env::current_dir()?);
    Ok(base.join("gamedev-cli").join("auth.json"))
}

fn cmd_login(args: LoginArgs) -> Result<()> {
    let token = gql_create_publish_token(&args.server_url, &args.user_id)?;
    let path = auth_db_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut db = load_auth_store(&path)?;
    db.retain(|e| e.server_url != args.server_url);
    db.push(AuthEntry {
        server_url: args.server_url,
        token: token.token,
        expires_at: token.expires_at,
        user_id: token.user_id,
    });
    fs::write(path, serde_json::to_vec_pretty(&db)?)?;
    println!("Login successful. Token expires at {}", token.expires_at);
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct AuthEntry {
    server_url: String,
    token: String,
    user_id: String,
    expires_at: i64,
}
struct StoredToken {
    token: String,
}

fn load_auth_store(path: &Path) -> Result<Vec<AuthEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn load_token(server_url: &str) -> Result<StoredToken> {
    let path = auth_db_path()?;
    let db = load_auth_store(&path)?;
    let tok = db
        .into_iter()
        .find(|e| e.server_url == server_url)
        .context("run login first")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    if tok.expires_at <= now {
        bail!("stored token expired, run login again");
    }
    Ok(StoredToken { token: tok.token })
}

fn cmd_deploy(args: DeployArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    cmd_build(BuildArgs {
        project_dir: Some(root.clone()),
        out: None,
    })?;
    let zip_path = root.join("dist/game.zip");
    let tok = load_token(&args.server_url)?;
    let resp = gql_upload_game_zip(&args.server_url, &tok.token, &zip_path)?;
    println!(
        "Upload {} report ok={} errors={} warnings={} infos={}",
        resp.upload_id, resp.report.ok, resp.report.errors, resp.report.warnings, resp.report.infos
    );
    for d in &resp.report.diagnostics {
        println!("[{}] {}: {}", d.severity, d.code, d.message);
    }
    if !resp.report.ok {
        bail!("validation failed");
    }
    let Some(draft) = resp.draft else {
        bail!("no draft returned");
    };
    println!(
        "Draft {} {} {} {}",
        draft.id, draft.game_name, draft.version, draft.status
    );
    if !args.draft_only && args.auto_publish {
        gql_simple_mutation(&args.server_url, &tok.token, "publishGameDraft", &draft.id)?;
        println!("Published draft {}", draft.id);
    }
    Ok(())
}

fn cmd_drafts(args: DraftsArgs) -> Result<()> {
    let tok = load_token(&args.server_url)?;
    match args.command {
        DraftsSubcommands::List => {
            let q = r#"query { myGameDrafts { id gameName version status } }"#;
            println!("{}", gql_raw(&args.server_url, &tok.token, q, json!({}))?);
        }
        DraftsSubcommands::Publish { draft_id } => {
            gql_simple_mutation(&args.server_url, &tok.token, "publishGameDraft", &draft_id)?
        }
        DraftsSubcommands::Unpublish { draft_id } => {
            gql_simple_mutation(&args.server_url, &tok.token, "unpublishGameDraft", &draft_id)?
        }
        DraftsSubcommands::Discard { draft_id } => {
            gql_simple_mutation(&args.server_url, &tok.token, "discardGameDraft", &draft_id)?
        }
    }
    Ok(())
}

fn cmd_manifest(args: ManifestArgs) -> Result<()> {
    let tok = load_token(&args.server_url)?;
    match args.command {
        ManifestSubcommands::Show { draft_id } => {
            let q = r#"query($id: ID!) { gameDraft(id: $id) { id gameName displayName version status manifestJson } }"#;
            println!(
                "{}",
                gql_raw(&args.server_url, &tok.token, q, json!({ "id": draft_id }))?
            );
        }
        ManifestSubcommands::Edit { draft_id, name, display_name, version, description } => {
            let q = r#"mutation($draftId: ID!, $name: String!, $displayName: String!, $version: String!, $description: String!) {
              updateGameDraftManifest(draftId: $draftId, name: $name, displayName: $displayName, version: $version, description: $description) { id gameName version status }
            }"#;
            println!("{}", gql_raw(&args.server_url, &tok.token, q, json!({
                "draftId": draft_id, "name": name, "displayName": display_name, "version": version, "description": description
            }))?);
        }
    }
    Ok(())
}

fn cmd_test(args: TestArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    let cfg = load_config(&root)?;
    match cfg.backend {
        BackendKind::Rust => {
            let logic_dir = resolve_logic_dir(&root);
            let status = Command::new("cargo")
                .arg("test")
                .current_dir(logic_dir)
                .status()?;
            if !status.success() {
                bail!("tests failed");
            }
        }
        _ => bail!("backend test adapter not implemented yet"),
    }
    Ok(())
}

fn resolve_component_dir(root: &Path) -> PathBuf {
    let flat = root.join("component");
    if flat.exists() {
        return flat;
    }
    root.join("backend").join("rust").join("component")
}

fn find_built_component_wasm(root: &Path, component_dir: &Path) -> Result<PathBuf> {
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

fn resolve_logic_dir(root: &Path) -> PathBuf {
    let flat = root.join("logic");
    if flat.exists() {
        return flat;
    }
    root.join("backend").join("rust").join("logic")
}

struct PublishTokenResp {
    token: String,
    user_id: String,
    expires_at: i64,
}

fn gql_create_publish_token(server_url: &str, user_id: &str) -> Result<PublishTokenResp> {
    let q = r#"mutation($ttlDays: Int!) { createPublishToken(ttlDays: $ttlDays) { token userId expiresAt } }"#;
    let body = gql_raw(server_url, user_id, q, json!({ "ttlDays": 7 }))?;
    let v: serde_json::Value = serde_json::from_str(&body)?;
    let t = &v["data"]["createPublishToken"];
    Ok(PublishTokenResp {
        token: t["token"].as_str().unwrap_or_default().to_string(),
        user_id: t["userId"].as_str().unwrap_or(user_id).to_string(),
        expires_at: t["expiresAt"].as_i64().unwrap_or(0),
    })
}

fn gql_upload_game_zip(server_url: &str, token: &str, zip: &Path) -> Result<UploadGameZip> {
    let bytes = fs::read(zip)?;
    let q = r#"mutation($filename: String!, $zipBase64: String!) {
      uploadGameZip(filename: $filename, zipBase64: $zipBase64) {
        uploadId
        report { ok errors warnings infos diagnostics { severity code message } }
        draft { id gameName version status }
      }
    }"#;
    let raw = gql_raw(
        server_url,
        token,
        q,
        json!({
            "filename": zip.file_name().unwrap_or_default().to_string_lossy(),
            "zipBase64": base64::engine::general_purpose::STANDARD.encode(bytes)
        }),
    )?;
    let parsed: UploadResp = serde_json::from_str(&raw)?;
    if let Some(errs) = parsed.errors {
        bail!("graphql errors: {errs}");
    }
    Ok(parsed.data.context("missing data")?.upload_game_zip)
}

fn gql_simple_mutation(server_url: &str, token: &str, field: &str, draft_id: &str) -> Result<()> {
    let q = format!(
        "mutation($draftId: ID!) {{ {}(draftId: $draftId) {{ id status }} }}",
        field
    );
    let _ = gql_raw(server_url, token, &q, json!({"draftId": draft_id}))?;
    Ok(())
}

fn gql_raw(
    server_url: &str,
    bearer: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<String> {
    let client = Client::new();
    let res = client
        .post(server_url)
        .header("Authorization", format!("Bearer {}", bearer))
        .json(&json!({ "query": query, "variables": variables }))
        .send()?
        .text()?;
    Ok(res)
}
