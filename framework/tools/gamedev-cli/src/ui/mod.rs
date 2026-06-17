//! Interactive terminal UI (ratatui 0.30 + route stack + tui-input + ratatui-interact breadcrumb).

mod app;
mod banner;
mod exec;
mod init_wizard;
mod job;
mod nav;
mod router;
mod status;

use std::io::{self, IsTerminal, Write};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;

use crate::cli::{
    BackendKind, BuildArgs, CodegenArgs, DEFAULT_GRAPHQL_URL, DeployArgs, DoctorArgs, DraftsArgs,
    FrontendKind, InitArgs, JsTemplate, LoginArgs, LogoutArgs, ManifestArgs, TestArgs, UpdateArgs,
    ValidateArgs,
};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static CTRL_C_HANDLER: OnceLock<()> = OnceLock::new();

pub enum UiCommand {
    Init(InitArgs),
    Login(LoginArgs),
    Logout(LogoutArgs),
    Build(BuildArgs),
    Deploy(DeployArgs),
    Drafts(DraftsArgs),
    Manifest(ManifestArgs),
    Test(TestArgs),
    Doctor(DoctorArgs),
    Validate(ValidateArgs),
    Codegen(CodegenArgs),
    Update(UpdateArgs),
    ExitProgram,
}

pub fn run_interactive() -> Result<()> {
    let home_status = status::build_home_status();
    if !io::stdout().is_terminal() {
        let cmd = run_fallback()?;
        return exec::dispatch(cmd);
    }
    install_ctrlc_handler();
    INTERRUPTED.store(false, Ordering::SeqCst);
    match app::run_terminal_session(home_status) {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            if msg == "cancelled" {
                return Ok(());
            }
            eprintln!("TUI error ({msg}). Falling back to simple text mode.");
            let cmd = run_fallback()?;
            exec::dispatch(cmd)
        }
    }
}

fn run_fallback() -> Result<UiCommand> {
    println!("Select command:");
    println!("1) init");
    println!("2) login (browser)");
    println!("3) login (password)");
    println!("4) build");
    println!("5) deploy");
    println!("6) drafts(list)");
    println!("7) manifest(show)");
    println!("8) test");
    println!("9) doctor");
    println!("10) validate");
    println!("11) codegen");
    println!("12) update (check)");
    println!("13) exit");
    print!("Choice: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let c = line.trim().parse::<usize>().unwrap_or(13);
    let cmd = match c {
        1 => {
            print!("Project name (empty=current dir): ");
            io::stdout().flush()?;
            let mut n = String::new();
            io::stdin().read_line(&mut n)?;
            print!("Backend [rust/java] (default rust): ");
            io::stdout().flush()?;
            let mut be = String::new();
            io::stdin().read_line(&mut be)?;
            let backend = match be.trim().to_lowercase().as_str() {
                "java" => BackendKind::Java,
                _ => BackendKind::Rust,
            };
            print!("Frontend [js/ts/bevy/dioxus] (default js): ");
            io::stdout().flush()?;
            let mut fe = String::new();
            io::stdin().read_line(&mut fe)?;
            let (frontend, js_template) = match fe.trim().to_lowercase().as_str() {
                "ts" => (FrontendKind::Ts, None),
                "bevy" => (FrontendKind::Bevy, None),
                "dioxus" | "dioxus_rust" => (FrontendKind::Dioxus, None),
                "js" => {
                    print!("JS template [vanilla_vite/plain_static] (default vanilla_vite): ");
                    io::stdout().flush()?;
                    let mut tpl = String::new();
                    io::stdin().read_line(&mut tpl)?;
                    let js_template = match tpl.trim().to_lowercase().as_str() {
                        "plain_static" => JsTemplate::PlainStatic,
                        _ => JsTemplate::VanillaVite,
                    };
                    (FrontendKind::Js, Some(js_template))
                }
                _ => (FrontendKind::Js, Some(JsTemplate::VanillaVite)),
            };
            UiCommand::Init(InitArgs {
                name: if n.trim().is_empty() {
                    None
                } else {
                    Some(n.trim().to_string())
                },
                backend: Some(backend),
                frontend: Some(frontend),
                js_template,
            })
        }
        2 => UiCommand::Login(LoginArgs {
            server_url: DEFAULT_GRAPHQL_URL.to_string(),
            profile: None,
            user_id: None,
            display_name: None,
            password: None,
            publish_token: None,
            web: true,
        }),
        3 => {
            print!("Display name: ");
            io::stdout().flush()?;
            let mut name = String::new();
            io::stdin().read_line(&mut name)?;
            print!("Password: ");
            io::stdout().flush()?;
            let mut pass = String::new();
            io::stdin().read_line(&mut pass)?;
            UiCommand::Login(LoginArgs {
                server_url: DEFAULT_GRAPHQL_URL.to_string(),
                profile: None,
                user_id: None,
                display_name: Some(name.trim().to_string()),
                password: Some(pass.trim().to_string()),
                publish_token: None,
                web: false,
            })
        }
        4 => UiCommand::Build(BuildArgs {
            project_dir: None,
            out: None,
            strict: false,
        }),
        5 => UiCommand::Deploy(DeployArgs {
            project_dir: None,
            server_url: DEFAULT_GRAPHQL_URL.to_string(),
            profile: None,
            publish: false,
            auto_publish: false,
            draft_only: false,
        }),
        6 => UiCommand::Drafts(crate::cli::DraftsArgs {
            command: crate::cli::DraftsSubcommands::List,
            server_url: DEFAULT_GRAPHQL_URL.to_string(),
            profile: None,
        }),
        7 => UiCommand::Manifest(ManifestArgs {
            command: crate::cli::ManifestSubcommands::Show {
                draft_id: None,
                project_dir: None,
            },
            server_url: DEFAULT_GRAPHQL_URL.to_string(),
            profile: None,
        }),
        8 => UiCommand::Test(TestArgs { project_dir: None }),
        9 => UiCommand::Doctor(DoctorArgs {
            project_dir: None,
            platform: None,
            matrix: false,
        }),
        10 => UiCommand::Validate(ValidateArgs {
            project_dir: None,
            logic_wasm: None,
        }),
        11 => UiCommand::Codegen(CodegenArgs { project_dir: None }),
        12 => UiCommand::Update(UpdateArgs {
            platform: crate::config::PROD_PLATFORM_BASE.to_string(),
            check: true,
        }),
        _ => UiCommand::ExitProgram,
    };
    Ok(cmd)
}

pub(crate) fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

fn install_ctrlc_handler() {
    let _ = CTRL_C_HANDLER.get_or_init(|| {
        let _ = ctrlc::set_handler(|| {
            INTERRUPTED.store(true, Ordering::SeqCst);
        });
    });
}
