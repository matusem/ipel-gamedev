use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::{BackendKind, FrontendKind, InitArgs, JsTemplate};
use crate::pack::copy_dir_recursive;
use crate::project::{self, ProjectConfig};

pub fn cmd_init(args: InitArgs) -> Result<()> {
    let root = match args.name {
        Some(name) => std::env::current_dir()?.join(name),
        None => std::env::current_dir()?,
    };
    fs::create_dir_all(&root).with_context(|| format!("create dir {}", root.display()))?;
    let name = root.file_name().unwrap_or_default().to_string_lossy().to_string();
    let backend = args.backend.unwrap_or(BackendKind::Rust);
    let frontend = args.frontend.unwrap_or(FrontendKind::Js);
    let cfg = ProjectConfig {
        name: name.clone(),
        backend: backend.clone(),
        frontend: frontend.clone(),
    };

    fs::write(root.join("gamedev.toml"), toml::to_string_pretty(&cfg)?)
        .with_context(|| format!("write {}", root.join("gamedev.toml").display()))?;
    fs::write(
        root.join("manifest.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "name": name.replace('-', "_"),
            "display_name": name,
            "version": "0.1.0",
            "min_players": 2,
            "max_players": 2,
            "description": "Game created by gamedev-cli"
        }))?,
    )
    .with_context(|| format!("write {}", root.join("manifest.json").display()))?;

    let rust_bevy_flat = matches!(backend, BackendKind::Rust) && matches!(frontend, FrontendKind::Bevy);
    if rust_bevy_flat {
        for d in ["logic", "component", "bevy", "tests", "client"] {
            fs::create_dir_all(root.join(d))
                .with_context(|| format!("create dir {}", root.join(d).display()))?;
        }
    } else {
        fs::create_dir_all(root.join("backend"))
            .with_context(|| format!("create dir {}", root.join("backend").display()))?;
        fs::create_dir_all(root.join("frontend"))
            .with_context(|| format!("create dir {}", root.join("frontend").display()))?;
        fs::create_dir_all(root.join("client"))
            .with_context(|| format!("create dir {}", root.join("client").display()))?;
        fs::create_dir_all(root.join("tests"))
            .with_context(|| format!("create dir {}", root.join("tests").display()))?;
    }

    fs::write(root.join("client/index.html"), include_str!("../../templates/client/index.html"))
        .with_context(|| format!("write {}", root.join("client/index.html").display()))?;
    fs::write(root.join("client/config.html"), include_str!("../../templates/client/config.html"))
        .with_context(|| format!("write {}", root.join("client/config.html").display()))?;
    fs::write(root.join("client/result.html"), include_str!("../../templates/client/result.html"))
        .with_context(|| format!("write {}", root.join("client/result.html").display()))?;
    fs::write(root.join("client/about.html"), include_str!("../../templates/client/about.html"))
        .with_context(|| format!("write {}", root.join("client/about.html").display()))?;
    if rust_bevy_flat {
        scaffold_tests_crate(&root, &cfg)?;
        fs::write(root.join(".gitignore"), include_str!("../../templates/gitignore_dist.txt"))
            .with_context(|| format!("write {}", root.join(".gitignore").display()))?;
    } else if matches!(cfg.backend, BackendKind::Rust) {
        fs::write(root.join("tests/logic_test.rs"), include_str!("../../templates/tests/logic_test.rs"))
            .with_context(|| format!("write {}", root.join("tests/logic_test.rs").display()))?;
    }

    if rust_bevy_flat {
        scaffold_rust_bevy_flat(&root, &cfg)?;
    } else if matches!(cfg.backend, BackendKind::Rust) {
        scaffold_rust_backend(
            &root,
            &cfg,
            matches!(frontend, FrontendKind::Bevy | FrontendKind::Dioxus),
        )?;
    } else if matches!(cfg.backend, BackendKind::Java) {
        scaffold_java_backend(&root, &cfg)?;
    }
    match frontend {
        FrontendKind::Js | FrontendKind::Ts => {
            scaffold_js_frontend(
                &root,
                args.js_template.unwrap_or(JsTemplate::VanillaVite),
                matches!(frontend, FrontendKind::Ts),
                matches!(cfg.backend, BackendKind::Rust | BackendKind::Java),
            )?;
        }
        FrontendKind::Bevy => {
            if !rust_bevy_flat {
                scaffold_bevy_frontend(&root, matches!(cfg.backend, BackendKind::Rust))?;
            }
        }
        FrontendKind::Dioxus => {
            scaffold_dioxus_frontend(&root, matches!(cfg.backend, BackendKind::Rust))?;
        }
        _ => {}
    }

    fs::write(root.join("README.md"), include_str!("../../templates/README.md"))
        .with_context(|| format!("write {}", root.join("README.md").display()))?;
    println!("Initialized game project at {}", root.display());
    Ok(())
}

