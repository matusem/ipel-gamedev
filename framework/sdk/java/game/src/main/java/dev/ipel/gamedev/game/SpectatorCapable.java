package dev.ipel.gamedev.game;

import java.util.List;
import java.util.Optional;

/** Optional spectator projection hooks (mirrors Rust {@code SpectatorState} / {@code derive_spectator_event}). */
public interface SpectatorCapable<
        ConfigT,
        StateT,
        ActionT,
        PlayerT,
        GameEventT,
        GameResultT,
        SpectatorViewEventT,
        SpectatorResultT,
        SpectatorStateT> {

    SpectatorStateT initSpectatorState(ConfigT config);

    Optional<SpectatorViewEventT> deriveSpectatorEvent(
            StateT state, InGameEvent<PlayerT, ActionT, GameEventT> event);

    SpectatorResultT deriveSpectatorResult(StateT state, GameResultT result);

    void applySpectatorEvent(SpectatorStateT spectatorState, SpectatorViewEventT event);

    default List<SpectatorEvent<SpectatorViewEventT, SpectatorResultT>> buildSpectatorEventsMap(
            FullState<ConfigT, StateT, ActionT, PlayerT> gameState,
            List<Event<PlayerT, ActionT, GameEventT, GameResultT>> gameEvents) {
        List<SpectatorEvent<SpectatorViewEventT, SpectatorResultT>> out = new java.util.ArrayList<>();
        for (Event<PlayerT, ActionT, GameEventT, GameResultT> event : gameEvents) {
            if (event instanceof Event.InGame<PlayerT, ActionT, GameEventT, GameResultT> ig) {
                deriveSpectatorEvent(gameState.state, ig.event())
                        .map(ev -> new SpectatorEvent.Visible<SpectatorViewEventT, SpectatorResultT>(ev))
                        .ifPresent(out::add);
            } else if (event instanceof Event.GameOver<PlayerT, ActionT, GameEventT, GameResultT> go) {
                out.add(
                        new SpectatorEvent.Terminal<>(
                                deriveSpectatorResult(gameState.state, go.result())));
            }
        }
        return out;
    }
}

