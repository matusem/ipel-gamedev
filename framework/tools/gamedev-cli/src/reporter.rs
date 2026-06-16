//! Unified CLI output for status, diagnostics, and hints.

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use comfy_table::{Cell, Table};
use indicatif::{ProgressBar, ProgressStyle};

use crate::api::ValidationDiagnostic;
use crate::theme;

thread_local! {
    static TUI_LOG: RefCell<Option<Arc<Mutex<Vec<String>>>>> = const { RefCell::new(None) };
}

/// Captures reporter output into the TUI log buffer for the current thread.
pub struct TuiLogScope {
    _private: (),
}

impl TuiLogScope {
    pub fn attach(logs: Arc<Mutex<Vec<String>>>) -> Self {
        TUI_LOG.with(|cell| {
            *cell.borrow_mut() = Some(logs);
        });
        Self { _private: () }
    }
}

impl Drop for TuiLogScope {
    fn drop(&mut self) {
        TUI_LOG.with(|cell| {
            *cell.borrow_mut() = None;
        });
    }
}

fn tui_sink() -> Option<Arc<Mutex<Vec<String>>>> {
    TUI_LOG.with(|cell| cell.borrow().clone())
}

fn push_line(line: String) {
    if let Some(sink) = tui_sink() {
        let mut guard = sink.lock().expect("tui log lock");
        guard.push(line);
        return;
    }
    let _ = writeln!(std::io::stdout(), "{line}");
}

fn push_err(line: String) {
    if let Some(sink) = tui_sink() {
        let mut guard = sink.lock().expect("tui log lock");
        guard.push(line);
        return;
    }
    let _ = writeln!(std::io::stderr(), "{line}");
}

pub fn status(label: &str, detail: &str) {
    let tag = if tui_sink().is_some() {
        format!("{} OK", theme::glyph_ok())
    } else {
        format!("{} {}", theme::glyph_ok(), theme::paint_ok("OK"))
    };
    push_line(format!("{tag} {label}{}{detail}", theme::ui_dash()));
}

pub fn warn(label: &str, detail: &str) {
    let tag = if tui_sink().is_some() {
        format!("{} WARN", theme::glyph_warn())
    } else {
        format!("{} {}", theme::glyph_warn(), theme::paint_warn("WARN"))
    };
    push_err(format!("{tag} {label}{}{detail}", theme::ui_dash()));
}

pub fn error(label: &str, detail: &str) {
    let tag = if tui_sink().is_some() {
        format!("{} FAIL", theme::glyph_fail())
    } else {
        format!("{} {}", theme::glyph_fail(), theme::paint_fail("FAIL"))
    };
    push_err(format!("{tag} {label}{}{detail}", theme::ui_dash()));
}

pub fn hint(line: &str) {
    if tui_sink().is_some() {
        push_line(format!("hint: {line}"));
    } else {
        let prefix = theme::paint_muted("hint:");
        push_line(format!("{prefix} {line}"));
    }
}

pub fn section(title: &str) {
    push_line(String::new());
    if tui_sink().is_some() {
        push_line(format!("== {title} =="));
    } else {
        push_line(theme::paint_bold(&format!("== {title} ==")));
    }
}

pub fn command_hint(cmd: &str) {
    if tui_sink().is_some() {
        push_line(format!("  {cmd}"));
    } else {
        push_line(format!("  {}", theme::paint_accent(cmd)));
    }
}

pub fn print_validation_report(
    upload_id: &str,
    ok: bool,
    errors: i32,
    warnings: i32,
    infos: i32,
    diagnostics: &[ValidationDiagnostic],
) {
    section("Upload validation");
    if ok {
        status(
            "upload",
            &format!("{upload_id} errors={errors} warnings={warnings} infos={infos}"),
        );
    } else {
        error(
            "upload",
            &format!("{upload_id} errors={errors} warnings={warnings} infos={infos}"),
        );
    }
    for d in diagnostics {
        let line = format!("{}: {}", d.code, d.message);
        match d.severity.as_str() {
            "ERROR" | "FAIL" => error(&d.severity, &line),
            "WARN" | "WARNING" => warn(&d.severity, &line),
            _ => {
                let tag = if tui_sink().is_some() {
                    format!("{} INFO", theme::glyph_info())
                } else {
                    format!("{} {}", theme::glyph_info(), theme::paint_muted("INFO"))
                };
                push_line(format!("{tag} {line}"));
            }
        }
    }
}

