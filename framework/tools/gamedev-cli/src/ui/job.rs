//! Background command runner + log viewer for the TUI.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph};

use crate::reporter::{self, TuiLogScope};
use crate::theme;

use super::UiCommand;
use super::exec;

const MAX_LOG_LINES: usize = 4_000;

pub struct JobRun {
    pub title: String,
    logs: Arc<Mutex<Vec<String>>>,
    done: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Result<()>>>>,
    handle: Option<JoinHandle<()>>,
    pub scroll: usize,
    pub auto_scroll: bool,
    pub spinner_frame: usize,
    /// Visible log lines (inner height of the log panel), updated each frame.
    log_viewport: usize,
    /// Wrapped display line count, updated each frame.
    display_lines: usize,
}

impl JobRun {
    pub fn start(cmd: UiCommand) -> Self {
        let title = exec::command_title(&cmd).to_string();
        let logs = Arc::new(Mutex::new(vec![format!(">> running {title}...")]));
        let done = Arc::new(AtomicBool::new(false));
        let result = Arc::new(Mutex::new(None));

        let logs_t = logs.clone();
        let done_t = done.clone();
        let result_t = result.clone();

        let title_for_thread = title.clone();
        let handle = std::thread::spawn(move || {
            let _scope = TuiLogScope::attach(logs_t.clone());
            let out = exec::dispatch(cmd);
            if let Ok(()) = &out {
                reporter::status("done", &format!("{title_for_thread} finished"));
            } else if let Err(e) = &out {
                reporter::error("failed", &e.to_string());
            }
            *result_t.lock().expect("job result lock") = Some(out);
            done_t.store(true, Ordering::SeqCst);
        });

        Self {
            title,
            logs,
            done,
            result,
            handle: Some(handle),
            scroll: 0,
            auto_scroll: true,
            spinner_frame: 0,
            log_viewport: 1,
            display_lines: 0,
        }
    }

    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }

    pub fn succeeded(&self) -> bool {
        self.result
            .lock()
            .expect("job result lock")
            .as_ref()
            .is_some_and(|r| r.is_ok())
    }

    pub fn tick(&mut self) {
        if !self.is_done() {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
        }
        if self.is_done() {
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
        }
    }

    fn log_lines(&self) -> Vec<String> {
        let mut lines = self.logs.lock().expect("job logs lock").clone();
        if lines.len() > MAX_LOG_LINES {
            lines.drain(0..lines.len() - MAX_LOG_LINES);
        }
        lines
    }

    fn output_block(title: &str) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(theme::tui_header_border())
            .title(title.to_string())
            .title_style(
                Style::default()
                    .fg(theme::tui_accent())
                    .add_modifier(Modifier::BOLD),
            )
            .padding(Padding::new(1, 1, 0, 0))
    }

    fn log_block() -> Block<'static> {
        theme::panel_block(" Log ")
    }

    fn wrap_log_line(line: &str, width: usize) -> Vec<String> {
        let width = width.max(8);
        if line.is_empty() {
            return vec![String::new()];
        }
        let mut out = Vec::new();
        let mut rest = line.to_string();
        while !rest.is_empty() {
            if rest.chars().count() <= width {
                out.push(rest);
                break;
            }
            let chunk: String = rest.chars().take(width).collect();
            if let Some(sp) = chunk.rfind(' ') {
                out.push(rest.chars().take(sp).collect());
                rest = rest.chars().skip(sp + 1).collect();
            } else {
                out.push(chunk);
                rest = rest.chars().skip(width).collect();
            }
        }
        out
    }

    fn display_log_lines(raw: &[String], width: usize) -> Vec<Line<'static>> {
        raw.iter()
            .flat_map(|l| Self::wrap_log_line(l, width))
            .map(Line::from)
            .collect()
    }

    fn scroll_for_bottom(total_lines: usize, viewport: usize) -> usize {
        total_lines.saturating_sub(viewport.max(1))
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(0)])
            .split(area);

        let status = if !self.is_done() {
            let ticks = theme::spinner_ticks();
            let tick = ticks[self.spinner_frame % ticks.len()];
            format!("{tick} running...")
        } else if self.succeeded() {
            format!("{} done", theme::glyph_ok())
        } else {
            format!("{} failed", theme::glyph_fail())
        };

        let header = vec![
            Line::from(vec![
                Span::styled(
                    &self.title,
                    Style::default()
                        .fg(theme::tui_accent())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    status,
                    if self.is_done() && !self.succeeded() {
                        theme::status_style(false)
                    } else if self.is_done() {
                        theme::status_style(true)
                    } else {
                        Style::default().fg(theme::tui_accent())
                    },
                ),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(header).block(Self::output_block(" Output ")),
            chunks[0],
        );

        let log_block = Self::log_block();
        let log_inner = log_block.inner(chunks[1]);
        self.log_viewport = log_inner.height.max(1) as usize;

        let log_text = self.log_lines();
        let lines = Self::display_log_lines(&log_text, log_inner.width as usize);
        let total = lines.len();
        self.display_lines = total;
        if self.auto_scroll {
            self.scroll = Self::scroll_for_bottom(total, self.log_viewport);
        }
        let scroll = self.scroll.min(Self::scroll_for_bottom(total, self.log_viewport));

        let log = Paragraph::new(lines)
            .block(log_block)
            .scroll((scroll as u16, 0));
        frame.render_widget(log, chunks[1]);
    }

    pub fn handle_key(&mut self, key: KeyCode) -> JobKeyAction {
        if self.is_done() {
            match key {
                KeyCode::Enter | KeyCode::Esc => return JobKeyAction::Dismiss,
                KeyCode::Up | KeyCode::Char('k') => {
                    self.auto_scroll = false;
                    self.scroll = self.scroll.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.auto_scroll = false;
                    let max =
                        Self::scroll_for_bottom(self.display_lines, self.log_viewport);
                    self.scroll = (self.scroll + 1).min(max);
                }
                _ => {}
            }
        }
        JobKeyAction::None
    }
}

pub enum JobKeyAction {
    None,
    Dismiss,
}
