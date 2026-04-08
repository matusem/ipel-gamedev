//! Single terminal session: route stack, chrome, and screens.

use std::time::Duration;

use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use ratatui_interact::components::{Breadcrumb, BreadcrumbStyle};

use crate::cli::{
    BuildArgs, DEFAULT_GRAPHQL_URL, DeployArgs, DraftsArgs, DraftsSubcommands, LoginArgs,
    ManifestArgs, ManifestSubcommands, TestArgs,
};

use super::init_wizard::{InitWizardOutcome, InitWizardState};
use super::router::{breadcrumb_state, DraftMenuAction, ManifestNext, RouteFrame};
use super::{interrupted, UiCommand};

const MAIN_ITEMS: &[&str] = &[
    "init",
    "login",
    "build",
    "deploy",
    "drafts",
    "manifest",
    "test",
    "exit program",
];

pub fn run_terminal_session(auth_user: Option<String>) -> Result<UiCommand> {
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

        terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Length(1),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .split(area);

            draw_header(frame, chunks[0], &auth_user);
            draw_breadcrumb(frame, chunks[1], &stack);
            draw_body(frame, chunks[2], &mut stack);
            draw_footer(frame, chunks[3], &stack);
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

        if let Some(cmd) = handle_event(&mut stack, key, &evt)? {
            ratatui::try_restore()?;
            return Ok(cmd);
        }
    }
}

fn project_hint() -> String {
    std::env::current_dir()
        .ok()
        .as_ref()
        .and_then(|root| crate::project::load_config(root).ok().map(|c| c.name))
        .unwrap_or_else(|| "—".to_string())
}

fn draw_header(frame: &mut Frame, area: Rect, auth_user: &Option<String>) {
    let user = auth_user
        .as_deref()
        .unwrap_or("not authenticated");
    let title = format!("gamedev-cli · project: {} · user: {}", project_hint(), user);
    let block = Block::new()
        .title(title)
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(Paragraph::new("").block(block), area);
}

fn draw_breadcrumb(frame: &mut Frame, area: Rect, stack: &[RouteFrame]) {
    let state = breadcrumb_state(stack);
    let bc = Breadcrumb::new(&state).style(BreadcrumbStyle::chevron());
    bc.render_stateful(area, frame.buffer_mut());
}

fn draw_footer(frame: &mut Frame, area: Rect, stack: &[RouteFrame]) {
    let hint = if stack.len() <= 1 {
        "↑↓/jk: menu · Enter: open · q: quit"
    } else {
        "Esc: back · Enter: confirm · Tab: next field"
    };
    let p = Paragraph::new(hint).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(p, area);
}

fn draw_body(frame: &mut Frame, area: Rect, stack: &mut [RouteFrame]) {
    let Some(top) = stack.last_mut() else {
        return;
    };
    match top {
        RouteFrame::MainMenu { list } => {
            let items: Vec<ListItem> = MAIN_ITEMS.iter().map(|i| ListItem::new(*i)).collect();
            let list_w = List::new(items)
                .block(Block::new().title("Main menu").borders(Borders::ALL))
                .highlight_symbol(">> ");
            frame.render_stateful_widget(list_w, area, list);
        }
        RouteFrame::Init(w) => w.draw(frame, area),
        RouteFrame::Login { user, server, field } => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Min(0)])
                .split(area);
            let u_title = if *field == 0 { "User id (UUID) »" } else { "User id" };
            let s_title = if *field == 1 { "Server URL »" } else { "Server URL" };
            let ub = Block::new().title(u_title).borders(Borders::ALL);
            let sb = Block::new().title(s_title).borders(Borders::ALL);
            frame.render_widget(Paragraph::new(user.to_string()).block(ub), chunks[0]);
            frame.render_widget(Paragraph::new(server.to_string()).block(sb), chunks[1]);
            frame.render_widget(
                Paragraph::new("Tab: switch field · Enter: submit (on server field)"),
                chunks[2],
            );
        }
        RouteFrame::BuildConfirm => {
            frame.render_widget(
                Paragraph::new("Build current directory into dist/game.zip?\n\nEnter: run build · Esc: back")
                    .block(Block::new().title("Build").borders(Borders::ALL)),
                area,
            );
        }
        RouteFrame::DeployMode { list } => {
            let items = vec![
                ListItem::new("deploy draft only"),
                ListItem::new("deploy and publish"),
            ];
            let list_w = List::new(items)
                .block(Block::new().title("Deploy mode").borders(Borders::ALL))
                .highlight_symbol(">> ");
            frame.render_stateful_widget(list_w, area, list);
        }
        RouteFrame::DeployServer { server, .. } => {
            frame.render_widget(
                Paragraph::new(server.to_string())
                    .block(Block::new().title("Server URL").borders(Borders::ALL)),
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
                .block(Block::new().title("Drafts").borders(Borders::ALL))
                .highlight_symbol(">> ");
            frame.render_stateful_widget(list_w, area, list);
        }
        RouteFrame::DraftsServer { server, .. } => {
            frame.render_widget(
                Paragraph::new(server.to_string())
                    .block(Block::new().title("Server URL").borders(Borders::ALL)),
                area,
            );
        }
        RouteFrame::DraftId { draft_id, .. } => {
            frame.render_widget(
                Paragraph::new(draft_id.to_string())
                    .block(Block::new().title("Draft id").borders(Borders::ALL)),
                area,
            );
        }
        RouteFrame::ManifestMode { list } => {
            let items = vec![ListItem::new("show"), ListItem::new("edit")];
            let list_w = List::new(items)
                .block(Block::new().title("Manifest").borders(Borders::ALL))
                .highlight_symbol(">> ");
            frame.render_stateful_widget(list_w, area, list);
        }
        RouteFrame::ManifestServer { server, .. } => {
            frame.render_widget(
                Paragraph::new(server.to_string())
                    .block(Block::new().title("Server URL").borders(Borders::ALL)),
                area,
            );
        }
        RouteFrame::ManifestDraftId { draft_id, .. } => {
            frame.render_widget(
                Paragraph::new(draft_id.to_string())
                    .block(Block::new().title("Draft id").borders(Borders::ALL)),
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
                    format!("{label} »")
                } else {
                    label.to_string()
                };
                let b = Block::new().title(t).borders(Borders::ALL);
                frame.render_widget(Paragraph::new(inp.to_string()).block(b), chunks[i]);
            }
        }
        RouteFrame::TestConfirm => {
            frame.render_widget(
                Paragraph::new("Run `cargo test` in logic crate?\n\nEnter: run · Esc: back")
                    .block(Block::new().title("Test").borders(Borders::ALL)),
                area,
            );
        }
    }
}