fn scaffold_rust_bevy_flat(root: &Path, cfg: &ProjectConfig) -> Result<()> {
    let logic_dir = root.join("logic");
    let component_dir = root.join("component");
    let bevy_dir = root.join("bevy");
    fs::create_dir_all(logic_dir.join("src"))?;
    fs::create_dir_all(component_dir.join("src"))?;
    fs::create_dir_all(bevy_dir.join("src"))?;

    let game_dep_path = project::find_framework_game_crate(root)
        .and_then(|game_crate| pathdiff::diff_paths(game_crate, &logic_dir))
        .unwrap_or_else(|| PathBuf::from("../../game"));
    let host_dep_path = project::find_framework_game_wasm_host_crate(root)
        .and_then(|p| pathdiff::diff_paths(p, &component_dir))
        .unwrap_or_else(|| PathBuf::from("../../game-wasm-host"));
    let logic_rel_for_component =
        pathdiff::diff_paths(&logic_dir, &component_dir).unwrap_or_else(|| PathBuf::from("../logic"));
    let logic_rel_for_bevy =
        pathdiff::diff_paths(&logic_dir, &bevy_dir).unwrap_or_else(|| PathBuf::from("../logic"));
    let game_dep_path_bevy = project::find_framework_game_crate(root)
        .and_then(|game_crate| pathdiff::diff_paths(game_crate, &bevy_dir))
        .unwrap_or_else(|| PathBuf::from("../../game"));
    let game_dep_path = game_dep_path.to_string_lossy().replace('\\', "/");
    let game_dep_path_bevy = game_dep_path_bevy.to_string_lossy().replace('\\', "/");
    let host_dep_path = host_dep_path.to_string_lossy().replace('\\', "/");
    let logic_rel_for_component = logic_rel_for_component.to_string_lossy().replace('\\', "/");
    let logic_rel_for_bevy = logic_rel_for_bevy.to_string_lossy().replace('\\', "/");
    let crate_name = cfg.name.replace('-', "_");
    let logic_name = format!("{crate_name}_logic");
    let component_name = format!("{crate_name}_component");
    let bevy_name = format!("{crate_name}_bevy");

    let logic_cargo = include_str!("../../templates/backend/rust_logic_flat_Cargo.toml")
        .replace("__CRATE_NAME__", &logic_name)
        .replace("__GAME_PATH__", &game_dep_path);
    fs::write(logic_dir.join("Cargo.toml"), logic_cargo)?;
    fs::write(
        logic_dir.join("src/lib.rs"),
        include_str!("../../templates/backend/rust_logic_flat_lib.rs"),
    )?;

    let component_cargo = include_str!("../../templates/backend/rust_component_Cargo.toml")
        .replace("__COMPONENT_NAME__", &component_name)
        .replace("__LOGIC_NAME__", &logic_name)
        .replace("__LOGIC_PATH__", &logic_rel_for_component)
        .replace("__HOST_PATH__", &host_dep_path);
    fs::write(component_dir.join("Cargo.toml"), component_cargo)?;
    fs::write(
        component_dir.join("src/lib.rs"),
        include_str!("../../templates/backend/rust_component_lib.rs").replace("__LOGIC_NAME__", &logic_name),
    )?;

    let bevy_cargo = include_str!("../../templates/frontend/bevy_flat_Cargo.toml")
        .replace("__BEVY_NAME__", &bevy_name)
        .replace("__LOGIC_NAME__", &logic_name)
        .replace("__LOGIC_PATH__", &logic_rel_for_bevy)
        .replace("__GAME_PATH__", &game_dep_path_bevy);
    fs::write(bevy_dir.join("Cargo.toml"), bevy_cargo)?;
    fs::write(
        bevy_dir.join("src/main.rs"),
        include_str!("../../templates/frontend/bevy_flat_main.rs").replace("__LOGIC_NAME__", &logic_name),
    )?;

    let dot_cargo = root.join(".cargo");
    fs::create_dir_all(&dot_cargo)
        .with_context(|| format!("create {}", dot_cargo.display()))?;
    fs::write(
        dot_cargo.join("config.toml"),
        include_str!("../../templates/misc/dot_cargo_config_wasm.toml"),
    )
    .with_context(|| format!("write {}", dot_cargo.join("config.toml").display()))?;

    let bevy_js = format!("{bevy_name}.js");
    let index_bevy = include_str!("../../templates/client/index_bevy.html")
        .replace("__BEVY_WASM_BINDGEN_JS__", &bevy_js);
    fs::write(root.join("client/index.html"), index_bevy)
        .with_context(|| format!("write {}", root.join("client/index.html").display()))?;

    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nresolver = \"3\"\nmembers = [\n  \"logic\",\n  \"component\",\n  \"bevy\",\n  \"tests\"\n]\n",
    )?;
    Ok(())
}

