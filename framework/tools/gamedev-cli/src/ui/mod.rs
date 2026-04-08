use std::io;
use std::io::Write;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Constraint;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::{
    BackendKind, BuildArgs, DeployArgs, DraftsArgs, DraftsSubcommands, FrontendKind, InitArgs,
    JsTemplate, LoginArgs, ManifestArgs, ManifestSubcommands, TestArgs,
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
    ExitProgram,
}

pub fn run_once(auth_user: Option<String>) -> Result<UiCommand> {
    let user_label = auth_user.unwrap_or_else(|| "not authenticated".to_string());
    let title = format!("Main menu (user: {user_label})");
    let action = match tui_select(
        &title,
        &[
            "init",
            "login",
            "build",
            "deploy",
            "drafts",
            "manifest",
            "test",
            "exit program",
        ],
    ) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("TUI unavailable ({e}). Falling back to simple text mode.");
            return run_once_fallback();
        }
    };
    match action {
        0 => screen_init(),
        1 => screen_login(),
        2 => screen_build(),
        3 => screen_deploy(),
        4 => screen_drafts(),
        5 => screen_manifest(),
        6 => screen_test(),
        _ => Ok(UiCommand::ExitProgram),
    }
}

fn run_once_fallback() -> Result<UiCommand> {
    println!("Select command:");
    println!("1) init");
    println!("2) login");
    println!("3) build");
    println!("4) deploy");
    println!("5) drafts(list)");
    println!("6) manifest(show)");
    println!("7) test");
    println!("8) exit");
    print!("Choice: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let c = line.trim().parse::<usize>().unwrap_or(8);
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
            server_url: "http://localhost:8080/graphql".to_string(),
            user_id: String::new(),
        }),
        3 => UiCommand::Build(BuildArgs { project_dir: None, out: None }),
        4 => UiCommand::Deploy(DeployArgs {
            project_dir: None,
            server_url: "http://localhost:8080/graphql".to_string(),
            auto_publish: false,
            draft_only: true,
        }),
        5 => UiCommand::Drafts(DraftsArgs {
            command: DraftsSubcommands::List,
            server_url: "http://localhost:8080/graphql".to_string(),
        }),
        6 => UiCommand::Manifest(ManifestArgs {
            command: ManifestSubcommands::Show {
                draft_id: String::new(),
            },
            server_url: "http://localhost:8080/graphql".to_string(),
        }),
        7 => UiCommand::Test(TestArgs { project_dir: None }),
        _ => UiCommand::ExitProgram,
    };
    Ok(cmd)
}

fn screen_init() -> Result<UiCommand> {
    Ok(UiCommand::Init(run_init_creation_screen(None)?))
}

fn screen_login() -> Result<UiCommand> {
    let action = tui_select("Login screen", &["login", "exit to main menu"])?;
    match action {
        0 => {
            let Some(user_id) = tui_input("Login", "User id (UUID)", "")? else {
                return run_once(None);
            };
            let Some(server_url) = tui_input("Login", "Server URL", "http://localhost:8080/graphql")? else {
                return run_once(None);
            };
            Ok(UiCommand::Login(LoginArgs { server_url, user_id }))
        }
        _ => run_once(None),
    }
}

fn screen_build() -> Result<UiCommand> {
    let action = tui_select("Build screen", &["build current project", "exit to main menu"])?;
    match action {
        0 => Ok(UiCommand::Build(BuildArgs {
            project_dir: None,
            out: None,
        })),
        _ => run_once(None),
    }
}

fn screen_deploy() -> Result<UiCommand> {
    let action = tui_select(
        "Deploy screen",
        &["deploy draft only", "deploy and publish", "exit to main menu"],
    )?;
    match action {
        0 | 1 => {
            let Some(server_url) = tui_input("Deploy", "Server URL", "http://localhost:8080/graphql")? else {
                return run_once(None);
            };
            Ok(UiCommand::Deploy(DeployArgs {
                project_dir: None,
                server_url,
                auto_publish: action == 1,
                draft_only: action == 0,
            }))
        }
        _ => run_once(None),
    }
}

