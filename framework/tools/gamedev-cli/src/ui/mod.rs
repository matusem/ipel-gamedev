//! Interactive terminal UI (ratatui 0.30 + route stack + tui-input + ratatui-interact breadcrumb).

mod app;
mod init_wizard;
mod router;

use std::io::{self, IsTerminal, Write};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;

use crate::cli::{
    BackendKind, BuildArgs, DeployArgs, DoctorArgs, DraftsArgs, FrontendKind, InitArgs, JsTemplate,
    LoginArgs, ManifestArgs, TestArgs, DEFAULT_GRAPHQL_URL,
};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static CTRL_C_HANDLER: OnceLock<()> = OnceLock::new();

pub enum UiCommand {
    Init(InitArgs),
    Login(LoginArgs),
    Build(BuildArgs),
    Deploy(DeployArgs),
    Drafts(DraftsArgs),
    Manifest(ManifestArgs),
    Test(TestArgs),
    Doctor(DoctorArgs),
    ExitProgram,
}

pub fn run_interactive(auth_user: Option<String>) -> Result<UiCommand> {
    if !io::stdout().is_terminal() {
        return run_fallback();
    }
    install_ctrlc_handler();
    INTERRUPTED.store(false, Ordering::SeqCst);
    match app::run_terminal_session(auth_user.clone()) {
        Ok(cmd) => Ok(cmd),
        Err(e) => {
            let msg = e.to_string();
            if msg == "cancelled" {
                return Ok(UiCommand::ExitProgram);
            }
            eprintln!("TUI error ({msg}). Falling back to simple text mode.");
            run_fallback()
        }
    }
}

fn run_fallback() -> Result<UiCommand> {
    println!("Select command:");
    println!("1) init");
    println!("2) login");
    println!("3) build");
    println!("4) deploy");
    println!("5) drafts(list)");
    println!("6) manifest(show)");
    println!("7) test");
    println!("8) doctor");
    println!("9) exit");
    print!("Choice: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let c = line.trim().parse::<usize>().unwrap_or(9);
    let cmd = match c {
        1 => {
            print!("Project name (empty=current dir): ");
            io::stdout().flush()?;
            let mut n = String::new();
            io::stdin().read_line(&mut n)?;
            UiCommand::Init(InitArgs {
                name: if n.trim().is_empty() { None } else { Some(n.trim().to_string()) },
                backend: Some(BackendKind::Rust),
                frontend: Some(FrontendKind::Js),
                js_template: Some(JsTemplate::VanillaVite),
            })
        }
        2 => UiCommand::Login(LoginArgs {
            server_url: DEFAULT_GRAPHQL_URL.to_string(),
            user_id: String::new(),
        }),
        3 => UiCommand::Build(BuildArgs {
            project_dir: None,
            out: None,
        }),
        4 => UiCommand::Deploy(DeployArgs {
            project_dir: None,
            server_url: DEFAULT_GRAPHQL_URL.to_string(),
            auto_publish: false,
            draft_only: true,
        }),
        5 => UiCommand::Drafts(crate::cli::DraftsArgs {
            command: crate::cli::DraftsSubcommands::List,
            server_url: DEFAULT_GRAPHQL_URL.to_string(),
        }),
        6 => UiCommand::Manifest(ManifestArgs {
            command: crate::cli::ManifestSubcommands::Show {
                draft_id: String::new(),
            },
            server_url: DEFAULT_GRAPHQL_URL.to_string(),
        }),
        7 => UiCommand::Test(TestArgs { project_dir: None }),
        8 => UiCommand::Doctor(DoctorArgs { project_dir: None }),
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
