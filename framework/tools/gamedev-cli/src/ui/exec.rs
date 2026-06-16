//! Run CLI commands from inside the TUI session.

use anyhow::Result;

use super::UiCommand;

pub fn command_title(cmd: &UiCommand) -> &'static str {
    match cmd {
        UiCommand::Init(_) => "init",
        UiCommand::Login(_) => "login",
        UiCommand::Logout(_) => "logout",
        UiCommand::Build(_) => "build",
        UiCommand::Deploy(_) => "deploy",
        UiCommand::Drafts(_) => "drafts",
        UiCommand::Manifest(_) => "manifest",
        UiCommand::Test(_) => "test",
        UiCommand::Doctor(_) => "doctor",
        UiCommand::Validate(_) => "validate",
        UiCommand::Codegen(_) => "codegen",
        UiCommand::Update(_) => "update",
        UiCommand::ExitProgram => "exit",
    }
}

pub fn dispatch(cmd: UiCommand) -> Result<()> {
    match cmd {
        UiCommand::Init(args) => crate::commands::run_init(args),
        UiCommand::Login(args) => crate::commands::run_login(args),
        UiCommand::Logout(args) => crate::commands::run_logout(args),
        UiCommand::Build(args) => crate::commands::run_build(args),
        UiCommand::Deploy(args) => crate::commands::run_deploy(args),
        UiCommand::Drafts(args) => crate::commands::run_drafts(args),
        UiCommand::Manifest(args) => crate::commands::run_manifest(args),
        UiCommand::Test(args) => crate::commands::run_test(args),
        UiCommand::Doctor(args) => crate::commands::run_doctor(args),
        UiCommand::Validate(args) => crate::commands::run_validate(args),
        UiCommand::Codegen(args) => crate::commands::run_codegen(args),
        UiCommand::Update(args) => crate::commands::run_update(args),
        UiCommand::ExitProgram => Ok(()),
    }
}
