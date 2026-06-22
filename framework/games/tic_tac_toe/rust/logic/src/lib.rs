use game::{
    Action, Config as GameConfig, GameCore, PlayerState as GamePlayerState,
    SpectatorState as GameSpectatorState,
};
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Debug)]
pub struct TicTacToe;

#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Player {
    X,
    O,
}

/// Authoritative end state: either a winning player or a full board with no winner.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameOutcome {
    Win(Player),
    Draw,
}

/// What this player sees when the game ends.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlayerOutcome {
    Win,
    Loss,
    Draw,
}

#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Position(pub u8, pub u8);

impl Position {
    pub fn new(row: u8, col: u8, side: u8) -> Option<Position> {
        if row < side && col < side {
            Some(Position(row, col))
        } else {
            None
        }
    }

    pub fn to_index(self, side: u8) -> usize {
        (self.0 as usize) * (side as usize) + (self.1 as usize)
    }
}

impl Action<TicTacToe> for Position {
    type Error = String;
}

#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Board(pub Vec<Option<Player>>);

impl Board {
    pub fn empty(size: usize) -> Self {
        Self(vec![None; size])
    }

    pub fn get(&self, pos: Position, side: u8) -> Option<Option<Player>> {
        let i = pos.to_index(side);
        self.0.get(i).copied()
    }

    pub fn set(&mut self, pos: Position, side: u8, player: Player) {
        let i = pos.to_index(side);
        if i < self.0.len() {
            self.0[i] = Some(player);
        }
    }

    pub fn is_full(&self) -> bool {
        self.0.iter().all(|c| c.is_some())
    }
}

#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub config: Config,
    pub current_player: Player,
    pub board: Board,
}

#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub player: Player,
    pub state: State,
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
        let side = self.state.config.side_length;
        if Position::new(action.0, action.1, side).is_none() {
            return Err("Position out of bounds".into());
        }
        if self.state.current_player != self.player {
            return Err("It's not your turn".into());
        }

        if self.state.board.get(*action, side).flatten().is_some() {
            return Err("Position already taken".into());
        }

        Ok(())
    }

    fn apply_event(&mut self, event: &<TicTacToe as game::GameCore>::PlayerEvent) {
        let PlayerEvent { player, action } = event;
        let side = self.state.config.side_length;
        self.state.board.set(*action, side, *player);
        self.state.current_player = match self.state.current_player {
            Player::X => Player::O,
            Player::O => Player::X,
        };
    }
}

#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerEvent {
    pub player: Player,
    pub action: Position,
}

/// Public observer view (open-information tic-tac-toe mirrors the board).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectatorStateView {
    pub current_player: Player,
    pub board: Board,
}

impl GameSpectatorState<TicTacToe> for SpectatorStateView {
    fn init(config: &Config) -> Self {
        let state = TicTacToe::init(config);
        Self {
            current_player: state.current_player,
            board: state.board,
        }
    }

    fn apply_event(&mut self, event: &PlayerEvent) {
        let PlayerEvent { player, action } = event;
        let cells = self.board.0.len();
        let side = (cells as f64).sqrt() as u8;
        self.board.set(*action, side, *player);
        self.current_player = match self.current_player {
            Player::X => Player::O,
            Player::O => Player::X,
        };
    }
}

fn default_side() -> u8 {
    3
}

fn default_win() -> u8 {
    3
}

/// Grid is `side_length × side_length`. A player wins with `win_length` marks in a row (horizontal, vertical, or diagonal).
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Config {
    #[serde(default = "default_side")]
    pub side_length: u8,
    #[serde(default = "default_win")]
    pub win_length: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            side_length: default_side(),
            win_length: default_win(),
        }
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ConfigVisitor;

        impl<'de> Visitor<'de> for ConfigVisitor {
            type Value = Config;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("null or tic-tac-toe config object")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Config::default())
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Config::default())
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut side_length: Option<u8> = None;
                let mut win_length: Option<u8> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "side_length" => {
                            if side_length.is_some() {
                                return Err(de::Error::duplicate_field("side_length"));
                            }
                            side_length = Some(map.next_value()?);
                        }
                        "win_length" => {
                            if win_length.is_some() {
                                return Err(de::Error::duplicate_field("win_length"));
                            }
                            win_length = Some(map.next_value()?);
                        }
                        _ => {
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }
                Ok(Config {
                    side_length: side_length.unwrap_or_else(default_side),
                    win_length: win_length.unwrap_or_else(default_win),
                })
            }
        }

        deserializer.deserialize_any(ConfigVisitor)
    }
}

