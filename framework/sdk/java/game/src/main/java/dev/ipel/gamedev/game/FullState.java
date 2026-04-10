package dev.ipel.gamedev.game;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/** Authoritative full state + action log (mirrors Rust {@code FullState}). */
public final class FullState<ConfigT, StateT, ActionT, PlayerT> {
    public final ConfigT config;
    public StateT state;
    public final List<PlayerAction<PlayerT, ActionT>> actionsMade;

    public FullState(ConfigT config, StateT state, List<PlayerAction<PlayerT, ActionT>> actionsMade) {
        this.config = Objects.requireNonNull(config);
        this.state = Objects.requireNonNull(state);
        this.actionsMade = Objects.requireNonNull(actionsMade);
    }

    public static <ConfigT, StateT, ActionT, PlayerT> FullState<ConfigT, StateT, ActionT, PlayerT> fresh(
            ConfigT config, StateT state) {
        return new FullState<>(config, state, new ArrayList<>());
    }
}
