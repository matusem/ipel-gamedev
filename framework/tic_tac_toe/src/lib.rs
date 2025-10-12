use game::{Action, Config as GameConfig, GameCore, PlayerState as GamePlayerState};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct TicTacToe;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Player {
    X,
    O,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position(u8, u8);
impl Position {
    pub fn new(row: u8, column: u8) -> Option<Position> {
        if row < 3 && column < 3 {
            Some(Position(row, column))
        } else {
            None
        }
    }

    pub fn to_index(&self) -> usize {
        (self.0 * 3 + self.1) as usize
    }
}
impl Action<TicTacToe> for Position {
    type Error = String;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board([Option<Player>; 9]);
impl Board {
    pub fn set(&mut self, position: Position, player: Player) {
        self.0[position.to_index()] = Some(player);
    }

    pub fn get(&self, position: Position) -> Option<Player> {
        self.0[position.to_index()]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    current_player: Player,
    board: Board,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    player: Player,
    state: State,
}

impl GamePlayerState<TicTacToe> for PlayerState {
    fn init(
        config: &<TicTacToe as game::GameCore>::Config,
        player: <TicTacToe as game::GameCore>::Player,
    ) -> <TicTacToe as game::GameCore>::PlayerState {
        PlayerState {
            player,
            state: TicTacToe::init(config),
        }
    }

    fn get_player(&self) -> <TicTacToe as game::GameCore>::Player {
        self.player
    }

    fn can_take_action(
        &self,
        action: &<TicTacToe as game::GameCore>::Action,
    ) -> Result<(), <<TicTacToe as game::GameCore>::Action as game::Action<TicTacToe>>::Error> {
        if self.state.current_player != self.player {
            return Err("It's not your turn".into());
        }

        if self.state.board.get(*action).is_some() {
            return Err("Position already taken".into());
        }

        Ok(())
    }

    fn apply_event(&mut self, event: &<TicTacToe as game::GameCore>::PlayerEvent) {
        let PlayerEvent { player, action } = event;
        self.state.board.set(*action, *player);
        self.state.current_player = match self.state.current_player {
            Player::X => Player::O,
            Player::O => Player::X,
        };
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerEvent {
    pub player: Player,
    pub action: Position,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Config;
impl GameConfig<TicTacToe> for Config {
    type ValidationError = ();

    fn validate(&self) -> Result<(), Self::ValidationError> {
        Ok(())
    }

    fn get_players(&self) -> Vec<<TicTacToe as game::GameCore>::Player> {
        vec![Player::X, Player::O]
    }
}

const GAME_OVER_POSITIONS: [[Position; 3]; 8] = [
    [Position(0, 0), Position(0, 1), Position(0, 2)],
    [Position(1, 0), Position(1, 1), Position(1, 2)],
    [Position(2, 0), Position(2, 1), Position(2, 2)],
    [Position(0, 0), Position(1, 0), Position(2, 0)],
    [Position(0, 1), Position(1, 1), Position(2, 1)],
    [Position(0, 2), Position(1, 2), Position(2, 2)],
    [Position(0, 0), Position(1, 1), Position(2, 2)],
    [Position(0, 2), Position(1, 1), Position(2, 0)],
];

impl GameCore for TicTacToe {
    type Config = Config;

    type State = State;

    type Action = Position;

    type Player = Player;
    type PlayerState = PlayerState;

    type Event = ();
    type PlayerEvent = PlayerEvent;

    type Result = Self::Player;
    type PlayerResult = Self::Result;

    fn init(_config: &Self::Config) -> Self::State {
        State {
            current_player: Player::X,
            board: Board([None; 9]),
        }
    }

    fn take_action(
        state: &mut Self::State,
        player_action: game::PlayerAction<Self>,
    ) -> Vec<Self::Event> {
        let game::PlayerAction { player, action } = player_action;

        state.board.set(action, player);
        state.current_player = match state.current_player {
            Player::X => Player::O,
            Player::O => Player::X,
        };

        vec![]
    }

    fn check_game_over(state: &Self::State) -> Option<Self::Result> {
        GAME_OVER_POSITIONS.iter().find_map(|positions| {
            let first = state.board.get(positions[0])?;
            if positions
                .iter()
                .all(|pos| state.board.get(*pos) == Some(first))
            {
                Some(first)
            } else {
                None
            }
        })
    }

    fn derive_player_event(
        _state: &Self::State,
        _player: &Self::Player,
        event: &game::InGameEvent<Self>,
    ) -> Option<Self::PlayerEvent> {
        match event {
            game::InGameEvent::PlayerAction(player_action) => Some(PlayerEvent {
                player: player_action.player,
                action: player_action.action,
            }),
            _ => None,
        }
    }

    fn derive_player_result(
        _state: &Self::State,
        _player: &Self::Player,
        result: &Self::Result,
    ) -> Self::PlayerResult {
        *result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        assert_eq!(4, 4);
    }
}
