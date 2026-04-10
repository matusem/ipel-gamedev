package wit.worlds;

import dev.ipel.gamedev.game.FullState;
import dev.ipel.gamedev.game.PlayerAction;
import dev.ipel.gamedev.tictactoe.Config;
import dev.ipel.gamedev.tictactoe.Player;
import dev.ipel.gamedev.tictactoe.Position;
import dev.ipel.gamedev.tictactoe.State;
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
