//! Wrap-around list navigation for TUI menus.

use crossterm::event::KeyCode;
use ratatui::widgets::ListState;

pub fn cycle_index(selected: Option<usize>, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let cur = selected.unwrap_or(0) as i32;
    ((cur + delta).rem_euclid(len as i32)) as usize
}

pub fn is_list_prev(key: KeyCode) -> bool {
    matches!(key, KeyCode::Up | KeyCode::Left | KeyCode::Char('k'))
}

pub fn is_list_next(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::Down | KeyCode::Right | KeyCode::Char('j') | KeyCode::Tab
    )
}

pub fn cycle_list(list: &mut ListState, delta: i32, len: usize) {
    list.select(Some(cycle_index(list.selected(), delta, len)));
}