fn scaffold_tests_crate(root: &Path, cfg: &ProjectConfig) -> Result<()> {
    let tests_dir = root.join("tests");
    fs::create_dir_all(tests_dir.join("src"))?;
    let logic_name = format!("{}_logic", cfg.name.replace('-', "_"));
    let game_dep_path_tests = project::find_framework_game_crate(root)
        .and_then(|game_crate| pathdiff::diff_paths(game_crate, &tests_dir))
        .unwrap_or_else(|| PathBuf::from("../../../game"));
    let game_dep_path_tests = game_dep_path_tests.to_string_lossy().replace('\\', "/");
    let cargo = include_str!("../../templates/tests/rust_tests_Cargo.toml")
        .replace("__TESTS_NAME__", &format!("{}_tests", cfg.name.replace('-', "_")))
        .replace("__LOGIC_NAME__", &logic_name)
        .replace("__GAME_PATH__", &game_dep_path_tests);
    fs::write(tests_dir.join("Cargo.toml"), cargo)?;
    fs::write(
        tests_dir.join("src/lib.rs"),
        include_str!("../../templates/tests/rust_tests_lib.rs").replace("__LOGIC_NAME__", &logic_name),
    )?;
    Ok(())
}

fn scaffold_java_backend(root: &Path, cfg: &ProjectConfig) -> Result<()> {
    let fw = project::find_framework_root(root).with_context(|| {
        "Java backend: could not locate framework root (ancestor with sdk/java/game/settings.gradle.kts). \
         Create the game under the framework tree, or copy sdk/java manually."
    })?;
    let java = root.join("backend/java");
    let component_dst = java.join("component");
    fs::create_dir_all(&java)?;
    let src_tpl = fw.join("sdk/java/component-template");
    copy_dir_recursive(&src_tpl, &component_dst)
        .with_context(|| format!("copy Java template from {}", src_tpl.display()))?;
    let _ = fs::remove_dir_all(component_dst.join("build"));
    let _ = fs::remove_dir_all(component_dst.join(".gradle"));
    fs::copy(fw.join("test.wit"), component_dst.join("game-core.wit"))
        .with_context(|| format!("copy {}", fw.join("test.wit").display()))?;
    let gradle_kts = fs::read_to_string(component_dst.join("build.gradle.kts"))?;
    let marker =
        "val wit = layout.projectDirectory.dir(\"../../..\").file(\"test.wit\").asFile";
    let replacement = "val wit = layout.projectDirectory.file(\"game-core.wit\").asFile";
    if !gradle_kts.contains(marker) {
        anyhow::bail!(
            "Java template build.gradle.kts is missing the WIT path marker; update scaffold_java_backend"
        );
    }
    let patched = gradle_kts.replace(marker, replacement);
    fs::write(component_dst.join("build.gradle.kts"), patched)?;
    let game_sdk = fw.join("sdk/java/game");
    let rel = pathdiff::diff_paths(&game_sdk, &java)
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .replace('\\', "/");
    let game_name = cfg.name.replace('-', "_");
    let settings = include_str!("../../templates/backend/java_settings.gradle.kts")
        .replace("__ROOT_NAME__", &format!("{game_name}-java"))
        .replace("__SDK_GAME_PATH__", &rel);
    fs::write(java.join("settings.gradle.kts"), settings)?;
    Ok(())
}

