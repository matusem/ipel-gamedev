//! Multi-step init project wizard (runs inside the shared terminal session).

use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use crate::cli::{BackendKind, FrontendKind, InitArgs, JsTemplate};
use crate::theme;

pub enum InitWizardOutcome {
    Submit(InitArgs),
    Cancel,
}

#[derive(Debug)]
enum Phase {
    Form,
    Confirm { summary: InitSummary, list: ListState },
}

#[derive(Clone, Debug)]
struct InitSummary {
    name: String,
    backend: String,
    frontend: String,
    template: String,
}

pub struct InitWizardState {
    phase: Phase,
    project_name: String,
    backend_idx: usize,
    frontend_idx: usize,
    template_idx: usize,
    focus: usize,
    button_confirm: bool,
}

impl InitWizardState {
    pub fn new() -> Self {
        Self {
            phase: Phase::Form,
            project_name: "my_game".to_string(),
            backend_idx: 0,
            frontend_idx: 0,
            template_idx: 0,
            focus: 0,
            button_confirm: true,
        }
    }

    fn backends() -> [&'static str; 2] {
        ["rust", "java"]
    }

    fn backends_disabled() -> [bool; 2] {
        [false, false]
    }

    fn frontends() -> [&'static str; 4] {
        ["js", "ts", "bevy", "dioxus_rust"]
    }

    fn templates() -> [&'static str; 2] {
        ["vanilla_vite", "plain_static"]
    }

    fn show_js_template(&self) -> bool {
        self.frontend_idx == 0
    }

    fn buttons_focus(&self) -> usize {
        if self.show_js_template() { 4 } else { 3 }
    }

    fn summary(&self) -> InitSummary {
        let backends = Self::backends();
        let frontends = Self::frontends();
        let templates = Self::templates();
        InitSummary {
            name: self.project_name.trim().to_string(),
            backend: backends[self.backend_idx].to_string(),
            frontend: frontends[self.frontend_idx].to_string(),
            template: if self.frontend_idx == 0 {
                templates[self.template_idx].to_string()
            } else {
                "n/a".to_string()
            },
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        match &mut self.phase {
            Phase::Form => self.draw_form(frame, area),
            Phase::Confirm { summary, list } => {
                let summary = summary.clone();
                Self::draw_confirm_static(frame, area, &summary, list);
            }
        }
    }

    fn draw_form(&self, frame: &mut Frame, area: Rect) {
        let show_js = self.show_js_template();
        let buttons_focus = self.buttons_focus();

        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Min(8),
                Constraint::Length(6),
            ])
            .split(area);

