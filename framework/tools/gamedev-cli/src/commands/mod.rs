//! Thin command entrypoints (CLI and TUI dispatch here).

use std::fs;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::api;
use crate::auth::{self, AuthEntry};
use crate::build;
use crate::cli::{
    BackendKind, BuildArgs, DeployArgs, DoctorArgs, DraftsArgs, DraftsSubcommands, LoginArgs,
    ManifestArgs, ManifestSubcommands, TestArgs, UpdateArgs, ValidateArgs,
};
use crate::doctor::{self, has_failures, print_report};
use crate::project::{game_cargo_command, load_config, resolve_java_backend_dir, resolve_test_dir};

pub fn run_init(args: crate::cli::InitArgs) -> Result<()> {
    crate::scaffold::cmd_init(args)
}

pub fn run_build(args: BuildArgs) -> Result<()> {
    build::run(args)
}

pub fn run_login(args: LoginArgs) -> Result<()> {
    let (token, user_id, expires_at) = if let (Some(name), Some(pass)) =
        (args.display_name.as_deref(), args.password.as_deref())
    {
        let session = api::gql_login_with_password(&args.server_url, name, pass)?;
        (session.token, session.user_id, session.expires_at)
    } else if let Some(uid) = args.user_id.as_deref() {
        let publish = api::gql_create_publish_token(&args.server_url, uid)?;
        (publish.token, publish.user_id, publish.expires_at)
    } else {
        bail!("provide --display-name and --password, or deprecated --user-id");
    };
    let path = auth::auth_db_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut db = auth::load_auth_store(&path)?;
    db.retain(|e| e.server_url != args.server_url);
    db.push(AuthEntry {
        server_url: args.server_url,
        token: token.clone(),
        expires_at,
        user_id: user_id.clone(),
    });
    fs::write(path, serde_json::to_vec_pretty(&db)?)?;
    println!("Login successful for {user_id}. Token expires at {expires_at}");
    Ok(())
}

pub fn run_deploy(args: DeployArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    let base = crate::update::base_from_server_url(&args.server_url);
    if let Ok(m) = crate::platform::fetch_platform_manifest(&base) {
        crate::platform::check_local_toolchain_against_platform(&m)?;
    } else {
        eprintln!("warning: could not fetch platform manifest from {base} — skipping version check");
    }
    run_build(BuildArgs {
        project_dir: Some(root.clone()),
        out: None,
        strict: false,
    })?;
    let zip_path = root.join("dist/game.zip");
    let tok = auth::load_token(&args.server_url)?;
    let resp = api::gql_upload_game_zip(&args.server_url, &tok.token, &zip_path)?;
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
        api::gql_simple_mutation(&args.server_url, &tok.token, "publishGameDraft", &draft.id)?;
        println!("Published draft {}", draft.id);
    }
    Ok(())
}

pub fn run_drafts(args: DraftsArgs) -> Result<()> {
    let tok = auth::load_token(&args.server_url)?;
    match args.command {
        DraftsSubcommands::List => {
            let q = r#"query { myGameDrafts { id gameName version status } }"#;
            println!("{}", api::gql_raw(&args.server_url, &tok.token, q, json!({}))?);
        }
        DraftsSubcommands::Publish { draft_id } => {
            api::gql_simple_mutation(&args.server_url, &tok.token, "publishGameDraft", &draft_id)?
        }
        DraftsSubcommands::Unpublish { draft_id } => {
            api::gql_simple_mutation(&args.server_url, &tok.token, "unpublishGameDraft", &draft_id)?
        }
        DraftsSubcommands::Discard { draft_id } => {
            api::gql_simple_mutation(&args.server_url, &tok.token, "discardGameDraft", &draft_id)?
        }
    }
    Ok(())
}

pub fn run_manifest(args: ManifestArgs) -> Result<()> {
    let tok = auth::load_token(&args.server_url)?;
    match args.command {
        ManifestSubcommands::Show { draft_id } => {
            let q = r#"query($id: ID!) { gameDraft(id: $id) { id gameName displayName version status manifestJson } }"#;
            println!(
                "{}",
                api::gql_raw(&args.server_url, &tok.token, q, json!({ "id": draft_id }))?
            );
        }
        ManifestSubcommands::Edit {
            draft_id,
            name,
            display_name,
            version,
            description,
        } => {
            let q = r#"mutation($draftId: ID!, $name: String!, $displayName: String!, $version: String!, $description: String!) {
              updateGameDraftManifest(draftId: $draftId, name: $name, displayName: $displayName, version: $version, description: $description) { id gameName version status }
            }"#;
            println!(
                "{}",
                api::gql_raw(
                    &args.server_url,
                    &tok.token,
                    q,
                    json!({
                        "draftId": draft_id,
                        "name": name,
                        "displayName": display_name,
                        "version": version,
                        "description": description
                    })
                )?
            );
        }
    }
    Ok(())
}

