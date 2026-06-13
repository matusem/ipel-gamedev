//! Project and toolchain health checks (`gamedev-cli doctor`).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;

use crate::cli::{BackendKind, FrontendKind};
use crate::project::{
    ProjectLayout, detect_layout, is_game_project, load_config, resolve_bevy_dir,
    resolve_component_dir, resolve_dioxus_dir, resolve_logic_dir,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub status: CheckStatus,
    pub label: String,
    pub detail: String,
}

pub fn run(root: &Path) -> Result<Vec<CheckResult>> {
    let mut checks = Vec::new();

    if !is_game_project(root) {
        checks.push(warn(
            "game project",
            "no gamedev.toml here (run `init` or cd into a game project for full checks)",
        ));
        checks.extend(toolchain_checks(None, None));
        return Ok(checks);
    }

    let cfg = load_config(root)?;
    let layout = detect_layout(root);

    checks.push(ok(
        "gamedev.toml",
        &format!(
            "name={} backend={:?} frontend={:?}",
            cfg.name, cfg.backend, cfg.frontend
        ),
    ));
    checks.push(if root.join("manifest.json").is_file() {
        ok("manifest.json", "present")
    } else {
        fail("manifest.json", "missing")
    });

    for f in crate::build::REQUIRED_CLIENT_HTML {
        let p = root.join("client").join(f);
        checks.push(if p.is_file() {
            ok(&format!("client/{f}"), "present")
        } else {
            fail(&format!("client/{f}"), "missing")
        });
    }

    match layout {
        ProjectLayout::FlatRustBevy => {
            checks.push(dir_ok("logic/", root.join("logic")));
            checks.push(dir_ok("component/", root.join("component")));
            checks.push(dir_ok("bevy/", root.join("bevy")));
            checks.push(dir_ok("tests/", root.join("tests")));
            checks.push(if root.join("Cargo.toml").is_file() {
                ok("Cargo.toml", "workspace root")
            } else {
                fail("Cargo.toml", "missing workspace manifest")
            });
        }
        ProjectLayout::NestedRust => {
            checks.push(dir_ok(
                "backend/rust/logic",
                root.join("backend/rust/logic"),
            ));
            checks.push(dir_ok(
                "backend/rust/component",
                root.join("backend/rust/component"),
            ));
            if matches!(cfg.frontend, FrontendKind::Bevy) {
                checks.push(if resolve_bevy_dir(root).is_some() {
                    ok("bevy frontend", "found")
                } else {
                    fail("bevy frontend", "frontend/bevy/Cargo.toml missing")
                });
            }
            if matches!(cfg.frontend, FrontendKind::Dioxus) {
                checks.push(if resolve_dioxus_dir(root).is_some() {
                    ok("dioxus frontend", "found")
                } else {
                    fail("dioxus frontend", "frontend/dioxus/Cargo.toml missing")
                });
            }
        }
        ProjectLayout::NestedJava => {
            checks.push(dir_ok("backend/java", root.join("backend/java")));
        }
        ProjectLayout::Unknown => {
            checks.push(warn("layout", "could not classify project tree"));
        }
    }

    checks.extend(toolchain_checks(Some(&cfg.backend), Some(&cfg.frontend)));
    checks.extend(layout_tooling_checks(
        root,
        &cfg.backend,
        &cfg.frontend,
        layout,
    ));

    Ok(checks)
}

