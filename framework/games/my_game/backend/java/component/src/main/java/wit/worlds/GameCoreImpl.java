package wit.worlds;

import dev.ipel.gamedev.game.GameOrchestrator;
import dev.ipel.gamedev.game.GameSerde;
import dev.ipel.gamedev.game.InitBundle;
import dev.ipel.gamedev.game.PlayerAction;
import dev.ipel.gamedev.game.PlayerEvent;
import dev.ipel.gamedev.game.PlayerStateEntry;
import dev.ipel.gamedev.game.SerializationFormat;
import dev.ipel.gamedev.tictactoe.Config;
import dev.ipel.gamedev.tictactoe.MoveEvent;
import dev.ipel.gamedev.tictactoe.Player;
import dev.ipel.gamedev.tictactoe.PlayerOutcome;
import dev.ipel.gamedev.tictactoe.Position;
import dev.ipel.gamedev.tictactoe.State;
import dev.ipel.gamedev.tictactoe.TttPlayerState;
import dev.ipel.gamedev.tictactoe.TttRules;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * WIT guest hooks (generated {@link GameCore} forwards here). Mirrors {@code game_wasm_host::MyHost} behaviour.
 */
public final class GameCoreImpl {

    private static final TttRules RULES = TttRules.INSTANCE;

    private GameCoreImpl() {}



    /** TeaVM entry (unused at runtime; keeps whole module reachable for wasm-gc compile). */
    public static void main(String[] args) {}

    private static SerializationFormat mapFormat(GameCore.SerializationFormat f) {
        return SerializationFormat.values()[f.ordinal()];
    }

    private static GameSerde serde(GameCore.SerializationFormat f) {
        return dev.ipel.gamedev.game.GameSerdeFactory.forFormat(mapFormat(f));
    }

    private static Config parseConfig(GameSerde serde, byte[] configBytes) throws GameSerde.SerializationException {
        String t = new String(configBytes, StandardCharsets.UTF_8).trim();
        if (t.isEmpty() || t.equals("null")) {
            return new Config();
        }
        return serde.deserialize(Config.class, configBytes);
    }

    public static GameCore.Result<GameCore.Game, GameCore.GameCoreError> init(
            GameCore.SerializationFormat format, byte[] configBytes) {
        GameSerde serde = serde(format);
        try {
            Config cfg = parseConfig(serde, configBytes);
            var outcome = GameOrchestrator.tryInitWithPlayers(cfg, RULES);
            if (!outcome.isOk()) {
                return GameCore.Result.err(GameCore.GameCoreError.gameCore(outcome.err()));
            }
            InitBundle<Config, State, Position, Player, TttPlayerState> bundle = outcome.ok();
            byte[] full = serde.serialize(FullStateWire.from(bundle.fullState()));
            ArrayList<GameCore.PlayerState> rows = new ArrayList<>();
            for (PlayerStateEntry<Player, TttPlayerState> e : bundle.playerStates()) {
                byte[] p = serde.serialize(e.player());
                byte[] ps = serde.serialize(e.state());
                rows.add(new GameCore.PlayerState(p, ps));
            }
            return GameCore.Result.ok(new GameCore.Game(full, rows));
        } catch (GameSerde.SerializationException e) {
            return GameCore.Result.err(GameCore.GameCoreError.deserialize(e.getMessage()));
        } catch (RuntimeException e) {
            return GameCore.Result.err(GameCore.GameCoreError.processing(String.valueOf(e.getMessage())));
        }
    }

    public static GameCore.Result<GameCore.TakeActionResult, GameCore.GameCoreError> takeAction(
            GameCore.SerializationFormat format,
            GameCore.Game game,
            GameCore.Tuple2<byte[], byte[]> playerAction) {
        GameSerde serde = serde(format);
        try {
            FullStateWire wire = serde.deserialize(FullStateWire.class, game.fullState);
            var full = wire.toDomain();
            List<PlayerStateEntry<Player, TttPlayerState>> entries = new ArrayList<>();
            for (GameCore.PlayerState row : game.playerStates) {
                Player p = serde.deserialize(Player.class, row.player);
                TttPlayerState cps = serde.deserialize(TttPlayerState.class, row.state);
                cps.state = full.state.clone();
                entries.add(new PlayerStateEntry<>(p, cps));
            }
            Player actor = serde.deserialize(Player.class, playerAction.f0);
            Position move = serde.deserialize(Position.class, playerAction.f1);
            PlayerAction<Player, Position> pa = new PlayerAction<>(actor, move);
            TttPlayerState acting =
                    entries.stream()
                            .filter(e -> e.player().equals(actor))
                            .map(PlayerStateEntry::state)
                            .findFirst()
                            .orElseThrow(() -> new IllegalStateException("Player state not found"));
            var applied = GameOrchestrator.applyAction(full, pa, acting, RULES);
            if (!applied.isOk()) {
                return GameCore.Result.err(GameCore.GameCoreError.gameCore(applied.err()));
            }
            Map<Player, List<PlayerEvent<MoveEvent, PlayerOutcome>>> result = applied.ok();
            byte[] newFull = serde.serialize(FullStateWire.from(full));
            ArrayList<GameCore.NewPlayerState> outRows = new ArrayList<>();
            for (PlayerStateEntry<Player, TttPlayerState> e : entries) {
                List<PlayerEvent<MoveEvent, PlayerOutcome>> evs = result.get(e.player());
                TttPlayerState ps = e.state();
                for (PlayerEvent<MoveEvent, PlayerOutcome> pe : evs) {
                    if (pe instanceof PlayerEvent.Visible<MoveEvent, PlayerOutcome> v) {
                        RULES.applyPlayerEvent(ps, v.event());
                    }
                }
                ArrayList<byte[]> evBytes = new ArrayList<>();
                for (PlayerEvent<MoveEvent, PlayerOutcome> pe : evs) {
                    evBytes.add(EventEncoding.playerEvent(serde.jacksonMapper(), pe));
                }
                byte[] pEnc = serde.serialize(e.player());
                byte[] psEnc = serde.serialize(ps);
                outRows.add(new GameCore.NewPlayerState(new GameCore.PlayerState(pEnc, psEnc), evBytes));
            }
            return GameCore.Result.ok(new GameCore.TakeActionResult(newFull, outRows));
        } catch (GameSerde.SerializationException e) {
            return GameCore.Result.err(GameCore.GameCoreError.deserialize(e.getMessage()));
        } catch (RuntimeException e) {
            return GameCore.Result.err(GameCore.GameCoreError.processing(String.valueOf(e.getMessage())));
        }
    }
}
