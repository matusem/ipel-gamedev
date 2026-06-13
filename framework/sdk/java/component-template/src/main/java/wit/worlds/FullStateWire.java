package wit.worlds;

import sk.upjs.gdd.game.FullState;
import sk.upjs.gdd.game.PlayerAction;
import sk.upjs.gdd.tictactoe.Config;
import sk.upjs.gdd.tictactoe.Player;
import sk.upjs.gdd.tictactoe.Position;
import sk.upjs.gdd.tictactoe.State;
import java.util.ArrayList;
import java.util.List;

/** Wire shape for {@code FullState} (snake_case via Jackson naming strategy). */
public record FullStateWire(Config config, State state, List<PlayerActionWire> actionsMade) {

    public static FullStateWire from(FullState<Config, State, Position, Player> fs) {
        List<PlayerActionWire> am = new ArrayList<>();
        for (PlayerAction<Player, Position> a : fs.actionsMade) {
            am.add(new PlayerActionWire(a.player, a.action));
        }
        return new FullStateWire(fs.config, fs.state, am);
    }

    public FullState<Config, State, Position, Player> toDomain() {
        FullState<Config, State, Position, Player> fs = FullState.fresh(config, state);
        for (PlayerActionWire w : actionsMade) {
            fs.actionsMade.add(new PlayerAction<>(w.player(), w.action()));
        }
        return fs;
    }
}