fn scaffold_rust_backend(root: &Path, cfg: &ProjectConfig, include_rust_frontend: bool) -> Result<()> {
    let rust_root = root.join("backend").join("rust");
    let logic_dir = rust_root.join("logic");
    let component_dir = rust_root.join("component");
    let shared_types_dir = rust_root.join("shared-types");
    let src_dir = logic_dir.join("src");
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(component_dir.join("src"))?;
    fs::create_dir_all(shared_types_dir.join("src"))?;

    let game_dep_path = project::find_framework_game_crate(root)
        .and_then(|game_crate| pathdiff::diff_paths(game_crate, &logic_dir))
        .unwrap_or_else(|| PathBuf::from("../../../../game"));
    let host_dep_path = project::find_framework_game_wasm_host_crate(root)
        .and_then(|p| pathdiff::diff_paths(p, &component_dir))
        .unwrap_or_else(|| PathBuf::from("../../../../game-wasm-host"));
    let logic_rel_for_component =
        pathdiff::diff_paths(&logic_dir, &component_dir).unwrap_or_else(|| PathBuf::from("../logic"));
    let game_dep_path = game_dep_path.to_string_lossy().replace('\\', "/");
    let host_dep_path = host_dep_path.to_string_lossy().replace('\\', "/");
    let logic_rel_for_component = logic_rel_for_component.to_string_lossy().replace('\\', "/");
    let crate_name = cfg.name.replace('-', "_");
    let shared_types_name = format!("{crate_name}_shared_types");

    let shared_types_cargo = include_str!("../../templates/backend/rust_shared_types_Cargo.toml")
        .replace("__SHARED_TYPES_NAME__", &shared_types_name);
    fs::write(shared_types_dir.join("Cargo.toml"), shared_types_cargo)?;
    fs::write(
        shared_types_dir.join("src/lib.rs"),
        include_str!("../../templates/backend/rust_shared_types_lib.rs"),
    )?;
    fs::create_dir_all(shared_types_dir.join("src/bin"))?;
    let export_ts = include_str!("../../templates/backend/rust_shared_types_export_ts.rs")
        .replace("__SHARED_TYPES_CRATE__", &shared_types_name.replace('-', "_"));
    fs::write(shared_types_dir.join("src/bin/export_ts.rs"), export_ts)?;

    let logic_cargo = include_str!("../../templates/backend/rust_logic_Cargo.toml")
        .replace("__CRATE_NAME__", &format!("{crate_name}_logic"))
        .replace("__GAME_PATH__", &game_dep_path)
        .replace("__SHARED_TYPES_NAME__", &shared_types_name);
    fs::write(logic_dir.join("Cargo.toml"), logic_cargo)?;

    fs::write(logic_dir.join("src/lib.rs"), include_str!("../../templates/backend/rust_logic_lib.rs"))?;

    let component_cargo = include_str!("../../templates/backend/rust_component_Cargo.toml")
        .replace("__COMPONENT_NAME__", &format!("{crate_name}_component"))
        .replace("__LOGIC_NAME__", &format!("{crate_name}_logic"))
        .replace("__LOGIC_PATH__", &logic_rel_for_component)
        .replace("__HOST_PATH__", &host_dep_path);
    fs::write(component_dir.join("Cargo.toml"), component_cargo)?;

    let component_lib = include_str!("../../templates/backend/rust_component_lib.rs")
        .replace("__LOGIC_NAME__", &format!("{crate_name}_logic"));
    fs::write(component_dir.join("src/lib.rs"), component_lib)?;

    let mut members = vec![
        "backend/rust/shared-types".to_string(),
        "backend/rust/logic".to_string(),
        "backend/rust/component".to_string(),
    ];
    if include_rust_frontend {
        if matches!(cfg.frontend, FrontendKind::Bevy) {
            members.push("frontend/bevy".to_string());
        }
        if matches!(cfg.frontend, FrontendKind::Dioxus) {
            members.push("frontend/dioxus".to_string());
        }
    }
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"3\"\nmembers = [\n{}\n]\n",
            members
                .iter()
                .map(|m| format!("  \"{}\"", m))
                .collect::<Vec<_>>()
                .join(",\n")
        ),
    )?;
    Ok(())
}

