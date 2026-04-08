#[cfg(test)]
mod tests {
    use __LOGIC_NAME__::{Config, Player, Position, TicTacToe};
    use game::GameCore;

    #[test]
    fn tictactoe_first_move_works() {
        let cfg = Config::default();
        let mut state = TicTacToe::init(&cfg);
        TicTacToe::take_action(
            &mut state,
            game::PlayerAction {
                player: Player::X,
                action: Position(0, 0),
            },
        );
        assert_eq!(state.current_player, Player::O);
        assert!(state.board.get(Position(0, 0), 3).flatten().is_some());
    }
}
