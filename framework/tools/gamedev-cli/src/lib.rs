//! Library surface for `gamedev-cli` (binary is a thin wrapper).

pub mod api;
pub mod auth;
pub mod auth_web;
pub mod build;
pub mod cli;
pub mod codegen;
pub mod commands;
pub mod config;
pub mod defaults;
pub mod doctor;
pub mod pack;
pub mod platform;
pub mod project;
pub mod matrix_test;
pub mod reporter;
pub mod scaffold;
pub mod theme;
pub mod ui;
pub mod update;
pub mod version;

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
            Commands::Logout(args) => commands::run_logout(args),
            Commands::Deploy(args) => commands::run_deploy(args),
            Commands::Drafts(args) => commands::run_drafts(args),
            Commands::Manifest(args) => commands::run_manifest(args),
            Commands::Test(args) => commands::run_test(args),
            Commands::Doctor(args) => commands::run_doctor(args),
            Commands::Validate(args) => commands::run_validate(args),
            Commands::Codegen(args) => commands::run_codegen(args),
            Commands::Update(args) => commands::run_update(args),
        };
    }
    ui::run_interactive()?;
    Ok(())
}