fn handle_event(stack: &mut Vec<RouteFrame>, key: event::KeyEvent, evt: &Event) -> Result<Option<UiCommand>> {
    match key.code {
        KeyCode::Char('q') if stack.len() == 1 => return Ok(Some(UiCommand::ExitProgram)),
        KeyCode::Esc => {
            if stack.len() > 1 {
                stack.pop();
            } else {
                return Ok(Some(UiCommand::ExitProgram));
            }
            return Ok(None);
        }
        _ => {}
    }

    if matches!(stack.last(), Some(RouteFrame::MainMenu { .. })) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(RouteFrame::MainMenu { list }) = stack.last_mut() {
                    let i = list.selected().unwrap_or(0).saturating_sub(1);
                    list.select(Some(i));
                }
                return Ok(None);
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                if let Some(RouteFrame::MainMenu { list }) = stack.last_mut() {
                    let i = (list.selected().unwrap_or(0) + 1).min(MAIN_ITEMS.len() - 1);
                    list.select(Some(i));
                }
                return Ok(None);
            }
            KeyCode::Enter => {
                let idx = if let Some(RouteFrame::MainMenu { list }) = stack.last_mut() {
                    list.selected().unwrap_or(0)
                } else {
                    return Ok(None);
                };
                match idx {
                    0 => stack.push(RouteFrame::Init(InitWizardState::new())),
                    1 => stack.push(RouteFrame::Login {
                        user: tui_input::Input::default(),
                        server: tui_input::Input::new(DEFAULT_GRAPHQL_URL.to_string()),
                        field: 0,
                    }),
                    2 => stack.push(RouteFrame::BuildConfirm),
                    3 => stack.push(RouteFrame::DeployMode {
                        list: ratatui::widgets::ListState::default().with_selected(Some(0)),
                    }),
                    4 => stack.push(RouteFrame::DraftsMode {
                        list: ratatui::widgets::ListState::default().with_selected(Some(0)),
                    }),
                    5 => stack.push(RouteFrame::ManifestMode {
                        list: ratatui::widgets::ListState::default().with_selected(Some(0)),
                    }),
                    6 => stack.push(RouteFrame::TestConfirm),
                    _ => return Ok(Some(UiCommand::ExitProgram)),
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
        RouteFrame::MainMenu { .. } => Ok(None),
        RouteFrame::Init(w) => {
            if let Some(out) = w.handle_key(key) {
                stack.pop();
                match out {
                    InitWizardOutcome::Submit(args) => return Ok(Some(UiCommand::Init(args))),
                    InitWizardOutcome::Cancel => {}
                }
            }
            Ok(None)
        }
        RouteFrame::Login { user, server, field } => {
            match key.code {
                KeyCode::Tab => *field = (*field + 1) % 2,
                KeyCode::Enter if *field == 1 => {
                    let u = user.to_string().trim().to_string();
                    let s = server.to_string().trim().to_string();
                    if u.is_empty() || s.is_empty() {
                        return Ok(None);
                    }
                    stack.pop();
                    return Ok(Some(UiCommand::Login(LoginArgs {
                        server_url: s,
                        user_id: u,
                    })));
                }
                _ => {
                    if let Some(req) = tui_input::backend::crossterm::to_input_request(evt) {
                        match *field {
                            0 => {
                                user.handle(req);
                            }
                            1 => {
                                server.handle(req);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(None)
        }
        RouteFrame::BuildConfirm => match key.code {
            KeyCode::Enter => {
                stack.pop();
                Ok(Some(UiCommand::Build(BuildArgs {
                    project_dir: None,
                    out: None,
                })))
            }
            _ => Ok(None),
        },
        RouteFrame::DeployMode { list } => match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let i = list.selected().unwrap_or(0).saturating_sub(1);
                list.select(Some(i));
                Ok(None)
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                let i = (list.selected().unwrap_or(0) + 1).min(1);
                list.select(Some(i));
                Ok(None)
            }
            KeyCode::Enter => {
                let i = list.selected().unwrap_or(0);
                let (draft_only, auto_publish) = if i == 0 {
                    (true, false)
                } else {
                    (false, true)
                };
                stack.pop();
                stack.push(RouteFrame::DeployServer {
                    server: tui_input::Input::new(DEFAULT_GRAPHQL_URL.to_string()),
                    draft_only,
                    auto_publish,
                });
                Ok(None)
            }
            _ => Ok(None),
        },
        RouteFrame::DeployServer {
            server,
            draft_only,
            auto_publish,
        } => {
            if let Some(req) = tui_input::backend::crossterm::to_input_request(evt) {
                server.handle(req);
            }
            if key.code == KeyCode::Enter {
                let s = server.to_string().trim().to_string();
                if s.is_empty() {
                    return Ok(None);
                }
                let dp = *draft_only;
                let ap = *auto_publish;
                stack.pop();
                return Ok(Some(UiCommand::Deploy(DeployArgs {
                    project_dir: None,
                    server_url: s,
                    auto_publish: ap,
                    draft_only: dp,
                })));
            }
            Ok(None)
        }
        RouteFrame::DraftsMode { list } => match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let i = list.selected().unwrap_or(0).saturating_sub(1);
                list.select(Some(i));
                Ok(None)
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                let i = (list.selected().unwrap_or(0) + 1).min(3);
                list.select(Some(i));
                Ok(None)
            }
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
        },
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
                    return Ok(Some(UiCommand::Drafts(DraftsArgs {
                        command: DraftsSubcommands::List,
                        server_url: s,
                    })));
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
                return Ok(Some(UiCommand::Drafts(DraftsArgs {
                    command: cmd,
                    server_url: srv,
                })));
            }
            Ok(None)
        }
        RouteFrame::ManifestMode { list } => match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                list.select(Some(list.selected().unwrap_or(0).saturating_sub(1)));
                Ok(None)
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                let i = (list.selected().unwrap_or(0) + 1).min(1);
                list.select(Some(i));
                Ok(None)
            }
            KeyCode::Enter => {
                let next = if list.selected() == Some(0) {
                    ManifestNext::Show
                } else {
                    ManifestNext::Edit
                };
                stack.pop();
                stack.push(RouteFrame::ManifestServer {
                    server: tui_input::Input::new(DEFAULT_GRAPHQL_URL.to_string()),
                    next,
                });
                Ok(None)
            }
            _ => Ok(None),
        },
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
                        return Ok(Some(UiCommand::Manifest(ManifestArgs {
                            command: ManifestSubcommands::Show { draft_id: id },
                            server_url: srv,
                        })));
                    }
                    ManifestNext::Edit => {
                        stack.push(RouteFrame::ManifestEditFields {
                            server: srv,
                            draft_id: id,
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
                    let srv = server.clone();
                    let did = draft_id.clone();
                    stack.pop();
                    return Ok(Some(UiCommand::Manifest(ManifestArgs {
                        command: ManifestSubcommands::Edit {
                            draft_id: did,
                            name: n,
                            display_name: d,
                            version: v,
                            description: desc,
                        },
                        server_url: srv,
                    })));
                }
                _ => {}
            }
            Ok(None)
        }
        RouteFrame::TestConfirm => match key.code {
            KeyCode::Enter => {
                stack.pop();
                Ok(Some(UiCommand::Test(TestArgs {
                    project_dir: None,
                })))
            }
            _ => Ok(None),
        },
    }
}
