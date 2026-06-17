//! Thin command entrypoints (CLI and TUI dispatch here).

use std::fs;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::api;
use crate::auth::{self, AuthEntry};
use crate::build;
use crate::cli::{
    BackendKind, BuildArgs, CodegenArgs, DeployArgs, DoctorArgs, DraftsArgs, DraftsSubcommands,
    LoginArgs, ManifestArgs, ManifestSubcommands, LogoutArgs, TestArgs, UpdateArgs, ValidateArgs,
};
use crate::config;
use crate::doctor::{self, has_failures, print_report};
use crate::project::{
    game_cargo_command, load_config, resolve_java_backend_dir, resolve_rust_logic_wasm_path,
    resolve_test_dir,
};
use crate::reporter::{self, LoggedCommand, SpinnerFinish};

pub fn run_init(args: crate::cli::InitArgs) -> Result<()> {
    crate::scaffold::cmd_init(args)
}

pub fn run_build(args: BuildArgs) -> Result<()> {
    build::run(args)
}

pub fn run_login(args: LoginArgs) -> Result<()> {
    let server_url = config::resolve_graphql_url(args.profile.as_deref(), &args.server_url)?;
    let has_password_field = args.display_name.is_some() || args.password.is_some();
    if has_password_field && (args.display_name.is_none() || args.password.is_none()) {
        bail!("provide both --display-name and --password");
    }
    let use_web = args.web
        || (!has_password_field
            && args.publish_token.is_none()
            && args.user_id.is_none());

    let (token, user_id, expires_at) = if let Some(pt) = args.publish_token.as_deref() {
        let publish = api::store_publish_token(&server_url, pt, None)?;
        (publish.token, publish.user_id, publish.expires_at)
    } else if let (Some(name), Some(pass)) =
        (args.display_name.as_deref(), args.password.as_deref())
    {
        let session = api::gql_login_with_password(&server_url, name, pass)?;
        (session.token, session.user_id, session.expires_at)
    } else if let Some(uid) = args.user_id.as_deref() {
        reporter::warn(
            "login",
            "--user-id is deprecated; use browser login, --display-name/--password, or --publish-token",
        );
        let publish = api::gql_create_publish_token(&server_url, uid)?;
        (publish.token, publish.user_id, publish.expires_at)
    } else if use_web {
        let platform_base =
            config::resolve_platform_base(args.profile.as_deref(), &args.server_url)?;
        let web = crate::auth_web::login_via_browser(&platform_base)?;
        let publish = api::store_publish_token(&server_url, &web.token, Some(web.expires_at))?;
        (publish.token, publish.user_id, publish.expires_at)
    } else {
        bail!("provide credentials, --publish-token, or use browser login (default)");
    };
    let path = auth::auth_db_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut db = auth::load_auth_store(&path)?;
    db.retain(|e| e.server_url != server_url);
    db.push(AuthEntry {
        server_url: server_url.clone(),
        token: token.clone(),
        expires_at,
        user_id: user_id.clone(),
    });
    fs::write(path, serde_json::to_vec_pretty(&db)?)?;
    reporter::status(
        "login",
        &format!("authenticated as {user_id} on {server_url} (expires {expires_at})"),
    );
    Ok(())
}

pub fn run_logout(args: LogoutArgs) -> Result<()> {
    if args.all {
        auth::clear_all_auth()?;
        reporter::status("logout", "cleared all stored credentials");
        return Ok(());
    }

    let server_url = if args.profile.is_some() || args.server_url != crate::cli::DEFAULT_GRAPHQL_URL
    {
        config::resolve_graphql_url(args.profile.as_deref(), &args.server_url)?
    } else if let Some(auth) = auth::current_auth_summary() {
        auth.server_url
    } else {
        bail!("not logged in");
    };

    if auth::logout_server(&server_url)? {
        reporter::status("logout", &format!("signed out from {server_url}"));
    } else {
        reporter::warn("logout", &format!("no credentials stored for {server_url}"));
    }
    Ok(())
}

