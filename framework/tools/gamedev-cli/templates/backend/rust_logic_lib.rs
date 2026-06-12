use game::{
    Action, Config as GameConfig, GameCore, PlayerState as GamePlayerState,
    SpectatorState as GameSpectatorState,
};
use serde::{Deserialize, Serialize};
use shared_types::{Move, Player};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicTacToe;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub board: [Option<Player>; 9],
    pub next_player: Player,
    pub moves_made: u8,
    pub winner: Option<Player>,
}

impl Action<TicTacToe> for Move {
    type Error = String;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub player: Player,
}

impl GamePlayerState<TicTacToe> for PlayerState {
    fn init(_config: &Config, player: Player) -> Self {
        Self { player }
    }

    fn get_player(&self) -> Player {
        self.player
    }

    fn can_take_action(&self, _action: &Move) -> Result<(), String> {
        Ok(())
    }

    fn apply_event(&mut self, _event: &String) {}
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpectatorStateStub;

impl GameSpectatorState<TicTacToe> for SpectatorStateStub {
    fn init(_config: &Config) -> Self {
        Self
    }

    fn apply_event(&mut self, _event: &String) {}
}

impl GameConfig<TicTacToe> for Config {
    type ValidationError = String;

    fn validate(&self) -> Result<(), Self::ValidationError> {
        Ok(())
    }

    fn get_players(&self) -> Vec<Player> {
        vec![Player::Player1, Player::Player2]
    }
}

impl GameCore for TicTacToe {
    type Config = Config;
    type State = State;
    type Action = Move;
    type Player = Player;
    type PlayerState = PlayerState;
    type Event = String;
    type PlayerEvent = String;
    type Result = String;
    type PlayerResult = String;
    type SpectatorEvent = String;
    type SpectatorResult = String;
    type SpectatorState = SpectatorStateStub;

    fn init(_config: &Self::Config) -> Self::State {
        State {
            board: [None; 9],
            next_player: Player::Player1,
            moves_made: 0,
            winner: None,
        }
    }

    fn take_action(state: &mut Self::State, player_action: game::PlayerAction<Self>) -> Vec<Self::Event> {
        if state.winner.is_some() {
            return vec!["game already over".to_string()];
        }
        if player_action.player != state.next_player {
            return vec!["not your turn".to_string()];
        }
        let Move::Place { index } = player_action.action;
        if index >= 9 {
            return vec!["index out of range".to_string()];
        }
        let idx = index as usize;
        if state.board[idx].is_some() {
            return vec!["cell already occupied".to_string()];
        }
        state.board[idx] = Some(player_action.player);
        state.moves_made = state.moves_made.saturating_add(1);
        state.winner = check_winner(&state.board);
        state.next_player = match player_action.player {
            Player::Player1 => Player::Player2,
            Player::Player2 => Player::Player1,
        };
        vec![format!("placed:{index}")]
    }

    fn check_game_over(state: &Self::State) -> Option<Self::Result> {
        if let Some(w) = state.winner {
            return Some(match w {
                Player::Player1 => "Player1 wins".to_string(),
                Player::Player2 => "Player2 wins".to_string(),
            });
        }
        if state.moves_made >= 9 {
            return Some("draw".to_string());
        }
        None
    }

    fn derive_player_event(
        _state: &Self::State,
        _player: &Self::Player,
        _event: &game::InGameEvent<Self>,
    ) -> Option<Self::PlayerEvent> {
        Some("event".to_string())
    }

    fn derive_player_result(
        _state: &Self::State,
        _player: &Self::Player,
        result: &Self::Result,
    ) -> Self::PlayerResult {
        result.clone()
    }

    fn derive_spectator_event(
        _state: &Self::State,
        _event: &game::InGameEvent<Self>,
    ) -> Option<Self::SpectatorEvent> {
        Some("spectator-event".to_string())
    }

    fn derive_spectator_result(_state: &Self::State, result: &Self::Result) -> Self::SpectatorResult {
        result.clone()
    }

    fn scores_at_end(result: &Self::Result) -> Vec<(Self::Player, f64)> {
        match result.as_str() {
            "Player1 wins" => vec![(Player::Player1, 1.0), (Player::Player2, 0.0)],
            "Player2 wins" => vec![(Player::Player1, 0.0), (Player::Player2, 1.0)],
            _ => vec![(Player::Player1, 0.5), (Player::Player2, 0.5)],
        }
    }
}

fn check_winner(board: &[Option<Player>; 9]) -> Option<Player> {
    const LINES: [[usize; 3]; 8] = [
        [0, 1, 2],
        [3, 4, 5],
        [6, 7, 8],
        [0, 3, 6],
        [1, 4, 7],
        [2, 5, 8],
        [0, 4, 8],
        [2, 4, 6],
    ];
    for line in LINES {
        if let Some(p) = board[line[0]] {
            if board[line[1]] == Some(p) && board[line[2]] == Some(p) {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_initializes_and_can_place_mark() {
        let config = Config;
        let mut state = TicTacToe::init(&config);
        assert_eq!(state.moves_made, 0);
        TicTacToe::take_action(
            &mut state,
            game::PlayerAction {
                player: Player::Player1,
                action: Move::Place { index: 0 },
            },
        );
        assert_eq!(state.moves_made, 1);
        assert_eq!(state.board[0], Some(Player::Player1));
    }

    #[test]
    fn game_detects_winner() {
        let config = Config;
        let mut state = TicTacToe::init(&config);
        let seq = [
            (Player::Player1, 0),
            (Player::Player2, 3),
            (Player::Player1, 1),
            (Player::Player2, 4),
            (Player::Player1, 2),
        ];
        for (player, idx) in seq {
            TicTacToe::take_action(
                &mut state,
                game::PlayerAction {
                    player,
                    action: Move::Place { index: idx },
                },
            );
        }
        assert_eq!(TicTacToe::check_game_over(&state), Some("Player1 wins".to_string()));
    }
}
