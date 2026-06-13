package sk.upjs.gdd.tictactoe;

import sk.upjs.gdd.game.GameRules;
import sk.upjs.gdd.game.InGameEvent;
import sk.upjs.gdd.game.PlayerAction;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Optional;

/**
 * Tic-tac-toe rules aligned with the Rust flat template
 * ({@code tools/gamedev-cli/templates/backend/rust_logic_flat_lib.rs}).
 */
public final class TttRules
        implements GameRules<
                Config,
                State,
                Position,
                Player,
                TttPlayerState,
                Void,
                MoveEvent,
                GameOutcome,
                PlayerOutcome> {

    public static final TttRules INSTANCE = new TttRules();

    private TttRules() {}

    @Override
    public Optional<byte[]> validateConfig(Config config) {
        return config.validate();
    }

    @Override
    public Config copyConfig(Config config) {
        return config.clone();
    }

    @Override
    public List<Player> players(Config config) {
        return List.of(Player.X, Player.O);
    }

    @Override
    public State init(Config config) {
        return State.initial(config);
    }

    @Override
    public TttPlayerState initPlayerState(Config config, Player player, State sharedGameState) {
        return new TttPlayerState(player, sharedGameState.clone());
    }

    @Override
    public Optional<byte[]> canTakeAction(TttPlayerState playerState, Position action) {
        State st = playerState.state;
        int side = st.config.sideLength;
        if (!action.inBounds(side)) {
            return Optional.of("Position out of bounds".getBytes(StandardCharsets.UTF_8));
        }
        if (st.currentPlayer != playerState.player) {
            return Optional.of("It's not your turn".getBytes(StandardCharsets.UTF_8));
        }
        if (st.getCell(action) != null) {
            return Optional.of("Position already taken".getBytes(StandardCharsets.UTF_8));
        }
        return Optional.empty();
    }

    @Override
    public List<Void> takeAction(State state, PlayerAction<Player, Position> playerAction) {
        int side = state.config.sideLength;
        state.setCell(playerAction.action, playerAction.player);
        state.currentPlayer = state.currentPlayer.other();
        return List.of();
    }

    @Override
    public Optional<GameOutcome> checkGameOver(State state) {
        Optional<Player> w = winner(state);
        if (w.isPresent()) {
            return Optional.of(new GameOutcome.Win(w.get()));
        }
        if (state.isFull()) {
            return Optional.of(new GameOutcome.Draw());
        }
        return Optional.empty();
    }

    @Override
    public Optional<MoveEvent> derivePlayerEvent(
            State state, Player viewer, InGameEvent<Player, Position, Void> event) {
        if (event instanceof InGameEvent.PlayerActionEvent<Player, Position, Void> pa) {
            PlayerAction<Player, Position> a = pa.action();
            return Optional.of(new MoveEvent(a.player, a.action));
        }
        if (event instanceof InGameEvent.DomainEvent<Player, Position, Void>) {
            return Optional.empty();
        }
        throw new IllegalStateException("unreachable");
    }

    @Override
    public PlayerOutcome derivePlayerResult(State state, Player viewer, GameOutcome result) {
        if (result instanceof GameOutcome.Win win) {
            return win.winner() == viewer ? PlayerOutcome.Win : PlayerOutcome.Loss;
        }
        if (result instanceof GameOutcome.Draw) {
            return PlayerOutcome.Draw;
        }
        throw new IllegalStateException("unreachable");
    }

    @Override
    public void applyPlayerEvent(TttPlayerState playerState, MoveEvent event) {
        State st = playerState.state;
        int side = st.config.sideLength;
        st.setCell(event.action(), event.player());
        st.currentPlayer = st.currentPlayer.other();
    }

    private static Optional<Player> tryWinSegment(
            State state, int side, int win, int r0, int c0, int dr, int dc) {
        Position p0 = new Position(r0, c0);
        Player first = state.getCell(p0);
        if (first == null) {
            return Optional.empty();
        }
        for (int k = 1; k < win; k++) {
            int r = r0 + dr * k;
            int c = c0 + dc * k;
            if (r < 0 || c < 0 || r >= side || c >= side) {
                return Optional.empty();
            }
            Player cell = state.getCell(new Position(r, c));
            if (cell != first) {
                return Optional.empty();
            }
        }
        return Optional.of(first);
    }

    static Optional<Player> winner(State state) {
        int side = state.config.sideLength;
        int win = state.config.winLength;
        int last = side - win;
        if (last < 0) {
            return Optional.empty();
        }
        for (int r = 0; r < side; r++) {
            for (int c = 0; c <= last; c++) {
                Optional<Player> w = tryWinSegment(state, side, win, r, c, 0, 1);
                if (w.isPresent()) {
                    return w;
                }
            }
        }
        for (int r = 0; r <= last; r++) {
            for (int c = 0; c < side; c++) {
                Optional<Player> w = tryWinSegment(state, side, win, r, c, 1, 0);
                if (w.isPresent()) {
                    return w;
                }
            }
        }
        for (int r = 0; r <= last; r++) {
            for (int c = 0; c <= last; c++) {
                Optional<Player> w = tryWinSegment(state, side, win, r, c, 1, 1);
                if (w.isPresent()) {
                    return w;
                }
            }
        }
        for (int r = 0; r <= last; r++) {
            for (int c = win - 1; c < side; c++) {
                Optional<Player> w = tryWinSegment(state, side, win, r, c, 1, -1);
                if (w.isPresent()) {
                    return w;
                }
            }
        }
        return Optional.empty();
    }
}
