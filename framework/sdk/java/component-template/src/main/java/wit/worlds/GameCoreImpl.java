package wit.worlds;

import sk.upjs.gdd.game.ActionApplicationResult;
import sk.upjs.gdd.game.GameOrchestrator;
import sk.upjs.gdd.game.GameSerde;
import sk.upjs.gdd.game.InitBundle;
import sk.upjs.gdd.game.PlayerAction;
import sk.upjs.gdd.game.PlayerEvent;
import sk.upjs.gdd.game.PlayerStateEntry;
import sk.upjs.gdd.game.SerializationFormat;
import sk.upjs.gdd.game.SpectatorEvent;
import sk.upjs.gdd.tictactoe.Config;
import sk.upjs.gdd.tictactoe.GameOutcome;
import sk.upjs.gdd.tictactoe.MoveEvent;
import sk.upjs.gdd.tictactoe.Player;
import sk.upjs.gdd.tictactoe.PlayerOutcome;
import sk.upjs.gdd.tictactoe.Position;
import sk.upjs.gdd.tictactoe.State;
import sk.upjs.gdd.tictactoe.TttPlayerState;
import sk.upjs.gdd.tictactoe.TttRules;
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

    private static SerializationFormat mapFormat(GameCore.SerializationFormat f) {
        return SerializationFormat.values()[f.ordinal()];
    }

    private static GameSerde serde(GameCore.SerializationFormat f) {
        return TeaVmGameSerde.forFormat(mapFormat(f));
    }

    private static Config parseConfig(GameSerde serde, byte[] configBytes) throws GameSerde.SerializationException {
        String t = new String(configBytes, StandardCharsets.UTF_8).trim();
        if (t.isEmpty() || t.equals("null")) {
            return new Config();
        }
        return serde.deserialize(Config.class, configBytes);
    }

    public static GameCore.Result<byte[], GameCore.GameCoreError> defaultConfig(
            GameCore.SerializationFormat format) {
        GameSerde serde = serde(format);
        try {
            Config cfg = new Config();
            return GameCore.Result.ok(serde.serialize(cfg));
        } catch (GameSerde.SerializationException e) {
            return GameCore.Result.err(GameCore.GameCoreError.serialize(e.getMessage()));
        }
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
            State spectator = RULES.initSpectatorState(cfg);
            byte[] spectatorState = serde.serialize(spectator);
            return GameCore.Result.ok(new GameCore.Game(full, rows, spectatorState));
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
            State spectatorState = serde.deserialize(State.class, game.spectatorState);
            Player actor = serde.deserialize(Player.class, playerAction.f0);
            Position move = serde.deserialize(Position.class, playerAction.f1);
            PlayerAction<Player, Position> pa = new PlayerAction<>(actor, move);
            TttPlayerState acting =
                    entries.stream()
                            .filter(e -> e.player().equals(actor))
                            .map(PlayerStateEntry::state)
                            .findFirst()
                            .orElseThrow(() -> new IllegalStateException("Player state not found"));
            var applied =
                    GameOrchestrator.applyActionFull(full, pa, acting, RULES, RULES);
            if (!applied.isOk()) {
                return GameCore.Result.err(GameCore.GameCoreError.gameCore(applied.err()));
            }
            ActionApplicationResult<Player, MoveEvent, PlayerOutcome, MoveEvent, GameOutcome> result =
                    applied.ok();
            byte[] newFull = serde.serialize(FullStateWire.from(full));
            ArrayList<GameCore.NewPlayerState> outRows = new ArrayList<>();
            for (PlayerStateEntry<Player, TttPlayerState> e : entries) {
                List<PlayerEvent<MoveEvent, PlayerOutcome>> evs = result.playerEvents().get(e.player());
                TttPlayerState ps = e.state();
                for (PlayerEvent<MoveEvent, PlayerOutcome> pe : evs) {
                    if (pe instanceof PlayerEvent.Visible<MoveEvent, PlayerOutcome> v) {
                        RULES.applyPlayerEvent(ps, v.event());
                    }
                }
                ArrayList<byte[]> evBytes = new ArrayList<>();
                for (PlayerEvent<MoveEvent, PlayerOutcome> pe : evs) {
                    evBytes.add(EventEncoding.playerEvent(pe));
                }
                byte[] pEnc = serde.serialize(e.player());
                byte[] psEnc = serde.serialize(ps);
                outRows.add(new GameCore.NewPlayerState(new GameCore.PlayerState(pEnc, psEnc), evBytes));
            }
            for (SpectatorEvent<MoveEvent, GameOutcome> se : result.spectatorEvents()) {
                if (se instanceof SpectatorEvent.Visible<MoveEvent, GameOutcome> v) {
                    RULES.applySpectatorEvent(spectatorState, v.event());
                }
            }
            ArrayList<byte[]> spectatorEventBytes = new ArrayList<>();
            for (SpectatorEvent<MoveEvent, GameOutcome> se : result.spectatorEvents()) {
                spectatorEventBytes.add(EventEncoding.spectatorEvent(se));
            }
            byte[] spectatorOut = serde.serialize(spectatorState);
            return GameCore.Result.ok(
                    new GameCore.TakeActionResult(newFull, outRows, spectatorEventBytes, spectatorOut));
        } catch (GameSerde.SerializationException e) {
            return GameCore.Result.err(GameCore.GameCoreError.deserialize(e.getMessage()));
        } catch (RuntimeException e) {
            return GameCore.Result.err(GameCore.GameCoreError.processing(String.valueOf(e.getMessage())));
        }
    }
}
