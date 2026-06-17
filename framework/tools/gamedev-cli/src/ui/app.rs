//! Single terminal session: route stack, chrome, and screens.

use std::time::Duration;

use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui_interact::components::{Breadcrumb, BreadcrumbStyle};

use crate::cli::{
    BuildArgs, CodegenArgs, DEFAULT_GRAPHQL_URL, DeployArgs, DoctorArgs, DraftsArgs,
    DraftsSubcommands, LoginArgs, LogoutArgs, ManifestArgs, ManifestSubcommands, TestArgs, UpdateArgs,
    ValidateArgs,
};
use crate::project::resolve_test_dir;
use crate::theme;

use super::banner;
use super::init_wizard::{InitWizardOutcome, InitWizardState};
use super::nav;
use super::job::{JobKeyAction, JobRun};
use super::router::{DraftMenuAction, ManifestNext, RouteFrame, breadcrumb_state};
use super::status::{self, HomeStatus, layout_label};
use super::{UiCommand, interrupted};

enum SessionAction {
    Exit,
    Run(UiCommand),
}

#[derive(Clone, Copy)]
enum MainMenuAction {
    Init,
    Login,
    Logout,
    Build,
    Deploy,
    Drafts,
    Manifest,
    Test,
    Doctor,
    Validate,
    Codegen,
    Update,
    Exit,
}

impl MainMenuAction {
    fn label(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Login => "login",
            Self::Logout => "logout",
            Self::Build => "build",
            Self::Deploy => "deploy",
            Self::Drafts => "drafts",
            Self::Manifest => "manifest",
            Self::Test => "test",
            Self::Doctor => "doctor",
            Self::Validate => "validate",
            Self::Codegen => "codegen",
            Self::Update => "update",
            Self::Exit => "exit program",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Init => "*",
            Self::Login => ">",
            Self::Logout => "<",
            Self::Build => "#",
            Self::Deploy => "^",
            Self::Drafts => "=",
            Self::Manifest => "@",
            Self::Test => "t",
            Self::Doctor => "+",
            Self::Validate => "?",
            Self::Codegen => "%",
            Self::Update => "v",
            Self::Exit => "!",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Init => "Scaffold a new game project with backend, frontend, and client templates.",
            Self::Login => "Authenticate via browser or password to obtain a publish token.",
            Self::Logout => "Remove the stored publish token and sign out locally.",
            Self::Build => "Compile logic WASM and package dist/game.zip for upload.",
            Self::Deploy => "Upload your build to the platform as a draft or published game.",
            Self::Drafts => "List, publish, unpublish, or discard server-side game drafts.",
            Self::Manifest => "Show or edit manifest.json locally, or a server draft.",
            Self::Test => "Run cargo test in the project workspace.",
            Self::Doctor => "Check project layout, client files, and toolchain prerequisites.",
            Self::Validate => "Validate logic.wasm as a WebAssembly component.",
            Self::Codegen => "Generate typed client bindings from game types.",
            Self::Update => "Check for CLI updates against the production platform.",
            Self::Exit => "Leave the interactive session.",
        }
    }

    fn shell_hint(self) -> &'static str {
        match self {
            Self::Init => "gamedev init",
            Self::Login => "gamedev login",
            Self::Logout => "gamedev logout",
            Self::Build => "gamedev build",
            Self::Deploy => "gamedev deploy --publish",
            Self::Drafts => "gamedev drafts list",
            Self::Manifest => "gamedev manifest show",
            Self::Test => "gamedev test",
            Self::Doctor => "gamedev doctor",
            Self::Validate => "gamedev validate",
            Self::Codegen => "gamedev codegen",
            Self::Update => "gamedev update --check",
            Self::Exit => "q / Esc",
        }
    }

    fn requires_project(self) -> bool {
        !matches!(
            self,
            Self::Init | Self::Login | Self::Logout | Self::Doctor | Self::Exit
        )
    }

    fn requires_login(self) -> bool {
        matches!(self, Self::Logout)
    }

    fn requires_auth(self) -> bool {
        matches!(self, Self::Deploy | Self::Drafts)
    }
}

fn all_menu_actions() -> Vec<MainMenuAction> {
    vec![
        MainMenuAction::Init,
        MainMenuAction::Login,
        MainMenuAction::Logout,
        MainMenuAction::Build,
        MainMenuAction::Deploy,
        MainMenuAction::Drafts,
        MainMenuAction::Manifest,
        MainMenuAction::Test,
        MainMenuAction::Doctor,
        MainMenuAction::Validate,
        MainMenuAction::Codegen,
        MainMenuAction::Update,
        MainMenuAction::Exit,
    ]
}

fn action_enabled(action: MainMenuAction, in_project: bool, logged_in: bool) -> bool {
    if action.requires_login() {
        return logged_in;
    }
    !action.requires_project() || in_project
}

