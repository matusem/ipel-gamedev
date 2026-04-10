//! Thin command entrypoints (CLI and TUI dispatch here).

use std::fs;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::api;
use crate::auth::{self, AuthEntry};
use crate::build;
use crate::cli::{
    BackendKind, BuildArgs, DeployArgs, DraftsArgs, DraftsSubcommands, LoginArgs, ManifestArgs,
    ManifestSubcommands, TestArgs,
};
use crate::project::{load_config, resolve_java_backend_dir, resolve_logic_dir};

pub fn run_init(args: crate::cli::InitArgs) -> Result<()> {
    crate::scaffold::cmd_init(args)
}

pub fn run_build(args: BuildArgs) -> Result<()> {
    build::run(args)
}

pub fn run_login(args: LoginArgs) -> Result<()> {
    let token = api::gql_create_publish_token(&args.server_url, &args.user_id)?;
    let path = auth::auth_db_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut db = auth::load_auth_store(&path)?;
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

pub fn run_deploy(args: DeployArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    run_build(BuildArgs {
        project_dir: Some(root.clone()),
        out: None,
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

pub fn run_test(args: TestArgs) -> Result<()> {
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
        BackendKind::Java => {
            build::ensure_java_for_gradle()?;
            let java_dir = resolve_java_backend_dir(&root);
            if !java_dir.join("settings.gradle.kts").is_file() {
                bail!("Java backend missing {}", java_dir.join("settings.gradle.kts").display());
            }
            let gradlew = java_dir.join("gradlew.bat");
            let gradlew_unix = java_dir.join("gradlew");
            let mut cmd = if gradlew.is_file() {
                Command::new(gradlew)
            } else if gradlew_unix.is_file() {
                Command::new(gradlew_unix)
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
        }
        _ => bail!("backend test adapter not implemented yet"),
    }
    Ok(())
}