fn screen_drafts() -> Result<UiCommand> {
    let action = tui_select(
        "Drafts screen",
        &["list", "publish", "unpublish", "discard", "exit to main menu"],
    )?;
    match action {
        0..=3 => {
            let Some(server_url) = tui_input("Drafts", "Server URL", "http://localhost:8080/graphql")? else {
                return run_once(None);
            };
            let command = match action {
                0 => DraftsSubcommands::List,
                1 => DraftsSubcommands::Publish {
                    draft_id: tui_input_required("Drafts", "Draft id")?,
                },
                2 => DraftsSubcommands::Unpublish {
                    draft_id: tui_input_required("Drafts", "Draft id")?,
                },
                _ => DraftsSubcommands::Discard {
                    draft_id: tui_input_required("Drafts", "Draft id")?,
                },
            };
            Ok(UiCommand::Drafts(DraftsArgs {
                command,
                server_url,
            }))
        }
        _ => run_once(None),
    }
}

fn screen_manifest() -> Result<UiCommand> {
    let action = tui_select("Manifest screen", &["show", "edit", "exit to main menu"])?;
    match action {
        0 | 1 => {
            let Some(server_url) = tui_input("Manifest", "Server URL", "http://localhost:8080/graphql")? else {
                return run_once(None);
            };
            let command = if action == 0 {
                ManifestSubcommands::Show {
                    draft_id: tui_input_required("Manifest", "Draft id")?,
                }
            } else {
                ManifestSubcommands::Edit {
                    draft_id: tui_input_required("Manifest", "Draft id")?,
                    name: tui_input_required("Manifest", "Name")?,
                    display_name: tui_input_required("Manifest", "Display name")?,
                    version: tui_input_required("Manifest", "Version")?,
                    description: tui_input_required("Manifest", "Description")?,
                }
            };
            Ok(UiCommand::Manifest(ManifestArgs {
                command,
                server_url,
            }))
        }
        _ => run_once(None),
    }
}

fn screen_test() -> Result<UiCommand> {
    let action = tui_select("Test screen", &["run tests", "exit to main menu"])?;
    match action {
        0 => Ok(UiCommand::Test(TestArgs { project_dir: None })),
        _ => run_once(None),
    }
}

