package dev.ipel.gamedev.game;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;

/**
 * Default orchestration matching Rust {@code GameCore::try_init}, {@code build_player_events_map}, and
 * {@code apply_action}.
 */
public final class GameOrchestrator {

    private GameOrchestrator() {}

    public static <
                    ConfigT,
                    StateT,
                    ActionT,
                    PlayerT,
                    PlayerStateT,
                    GameEventT,
                    PlayerViewEventT,
                    GameResultT,
                    PlayerResultT>
            Either<InitBundle<ConfigT, StateT, ActionT, PlayerT, PlayerStateT>, byte[]> tryInitWithPlayers(
                    ConfigT config,
                    GameRules<
                                    ConfigT,
                                    StateT,
                                    ActionT,
                                    PlayerT,
                                    PlayerStateT,
                                    GameEventT,
                                    PlayerViewEventT,
                                    GameResultT,
                                    PlayerResultT>
                            rules) {
        Optional<byte[]> bad = rules.validateConfig(config);
        if (bad.isPresent()) {
            return Either.err(bad.get());
        }
        ConfigT cfg = rules.copyConfig(config);
        StateT shared = rules.init(cfg);
        FullState<ConfigT, StateT, ActionT, PlayerT> full = FullState.fresh(cfg, shared);
        List<PlayerStateEntry<PlayerT, PlayerStateT>> rows = new ArrayList<>();
        for (PlayerT p : rules.players(cfg)) {
            rows.add(new PlayerStateEntry<>(p, rules.initPlayerState(cfg, p, full.state)));
        }
        return Either.ok(new InitBundle<>(full, rows));
    }

    public static <
                    ConfigT,
                    StateT,
                    ActionT,
                    PlayerT,
                    PlayerStateT,
                    GameEventT,
                    PlayerViewEventT,
                    GameResultT,
                    PlayerResultT>
            Map<PlayerT, List<PlayerEvent<PlayerViewEventT, PlayerResultT>>> buildPlayerEventsMap(
                    FullState<ConfigT, StateT, ActionT, PlayerT> gameState,
                    List<Event<PlayerT, ActionT, GameEventT, GameResultT>> gameEvents,
                    GameRules<
                                    ConfigT,
                                    StateT,
                                    ActionT,
                                    PlayerT,
                                    PlayerStateT,
                                    GameEventT,
                                    PlayerViewEventT,
                                    GameResultT,
                                    PlayerResultT>
                            rules) {
        Map<PlayerT, List<PlayerEvent<PlayerViewEventT, PlayerResultT>>> out = new LinkedHashMap<>();
        List<PlayerT> players = rules.players(gameState.config);
        for (PlayerT player : players) {
            out.put(player, new ArrayList<>());
        }
        for (PlayerT player : players) {
            List<PlayerEvent<PlayerViewEventT, PlayerResultT>> bucket = out.get(player);
            for (Event<PlayerT, ActionT, GameEventT, GameResultT> event : gameEvents) {
                if (event instanceof Event.InGame<PlayerT, ActionT, GameEventT, GameResultT> ig) {
                    rules.derivePlayerEvent(gameState.state, player, ig.event())
                            .map(pe -> new PlayerEvent.Visible<PlayerViewEventT, PlayerResultT>(pe))
                            .ifPresent(bucket::add);
                } else if (event instanceof Event.GameOver<PlayerT, ActionT, GameEventT, GameResultT> go) {
                    bucket.add(
                            new PlayerEvent.Terminal<>(
                                    rules.derivePlayerResult(gameState.state, player, go.result())));
                }
            }
        }
        return out;
    }

    public static <
                    ConfigT,
                    StateT,
                    ActionT,
                    PlayerT,
                    PlayerStateT,
                    GameEventT,
                    PlayerViewEventT,
                    GameResultT,
                    PlayerResultT>
            Either<Map<PlayerT, List<PlayerEvent<PlayerViewEventT, PlayerResultT>>>, byte[]> applyAction(
                    FullState<ConfigT, StateT, ActionT, PlayerT> gameState,
                    PlayerAction<PlayerT, ActionT> playerAction,
                    PlayerStateT actingPlayerState,
                    GameRules<
                                    ConfigT,
                                    StateT,
                                    ActionT,
                                    PlayerT,
                                    PlayerStateT,
                                    GameEventT,
                                    PlayerViewEventT,
                                    GameResultT,
                                    PlayerResultT>
                            rules) {
        Optional<byte[]> deny = rules.canTakeAction(actingPlayerState, playerAction.action);
        if (deny.isPresent()) {
            return Either.err(deny.get());
        }
        gameState.actionsMade.add(playerAction.copy());
        List<GameEventT> emitted = rules.takeAction(gameState.state, playerAction);
        List<Event<PlayerT, ActionT, GameEventT, GameResultT>> gameEvents = new ArrayList<>();
        gameEvents.add(new Event.InGame<>(new InGameEvent.PlayerActionEvent<>(playerAction)));
        for (GameEventT e : emitted) {
            gameEvents.add(new Event.InGame<>(new InGameEvent.DomainEvent<>(e)));
        }
        Optional<GameResultT> over = rules.checkGameOver(gameState.state);
        over.ifPresent(r -> gameEvents.add(new Event.GameOver<>(r)));
        return Either.ok(buildPlayerEventsMap(gameState, gameEvents, rules));
    }

    public static final class Either<ValueT, ErrorT> {
        private final ValueT ok;
        private final ErrorT err;

        private Either(ValueT ok, ErrorT err) {
            this.ok = ok;
            this.err = err;
        }

        public static <ValueT, ErrorT> Either<ValueT, ErrorT> ok(ValueT value) {
            return new Either<>(value, null);
        }

        public static <ValueT, ErrorT> Either<ValueT, ErrorT> err(ErrorT error) {
            return new Either<>(null, error);
        }

        public boolean isOk() {
            return err == null;
        }

        public ValueT ok() {
            if (err != null) {
                throw new IllegalStateException("not ok");
            }
            return ok;
        }

        public ErrorT err() {
            if (err == null) {
                throw new IllegalStateException("not err");
            }
            return err;
        }
    }
}
