//! Route stack for nested TUI screens (breadcrumb + Esc = pop).

use ratatui_interact::components::{BreadcrumbItem, BreadcrumbState};

use super::init_wizard::InitWizardState;

#[derive(Clone, Copy)]
pub enum DraftMenuAction {
    List,
    Publish,
    Unpublish,
    Discard,
}

pub enum RouteFrame {
    MainMenu {
        list: ratatui::widgets::ListState,
    },
    Init(InitWizardState),
    Login {
        user: tui_input::Input,
        server: tui_input::Input,
        field: usize,
    },
    BuildConfirm,
    DeployMode {
        list: ratatui::widgets::ListState,
    },
    DeployServer {
        server: tui_input::Input,
        draft_only: bool,
        auto_publish: bool,
    },
    DraftsMode {
        list: ratatui::widgets::ListState,
    },
    DraftsServer {
        server: tui_input::Input,
        action: DraftMenuAction,
    },
    DraftId {
        server: String,
        action: DraftMenuAction,
        draft_id: tui_input::Input,
    },
    ManifestMode {
        list: ratatui::widgets::ListState,
    },
    ManifestServer {
        server: tui_input::Input,
        next: ManifestNext,
    },
    ManifestDraftId {
        server: String,
        next: ManifestNext,
        draft_id: tui_input::Input,
    },
    ManifestEditFields {
        server: String,
        draft_id: String,
        name: tui_input::Input,
        display_name: tui_input::Input,
        version: tui_input::Input,
        description: tui_input::Input,
        field: usize,
    },
    TestConfirm,
}

#[derive(Clone, Copy)]
pub enum ManifestNext {
    Show,
    Edit,
}

impl RouteFrame {
    pub fn label(&self) -> &'static str {
        match self {
            RouteFrame::MainMenu { .. } => "Home",
            RouteFrame::Init(_) => "Init",
            RouteFrame::Login { .. } => "Login",
            RouteFrame::BuildConfirm => "Build",
            RouteFrame::DeployMode { .. } => "Deploy",
            RouteFrame::DeployServer { .. } => "Deploy · Server",
            RouteFrame::DraftsMode { .. } => "Drafts",
            RouteFrame::DraftsServer { .. } => "Drafts · Server",
            RouteFrame::DraftId { .. } => "Drafts · Id",
            RouteFrame::ManifestMode { .. } => "Manifest",
            RouteFrame::ManifestServer { .. } => "Manifest · Server",
            RouteFrame::ManifestDraftId { .. } => "Manifest · Draft",
            RouteFrame::ManifestEditFields { .. } => "Manifest · Edit",
            RouteFrame::TestConfirm => "Test",
        }
    }
}

pub fn breadcrumb_state(stack: &[RouteFrame]) -> BreadcrumbState {
    let items: Vec<BreadcrumbItem> = stack
        .iter()
        .enumerate()
        .map(|(i, f)| BreadcrumbItem::new(format!("r{i}"), f.label()).enabled(false))
        .collect();
    BreadcrumbState::new(items)
}