fn run_init_creation_screen(existing: Option<InitArgs>) -> Result<InitArgs> {
    install_ctrlc_handler();
    INTERRUPTED.store(false, Ordering::SeqCst);
    let _ = disable_raw_mode();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut project_name = existing
        .as_ref()
        .and_then(|x| x.name.clone())
        .unwrap_or_else(|| "my_game".to_string());
    let backends = [
        "rust",
        "java (not supported yet)",
        "csharp (not supported yet)",
        "cpp (not supported yet)",
    ];
    let backends_disabled = [false, true, true, true];
    let mut backend_idx = 0usize;
    let frontends = ["js", "ts", "bevy", "dioxus_rust"];
    let mut frontend_idx = 0usize;
    let templates = ["vanilla_vite", "plain_static", "react_vite"];
    let mut template_idx = 0usize;
    let mut focus = 0usize; // 0=name,1=backend,2=frontend,3=template(if js),4=buttons
    let mut button_confirm = true;

    loop {
        let show_js_template = frontend_idx == 0;
        let buttons_focus = if show_js_template { 4 } else { 3 };
        if !show_js_template && focus == 3 {
            focus = buttons_focus;
        }
        terminal.draw(|f| {
            let area = f.area();
            let block = Block::default().title("Init Project").borders(Borders::ALL);
            let mut lines = vec![
                Line::from(vec![
                    Span::styled(
                        if focus == 0 { ">> " } else { "   " },
                        Style::default()
                            .fg(if focus == 0 { Color::Yellow } else { Color::DarkGray })
                            .add_modifier(if focus == 0 { Modifier::BOLD } else { Modifier::empty() }),
                    ),
                    Span::raw(format!("Project name: {}", project_name)),
                ]),
                Line::from(vec![
                    Span::styled(
                        if focus == 1 { ">> " } else { "   " },
                        Style::default()
                            .fg(if focus == 1 { Color::Yellow } else { Color::DarkGray })
                            .add_modifier(if focus == 1 { Modifier::BOLD } else { Modifier::empty() }),
                    ),
                    Span::raw("Backend: "),
                    Span::styled(
                        backends[0],
                        if backend_idx == 0 {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        },
                    ),
                    Span::styled(" | ", Style::default().fg(Color::LightRed)),
                    Span::styled(
                        backends[1],
                        if backend_idx == 1 {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        },
                    ),
                    Span::styled(" | ", Style::default().fg(Color::LightRed)),
                    Span::styled(
                        backends[2],
                        if backend_idx == 2 {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        },
                    ),
                    Span::styled(" | ", Style::default().fg(Color::LightRed)),
                    Span::styled(
                        backends[3],
                        if backend_idx == 3 {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        },
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        if focus == 2 { ">> " } else { "   " },
                        Style::default()
                            .fg(if focus == 2 { Color::Yellow } else { Color::DarkGray })
                            .add_modifier(if focus == 2 { Modifier::BOLD } else { Modifier::empty() }),
                    ),
                    Span::raw("Frontend: "),
                    Span::styled(
                        frontends[0],
                        if frontend_idx == 0 {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        },
                    ),
                    Span::styled(" | ", Style::default().fg(Color::LightRed)),
                    Span::styled(
                        frontends[1],
                        if frontend_idx == 1 {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        },
                    ),
                    Span::styled(" | ", Style::default().fg(Color::LightRed)),
                    Span::styled(
                        frontends[2],
                        if frontend_idx == 2 {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        },
                    ),
                    Span::styled(" | ", Style::default().fg(Color::LightRed)),
                    Span::styled(
                        frontends[3],
                        if frontend_idx == 3 {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        },
                    ),
                ]),
            ];
            if show_js_template {
                lines.push(Line::from(vec![
                    Span::styled(
                        if focus == 3 { ">> " } else { "   " },
                        Style::default()
                            .fg(if focus == 3 { Color::Yellow } else { Color::DarkGray })
                            .add_modifier(if focus == 3 { Modifier::BOLD } else { Modifier::empty() }),
                    ),
                    Span::raw("JS template: "),
                    Span::styled(
                        templates[0],
                        if template_idx == 0 {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        },
                    ),
                    Span::styled(" | ", Style::default().fg(Color::LightRed)),
                    Span::styled(
                        templates[1],
                        if template_idx == 1 {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        },
                    ),
                    Span::styled(" | ", Style::default().fg(Color::LightRed)),
                    Span::styled(
                        templates[2],
                        if template_idx == 2 {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        },
                    ),
                ]));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                    Span::styled(
                        if focus == buttons_focus { ">> " } else { "   " },
                        Style::default()
                            .fg(if focus == buttons_focus { Color::Yellow } else { Color::DarkGray })
                            .add_modifier(if focus == buttons_focus { Modifier::BOLD } else { Modifier::empty() }),
                    ),
                    if focus == buttons_focus && button_confirm {
                        Span::styled("[X]", Style::default().fg(Color::Yellow))
                    } else {
                        Span::styled("[ ]", Style::default().fg(Color::DarkGray))
                    },
                    Span::raw(" Confirm   "),
                    if focus == buttons_focus && !button_confirm {
                        Span::styled("[X]", Style::default().fg(Color::Yellow))
                    } else {
                        Span::styled("[ ]", Style::default().fg(Color::DarkGray))
                    },
                    Span::raw(" Cancel"),
                ]));
            lines.push(Line::from(""));
            lines.push(Line::from("Use Tab/Shift+Tab to move, Left/Right to change options."));
            f.render_widget(Paragraph::new(lines).block(block), area);
        })?;

        if INTERRUPTED.load(Ordering::SeqCst) {
            cleanup_tui(&mut terminal)?;
            bail!("cancelled");
        }
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => {
                    cleanup_tui(&mut terminal)?;
                    bail!("cancelled");
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    cleanup_tui(&mut terminal)?;
                    bail!("cancelled");
                }
                KeyCode::Tab | KeyCode::Down => {
                    focus = match focus {
                        0 => 1,
                        1 => 2,
                        2 => {
                            if show_js_template { 3 } else { buttons_focus }
                        }
                        3 => buttons_focus,
                        _ => 0,
                    };
                }
                KeyCode::BackTab | KeyCode::Up => {
                    focus = match focus {
                        0 => buttons_focus,
                        1 => 0,
                        2 => 1,
                        3 => 2,
                        _ => {
                            if show_js_template { 3 } else { 2 }
                        }
                    };
                }
                KeyCode::Left => match focus {
                    1 => backend_idx = prev_enabled(&backends_disabled, backend_idx),
                    2 => frontend_idx = (frontend_idx + frontends.len() - 1) % frontends.len(),
                    3 if show_js_template => {
                        template_idx = (template_idx + templates.len() - 1) % templates.len()
                    }
                    x if x == buttons_focus => button_confirm = true,
                    _ => {}
                },
                KeyCode::Right => match focus {
                    1 => backend_idx = next_enabled(&backends_disabled, backend_idx),
                    2 => frontend_idx = (frontend_idx + 1) % frontends.len(),
                    3 if show_js_template => template_idx = (template_idx + 1) % templates.len(),
                    x if x == buttons_focus => button_confirm = false,
                    _ => {}
                },
                KeyCode::Backspace if focus == 0 => {
                    project_name.pop();
                }
                KeyCode::Char(ch) if focus == 0 => {
                    project_name.push(ch);
                }
                KeyCode::Enter => {
                    if focus == buttons_focus {
                        if button_confirm {
                            if project_name.trim().is_empty() {
                                continue;
                            }
                            let backend = match backend_idx {
                                0 => Some(BackendKind::Rust),
                                1 => Some(BackendKind::Java),
                                2 => Some(BackendKind::Csharp),
                                _ => Some(BackendKind::Cpp),
                            };
                            let frontend = match frontend_idx {
                                0 => Some(FrontendKind::Js),
                                1 => Some(FrontendKind::Ts),
                                2 => Some(FrontendKind::Bevy),
                                _ => Some(FrontendKind::Dioxus),
                            };
                            let js_template = if frontend_idx == 0 {
                                Some(match template_idx {
                                    0 => JsTemplate::VanillaVite,
                                    1 => JsTemplate::PlainStatic,
                                    _ => JsTemplate::ReactVite,
                                })
                            } else {
                                None
                            };

                            let confirm_title = format!(
                                "Confirm init: name={}, backend={}, frontend={}, template={}",
                                project_name.trim(),
                                backends[backend_idx],
                                frontends[frontend_idx],
                                if frontend_idx <= 1 { templates[template_idx] } else { "n/a" }
                            );
                            let do_confirm = tui_select(&confirm_title, &["confirm", "cancel"])? == 0;
                            if do_confirm {
                                cleanup_tui(&mut terminal)?;
                                return Ok(InitArgs {
                                    name: Some(project_name.trim().to_string()),
                                    backend,
                                    frontend,
                                    js_template,
                                });
                            }
                        } else {
                            cleanup_tui(&mut terminal)?;
                            bail!("cancelled");
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn tui_input_required(screen: &str, label: &str) -> Result<String> {
    let Some(value) = tui_input(screen, label, "")? else {
        bail!("cancelled");
    };
    if value.trim().is_empty() {
        bail!("{label} is required");
    }
    Ok(value)
}

fn tui_input(screen: &str, label: &str, default: &str) -> Result<Option<String>> {
    install_ctrlc_handler();
    INTERRUPTED.store(false, Ordering::SeqCst);
    let _ = disable_raw_mode();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut value = default.to_string();
    loop {
        terminal.draw(|f| {
            let area = f.area();
            let block = Block::default()
                .title(format!("{screen}: {label}"))
                .borders(Borders::ALL);
            let paragraph = Paragraph::new(value.as_str()).block(block);
            f.render_widget(paragraph, area);
        })?;
        if INTERRUPTED.load(Ordering::SeqCst) {
            cleanup_tui(&mut terminal)?;
            bail!("cancelled");
        }
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Enter => {
                    cleanup_tui(&mut terminal)?;
                    return Ok(Some(value.trim().to_string()));
                }
                KeyCode::Esc => {
                    cleanup_tui(&mut terminal)?;
                    return Ok(None);
                }
                KeyCode::Backspace => {
                    value.pop();
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    cleanup_tui(&mut terminal)?;
                    bail!("cancelled");
                }
                KeyCode::Char(ch) => value.push(ch),
                _ => {}
            }
        }
    }
}

fn tui_select(title: &str, options: &[&str]) -> Result<usize> {
    let disabled = vec![false; options.len()];
    tui_select_with_disabled(title, options, &disabled)
}

fn tui_select_with_disabled(title: &str, options: &[&str], disabled: &[bool]) -> Result<usize> {
    install_ctrlc_handler();
    INTERRUPTED.store(false, Ordering::SeqCst);
    if options.is_empty() {
        bail!("no options available");
    }
    if disabled.len() != options.len() {
        bail!("disabled options length mismatch");
    }
    if disabled.iter().all(|v| *v) {
        bail!("all options are disabled");
    }
    let _ = disable_raw_mode();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut state = ListState::default();
    state.select(Some(first_enabled(disabled).unwrap_or(0)));
    loop {
        terminal.draw(|f| {
            let area = f.area();
            let items: Vec<ListItem> = options
                .iter()
                .enumerate()
                .map(|(idx, o)| {
                    if disabled[idx] {
                        ListItem::new(*o).style(
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM),
                        )
                    } else {
                        ListItem::new(*o)
                    }
                })
                .collect();
            let list = List::new(items)
                .block(Block::default().title(title).borders(Borders::ALL))
                .highlight_symbol(">> ")
                .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);
            f.render_stateful_widget(list, area, &mut state);
            let _ = Constraint::Percentage(100);
        })?;
        if INTERRUPTED.load(Ordering::SeqCst) {
            cleanup_tui(&mut terminal)?;
            bail!("cancelled");
        }
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Up => {
                    let i = state.selected().unwrap_or(0);
                    state.select(Some(prev_enabled(disabled, i)));
                }
                KeyCode::Down => {
                    let i = state.selected().unwrap_or(0);
                    state.select(Some(next_enabled(disabled, i)));
                }
                KeyCode::Tab => {
                    let i = state.selected().unwrap_or(0);
                    state.select(Some(next_enabled(disabled, i)));
                }
                KeyCode::BackTab => {
                    let i = state.selected().unwrap_or(0);
                    state.select(Some(prev_enabled(disabled, i)));
                }
                KeyCode::Enter => {
                    if !disabled[state.selected().unwrap_or(0)] {
                        break;
                    }
                }
                KeyCode::Esc => {
                    cleanup_tui(&mut terminal)?;
                    bail!("cancelled");
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    cleanup_tui(&mut terminal)?;
                    bail!("cancelled");
                }
                _ => {}
            }
        }
    }
    let selected = state.selected().unwrap_or(0);
    cleanup_tui(&mut terminal)?;
    Ok(selected)
}

fn first_enabled(disabled: &[bool]) -> Option<usize> {
    disabled.iter().position(|v| !*v)
}


fn next_enabled(disabled: &[bool], current: usize) -> usize {
    let len = disabled.len();
    for step in 1..=len {
        let i = (current + step) % len;
        if !disabled[i] {
            return i;
        }
    }
    current
}

fn prev_enabled(disabled: &[bool], current: usize) -> usize {
    let len = disabled.len();
    for step in 1..=len {
        let i = (current + len - step) % len;
        if !disabled[i] {
            return i;
        }
    }
    current
}

fn cleanup_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
    Ok(())
}

fn install_ctrlc_handler() {
    let _ = CTRL_C_HANDLER.get_or_init(|| {
        let _ = ctrlc::set_handler(|| {
            INTERRUPTED.store(true, Ordering::SeqCst);
        });
    });
}
