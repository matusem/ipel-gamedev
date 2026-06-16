//! Shared visual theme for plain CLI output and ratatui.

use std::io::IsTerminal;
use std::sync::OnceLock;

use owo_colors::OwoColorize;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Padding};

static COLOR_ENABLED: OnceLock<bool> = OnceLock::new();

pub fn use_color() -> bool {
    *COLOR_ENABLED.get_or_init(|| {
        if std::env::var_os("NO_COLOR").is_some() {
            return false;
        }
        std::io::stdout().is_terminal() || std::io::stderr().is_terminal()
    })
}

pub fn glyph_ok() -> &'static str {
    "[+]"
}

pub fn glyph_warn() -> &'static str {
    "[!]"
}

pub fn glyph_fail() -> &'static str {
    "[x]"
}

pub fn glyph_info() -> &'static str {
    "[i]"
}

/// Selected list row marker (TUI).
pub fn tui_list_marker() -> &'static str {
    "> "
}

/// Prefix for disabled / locked menu items.
pub fn tui_lock_prefix() -> &'static str {
    "[lock] "
}

/// Inline separator for hints and breadcrumbs.
pub fn ui_sep() -> &'static str {
    " | "
}

/// Phrase separator (replaces em dash / middle dot chains).
pub fn ui_dash() -> &'static str {
    " - "
}

pub fn ui_arrow() -> &'static str {
    "->"
}

/// Placeholder when a value is missing.
pub fn ui_none() -> &'static str {
    "-"
}

/// Footer label for wrap-around list keys.
pub fn key_cycle() -> &'static str {
    "^v<>"
}

/// Footer label for Enter / confirm.
pub fn key_enter() -> &'static str {
    "ret"
}

/// Spinner frames for indicatif (ASCII only).
pub fn spinner_ticks() -> &'static [&'static str] {
    &["|", "/", "-", "\\"]
}

/// Progress bar fill characters (ASCII only).
pub fn progress_chars() -> &'static str {
    "#=-"
}

pub fn paint_ok(s: &str) -> String {
    if use_color() {
        s.green().to_string()
    } else {
        s.to_string()
    }
}

pub fn paint_warn(s: &str) -> String {
    if use_color() {
        s.yellow().to_string()
    } else {
        s.to_string()
    }
}

pub fn paint_fail(s: &str) -> String {
    if use_color() {
        s.red().to_string()
    } else {
        s.to_string()
    }
}

pub fn paint_muted(s: &str) -> String {
    if use_color() {
        s.dimmed().to_string()
    } else {
        s.to_string()
    }
}

pub fn paint_accent(s: &str) -> String {
    if use_color() {
        s.cyan().to_string()
    } else {
        s.to_string()
    }
}

pub fn paint_bold(s: &str) -> String {
    if use_color() {
        s.bold().to_string()
    } else {
        s.to_string()
    }
}

pub fn tui_accent() -> Color {
    Color::Cyan
}

pub fn tui_success() -> Color {
    Color::Green
}

pub fn tui_warn() -> Color {
    Color::Yellow
}

pub fn tui_danger() -> Color {
    Color::Red
}

pub fn tui_muted() -> Color {
    Color::DarkGray
}

pub fn status_style(ok: bool) -> Style {
    if ok {
        Style::default().fg(tui_success())
    } else {
        Style::default().fg(tui_danger())
    }
}

pub fn tui_highlight() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(tui_accent())
        .add_modifier(Modifier::BOLD)
}

pub fn tui_header_border() -> Style {
    Style::default().fg(tui_accent())
}

pub fn tui_footer() -> Style {
    Style::default().fg(tui_muted())
}

pub fn tui_keycap() -> Style {
    Style::default().fg(tui_accent()).add_modifier(Modifier::BOLD)
}

pub fn tui_disabled() -> Style {
    Style::default().fg(tui_muted())
}

/// Inner padding for bordered panels (left, right, top, bottom).
pub fn panel_padding() -> Padding {
    Padding::new(1, 1, 1, 1)
}

/// Bordered panel with accent title (dashboard cards). Plain ASCII borders.
pub fn card_block(title: impl Into<String>) -> Block<'static> {
    panel_block(title)
}

/// Bordered panel for nested screens and forms.
pub fn panel_block(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(tui_header_border())
        .title(title.into())
        .title_style(Style::default().fg(tui_accent()).add_modifier(Modifier::BOLD))
        .padding(panel_padding())
}

/// Compact titled field box (horizontal padding only).
pub fn field_block(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(tui_header_border())
        .title(title.into())
        .padding(Padding::new(1, 1, 0, 0))
}

/// Same as `field_block` but with accent border when the row is focused.
pub fn focused_field_block(title: impl Into<String>) -> Block<'static> {
    field_block(title)
        .border_style(Style::default().fg(tui_accent()).add_modifier(Modifier::BOLD))
        .title_style(Style::default().fg(tui_accent()).add_modifier(Modifier::BOLD))
}