fn scaffold_js_frontend(root: &Path, template: JsTemplate, use_ts: bool, rust_backend: bool) -> Result<()> {
    let web = root.join("frontend").join("web");
    fs::create_dir_all(web.join("src"))?;

    let pkg = match template {
        JsTemplate::VanillaVite => include_str!("../../templates/frontend/vanilla_vite_package.json"),
        JsTemplate::PlainStatic => include_str!("../../templates/frontend/plain_static_package.json"),
        JsTemplate::ReactVite => include_str!("../../templates/frontend/react_vite_package.json"),
    };
    fs::write(web.join("package.json"), pkg)?;

    let (entry_path, entry_source) = if use_ts {
        ("src/main.ts", include_str!("../../templates/frontend/main.ts"))
    } else {
        ("src/main.js", include_str!("../../templates/frontend/main.js"))
    };
    fs::write(web.join(entry_path), entry_source)?;
    let index_html =
        include_str!("../../templates/frontend/index.html").replace("__ENTRY_PATH__", entry_path);
    fs::write(web.join("index.html"), index_html)?;
    if rust_backend {
        let generated_types = r#"// Generated from Rust shared types.
// Regenerate after model changes:
// cargo run --manifest-path ../../backend/rust/shared-types/Cargo.toml --features typegen --bin export_ts
export type Player = "Player1" | "Player2";
export type Move = { Place: { index: number } };
"#;
        fs::create_dir_all(web.join("src/generated"))?;
        fs::write(web.join("src/generated/types.ts"), generated_types)?;
    }
    Ok(())
}

fn scaffold_bevy_frontend(root: &Path, in_workspace: bool) -> Result<()> {
    let bevy = root.join("frontend").join("bevy");
    fs::create_dir_all(bevy.join("src"))?;
    let mut cargo = include_str!("../../templates/frontend/bevy_Cargo.toml").to_string();
    if !in_workspace {
        cargo.push_str("\n[workspace]\n");
    }
    fs::write(bevy.join("Cargo.toml"), cargo)?;
    fs::write(bevy.join("src/main.rs"), include_str!("../../templates/frontend/bevy_main.rs"))?;
    Ok(())
}

fn scaffold_dioxus_frontend(root: &Path, in_workspace: bool) -> Result<()> {
    let dioxus = root.join("frontend").join("dioxus");
    fs::create_dir_all(dioxus.join("src"))?;
    let mut cargo = include_str!("../../templates/frontend/dioxus_Cargo.toml").to_string();
    if !in_workspace {
        cargo.push_str("\n[workspace]\n");
    }
    fs::write(dioxus.join("Cargo.toml"), cargo)?;
    fs::write(
        dioxus.join("src/main.rs"),
        include_str!("../../templates/frontend/dioxus_main.rs"),
    )?;
    Ok(())
}