pub fn run_deploy(args: DeployArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    let server_url = config::resolve_graphql_url(args.profile.as_deref(), &args.server_url)?;
    let base = config::resolve_platform_base(args.profile.as_deref(), &args.server_url)?;
    if let Ok(m) = crate::platform::fetch_platform_manifest(&base) {
        crate::platform::check_local_toolchain_against_platform(&m)?;
    } else {
        reporter::warn(
            "platform-manifest",
            &format!("could not fetch from {base} - skipping version check"),
        );
    }
    run_build(BuildArgs {
        project_dir: Some(root.clone()),
        out: None,
        strict: false,
    })?;
    let zip_path = root.join("dist/game.zip");
    let tok = auth::load_token(&server_url)?;
    let zip_len = fs::metadata(&zip_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let resp = if zip_len > 0 {
        let upload_pb = reporter::progress_bytes("Uploading game package", zip_len);
        let resp = api::gql_upload_game_zip(&server_url, &tok.token, &zip_path)?;
        upload_pb.finish_and_clear();
        reporter::status("upload", "package sent to server");
        resp
    } else {
        let upload_pb = reporter::spinner("Uploading game package...");
        let resp = api::gql_upload_game_zip(&server_url, &tok.token, &zip_path)?;
        upload_pb.finish_ok("package sent to server");
        resp
    };
    reporter::print_validation_report(
        &resp.upload_id,
        resp.report.ok,
        resp.report.errors,
        resp.report.warnings,
        resp.report.infos,
        &resp.report.diagnostics,
    );
    if !resp.report.ok {
        bail!("validation failed");
    }
    let Some(draft) = resp.draft else {
        bail!("no draft returned");
    };
    reporter::status(
        "draft",
        &format!(
            "{} slug={} name={} {} {}",
            draft.id, draft.slug, draft.game_name, draft.version, draft.status
        ),
    );
    if !draft.slug.is_empty() {
        reporter::hint(&format!(
            "Live catalog URL path: /games/{}/",
            draft.slug
        ));
    }
    let should_publish = (args.publish || args.auto_publish) && !args.draft_only;
    if should_publish {
        api::gql_simple_mutation(&server_url, &tok.token, "publishGameDraft", &draft.id)?;
        reporter::status("publish", &format!("published draft {}", draft.id));
    } else {
        reporter::hint("upload-only - pass --publish to publish the draft");
    }
    Ok(())
}

pub fn run_drafts(args: DraftsArgs) -> Result<()> {
    let server_url = config::resolve_graphql_url(args.profile.as_deref(), &args.server_url)?;
    let tok = auth::load_token(&server_url)?;
    match args.command {
        DraftsSubcommands::List => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Draft {
                id: String,
                slug: String,
                game_name: String,
                version: String,
                status: String,
            }
            let q = r#"query { myGameDrafts { id slug gameName version status } }"#;
            let raw = api::gql_raw(&server_url, &tok.token, q, json!({}))?;
            let v: serde_json::Value = serde_json::from_str(&raw)?;
            let drafts: Vec<Draft> = serde_json::from_value(
                v.get("data")
                    .and_then(|d| d.get("myGameDrafts"))
                    .cloned()
                    .unwrap_or(json!([])),
            )?;
            if drafts.is_empty() {
                reporter::hint("no drafts found");
            } else {
                reporter::print_table(
                    &["ID", "Slug", "Name", "Version", "Status"],
                    drafts
                        .iter()
                        .map(|d| {
                            vec![
                                d.id.clone(),
                                d.slug.clone(),
                                d.game_name.clone(),
                                d.version.clone(),
                                d.status.clone(),
                            ]
                        })
                        .collect(),
                );
            }
            reporter::status("drafts", &format!("{} draft(s)", drafts.len()));
        }
        DraftsSubcommands::Publish { draft_id } => {
            api::gql_simple_mutation(&server_url, &tok.token, "publishGameDraft", &draft_id)?;
            reporter::status("publish", &format!("published draft {draft_id}"));
        }
        DraftsSubcommands::Unpublish { draft_id } => {
            api::gql_simple_mutation(&server_url, &tok.token, "unpublishGameDraft", &draft_id)?;
            reporter::status("unpublish", &format!("unpublished draft {draft_id}"));
        }
        DraftsSubcommands::Discard { draft_id } => {
            api::gql_simple_mutation(&server_url, &tok.token, "discardGameDraft", &draft_id)?;
            reporter::status("discard", &format!("discarded draft {draft_id}"));
        }
    }
    Ok(())
}

pub fn run_manifest(args: ManifestArgs) -> Result<()> {
    match args.command {
        ManifestSubcommands::Show {
            draft_id: None,
            project_dir,
        } => {
            let root = project_dir.unwrap_or(std::env::current_dir()?);
            crate::manifest::show_local(&root)?;
        }
        ManifestSubcommands::Show {
            draft_id: Some(draft_id),
            project_dir: _,
        } => {
            let server_url = config::resolve_graphql_url(args.profile.as_deref(), &args.server_url)?;
            let tok = auth::load_token(&server_url)?;
            show_draft_manifest(&server_url, &tok.token, &draft_id)?;
        }
        ManifestSubcommands::Edit {
            draft_id: None,
            project_dir,
            name,
            display_name,
            version,
            description,
        } => {
            let root = project_dir.unwrap_or(std::env::current_dir()?);
            crate::manifest::edit_local(
                &root,
                &crate::manifest::ManifestFields {
                    name,
                    display_name,
                    version,
                    description,
                },
            )?;
        }
        ManifestSubcommands::Edit {
            draft_id: Some(draft_id),
            project_dir: _,
            name,
            display_name,
            version,
            description,
        } => {
            let server_url = config::resolve_graphql_url(args.profile.as_deref(), &args.server_url)?;
            let tok = auth::load_token(&server_url)?;
            let q = r#"mutation($draftId: ID!, $name: String!, $displayName: String!, $version: String!, $description: String!) {
              updateGameDraftManifest(draftId: $draftId, name: $name, displayName: $displayName, version: $version, description: $description) { id gameName version status }
            }"#;
            let raw = api::gql_raw(
                &server_url,
                &tok.token,
                q,
                json!({
                    "draftId": draft_id,
                    "name": name,
                    "displayName": display_name,
                    "version": version,
                    "description": description
                }),
            )?;
            reporter::hint(&raw);
        }
    }
    Ok(())
}

