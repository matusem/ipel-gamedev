//! Multi-step init project wizard (runs inside the shared terminal session).

use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::cli::{BackendKind, FrontendKind, InitArgs, JsTemplate};

pub enum InitWizardOutcome {
    Submit(InitArgs),
    Cancel,
}

#[derive(Debug)]
enum Phase {
    Form,
    Confirm { summary: String, list: ListState },
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

    fn backends() -> [&'static str; 4] {
        [
            "rust",
            "java (not supported yet)",
            "csharp (not supported yet)",
            "cpp (not supported yet)",
        ]
    }

    fn backends_disabled() -> [bool; 4] {
        [false, true, true, true]
    }

    fn frontends() -> [&'static str; 4] {
        ["js", "ts", "bevy", "dioxus_rust"]
    }

    fn templates() -> [&'static str; 3] {
        ["vanilla_vite", "plain_static", "react_vite"]
    }

    fn show_js_template(&self) -> bool {
        self.frontend_idx == 0
    }

    fn buttons_focus(&self) -> usize {
        if self.show_js_template() {
            4
        } else {
            3
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        match &mut self.phase {
            Phase::Form => self.draw_form(frame, area),
            Phase::Confirm { summary, list } => {
                let items = vec![ListItem::new("confirm"), ListItem::new("cancel")];
                let list_w = List::new(items)
                    .block(Block::new().title(summary.as_str()).borders(Borders::ALL))
                    .highlight_symbol(">> ");
                frame.render_stateful_widget(list_w, area, list);
            }
        }
    }

    fn draw_form(&self, frame: &mut Frame, area: Rect) {
        let backends = Self::backends();
        let backends_disabled = Self::backends_disabled();
        let frontends = Self::frontends();
        let templates = Self::templates();
        let show_js = self.show_js_template();
        let buttons_focus = self.buttons_focus();

        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled(
                    if self.focus == 0 { ">> " } else { "   " },
                    Style::default()
                        .fg(if self.focus == 0 {
                            Color::Yellow
                        } else {
                            Color::DarkGray
                        })
                        .add_modifier(if self.focus == 0 {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::raw(format!("Project name: {}", self.project_name)),
            ]),
            Line::from(vec![
                Span::styled(
                    if self.focus == 1 { ">> " } else { "   " },
                    Style::default()
                        .fg(if self.focus == 1 {
                            Color::Yellow
                        } else {
                            Color::DarkGray
                        })
                        .add_modifier(if self.focus == 1 {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::raw("Backend: "),
                self.backend_span(0, &backends, &backends_disabled),
                Span::styled(" | ", Style::default().fg(Color::LightRed)),
                self.backend_span(1, &backends, &backends_disabled),
                Span::styled(" | ", Style::default().fg(Color::LightRed)),
                self.backend_span(2, &backends, &backends_disabled),
                Span::styled(" | ", Style::default().fg(Color::LightRed)),
                self.backend_span(3, &backends, &backends_disabled),
            ]),
            Line::from(vec![
                Span::styled(
                    if self.focus == 2 { ">> " } else { "   " },
                    Style::default()
                        .fg(if self.focus == 2 {
                            Color::Yellow
                        } else {
                            Color::DarkGray
                        })
                        .add_modifier(if self.focus == 2 {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::raw("Frontend: "),
                self.frontend_span(0, &frontends),
                Span::styled(" | ", Style::default().fg(Color::LightRed)),
                self.frontend_span(1, &frontends),
                Span::styled(" | ", Style::default().fg(Color::LightRed)),
                self.frontend_span(2, &frontends),
                Span::styled(" | ", Style::default().fg(Color::LightRed)),
                self.frontend_span(3, &frontends),
            ]),
        ];

        if show_js {
            lines.push(Line::from(vec![
                Span::styled(
                    if self.focus == 3 { ">> " } else { "   " },
                    Style::default()
                        .fg(if self.focus == 3 {
                            Color::Yellow
                        } else {
                            Color::DarkGray
                        })
                        .add_modifier(if self.focus == 3 {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::raw("JS template: "),
                self.template_span(0, &templates),
                Span::styled(" | ", Style::default().fg(Color::LightRed)),
                self.template_span(1, &templates),
                Span::styled(" | ", Style::default().fg(Color::LightRed)),
                self.template_span(2, &templates),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                if self.focus == buttons_focus { ">> " } else { "   " },
                Style::default()
                    .fg(if self.focus == buttons_focus {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    })
                    .add_modifier(if self.focus == buttons_focus {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            if self.focus == buttons_focus && self.button_confirm {
                Span::styled("[X]", Style::default().fg(Color::Yellow))
            } else {
                Span::styled("[ ]", Style::default().fg(Color::DarkGray))
            },
            Span::raw(" Confirm   "),
            if self.focus == buttons_focus && !self.button_confirm {
                Span::styled("[X]", Style::default().fg(Color::Yellow))
            } else {
                Span::styled("[ ]", Style::default().fg(Color::DarkGray))
            },
            Span::raw(" Cancel"),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(
            "Tab/Shift+Tab: move · Left/Right: change · Enter: activate row",
        ));

        let block = Block::new().title("Init project").borders(Borders::ALL);
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn backend_span<'a>(
        &'a self,
        idx: usize,
        backends: &[&'a str; 4],
        disabled: &[bool; 4],
    ) -> Span<'a> {
        let style = if self.backend_idx == idx {
            if disabled[idx] {
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)
            } else {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            }
        } else {
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)
        };
        Span::styled(backends[idx], style)
    }

    fn frontend_span<'a>(&'a self, idx: usize, frontends: &[&'a str; 4]) -> Span<'a> {
        let style = if self.frontend_idx == idx {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)
        };
        Span::styled(frontends[idx], style)
    }

    fn template_span<'a>(&'a self, idx: usize, templates: &[&'a str; 3]) -> Span<'a> {
        let style = if self.template_idx == idx {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)
        };
        Span::styled(templates[idx], style)
    }

    fn next_enabled(disabled: &[bool; 4], current: usize) -> usize {
        let len = disabled.len();
        for step in 1..=len {
            let i = (current + step) % len;
            if !disabled[i] {
                return i;
            }
        }
        current
    }

    fn prev_enabled(disabled: &[bool; 4], current: usize) -> usize {
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
                match key.code {
                    Up | Char('k') => {
                        let i = list.selected().unwrap_or(0).saturating_sub(1);
                        list.select(Some(i));
                    }
                    Down | Char('j') | Tab => {
                        let i = (list.selected().unwrap_or(0) + 1).min(1);
                        list.select(Some(i));
                    }
                    Enter => {
                        if list.selected() == Some(0) {
                            return self.build_init_args();
                        }
                        return Some(InitWizardOutcome::Cancel);
                    }
                    Esc => return Some(InitWizardOutcome::Cancel),
                    _ => {}
                }
                None
            }
            Phase::Form => self.handle_form_key(key),
        }
    }

    fn build_init_args(&self) -> Option<InitWizardOutcome> {
        let backend = match self.backend_idx {
            0 => Some(BackendKind::Rust),
            1 => Some(BackendKind::Java),
            2 => Some(BackendKind::Csharp),
            _ => Some(BackendKind::Cpp),
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
                1 => JsTemplate::PlainStatic,
                _ => JsTemplate::ReactVite,
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
                3 if show_js => self.template_idx = (self.template_idx + 2) % 3,
                x if x == buttons_focus => self.button_confirm = true,
                _ => {}
            },
            Right => match self.focus {
                1 => {
                    self.backend_idx =
                        Self::next_enabled(&Self::backends_disabled(), self.backend_idx)
                }
                2 => self.frontend_idx = (self.frontend_idx + 1) % 4,
                3 if show_js => self.template_idx = (self.template_idx + 1) % 3,
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
                    let backends = Self::backends();
                    let frontends = Self::frontends();
                    let templates = Self::templates();
                    let summary = format!(
                        "Confirm init: name={}, backend={}, frontend={}, template={}",
                        self.project_name.trim(),
                        backends[self.backend_idx],
                        frontends[self.frontend_idx],
                        if self.frontend_idx <= 1 {
                            templates[self.template_idx]
                        } else {
                            "n/a"
                        }
                    );
                    self.phase = Phase::Confirm {
                        summary,
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