pub fn run_terminal_session(mut home_status: HomeStatus) -> Result<()> {
    let mut terminal = ratatui::try_init().map_err(|e| anyhow::anyhow!(e))?;
    let mut stack: Vec<RouteFrame> = vec![RouteFrame::MainMenu {
        list: ratatui::widgets::ListState::default().with_selected(Some(0)),
    }];
    let poll = Duration::from_millis(100);

    loop {
        if interrupted() {
            ratatui::try_restore()?;
            bail!("cancelled");
        }

        if let Some(RouteFrame::JobRun(job)) = stack.last_mut() {
            job.tick();
        }

        terminal.draw(|frame| {
            let area = frame.area();
            let on_home = stack.len() == 1;

            if on_home {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(1)])
                    .split(area);
                draw_home_dashboard(frame, chunks[0], &home_status, &mut stack);
                draw_footer(frame, chunks[1], &stack);
            } else {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2),
                        Constraint::Length(1),
                        Constraint::Min(0),
                        Constraint::Length(1),
                    ])
                    .split(area);

                draw_header(frame, chunks[0], &home_status);
                draw_breadcrumb(frame, chunks[1], &stack);
                draw_body(frame, chunks[2], &mut stack, &home_status);
                draw_footer(frame, chunks[3], &stack);
            }
        })?;

        if !event::poll(poll)? {
            continue;
        }
        let evt = event::read()?;
        let Event::Key(key) = evt else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if let Some(action) = handle_event(&mut stack, key, &evt, &mut home_status)? {
            match action {
                SessionAction::Exit => {
                    ratatui::try_restore()?;
                    return Ok(());
                }
                SessionAction::Run(cmd) => {
                    while stack.len() > 1 {
                        stack.pop();
                    }
                    stack.push(RouteFrame::JobRun(JobRun::start(cmd)));
                }
            }
        }
    }
}

fn draw_header(frame: &mut Frame, area: Rect, home: &HomeStatus) {
    let user = home
        .auth
        .as_ref()
        .map(|a| a.user_id.as_str())
        .unwrap_or("not authenticated");
    let project = home
        .project
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or(theme::ui_none());
    let title = format!(
        "gamedev-cli{}project: {project}{}user: {user}",
        theme::ui_sep(),
        theme::ui_sep()
    );
    let block = Block::new()
        .title(title)
        .borders(Borders::BOTTOM)
        .border_style(theme::tui_header_border());
    frame.render_widget(Paragraph::new("").block(block), area);
}

fn draw_home_dashboard(
    frame: &mut Frame,
    area: Rect,
    home: &HomeStatus,
    stack: &mut [RouteFrame],
) {
    let banner_h = banner::banner_height(area.width);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(banner_h),
            Constraint::Length(8),
            Constraint::Min(0),
        ])
        .split(area);

    banner::draw_banner(frame, chunks[0], &home.cli_version);
    draw_status_cards(frame, chunks[1], home);
    if let Some(RouteFrame::MainMenu { list }) = stack.last_mut() {
        draw_menu_panels(frame, chunks[2], home, list);
    }
}