pub fn run_doctor(args: DoctorArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    let checks = doctor::run(&root)?;
    print_report(&checks);
    if let Some(base) = args.platform.as_deref() {
        let m = crate::platform::fetch_platform_manifest(base)?;
        crate::platform::check_local_toolchain_against_platform(&m)?;
        println!("Platform compatibility: OK ({})", m.framework_version);
    }
    if has_failures(&checks) {
        bail!("doctor found blocking issues");
    }
    Ok(())
}

pub fn run_update(args: UpdateArgs) -> Result<()> {
    crate::update::run_update(&args.platform, args.check)
}

pub fn run_validate(args: ValidateArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    let logic = if let Some(p) = args.logic_wasm {
        p
    } else {
        let zip = root.join("dist/game.zip");
        if zip.is_file() {
            let f = fs::File::open(&zip)?;
            let mut archive = zip::ZipArchive::new(f)?;
            let mut entry = archive.by_name("logic.wasm")?;
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes)?;
            let tmp = tempfile::NamedTempFile::new()?;
            fs::write(tmp.path(), &bytes)?;
            tmp.path().to_path_buf()
        } else {
            let cfg = load_config(&root)?;
            match cfg.backend {
                BackendKind::Rust => {
                    root.join("backend/rust/component/target/wasm32-wasip2/release/logic.wasm")
                }
                BackendKind::Java => {
                    resolve_java_backend_dir(&root).join("component/build/out/logic.wasm")
                }
                _ => bail!("validate: specify --logic-wasm or run gamedev build first"),
            }
        }
    };
    if !logic.is_file() {
        bail!(
            "logic.wasm not found at {}. Run `gamedev build` or pass --logic-wasm",
            logic.display()
        );
    }
    build::validate_logic_component_file(&logic)?;
    println!("OK: {} is a valid WebAssembly component", logic.display());
    Ok(())
}

pub fn run_test(args: TestArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    let cfg = load_config(&root)?;
    match cfg.backend {
        BackendKind::Rust => {
            let test_dir = resolve_test_dir(&root);
            let status = game_cargo_command()
                .arg("test")
                .current_dir(&test_dir)
                .status()?;
            if !status.success() {
                bail!("tests failed");
            }
        }
        BackendKind::Java => {
            build::ensure_java_for_gradle()?;
            let java_dir = resolve_java_backend_dir(&root);
            if !java_dir.join("settings.gradle.kts").is_file() {
                bail!("Java backend missing {}", java_dir.join("settings.gradle.kts").display());
            }
            let gradlew = java_dir.join("gradlew.bat");
            let gradlew_unix = java_dir.join("gradlew");
            let mut cmd = if gradlew.is_file() {
                Command::new(&gradlew)
            } else if gradlew_unix.is_file() {
                Command::new(&gradlew_unix)
            } else {
                Command::new("gradle")
            };
            cmd.current_dir(&java_dir);
            // Game sources live in the included `game` build; the `component` project delegates to Maven.
            let compile_task = ":game:compileJava";
            let status = cmd
                .arg(compile_task)
                .args(["--no-daemon", "-q"])
                .status()
                .context("failed to run Gradle for Java backend")?;
            if !status.success() {
                bail!("Java compile failed");
            }
            let export_task = if java_dir.join("component/build.gradle.kts").is_file() {
                ":component:exportLogicComponent"
            } else {
                "exportLogicComponent"
            };
            let mut export_cmd = if gradlew.is_file() {
                Command::new(&gradlew)
            } else if gradlew_unix.is_file() {
                Command::new(&gradlew_unix)
            } else {
                Command::new("gradle")
            };
            export_cmd.current_dir(&java_dir);
            let export = export_cmd
                .arg(export_task)
                .args(["--no-daemon", "-q"])
                .status()
                .context("failed to export Java logic component")?;
            if !export.success() {
                bail!("Java exportLogicComponent failed");
            }
            let logic_out = java_dir.join("component/build/out/logic.wasm");
            let logic_out = if logic_out.is_file() {
                logic_out
            } else {
                java_dir.join("build/out/logic.wasm")
            };
            if logic_out.is_file() {
                build::validate_logic_component_file(&logic_out)?;
            }
        }
        _ => bail!("backend test adapter not implemented yet"),
    }
    Ok(())
}