pub fn print_doctor_summary(fails: u32, warns: u32) {
    push_line(String::new());
    if fails > 0 {
        error(
            "doctor",
            &format!(
                "{fails} failure(s), {warns} warning(s){dash}fix FAIL items before build/deploy",
                dash = theme::ui_dash()
            ),
        );
    } else if warns > 0 {
        warn(
            "doctor",
            &format!("all required checks passed; {warns} warning(s)"),
        );
    } else {
        status("doctor", "all checks passed");
    }
}

pub enum ReporterSpinner {
    Bar(ProgressBar),
    Tui,
}

/// Spinner for long-running steps; logs to TUI when a sink is active.
pub fn spinner(msg: &str) -> ReporterSpinner {
    if tui_sink().is_some() {
        push_line(format!("[..] {msg}"));
        ReporterSpinner::Tui
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_strings(theme::spinner_ticks()),
        );
        pb.enable_steady_tick(Duration::from_millis(80));
        pb.set_message(msg.to_string());
        ReporterSpinner::Bar(pb)
    }
}

pub trait SpinnerFinish {
    fn finish_ok(self, msg: &str);
    fn finish_fail(self, msg: &str);
}

impl SpinnerFinish for ReporterSpinner {
    fn finish_ok(self, msg: &str) {
        match self {
            ReporterSpinner::Bar(b) => {
                b.finish_and_clear();
                status("step", msg);
            }
            ReporterSpinner::Tui => status("step", msg),
        }
    }

    fn finish_fail(self, msg: &str) {
        match self {
            ReporterSpinner::Bar(b) => {
                b.finish_and_clear();
                error("step", msg);
            }
            ReporterSpinner::Tui => error("step", msg),
        }
    }
}

pub enum ReporterProgress {
    Bar(ProgressBar),
    Tui { label: String },
}

/// Byte progress bar for uploads/downloads.
pub fn progress_bytes(msg: &str, total: u64) -> ReporterProgress {
    if tui_sink().is_some() {
        push_line(format!("[..] {msg}"));
        ReporterProgress::Tui {
            label: msg.to_string(),
        }
    } else {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars(theme::progress_chars()),
        );
        pb.set_message(msg.to_string());
        ReporterProgress::Bar(pb)
    }
}

impl ReporterProgress {
    pub fn set_position(&self, pos: u64) {
        match self {
            ReporterProgress::Bar(b) => b.set_position(pos),
            ReporterProgress::Tui { label } => {
                push_line(format!("[..] {label} ({pos} bytes)"));
            }
        }
    }

    pub fn finish_and_clear(self) {
        if let ReporterProgress::Bar(b) = self {
            b.finish_and_clear();
        }
    }
}

pub fn print_table(headers: &[&str], rows: Vec<Vec<String>>) {
    let mut table = Table::new();
    table.set_header(headers.iter().map(|h| Cell::new(*h)).collect::<Vec<_>>());
    for row in rows {
        table.add_row(row.iter().map(|c| Cell::new(c)).collect::<Vec<_>>());
    }
    push_line(table.to_string());
}

fn format_command(cmd: &Command) -> String {
    let program = cmd.get_program().to_string_lossy();
    let args = cmd
        .get_args()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {args}")
    }
}

fn drain_stream<R: std::io::Read + Send + 'static>(
    reader: R,
    prefix: &'static str,
    sink: Arc<Mutex<Vec<String>>>,
) {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            match line {
                Ok(l) if l.is_empty() => {}
                Ok(l) => {
                    let mut guard = sink.lock().expect("tui log lock");
                    if prefix.is_empty() {
                        guard.push(l);
                    } else {
                        guard.push(format!("{prefix}{l}"));
                    }
                }
                Err(_) => break,
            }
        }
    });
}

/// Run a subprocess; stream stdout/stderr into the TUI log when active.
pub fn command_status(cmd: &mut Command) -> Result<ExitStatus> {
    if tui_sink().is_none() {
        return cmd.status().context("failed to run command");
    }

    push_line(format!("$ {}", format_command(cmd)));
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().context("failed to spawn command")?;
    let sink = tui_sink().expect("tui sink");

    if let Some(stdout) = child.stdout.take() {
        drain_stream(stdout, "", sink.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        drain_stream(stderr, "[stderr] ", sink);
    }

    child.wait().context("failed to wait on command")
}

pub trait LoggedCommand {
    fn status_logged(&mut self) -> Result<ExitStatus>;
}

impl LoggedCommand for Command {
    fn status_logged(&mut self) -> Result<ExitStatus> {
        command_status(self)
    }
}