impl GameConfig<TicTacToe> for Config {
    type ValidationError = String;

    fn validate(&self) -> Result<(), Self::ValidationError> {
        const MAX_SIDE: u8 = 20;
        if self.side_length < 2 {
            return Err("side_length must be at least 2".into());
        }
        if self.side_length > MAX_SIDE {
            return Err(format!("side_length must be at most {MAX_SIDE}"));
        }
        if self.win_length < 2 {
            return Err("win_length must be at least 2".into());
        }
        if self.win_length > self.side_length {
            return Err("win_length cannot exceed side_length".into());
        }
        Ok(())
    }

    fn get_players(&self) -> Vec<<TicTacToe as game::GameCore>::Player> {
        vec![Player::X, Player::O]
    }
}

fn try_win_segment(
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

fn winner(state: &State) -> Option<Player> {
    let side = state.config.side_length;
    let win = state.config.win_length;
    let last = side.saturating_sub(win);

    for r in 0..side {
        for c in 0..=last {
            if let Some(w) = try_win_segment(&state.board, side, win, r, c, 0, 1) {
                return Some(w);
            }
        }
    }
    for r in 0..=last {
        for c in 0..side {
            if let Some(w) = try_win_segment(&state.board, side, win, r, c, 1, 0) {
                return Some(w);
            }
        }
    }
    for r in 0..=last {
        for c in 0..=last {
            if let Some(w) = try_win_segment(&state.board, side, win, r, c, 1, 1) {
                return Some(w);
            }
        }
    }
    for r in 0..=last {
        for c in (win - 1)..side {
            if let Some(w) = try_win_segment(&state.board, side, win, r, c, 1, -1) {
                return Some(w);
            }
        }
    }
    None
}

fn check_game_over_state(state: &State) -> Option<GameOutcome> {
    if let Some(w) = winner(state) {
        return Some(GameOutcome::Win(w));
    }
    if state.board.is_full() {
        return Some(GameOutcome::Draw);
    }
    None
}

impl GameCore for TicTacToe {
    type Config = Config;

    type State = State;

    type Action = Position;

    type Player = Player;
    type PlayerState = PlayerState;

    type Event = ();
    type PlayerEvent = PlayerEvent;

    type Result = GameOutcome;
    type PlayerResult = PlayerOutcome;

    type SpectatorEvent = PlayerEvent;
    type SpectatorResult = GameOutcome;
    type SpectatorState = SpectatorStateView;

    fn init(config: &Self::Config) -> Self::State {
        let n = (config.side_length as usize).saturating_pow(2);
        State {
            config: config.clone(),
            current_player: Player::X,
            board: Board::empty(n),
        }
    }

    fn take_action(
        state: &mut Self::State,
        player_action: game::PlayerAction<Self>,
    ) -> Vec<Self::Event> {
        let game::PlayerAction { player, action } = player_action;
        let side = state.config.side_length;
        state.board.set(action, side, player);
        state.current_player = match state.current_player {
            Player::X => Player::O,
            Player::O => Player::X,
        };

        vec![]
    }

    fn check_game_over(state: &Self::State) -> Option<Self::Result> {
        check_game_over_state(state)
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
        player: &Self::Player,
        result: &Self::Result,
    ) -> Self::PlayerResult {
        match result {
            GameOutcome::Win(w) if *w == *player => PlayerOutcome::Win,
            GameOutcome::Win(_) => PlayerOutcome::Loss,
            GameOutcome::Draw => PlayerOutcome::Draw,
        }
    }

    fn derive_spectator_event(
        _state: &Self::State,
        event: &game::InGameEvent<Self>,
    ) -> Option<Self::SpectatorEvent> {
        match event {
            game::InGameEvent::PlayerAction(player_action) => Some(PlayerEvent {
                player: player_action.player,
                action: player_action.action,
            }),
            _ => None,
        }
    }

    fn derive_spectator_result(
        _state: &Self::State,
        result: &Self::Result,
    ) -> Self::SpectatorResult {
        *result
    }

    fn scores_at_end(result: &Self::Result) -> Vec<(Self::Player, f64)> {
        match result {
            GameOutcome::Win(w) => vec![
                (Player::X, if *w == Player::X { 1.0 } else { 0.0 }),
                (Player::O, if *w == Player::O { 1.0 } else { 0.0 }),
            ],
            GameOutcome::Draw => vec![(Player::X, 0.5), (Player::O, 0.5)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::PlayerEvent;

    #[test]
    fn player_event_game_over_json_shape() {
        let win: PlayerEvent<TicTacToe> = PlayerEvent::GameOver(PlayerOutcome::Win);
        let loss: PlayerEvent<TicTacToe> = PlayerEvent::GameOver(PlayerOutcome::Loss);
        let draw: PlayerEvent<TicTacToe> = PlayerEvent::GameOver(PlayerOutcome::Draw);
        assert_eq!(
            serde_json::to_string(&win).unwrap(),
            r#"{"GameOver":"Win"}"#
        );
        assert_eq!(
            serde_json::to_string(&loss).unwrap(),
            r#"{"GameOver":"Loss"}"#
        );
        assert_eq!(
            serde_json::to_string(&draw).unwrap(),
            r#"{"GameOver":"Draw"}"#
        );
    }

    #[test]
    fn scores_at_end_draw_and_win() {
        let s = TicTacToe::scores_at_end(&GameOutcome::Draw);
        assert_eq!(s.len(), 2);
        assert!((s.iter().find(|(p, _)| *p == Player::X).unwrap().1 - 0.5).abs() < 1e-9);
        let s2 = TicTacToe::scores_at_end(&GameOutcome::Win(Player::X));
        assert_eq!(s2.iter().find(|(p, _)| *p == Player::X).unwrap().1, 1.0);
        assert_eq!(s2.iter().find(|(p, _)| *p == Player::O).unwrap().1, 0.0);
    }

    #[test]
    fn default_config_deserializes_from_null() {
        let c: Config = serde_json::from_str("null").unwrap();
        assert_eq!(c.side_length, 3);
        assert_eq!(c.win_length, 3);
    }

    #[test]
    fn win_on_5x5_length_4() {
        let cfg = Config {
            side_length: 5,
            win_length: 4,
        };
        cfg.validate().unwrap();
        let mut state = TicTacToe::init(&cfg);
        // X wins on top row 0..4
        for c in 0..4u8 {
            state.board.set(Position(0, c), 5, Player::X);
        }
        assert_eq!(
            check_game_over_state(&state),
            Some(GameOutcome::Win(Player::X))
        );
    }

    #[test]
    fn spectator_receives_public_move_events() {
        let cfg = Config::default();
        cfg.validate().unwrap();
        let mut fs = TicTacToe::try_init(&cfg).unwrap();
        let ps = PlayerState::init(&cfg, Player::X);
        let result = TicTacToe::apply_action(
            &mut fs,
            game::PlayerAction {
                player: Player::X,
                action: Position(0, 0),
            },
            &ps,
        )
        .expect("legal move");
        assert!(!result.spectator_events.is_empty());
    }

    #[test]
    fn draw_when_board_full_no_winner() {
        let cfg = Config::default();
        cfg.validate().unwrap();
        let mut state = TicTacToe::init(&cfg);
        // Cat's game pattern on 3x3
        let moves = [
            (Player::X, Position(0, 0)),
            (Player::O, Position(0, 1)),
            (Player::X, Position(0, 2)),
            (Player::O, Position(1, 1)),
            (Player::X, Position(1, 0)),
            (Player::O, Position(1, 2)),
            (Player::X, Position(2, 1)),
            (Player::O, Position(2, 0)),
            (Player::X, Position(2, 2)),
        ];
        for (p, pos) in moves {
            state.board.set(pos, 3, p);
        }
        assert_eq!(winner(&state), None);
        assert_eq!(check_game_over_state(&state), Some(GameOutcome::Draw));
    }
}