fn layout_tooling_checks(
    root: &Path,
    backend: &BackendKind,
    frontend: &FrontendKind,
    layout: ProjectLayout,
) -> Vec<CheckResult> {
    let mut out = Vec::new();
    if matches!(backend, BackendKind::Rust) {
        let comp = resolve_component_dir(root);
        out.push(if comp.join("Cargo.toml").is_file() {
            ok("component crate", &comp.display().to_string())
        } else {
            fail("component crate", "Cargo.toml not found")
        });
        let logic = resolve_logic_dir(root);
        out.push(if logic.join("src").is_dir() {
            ok("logic crate", &logic.display().to_string())
        } else {
            fail("logic crate", "src/ missing")
        });
        out.push(if command_ok("cargo", &["component", "--version"]) {
            ok("cargo-component", "available")
        } else {
            warn(
                "cargo-component",
                "not on PATH — install with: cargo install cargo-component",
            )
        });
    }
    if matches!(frontend, FrontendKind::Bevy | FrontendKind::Dioxus) {
        let crate_label = if matches!(frontend, FrontendKind::Bevy) {
            "bevy"
        } else {
            "dioxus"
        };
        let crate_found = if matches!(frontend, FrontendKind::Bevy) {
            resolve_bevy_dir(root).is_some()
        } else {
            resolve_dioxus_dir(root).is_some()
        };
        out.push(if crate_found {
            ok(&format!("{crate_label} crate"), "found")
        } else {
            fail(&format!("{crate_label} crate"), "missing")
        });
        out.push(if command_ok("wasm-bindgen", &["--version"]) {
            ok("wasm-bindgen-cli", "available")
        } else {
            warn(
                "wasm-bindgen-cli",
                "not on PATH — `cargo install wasm-bindgen-cli` required for wasm web build",
            )
        });
        let target_installed = Command::new("rustup")
            .args(["target", "list", "--installed"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("wasm32-unknown-unknown"))
            .unwrap_or(false);
        out.push(if target_installed {
            ok("wasm32-unknown-unknown", "rustup target installed")
        } else {
            warn(
                "wasm32-unknown-unknown",
                "run: rustup target add wasm32-unknown-unknown",
            )
        });
        let dot_cargo = root.join(".cargo/config.toml");
        let needs_dot_cargo = matches!(layout, ProjectLayout::FlatRustBevy)
            || matches!(frontend, FrontendKind::Dioxus);
        if needs_dot_cargo {
            out.push(if dot_cargo.is_file() {
                ok(".cargo/config.toml", "wasm browser linker config present")
            } else {
                warn(
                    ".cargo/config.toml",
                    "missing — build will auto-create; needed for getrandom on wasm",
                )
            });
        }
    }
    if matches!(frontend, FrontendKind::Js | FrontendKind::Ts) {
        let web = root.join("frontend/web");
        out.push(if web.join("package.json").is_file() {
            ok("frontend/web", "package.json present")
        } else {
            warn("frontend/web", "package.json missing")
        });
        out.push(if command_ok("npm", &["--version"]) {
            ok("npm", "available")
        } else {
            warn("npm", "not on PATH — static frontend merge only")
        });
    }
    out
}

fn toolchain_checks(
    backend: Option<&BackendKind>,
    frontend: Option<&FrontendKind>,
) -> Vec<CheckResult> {
    let mut out = Vec::new();
    let need_cargo = backend.is_none() || matches!(backend, Some(BackendKind::Rust));
    let need_wasm_frontend =
        frontend.is_some_and(|f| matches!(f, FrontendKind::Bevy | FrontendKind::Dioxus));
    if need_cargo || need_wasm_frontend {
        out.push(if command_ok("cargo", &["--version"]) {
            ok("cargo", "available")
        } else {
            fail("cargo", "not on PATH")
        });
    }
    if backend.is_some_and(|b| matches!(b, BackendKind::Java)) {
        out.push(if command_ok("java", &["-version"]) {
            ok("java", "available")
        } else {
            fail("java", "not on PATH — JDK 21+ required")
        });
    }
    out
}

fn command_ok(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn dir_ok(label: &str, path: PathBuf) -> CheckResult {
    if path.is_dir() {
        ok(label, &path.display().to_string())
    } else {
        fail(label, &format!("missing ({})", path.display()))
    }
}

fn ok(label: &str, detail: &str) -> CheckResult {
    CheckResult {
        status: CheckStatus::Ok,
        label: label.to_string(),
        detail: detail.to_string(),
    }
}

fn warn(label: &str, detail: &str) -> CheckResult {
    CheckResult {
        status: CheckStatus::Warn,
        label: label.to_string(),
        detail: detail.to_string(),
    }
}

fn fail(label: &str, detail: &str) -> CheckResult {
    CheckResult {
        status: CheckStatus::Fail,
        label: label.to_string(),
        detail: detail.to_string(),
    }
}

pub fn print_report(checks: &[CheckResult]) {
    let mut fails = 0u32;
    let mut warns = 0u32;
    for c in checks {
        let (tag, color_hint) = match c.status {
            CheckStatus::Ok => ("OK", ""),
            CheckStatus::Warn => {
                warns += 1;
                ("WARN", "")
            }
            CheckStatus::Fail => {
                fails += 1;
                ("FAIL", "")
            }
        };
        let _ = color_hint;
        println!("[{tag}] {} — {}", c.label, c.detail);
    }
    println!();
    if fails > 0 {
        println!(
            "Doctor: {fails} failure(s), {warns} warning(s). Fix FAIL items before build/deploy."
        );
    } else if warns > 0 {
        println!("Doctor: all required checks passed; {warns} warning(s).");
    } else {
        println!("Doctor: all checks passed.");
    }
}

pub fn has_failures(checks: &[CheckResult]) -> bool {
    checks.iter().any(|c| c.status == CheckStatus::Fail)
}