fn show_draft_manifest(server_url: &str, token: &str, draft_id: &str) -> Result<()> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Draft {
        id: String,
        game_name: String,
        slug: String,
        display_name: String,
        version: String,
        status: String,
        manifest_json: String,
    }
    let q = r#"query($id: ID!) { gameDraft(id: $id) { id slug gameName displayName version status manifestJson } }"#;
    let raw = api::gql_raw(server_url, token, q, json!({ "id": draft_id }))?;
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    let draft: Draft = serde_json::from_value(
        v.get("data")
            .and_then(|d| d.get("gameDraft"))
            .cloned()
            .context("missing gameDraft")?,
    )?;
    reporter::print_table(
        &["Field", "Value"],
        vec![
            vec!["id".into(), draft.id],
            vec!["slug".into(), draft.slug],
            vec!["gameName".into(), draft.game_name],
            vec!["displayName".into(), draft.display_name],
            vec!["version".into(), draft.version],
            vec!["status".into(), draft.status],
        ],
    );
    if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&draft.manifest_json) {
        reporter::section("manifest.json");
        if let Some(obj) = manifest.as_object() {
            let rows: Vec<Vec<String>> = obj
                .iter()
                .map(|(k, v)| vec![k.clone(), v.to_string()])
                .collect();
            reporter::print_table(&["Key", "Value"], rows);
        } else {
            reporter::hint(&draft.manifest_json);
        }
    }
    reporter::status("manifest", &format!("draft {draft_id}"));
    Ok(())
}

pub fn run_doctor(args: DoctorArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    let checks = doctor::run(&root)?;
    print_report(&checks);
    if args.matrix {
        doctor::print_matrix_report();
    }
    if let Some(base) = args.platform.as_deref() {
        let m = crate::platform::fetch_platform_manifest(base)?;
        crate::platform::check_local_toolchain_against_platform(&m)?;
        reporter::status("platform", &format!("compatibility OK ({})", m.framework_version));
    }
    if has_failures(&checks) {
        bail!("doctor found blocking issues");
    }
    Ok(())
}

pub fn run_codegen(args: CodegenArgs) -> Result<()> {
    crate::codegen::run(args)
}

pub fn run_update(args: UpdateArgs) -> Result<()> {
    crate::update::run_update(&args.platform, args.check)
}

pub fn run_validate(args: ValidateArgs) -> Result<()> {
    let root = args.project_dir.unwrap_or(std::env::current_dir()?);
    // Keep zip extraction on disk until validation finishes (NamedTempFile deletes on drop).
    let mut extracted_wasm: Option<tempfile::NamedTempFile> = None;
    let (logic, label) = if let Some(p) = args.logic_wasm {
        (p, None)
    } else {
        let zip = root.join("dist/game.zip");
        if zip.is_file() {
            let f = fs::File::open(&zip)?;
            let mut archive = zip::ZipArchive::new(f)?;
            let mut entry = archive
                .by_name("logic.wasm")
                .with_context(|| format!("{} has no logic.wasm entry; run `gamedev build`", zip.display()))?;
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes)?;
            let tmp = tempfile::NamedTempFile::new()?;
            fs::write(tmp.path(), &bytes)?;
            let path = tmp.path().to_path_buf();
            extracted_wasm = Some(tmp);
            (path, Some(zip))
        } else {
            let cfg = load_config(&root)?;
            let path = match cfg.backend {
                BackendKind::Rust => resolve_rust_logic_wasm_path(&root),
                BackendKind::Java => {
                    resolve_java_backend_dir(&root).join("component/build/out/logic.wasm")
                }
                _ => bail!("validate: specify --logic-wasm or run gamedev build first"),
            };
            (path, None)
        }
    };
    if !logic.is_file() {
        bail!(
            "logic.wasm not found at {}. Run `gamedev build` or pass --logic-wasm",
            logic.display()
        );
    }
    build::validate_logic_component_file(&logic)?;
    let msg = if let Some(zip) = label {
        format!("logic.wasm in {} is a valid WebAssembly component", zip.display())
    } else {
        format!("{} is a valid WebAssembly component", logic.display())
    };
    reporter::status("validate", &msg);
    let _ = extracted_wasm;
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
                .status_logged()?;
            if !status.success() {
                bail!("tests failed");
            }
        }
        BackendKind::Java => {
            build::ensure_java_for_gradle()?;
            let java_dir = resolve_java_backend_dir(&root);
            if !java_dir.join("settings.gradle.kts").is_file() {
                bail!(
                    "Java backend missing {}",
                    java_dir.join("settings.gradle.kts").display()
                );
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
                .status_logged()
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
                .status_logged()
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
    reporter::status("test", "all tests passed");
    Ok(())
}