        let intro = vec![
            Line::from(Span::styled(
                "Scaffold a new game project",
                Style::default()
                    .fg(theme::tui_accent())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("Choose stack and name, then confirm to run `gamedev init`."),
        ];
        frame.render_widget(
            Paragraph::new(intro).block(theme::panel_block(" Init project ")),
            outer[0],
        );

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(outer[1]);

        self.draw_fields_column(frame, body[0], show_js, buttons_focus);
        self.draw_help_column(frame, body[1], show_js, buttons_focus);

        let preview = self.preview_lines();
        frame.render_widget(
            Paragraph::new(preview).block(theme::card_block(" Preview ")),
            outer[2],
        );
    }

    fn draw_fields_column(&self, frame: &mut Frame, area: Rect, show_js: bool, buttons_focus: usize) {
        let mut constraints = vec![
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
        ];
        if show_js {
            constraints.push(Constraint::Length(5));
        }
        constraints.push(Constraint::Min(5));

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let backends = Self::backends();
        let frontends = Self::frontends();
        let templates = Self::templates();

        let mut row = 0;
        self.draw_field_panel(
            frame,
            rows[row],
            " Project name ",
            self.focus == 0,
            vec![Line::from(Span::styled(
                self.project_name.clone(),
                self.value_style(self.focus == 0),
            ))],
        );
        row += 1;

        self.draw_field_panel(
            frame,
            rows[row],
            " Backend ",
            self.focus == 1,
            vec![self.options_line(
                &backends,
                self.backend_idx,
                &Self::backends_disabled(),
                self.focus == 1,
            )],
        );
        row += 1;

        self.draw_field_panel(
            frame,
            rows[row],
            " Frontend ",
            self.focus == 2,
            vec![self.options_line(&frontends, self.frontend_idx, &[false; 4], self.focus == 2)],
        );
        row += 1;

        if show_js {
            self.draw_field_panel(
                frame,
                rows[row],
                " JS template ",
                self.focus == 3,
                vec![self.options_line(&templates, self.template_idx, &[false; 2], self.focus == 3)],
            );
            row += 1;
        }

        let action_lines = vec![
            Line::from(vec![
                self.toggle_span(self.focus == buttons_focus && self.button_confirm, "Confirm"),
                Span::raw("  "),
                self.toggle_span(self.focus == buttons_focus && !self.button_confirm, "Cancel"),
            ]),
            Line::from(Span::styled(
                "ret on Actions to continue",
                theme::tui_footer(),
            )),
        ];
        self.draw_field_panel(
            frame,
            rows[row],
            " Actions ",
            self.focus == buttons_focus,
            action_lines,
        );
    }

    fn draw_help_column(&self, frame: &mut Frame, area: Rect, show_js: bool, buttons_focus: usize) {
        let mut lines = self.field_help(self.focus, show_js, buttons_focus);
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("tab", theme::tui_keycap()),
            Span::raw(" next field"),
            Span::styled(theme::ui_sep(), theme::tui_footer()),
            Span::styled(theme::key_cycle(), theme::tui_keycap()),
            Span::raw(" change option"),
        ]));
        frame.render_widget(
            Paragraph::new(lines).block(theme::card_block(" Help ")),
            area,
        );
    }

    fn draw_confirm_static(
        frame: &mut Frame,
        area: Rect,
        summary: &InitSummary,
        list: &mut ListState,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8), Constraint::Length(8)])
            .split(area);

        let review = vec![
            Line::from(vec![
                Span::raw("name: "),
                Span::styled(&summary.name, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(format!(
                "backend: {} {} frontend: {}",
                summary.backend,
                theme::ui_arrow(),
                summary.frontend
            )),
            Line::from(format!("js template: {}", summary.template)),
            Line::from(""),
            Line::from("Creates gamedev.toml, backend, and client scaffold."),
        ];
        frame.render_widget(
            Paragraph::new(review).block(theme::panel_block(" Review ")),
            chunks[0],
        );

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[1]);

        let items = vec![
            ListItem::new("*  run init"),
            ListItem::new("!  go back"),
        ];
        let list_w = List::new(items)
            .block(theme::card_block(" Next "))
            .highlight_symbol(theme::tui_list_marker())
            .highlight_style(theme::tui_highlight());
        frame.render_stateful_widget(list_w, body[0], list);

        let hint = vec![
            Line::from(Span::styled(
                "Run init in the current directory tree.",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("command: gamedev init {}", summary.name)),
            Line::from(""),
            Line::from(vec![
                Span::styled(theme::key_cycle(), theme::tui_keycap()),
                Span::raw(" select"),
                Span::styled(theme::ui_sep(), theme::tui_footer()),
                Span::styled(theme::key_enter(), theme::tui_keycap()),
                Span::raw(" confirm"),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(hint).block(theme::card_block(" Details ")),
            body[1],
        );
    }

    fn draw_field_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        focused: bool,
        lines: Vec<Line<'static>>,
    ) {
        let block = if focused {
            theme::focused_field_block(title)
        } else {
            theme::field_block(title)
        };
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn value_style(&self, focused: bool) -> Style {
        if focused {
            Style::default()
                .fg(theme::tui_accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        }
    }

    fn options_line(
        &self,
        options: &[&str],
        selected: usize,
        disabled: &[bool],
        focused: bool,
    ) -> Line<'static> {
        if !focused {
            let opt = options.get(selected).copied().unwrap_or("?");
            let dimmed = disabled.get(selected).copied().unwrap_or(false);
            let style = if dimmed {
                theme::tui_disabled()
            } else {
                Style::default().add_modifier(Modifier::BOLD)
            };
            return Line::from(Span::styled(format!("[{opt}]"), style));
        }

        let mut spans = Vec::new();
        for (i, opt) in options.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(theme::ui_sep(), theme::tui_footer()));
            }
            let dimmed = disabled.get(i).copied().unwrap_or(false);
            let style = if dimmed {
                theme::tui_disabled()
            } else if i == selected {
                if focused {
                    Style::default()
                        .fg(theme::tui_accent())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().add_modifier(Modifier::BOLD)
                }
            } else {
                theme::tui_disabled()
            };
            spans.push(Span::styled(format!("[{opt}]"), style));
        }
        Line::from(spans)
    }

    fn toggle_span(&self, on: bool, label: &str) -> Span<'static> {
        let mark = if on { "[x]" } else { "[ ]" };
        let style = if on {
            Style::default()
                .fg(theme::tui_accent())
                .add_modifier(Modifier::BOLD)
        } else {
            theme::tui_disabled()
        };
        Span::styled(format!("{mark} {label}"), style)
    }

    fn preview_lines(&self) -> Vec<Line<'static>> {
        let s = self.summary();
        let name = s.name.clone();
        let backend = s.backend.clone();
        let frontend = s.frontend.clone();
        let template_suffix = if s.template == "n/a" {
            String::new()
        } else {
            format!("  ({})", s.template)
        };
        vec![
            Line::from(vec![
                Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(backend, Style::default().fg(theme::tui_accent())),
                Span::raw(format!(" {} ", theme::ui_arrow())),
                Span::styled(frontend, Style::default().fg(theme::tui_accent())),
                Span::raw(template_suffix),
            ]),
            Line::from(Span::styled(
                "gamedev init scaffold",
                theme::tui_footer(),
            )),
        ]
    }

    fn field_help(&self, focus: usize, show_js: bool, buttons_focus: usize) -> Vec<Line<'static>> {
        if focus == buttons_focus {
            return vec![
                Line::from(Span::styled(
                    "Confirm or cancel",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from("Confirm opens the review step."),
                Line::from("Cancel returns to the home menu."),
                Line::from(""),
                Line::from("Use left/right to pick Confirm or Cancel."),
            ];
        }
        match focus {
            0 => vec![
                Line::from(Span::styled(
                    "Project name",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from("Folder name for the new game."),
                Line::from("Use letters, numbers, and underscores."),
            ],
            1 => vec![
                Line::from(Span::styled(
                    "Backend",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from("Rust compiles to WASM (recommended)."),
                Line::from("Java uses Gradle backend layout."),
            ],
            2 => vec![
                Line::from(Span::styled(
                    "Frontend",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from("js/ts: static or Vite client."),
                Line::from("bevy/dioxus: Rust WASM client."),
            ],
            3 if show_js => vec![
                Line::from(Span::styled(
                    "JS template",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from("vanilla_vite: Vite + npm toolchain."),
                Line::from("plain_static: HTML/JS without bundler."),
            ],
            _ => vec![Line::from("Select a field on the left.")],
        }
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

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<InitWizardOutcome> {
        match &mut self.phase {
            Phase::Confirm { list, .. } => {
                use crossterm::event::KeyCode::*;
                if super::nav::is_list_prev(key.code) {
                    super::nav::cycle_list(list, -1, 2);
                } else if super::nav::is_list_next(key.code) {
                    super::nav::cycle_list(list, 1, 2);
                } else if matches!(key.code, Enter) {
                    if list.selected() == Some(0) {
                        return self.build_init_args();
                    }
                    self.phase = Phase::Form;
                } else if matches!(key.code, Esc) {
                    self.phase = Phase::Form;
                }
                None
            }
            Phase::Form => self.handle_form_key(key),
        }
    }

    fn build_init_args(&self) -> Option<InitWizardOutcome> {
        let backend = match self.backend_idx {
            0 => Some(BackendKind::Rust),
            _ => Some(BackendKind::Java),
        };
        let frontend = match self.frontend_idx {
            0 => Some(FrontendKind::Js),
            1 => Some(FrontendKind::Ts),
            2 => Some(FrontendKind::Bevy),
            _ => Some(FrontendKind::Dioxus),
        };
        let js_template = if self.frontend_idx == 0 {
            Some(match self.template_idx {
                0 => JsTemplate::VanillaVite,
                _ => JsTemplate::PlainStatic,
            })
        } else {
            None
        };
        let name_trim = self.project_name.trim();
        if name_trim.is_empty() {
            return None;
        }
        Some(InitWizardOutcome::Submit(InitArgs {
            name: Some(name_trim.to_string()),
            bot: false,
            game: None,
            game_version: None,
            server_url: crate::defaults::DEFAULT_GRAPHQL_URL.to_string(),
            backend,
            frontend,
            js_template,
        }))
    }

    fn handle_form_key(&mut self, key: KeyEvent) -> Option<InitWizardOutcome> {
        let show_js = self.show_js_template();
        let buttons_focus = self.buttons_focus();
        if !show_js && self.focus == 3 {
            self.focus = buttons_focus;
        }

        use crossterm::event::KeyCode::*;
        match key.code {
            Esc => return Some(InitWizardOutcome::Cancel),
            Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(InitWizardOutcome::Cancel);
            }
            Tab | Down => {
                self.focus = match self.focus {
                    0 => 1,
                    1 => 2,
                    2 => {
                        if show_js {
                            3
                        } else {
                            buttons_focus
                        }
                    }
                    3 => buttons_focus,
                    _ => 0,
                };
            }
            BackTab | Up => {
                self.focus = match self.focus {
                    0 => buttons_focus,
                    1 => 0,
                    2 => 1,
                    3 => 2,
                    _ => {
                        if show_js {
                            3
                        } else {
                            2
                        }
                    }
                };
            }
            Left => match self.focus {
                1 => {
                    self.backend_idx =
                        Self::prev_enabled(&Self::backends_disabled(), self.backend_idx)
                }
                2 => self.frontend_idx = (self.frontend_idx + 3) % 4,
                3 if show_js => self.template_idx = (self.template_idx + 1) % 2,
                x if x == buttons_focus => self.button_confirm = true,
                _ => {}
            },
            Right => match self.focus {
                1 => {
                    self.backend_idx =
                        Self::next_enabled(&Self::backends_disabled(), self.backend_idx)
                }
                2 => self.frontend_idx = (self.frontend_idx + 1) % 4,
                3 if show_js => self.template_idx = (self.template_idx + 1) % 2,
                x if x == buttons_focus => self.button_confirm = false,
                _ => {}
            },
            Backspace if self.focus == 0 => {
                self.project_name.pop();
            }
            Char(ch) if self.focus == 0 => {
                self.project_name.push(ch);
            }
            Enter if self.focus == buttons_focus => {
                if self.button_confirm {
                    if self.project_name.trim().is_empty() {
                        return None;
                    }
                    self.phase = Phase::Confirm {
                        summary: self.summary(),
                        list: ListState::default().with_selected(Some(0)),
                    };
                } else {
                    return Some(InitWizardOutcome::Cancel);
                }
            }
            _ => {}
        }
        None
    }
}

impl Default for InitWizardState {
    fn default() -> Self {
        Self::new()
    }
}
