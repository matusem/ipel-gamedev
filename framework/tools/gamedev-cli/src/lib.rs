//! Library surface for `gamedev-cli` (binary is a thin wrapper).

pub mod api;
pub mod auth;
pub mod build;
pub mod cli;
pub mod commands;
pub mod pack;
pub mod project;
pub mod scaffold;
pub mod ui;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Commands};

pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    if let Some(cmd) = cli.command {
        return match cmd {
            Commands::Init(args) => commands::run_init(args),
            Commands::Build(args) => commands::run_build(args),
            Commands::Login(args) => commands::run_login(args),
            Commands::Deploy(args) => commands::run_deploy(args),
            Commands::Drafts(args) => commands::run_drafts(args),
            Commands::Manifest(args) => commands::run_manifest(args),
            Commands::Test(args) => commands::run_test(args),
        };
    }
    loop {
        match ui::run_interactive(auth::current_user_label())? {
            ui::UiCommand::Init(args) => commands::run_init(args)?,
            ui::UiCommand::Login(args) => commands::run_login(args)?,
            ui::UiCommand::Build(args) => commands::run_build(args)?,
            ui::UiCommand::Deploy(args) => commands::run_deploy(args)?,
            ui::UiCommand::Drafts(args) => commands::run_drafts(args)?,
            ui::UiCommand::Manifest(args) => commands::run_manifest(args)?,
            ui::UiCommand::Test(args) => commands::run_test(args)?,
            ui::UiCommand::ExitProgram => break,
        }
    }
    Ok(())
}