fn draw_status_cards(frame: &mut Frame, area: Rect, home: &HomeStatus) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(area);

    // User card
    let user_lines = if let Some(auth) = &home.auth {
        vec![
            Line::from(vec![
                Span::styled(format!("{} logged in as ", theme::glyph_ok()), theme::status_style(true)),
                Span::styled(&auth.user_id, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(format!("server {}", auth.server_url)),
            Line::from(format!(
                "expires in {}",
                crate::auth::expires_in_human(auth.expires_at)
            )),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                format!("{} not authenticated", theme::glyph_fail()),
                theme::status_style(false),
            )),
            Line::from("run login to connect"),
            Line::from(""),
        ]
    };
    frame.render_widget(
        Paragraph::new(user_lines).block(theme::card_block(" User ")),
        cols[0],
    );

    // Project card
    let project_lines = if home.in_project {
        if let Some(cfg) = &home.project {
            vec![
                Line::from(Span::styled(
                    &cfg.name,
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(format!(
                    "{:?} {} {:?}",
                    cfg.backend,
                    theme::ui_arrow(),
                    cfg.frontend
                )),
                Line::from(format!("layout: {}", layout_label(home.layout))),
            ]
        } else {
            vec![Line::from("gamedev.toml present")]
        }
    } else {
        vec![Line::from(Span::styled(
            "no game project",
            theme::tui_footer(),
        ))]
        .into_iter()
        .chain([
            Line::from("init / login / doctor only"),
            Line::from(""),
        ])
        .collect()
    };
    frame.render_widget(
        Paragraph::new(project_lines).block(theme::card_block(" Project ")),
        cols[1],
    );

    // Environment card
    let cargo = if home.cargo_ok {
        format!("{} cargo", theme::glyph_ok())
    } else {
        format!("{} cargo", theme::glyph_fail())
    };
    let wasm = if home.wasm_bindgen_ok {
        format!("{} wasm-bindgen", theme::glyph_ok())
    } else {
        format!("{} wasm-bindgen", theme::glyph_warn())
    };
    let env_lines = vec![
        Line::from(format!("CLI v{}", home.cli_version)),
        Line::from(format!("profile: {}", home.default_profile)),
        Line::from(vec![
            Span::raw(cargo),
            Span::raw("  "),
            Span::raw(wasm),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(env_lines).block(theme::card_block(" Environment ")),
        cols[2],
    );
}

fn draw_menu_panels(
    frame: &mut Frame,
    area: Rect,
    home: &HomeStatus,
    list: &mut ratatui::widgets::ListState,
) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let actions = all_menu_actions();
    let selected = list.selected().unwrap_or(0);
    let logged_in = home.auth.is_some();
    let items: Vec<ListItem> = actions
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let enabled = action_enabled(*a, home.in_project, logged_in);
            let prefix = if !enabled { theme::tui_lock_prefix() } else { "" };
            let text = format!("{}{}  {}", prefix, a.icon(), a.label());
            let style = if i == selected {
                theme::tui_highlight()
            } else if !enabled {
                theme::tui_disabled()
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let menu = List::new(items)
        .block(theme::card_block(" Commands "))
        .highlight_symbol(theme::tui_list_marker())
        .highlight_style(theme::tui_highlight());
    frame.render_stateful_widget(menu, panes[0], list);

    let action = actions.get(selected).copied().unwrap_or(MainMenuAction::Init);
    let enabled = action_enabled(action, home.in_project, logged_in);
    let mut detail_lines = vec![
        Line::from(Span::styled(
            action.description(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("command: "),
            Span::styled(
                action.shell_hint(),
                Style::default().fg(theme::tui_accent()),
            ),
        ]),
    ];
    if action.requires_login() && home.auth.is_none() {
        detail_lines.push(Line::from(Span::styled(
            "not logged in",
            Style::default().fg(theme::tui_warn()),
        )));
    }
    if action.requires_auth() && home.auth.is_none() {
        detail_lines.push(Line::from(Span::styled(
            "requires login",
            Style::default().fg(theme::tui_warn()),
        )));
    }
    if action.requires_project() && !home.in_project {
        detail_lines.push(Line::from(Span::styled(
            "requires game project (gamedev.toml)",
            Style::default().fg(theme::tui_warn()),
        )));
    }
    if !enabled {
        detail_lines.push(Line::from(Span::styled(
            format!("locked{}open a game project directory first", theme::ui_dash()),
            theme::tui_disabled(),
        )));
    }
    frame.render_widget(
        Paragraph::new(detail_lines).block(theme::card_block(" Details ")),
        panes[1],
    );
}

fn footer_sep() -> Span<'static> {
    Span::styled(theme::ui_sep(), theme::tui_footer())
}

fn draw_footer(frame: &mut Frame, area: Rect, stack: &[RouteFrame]) {
    let line = match stack.last() {
        Some(RouteFrame::MainMenu { .. }) if stack.len() == 1 => Line::from(vec![
            Span::styled(theme::key_cycle(), theme::tui_keycap()),
            Span::raw(" cycle"),
            footer_sep(),
            Span::styled(theme::key_enter(), theme::tui_keycap()),
            Span::raw(" open"),
            footer_sep(),
            Span::styled("q", theme::tui_keycap()),
            Span::raw(" quit"),
        ]),
        Some(RouteFrame::LoginBrowser { .. })
        | Some(RouteFrame::Login { .. })
        | Some(RouteFrame::DeployServer { .. })
        | Some(RouteFrame::DraftsServer { .. })
        | Some(RouteFrame::DraftId { .. })
        | Some(RouteFrame::ManifestServer { .. })
        | Some(RouteFrame::ManifestDraftId { .. })
        | Some(RouteFrame::ManifestEditFields { .. }) => Line::from(vec![
            Span::styled("esc", theme::tui_keycap()),
            Span::raw(" back"),
            footer_sep(),
            Span::styled(theme::key_enter(), theme::tui_keycap()),
            Span::raw(" confirm"),
            footer_sep(),
            Span::styled("tab", theme::tui_keycap()),
            Span::raw(" next field"),
        ]),
        Some(RouteFrame::JobRun(job)) => {
            let hints = if job.is_done() {
                vec![
                    Span::styled(theme::key_enter(), theme::tui_keycap()),
                    Span::raw(" home"),
                    footer_sep(),
                    Span::styled(theme::key_cycle(), theme::tui_keycap()),
                    Span::raw(" scroll log"),
                ]
            } else {
                vec![
                    Span::styled("[..]", theme::tui_keycap()),
                    Span::raw(" running"),
                ]
            };
            Line::from(hints)
        }
        Some(RouteFrame::Init(_)) => Line::from(vec![
            Span::styled("esc", theme::tui_keycap()),
            Span::raw(" back"),
            footer_sep(),
            Span::styled("tab", theme::tui_keycap()),
            Span::raw(" next field"),
            footer_sep(),
            Span::styled(theme::key_cycle(), theme::tui_keycap()),
            Span::raw(" change"),
            footer_sep(),
            Span::styled(theme::key_enter(), theme::tui_keycap()),
            Span::raw(" confirm"),
        ]),
        _ => Line::from(vec![
            Span::styled("esc", theme::tui_keycap()),
            Span::raw(" back"),
            footer_sep(),
            Span::styled(theme::key_enter(), theme::tui_keycap()),
            Span::raw(" confirm"),
            footer_sep(),
            Span::styled(theme::key_cycle(), theme::tui_keycap()),
            Span::raw(" select"),
        ]),
    };
    frame.render_widget(Paragraph::new(line).style(theme::tui_footer()), area);
}

fn draw_body(frame: &mut Frame, area: Rect, stack: &mut [RouteFrame], _home: &HomeStatus) {
    let Some(top) = stack.last_mut() else {
        return;
    };
    match top {
        RouteFrame::MainMenu { .. } => {
            frame.render_widget(
                Paragraph::new("").block(theme::card_block(" Main menu ")),
                area,
            );
        }
        RouteFrame::Init(w) => w.draw(frame, area),
        RouteFrame::LoginMode { list } => {
            let items = vec![
                ListItem::new("login via browser (recommended)"),
                ListItem::new("login with password / publish token"),
            ];
            let list_w = List::new(items)
                .block(theme::panel_block("Login"))
                .highlight_symbol(theme::tui_list_marker())
                .highlight_style(theme::tui_highlight());
            frame.render_stateful_widget(list_w, area, list);
        }
        RouteFrame::LoginBrowser { server } => {
            frame.render_widget(
                Paragraph::new(server.to_string())
                    .block(theme::panel_block("Server URL (browser login)")),
                area,
            );
        }
        RouteFrame::Login {
            display_name,
            password,
            publish_token,
            server,
            field,
        } => {
            let labels = [
                ("Display name", display_name, *field == 0),
                ("Password", password, *field == 1),
                ("Publish token (optional)", publish_token, *field == 2),
                ("Server URL", server, *field == 3),
            ];
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3); 4].as_ref())
                .split(area);
            for (i, (label, inp, active)) in labels.into_iter().enumerate() {
                let t = if active {
                    format!("{label} >")
                } else {
                    label.to_string()
                };
                let display = if i == 1 {
                    "*".repeat(inp.to_string().chars().count())
                } else {
                    inp.to_string()
                };
                let b = theme::field_block(t);
                frame.render_widget(Paragraph::new(display).block(b), chunks[i]);
            }
        }
        RouteFrame::BuildConfirm => {
            frame.render_widget(
                Paragraph::new(
                    "Build current directory into dist/game.zip?\n\nEnter: run build | Esc: back",
                )
                .block(theme::panel_block("Build")),
                area,
            );
        }
        RouteFrame::DeployMode { list } => {
            let items = vec![
                ListItem::new("upload draft only"),
                ListItem::new("upload and publish"),
            ];
            let list_w = List::new(items)
                .block(theme::panel_block("Deploy mode"))
                .highlight_symbol(theme::tui_list_marker())
                .highlight_style(theme::tui_highlight());
            frame.render_stateful_widget(list_w, area, list);
        }
        RouteFrame::DeployServer { server, publish } => {
            let title = if *publish {
                "Server URL - PUBLISH (live)"
            } else {
                "Server URL - upload draft only"
            };
            let border = if *publish {
                Style::default().fg(theme::tui_danger())
            } else {
                theme::tui_header_border()
            };
            frame.render_widget(
                Paragraph::new(server.to_string()).block(
                    theme::panel_block(title).border_style(border),
                ),
                area,
            );
        }
        RouteFrame::DraftsMode { list } => {
            let items = vec![
                ListItem::new("list"),
                ListItem::new("publish"),
                ListItem::new("unpublish"),
                ListItem::new("discard"),
            ];
            let list_w = List::new(items)
                .block(theme::panel_block("Drafts"))
                .highlight_symbol(theme::tui_list_marker())
                .highlight_style(theme::tui_highlight());
            frame.render_stateful_widget(list_w, area, list);
        }
        RouteFrame::DraftsServer { server, .. } => {
            frame.render_widget(
                Paragraph::new(server.to_string())
                    .block(theme::panel_block("Server URL")),
                area,
            );
        }
        RouteFrame::DraftId { draft_id, .. } => {
            frame.render_widget(
                Paragraph::new(draft_id.to_string())
                    .block(theme::panel_block("Draft id")),
                area,
            );
        }
        RouteFrame::ManifestMode { list } => {
            let items = vec![
                ListItem::new("show local manifest.json"),
                ListItem::new("edit local manifest.json"),
                ListItem::new("show server draft"),
                ListItem::new("edit server draft"),
            ];
            let list_w = List::new(items)
                .block(theme::panel_block("Manifest"))
                .highlight_symbol(theme::tui_list_marker())
                .highlight_style(theme::tui_highlight());
            frame.render_stateful_widget(list_w, area, list);
        }
        RouteFrame::ManifestServer { server, .. } => {
            frame.render_widget(
                Paragraph::new(server.to_string())
                    .block(theme::panel_block("Server URL")),
                area,
            );
        }
        RouteFrame::ManifestDraftId { draft_id, .. } => {
            frame.render_widget(
                Paragraph::new(draft_id.to_string())
                    .block(theme::panel_block("Draft id")),
                area,
            );
        }
        RouteFrame::ManifestEditFields {
            name,
            display_name,
            version,
            description,
            field,
            ..
        } => {
            let labels = [
                ("Name", name, *field == 0),
                ("Display name", display_name, *field == 1),
                ("Version", version, *field == 2),
                ("Description", description, *field == 3),
            ];
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3); 4].as_ref())
                .split(area);
            for (i, (label, inp, active)) in labels.into_iter().enumerate() {
                let t = if active {
                    format!("{label} >")
                } else {
                    label.to_string()
                };
                let b = theme::field_block(t);
                frame.render_widget(Paragraph::new(inp.to_string()).block(b), chunks[i]);
            }
        }
        RouteFrame::TestConfirm => {
            let hint = std::env::current_dir()
                .map(|root| {
                    let dir = resolve_test_dir(&root);
                    format!(
                        "Run `cargo test` in {}?\n\nEnter: run | Esc: back",
                        dir.display()
                    )
                })
                .unwrap_or_else(|_| {
                    "Run `cargo test` in project?\n\nEnter: run | Esc: back".to_string()
                });
            frame.render_widget(
                Paragraph::new(hint).block(theme::panel_block("Test")),
                area,
            );
        }
        RouteFrame::DoctorConfirm => {
            frame.render_widget(
                Paragraph::new(
                    "Check project layout, client files, and toolchain (cargo, wasm-bindgen, npm)?\n\nEnter: run doctor | Esc: back",
                )
                .block(theme::panel_block("Doctor")),
                area,
            );
        }
        RouteFrame::ValidateConfirm => {
            frame.render_widget(
                Paragraph::new(
                    "Validate logic.wasm as a WebAssembly component?\n\nEnter: run validate | Esc: back",
                )
                .block(theme::panel_block("Validate")),
                area,
            );
        }
        RouteFrame::CodegenConfirm => {
            frame.render_widget(
                Paragraph::new(
                    "Generate typed client bindings from game types?\n\nEnter: run codegen | Esc: back",
                )
                .block(theme::panel_block("Codegen")),
                area,
            );
        }
        RouteFrame::UpdateConfirm => {
            frame.render_widget(
                Paragraph::new(
                    "Check for CLI updates against production platform?\n\nEnter: run update --check | Esc: back",
                )
                .block(theme::panel_block("Update")),
                area,
            );
        }
        RouteFrame::LogoutConfirm { user_id, server_url } => {
            let msg = format!(
                "Sign out {user_id}?\n\nServer: {server_url}\n\nStored publish token will be removed from this machine.\n\nEnter: confirm | Esc: back"
            );
            frame.render_widget(
                Paragraph::new(msg).block(theme::panel_block("Logout")),
                area,
            );
        }
        RouteFrame::JobRun(job) => job.draw(frame, area),
    }
}

