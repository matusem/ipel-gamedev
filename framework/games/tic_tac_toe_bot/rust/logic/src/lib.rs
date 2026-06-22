//! Tic-tac-toe bot — typed against the game logic crate.

use bot::Bot;
use serde::{Deserialize, Serialize};
use tic_tac_toe::{Board, Player, PlayerState, Position, State};

/// Bot tuning parameters.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    /// Prefer the center cell when no immediate win/block is available.
    #[serde(default = "default_take_center")]
    pub take_center: bool,
    /// When true, occasionally pick a random legal move instead of the best heuristic.
    #[serde(default)]
    pub random_fallback: bool,
}

fn default_take_center() -> bool {
    true
}

pub struct TicTacToeBot;

impl Bot for TicTacToeBot {
    type Settings = Settings;
    type PlayerState = PlayerState;
    type Action = Position;

    fn settings_schema_json() -> &'static str {
        r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "take_center": {
      "type": "boolean",
      "title": "Take center",
      "description": "Prefer the center cell when available",
      "default": true
    },
    "random_fallback": {
      "type": "boolean",
      "title": "Random fallback",
      "description": "Sometimes play a random legal move for variety",
      "default": false
    }
  }
}"#
    }

    fn decide(settings: &Settings, view: &PlayerState) -> Option<Position> {
        if view.state.current_player != view.player {
            return None;
        }

        let legal = legal_moves(&view.state);
        if legal.is_empty() {
            return None;
        }

        if settings.random_fallback && legal.len() > 1 {
            let idx = simple_hash(&view.state) as usize % legal.len();
            return Some(legal[idx]);
        }

        let me = view.player;
        let opp = opponent(me);

        if let Some(pos) = find_winning_move(&view.state, me) {
            return Some(pos);
        }
        if let Some(pos) = find_winning_move(&view.state, opp) {
            return Some(pos);
        }

        if settings.take_center {
            let side = view.state.config.side_length;
            let center = side / 2;
            let center_pos = Position(center, center);
            if legal.contains(&center_pos) {
                return Some(center_pos);
            }
        }

        Some(legal[0])
    }
}

fn opponent(p: Player) -> Player {
    match p {
        Player::X => Player::O,
        Player::O => Player::X,
    }
}

fn legal_moves(state: &State) -> Vec<Position> {
    let side = state.config.side_length;
    let mut out = Vec::new();
    for r in 0..side {
        for c in 0..side {
            let pos = Position(r, c);
            if state.board.get(pos, side).flatten().is_none() {
                out.push(pos);
            }
        }
    }
    out
}

fn find_winning_move(state: &State, for_player: Player) -> Option<Position> {
    for pos in legal_moves(state) {
        let mut trial = state.clone();
        trial.board.set(pos, state.config.side_length, for_player);
        if has_win(&trial, for_player) {
            return Some(pos);
        }
    }
    None
}

fn has_win(state: &State, player: Player) -> bool {
    let side = state.config.side_length;
    let win = state.config.win_length;
    let last = side.saturating_sub(win);

    for r in 0..side {
        for c in 0..=last {
            if segment_winner(&state.board, side, win, r, c, 0, 1) == Some(player) {
                return true;
            }
        }
    }
    for r in 0..=last {
        for c in 0..side {
            if segment_winner(&state.board, side, win, r, c, 1, 0) == Some(player) {
                return true;
            }
        }
    }
    for r in 0..=last {
        for c in 0..=last {
            if segment_winner(&state.board, side, win, r, c, 1, 1) == Some(player) {
                return true;
            }
        }
    }
    for r in 0..=last {
        for c in (win - 1)..side {
            if segment_winner(&state.board, side, win, r, c, 1, -1) == Some(player) {
                return true;
            }
        }
    }
    false
}

fn segment_winner(
    board: &Board,
    side: u8,
    win: u8,
    r0: u8,
    c0: u8,
    dr: i8,
    dc: i8,
) -> Option<Player> {
    let first = board.get(Position(r0, c0), side)??;
    for k in 1..win {
        let r = i16::from(r0) + i16::from(dr) * i16::from(k);
        let c = i16::from(c0) + i16::from(dc) * i16::from(k);
        if r < 0 || c < 0 {
            return None;
        }
        let r = r as u8;
        let c = c as u8;
        if r >= side || c >= side {
            return None;
        }
        let cell = board.get(Position(r, c), side)??;
        if cell != first {
            return None;
        }
    }
    Some(first)
}

/// Deterministic pseudo-random index from board state (no external RNG in WASM).
fn simple_hash(state: &State) -> u32 {
    let mut h: u32 = 0;
    for (i, cell) in state.board.0.iter().enumerate() {
        h = h.wrapping_mul(31).wrapping_add(i as u32);
        if let Some(p) = cell {
            h = h.wrapping_add(match p {
                Player::X => 1,
                Player::O => 2,
            });
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use tic_tac_toe::{Board, Config, State};

    #[test]
    fn blocks_opponent_win() {
        let cfg = Config::default();
        let mut state = State {
            config: cfg,
            current_player: Player::X,
            board: Board::empty(9),
        };
        // O has two in top row; X must block at (0, 2)
        state.board.set(Position(0, 0), 3, Player::O);
        state.board.set(Position(0, 1), 3, Player::O);

        let view = PlayerState {
            player: Player::X,
            state,
        };
        let action = TicTacToeBot::decide(&Settings::default(), &view).unwrap();
        assert_eq!(action, Position(0, 2));
    }
}
