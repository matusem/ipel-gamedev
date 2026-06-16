//! ASCII-art banner for the home dashboard.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme;

const BANNER_LINES: &[&str] = &[
    "  ____   _    __  __ _____ ____  _______     __",
    " / ___| / \\  |  \\/  | ____|  _ \\| ____\\ \\   / /",
    "| |  _ / _ \\ | |\\/| |  _| | | | |  _|  \\ \\ / / ",
    "| |_| / ___ \\| |  | | |___| |_| | |___  \\ V /  ",
    " \\____/_/   \\_\\_|  |_|_____|____/|_____|  \\_/   ",
];

const BANNER_MIN_WIDTH: u16 = 52;

pub fn banner_height(width: u16) -> u16 {
    if width >= BANNER_MIN_WIDTH {
        7
    } else {
        2
    }
}

pub fn draw_banner(frame: &mut Frame, area: Rect, version: &str) {
    if area.width >= BANNER_MIN_WIDTH {
        let accent = Style::default().fg(theme::tui_accent()).add_modifier(Modifier::BOLD);
        let mut lines: Vec<Line> = BANNER_LINES
            .iter()
            .map(|l| Line::from(Span::styled(*l, accent)))
            .collect();
        lines.push(Line::from(vec![
            Span::styled("UPJS GDD Platform - developer CLI", theme::tui_footer()),
            Span::raw(" "),
            Span::styled(
                format!("v{version}"),
                Style::default()
                    .fg(theme::tui_accent())
                    .add_modifier(Modifier::DIM),
            ),
        ]));
        lines.push(Line::from(""));
        frame.render_widget(Paragraph::new(lines), area);
    } else {
        let line = Line::from(vec![
            Span::styled("* ", Style::default().fg(theme::tui_accent())),
            Span::styled(
                "gamedev-cli",
                Style::default()
                    .fg(theme::tui_accent())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" v{version}"), theme::tui_footer()),
        ]);
        frame.render_widget(Paragraph::new(vec![line, Line::from("")]), area);
    }
}