fn draw_breadcrumb(frame: &mut Frame, area: Rect, stack: &[RouteFrame]) {
    let state = breadcrumb_state(stack);
    let bc = Breadcrumb::new(&state).style(BreadcrumbStyle::chevron());
    bc.render_stateful(area, frame.buffer_mut());
}

fn handle_event(
    stack: &mut Vec<RouteFrame>,
    key: event::KeyEvent,
    evt: &Event,
    home_status: &mut HomeStatus,
) -> Result<Option<SessionAction>> {
    if let Some(RouteFrame::JobRun(job)) = stack.last_mut() {
        if matches!(job.handle_key(key.code), JobKeyAction::Dismiss) {
            stack.pop();
            *home_status = status::build_home_status();
        }
        return Ok(None);
    }

    match key.code {
        KeyCode::Char('q') if stack.len() == 1 => return Ok(Some(SessionAction::Exit)),
        KeyCode::Esc => {
            if stack.len() > 1 {
                stack.pop();
            } else {
                return Ok(Some(SessionAction::Exit));
            }
            return Ok(None);
        }
        _ => {}
    }

    if matches!(stack.last(), Some(RouteFrame::MainMenu { .. })) {
        if nav::is_list_prev(key.code) {
            if let Some(RouteFrame::MainMenu { list }) = stack.last_mut() {
                nav::cycle_list(list, -1, all_menu_actions().len());
            }
            return Ok(None);
        }
        if nav::is_list_next(key.code) {
            if let Some(RouteFrame::MainMenu { list }) = stack.last_mut() {
                nav::cycle_list(list, 1, all_menu_actions().len());
            }
            return Ok(None);
        }
        match key.code {
            KeyCode::Enter => {
                let idx = if let Some(RouteFrame::MainMenu { list }) = stack.last_mut() {
                    list.selected().unwrap_or(0)
                } else {
                    return Ok(None);
                };
                let actions = all_menu_actions();
                let Some(action) = actions.get(idx) else {
                    return Ok(None);
                };
                if !action_enabled(*action, home_status.in_project, home_status.auth.is_some()) {
                    return Ok(None);
                }
                match action {
                    MainMenuAction::Init => stack.push(RouteFrame::Init(InitWizardState::new())),
                    MainMenuAction::Login => stack.push(RouteFrame::LoginMode {
                        list: ratatui::widgets::ListState::default().with_selected(Some(0)),
                    }),
                    MainMenuAction::Logout => {
                        if let Some(auth) = home_status.auth.clone() {
                            stack.push(RouteFrame::LogoutConfirm {
                                user_id: auth.user_id,
                                server_url: auth.server_url,
                            });
                        }
                    }
                    MainMenuAction::Build => stack.push(RouteFrame::BuildConfirm),
                    MainMenuAction::Deploy => stack.push(RouteFrame::DeployMode {
                        list: ratatui::widgets::ListState::default().with_selected(Some(0)),
                    }),
                    MainMenuAction::Drafts => stack.push(RouteFrame::DraftsMode {
                        list: ratatui::widgets::ListState::default().with_selected(Some(0)),
                    }),
                    MainMenuAction::Manifest => stack.push(RouteFrame::ManifestMode {
                        list: ratatui::widgets::ListState::default().with_selected(Some(0)),
                    }),
                    MainMenuAction::Test => stack.push(RouteFrame::TestConfirm),
                    MainMenuAction::Doctor => stack.push(RouteFrame::DoctorConfirm),
                    MainMenuAction::Validate => stack.push(RouteFrame::ValidateConfirm),
                    MainMenuAction::Codegen => stack.push(RouteFrame::CodegenConfirm),
                    MainMenuAction::Update => stack.push(RouteFrame::UpdateConfirm),
                    MainMenuAction::Exit => return Ok(Some(SessionAction::Exit)),
                }
                return Ok(None);
            }
            _ => return Ok(None),
        }
    }

    let Some(top) = stack.last_mut() else {
        return Ok(None);
    };

    match top {
        RouteFrame::JobRun(_) => Ok(None),
        RouteFrame::MainMenu { .. } => Ok(None),
        RouteFrame::Init(w) => {
            if let Some(out) = w.handle_key(key) {
                stack.pop();
                match out {
                    InitWizardOutcome::Submit(args) => {
                        return Ok(Some(SessionAction::Run(UiCommand::Init(args))));
                    }
                    InitWizardOutcome::Cancel => {}
                }
            }
            Ok(None)
        }
        RouteFrame::LoginMode { list } => {
            if nav::is_list_prev(key.code) {
                nav::cycle_list(list, -1, 2);
                return Ok(None);
            }
            if nav::is_list_next(key.code) {
                nav::cycle_list(list, 1, 2);
                return Ok(None);
            }
            match key.code {
            KeyCode::Enter => {
                let i = list.selected().unwrap_or(0);
                stack.pop();
                if i == 0 {
                    stack.push(RouteFrame::LoginBrowser {
                        server: tui_input::Input::new(DEFAULT_GRAPHQL_URL.to_string()),
                    });
                } else {
                    stack.push(RouteFrame::Login {
                        display_name: tui_input::Input::default(),
                        password: tui_input::Input::default(),
                        publish_token: tui_input::Input::default(),
                        server: tui_input::Input::new(DEFAULT_GRAPHQL_URL.to_string()),
                        field: 0,
                    });
                }
                Ok(None)
            }
            _ => Ok(None),
            }
        }
        RouteFrame::LoginBrowser { server } => {
            if let Some(req) = tui_input::backend::crossterm::to_input_request(evt) {
                server.handle(req);
            }
            if key.code == KeyCode::Enter {
                let s = server.to_string().trim().to_string();
                if s.is_empty() {
                    return Ok(None);
                }
                stack.pop();
                return Ok(Some(SessionAction::Run(UiCommand::Login(LoginArgs {
                    server_url: s,
                    profile: None,
                    user_id: None,
                    display_name: None,
                    password: None,
                    publish_token: None,
                    web: true,
                }))));
            }
            Ok(None)
        }
        RouteFrame::Login {
            display_name,
            password,
            publish_token,
            server,
            field,
        } => {
            match key.code {
                KeyCode::Tab => *field = (*field + 1) % 4,
                KeyCode::Enter if *field == 3 => {
                    let s = server.to_string().trim().to_string();
                    if s.is_empty() {
                        return Ok(None);
                    }
                    let pt = publish_token.to_string().trim().to_string();
                    let name = display_name.to_string().trim().to_string();
                    let pass = password.to_string().trim().to_string();
                    stack.pop();
                    if !pt.is_empty() {
                        return Ok(Some(SessionAction::Run(UiCommand::Login(LoginArgs {
                            server_url: s,
                            profile: None,
                            user_id: None,
                            display_name: None,
                            password: None,
                            publish_token: Some(pt),
                            web: false,
                        }))));
                    }
                    if name.is_empty() || pass.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(SessionAction::Run(UiCommand::Login(LoginArgs {
                        server_url: s,
                        profile: None,
                        user_id: None,
                        display_name: Some(name),
                        password: Some(pass),
                        publish_token: None,
                        web: false,
                    }))));
                }
                _ => {
                    if let Some(req) = tui_input::backend::crossterm::to_input_request(evt) {
                        match *field {
                            0 => {
                                let _ = display_name.handle(req);
                            }
                            1 => {
                                let _ = password.handle(req);
                            }
                            2 => {
                                let _ = publish_token.handle(req);
                            }
                            _ => {
                                let _ = server.handle(req);
                            }
                        }
                    }
                }
            }
            Ok(None)
        }
        RouteFrame::BuildConfirm => match key.code {
            KeyCode::Enter => {
                stack.pop();
                Ok(Some(SessionAction::Run(UiCommand::Build(BuildArgs {
                    project_dir: None,
                    out: None,
                    strict: false,
                }))))
            }
            _ => Ok(None),
        },
        RouteFrame::DeployMode { list } => {
            if nav::is_list_prev(key.code) {
                nav::cycle_list(list, -1, 2);
                return Ok(None);
            }
            if nav::is_list_next(key.code) {
                nav::cycle_list(list, 1, 2);
                return Ok(None);
            }
            match key.code {
            KeyCode::Enter => {
                let i = list.selected().unwrap_or(0);
                let publish = i == 1;
                stack.pop();
                stack.push(RouteFrame::DeployServer {
                    server: tui_input::Input::new(DEFAULT_GRAPHQL_URL.to_string()),
                    publish,
                });
                Ok(None)
            }
            _ => Ok(None),
            }
        }
        RouteFrame::DeployServer { server, publish } => {
            if let Some(req) = tui_input::backend::crossterm::to_input_request(evt) {
                server.handle(req);
            }
            if key.code == KeyCode::Enter {
                let s = server.to_string().trim().to_string();
                if s.is_empty() {
                    return Ok(None);
                }
                let pub_flag = *publish;
                stack.pop();
                return Ok(Some(SessionAction::Run(UiCommand::Deploy(DeployArgs {
                    project_dir: None,
                    server_url: s,
                    profile: None,
                    publish: pub_flag,
                    auto_publish: false,
                    draft_only: false,
                }))));
            }
            Ok(None)
        }
        RouteFrame::DraftsMode { list } => {
            if nav::is_list_prev(key.code) {
                nav::cycle_list(list, -1, 4);
                return Ok(None);
            }
            if nav::is_list_next(key.code) {
                nav::cycle_list(list, 1, 4);
                return Ok(None);
            }
            match key.code {
            KeyCode::Enter => {
                let action = match list.selected().unwrap_or(0) {
                    0 => DraftMenuAction::List,
                    1 => DraftMenuAction::Publish,
                    2 => DraftMenuAction::Unpublish,
                    _ => DraftMenuAction::Discard,
                };
                stack.pop();
                stack.push(RouteFrame::DraftsServer {
                    server: tui_input::Input::new(DEFAULT_GRAPHQL_URL.to_string()),
                    action,
                });
                Ok(None)
            }
            _ => Ok(None),
            }
        }
        RouteFrame::DraftsServer { server, action } => {
            if let Some(req) = tui_input::backend::crossterm::to_input_request(evt) {
                server.handle(req);
            }
            if key.code == KeyCode::Enter {
                let s = server.to_string().trim().to_string();
                if s.is_empty() {
                    return Ok(None);
                }
                let a = *action;
                stack.pop();
                if matches!(a, DraftMenuAction::List) {
                    return Ok(Some(SessionAction::Run(UiCommand::Drafts(DraftsArgs {
                        command: DraftsSubcommands::List,
                        server_url: s,
                        profile: None,
                    }))));
                }
                stack.push(RouteFrame::DraftId {
                    server: s,
                    action: a,
                    draft_id: tui_input::Input::default(),
                });
            }
            Ok(None)
        }
        RouteFrame::DraftId {
            server,
            action,
            draft_id,
        } => {
            if let Some(req) = tui_input::backend::crossterm::to_input_request(evt) {
                draft_id.handle(req);
            }
            if key.code == KeyCode::Enter {
                let id = draft_id.to_string().trim().to_string();
                if id.is_empty() {
                    return Ok(None);
                }
                let srv = server.clone();
                let cmd = match action {
                    DraftMenuAction::List => unreachable!(),
                    DraftMenuAction::Publish => DraftsSubcommands::Publish { draft_id: id },
                    DraftMenuAction::Unpublish => DraftsSubcommands::Unpublish { draft_id: id },
                    DraftMenuAction::Discard => DraftsSubcommands::Discard { draft_id: id },
                };
                stack.pop();
                return Ok(Some(SessionAction::Run(UiCommand::Drafts(DraftsArgs {
                    command: cmd,
                    server_url: srv,
                    profile: None,
                }))));
            }
            Ok(None)
        }
        RouteFrame::ManifestMode { list } => {
            if nav::is_list_prev(key.code) {
                nav::cycle_list(list, -1, 4);
                return Ok(None);
            }
            if nav::is_list_next(key.code) {
                nav::cycle_list(list, 1, 4);
                return Ok(None);
            }
            match key.code {
            KeyCode::Enter => {
                let idx = list.selected().unwrap_or(0);
                stack.pop();
                match idx {
                    0 => {
                        return Ok(Some(SessionAction::Run(UiCommand::Manifest(ManifestArgs {
                            command: ManifestSubcommands::Show {
                                draft_id: None,
                                project_dir: None,
                            },
                            server_url: DEFAULT_GRAPHQL_URL.to_string(),
                            profile: None,
                        }))));
                    }
                    1 => {
                        let fields = crate::manifest::read_fields(&std::env::current_dir()?)
                            .unwrap_or(crate::manifest::ManifestFields {
                                name: String::new(),
                                display_name: String::new(),
                                version: String::new(),
                                description: String::new(),
                            });
                        stack.push(RouteFrame::ManifestEditFields {
                            server: None,
                            draft_id: None,
                            name: tui_input::Input::new(fields.name),
                            display_name: tui_input::Input::new(fields.display_name),
                            version: tui_input::Input::new(fields.version),
                            description: tui_input::Input::new(fields.description),
                            field: 0,
                        });
                    }
                    2 => {
                        stack.push(RouteFrame::ManifestServer {
                            server: tui_input::Input::new(DEFAULT_GRAPHQL_URL.to_string()),
                            next: ManifestNext::Show,
                        });
                    }
                    _ => {
                        stack.push(RouteFrame::ManifestServer {
                            server: tui_input::Input::new(DEFAULT_GRAPHQL_URL.to_string()),
                            next: ManifestNext::Edit,
                        });
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
            }
        }
        RouteFrame::ManifestServer { server, next } => {
            if let Some(req) = tui_input::backend::crossterm::to_input_request(evt) {
                server.handle(req);
            }
            if key.code == KeyCode::Enter {
                let s = server.to_string().trim().to_string();
                if s.is_empty() {
                    return Ok(None);
                }
                let n = *next;
                stack.pop();
                stack.push(RouteFrame::ManifestDraftId {
                    server: s,
                    next: n,
                    draft_id: tui_input::Input::default(),
                });
            }
            Ok(None)
        }
        RouteFrame::ManifestDraftId {
            server,
            next,
            draft_id,
        } => {
            if let Some(req) = tui_input::backend::crossterm::to_input_request(evt) {
                draft_id.handle(req);
            }
            if key.code == KeyCode::Enter {
                let id = draft_id.to_string().trim().to_string();
                if id.is_empty() {
                    return Ok(None);
                }
                let srv = server.clone();
                let n = *next;
                stack.pop();
                match n {
                    ManifestNext::Show => {
                        return Ok(Some(SessionAction::Run(UiCommand::Manifest(ManifestArgs {
                            command: ManifestSubcommands::Show {
                                draft_id: Some(id),
                                project_dir: None,
                            },
                            server_url: srv,
                            profile: None,
                        }))));
                    }
                    ManifestNext::Edit => {
                        stack.push(RouteFrame::ManifestEditFields {
                            server: Some(srv),
                            draft_id: Some(id),
                            name: tui_input::Input::default(),
                            display_name: tui_input::Input::default(),
                            version: tui_input::Input::default(),
                            description: tui_input::Input::default(),
                            field: 0,
                        });
                    }
                }
            }
            Ok(None)
        }
        RouteFrame::ManifestEditFields {
            server,
            draft_id,
            name,
            display_name,
            version,
            description,
            field,
        } => {
            if let Some(req) = tui_input::backend::crossterm::to_input_request(evt) {
                let _ = match *field {
                    0 => name.handle(req),
                    1 => display_name.handle(req),
                    2 => version.handle(req),
                    _ => description.handle(req),
                };
            }
            match key.code {
                KeyCode::Tab => *field = (*field + 1) % 4,
                KeyCode::Enter if *field == 3 => {
                    let n = name.to_string().trim().to_string();
                    let d = display_name.to_string().trim().to_string();
                    let v = version.to_string().trim().to_string();
                    let desc = description.to_string().trim().to_string();
                    if n.is_empty() || d.is_empty() || v.is_empty() || desc.is_empty() {
                        return Ok(None);
                    }
                    let srv = server
                        .clone()
                        .unwrap_or_else(|| DEFAULT_GRAPHQL_URL.to_string());
                    let did = draft_id.clone();
                    stack.pop();
                    return Ok(Some(SessionAction::Run(UiCommand::Manifest(ManifestArgs {
                        command: ManifestSubcommands::Edit {
                            draft_id: did,
                            project_dir: None,
                            name: n,
                            display_name: d,
                            version: v,
                            description: desc,
                        },
                        server_url: srv,
                        profile: None,
                    }))));
                }
                _ => {}
            }
            Ok(None)
        }
        RouteFrame::TestConfirm => match key.code {
            KeyCode::Enter => {
                stack.pop();
                Ok(Some(SessionAction::Run(UiCommand::Test(TestArgs { project_dir: None }))))
            }
            _ => Ok(None),
        },
        RouteFrame::DoctorConfirm => match key.code {
            KeyCode::Enter => {
                stack.pop();
                Ok(Some(SessionAction::Run(UiCommand::Doctor(DoctorArgs {
                    project_dir: None,
                    platform: None,
                    matrix: false,
                }))))
            }
            _ => Ok(None),
        },
        RouteFrame::ValidateConfirm => match key.code {
            KeyCode::Enter => {
                stack.pop();
                Ok(Some(SessionAction::Run(UiCommand::Validate(ValidateArgs {
                    project_dir: None,
                    logic_wasm: None,
                }))))
            }
            _ => Ok(None),
        },
        RouteFrame::CodegenConfirm => match key.code {
            KeyCode::Enter => {
                stack.pop();
                Ok(Some(SessionAction::Run(UiCommand::Codegen(CodegenArgs { project_dir: None }))))
            }
            _ => Ok(None),
        },
        RouteFrame::UpdateConfirm => match key.code {
            KeyCode::Enter => {
                stack.pop();
                Ok(Some(SessionAction::Run(UiCommand::Update(UpdateArgs {
                    platform: crate::config::PROD_PLATFORM_BASE.to_string(),
                    check: true,
                }))))
            }
            _ => Ok(None),
        },
        RouteFrame::LogoutConfirm { server_url, .. } => match key.code {
            KeyCode::Enter => {
                let url = server_url.clone();
                stack.pop();
                Ok(Some(SessionAction::Run(UiCommand::Logout(LogoutArgs {
                    server_url: url,
                    profile: None,
                    all: false,
                }))))
            }
            _ => Ok(None),
        },
    }
}
