package dev.ipel.gamedev.game;

import java.util.List;
import java.util.Objects;

public record InitBundle<ConfigT, StateT, ActionT, PlayerT, PlayerStateT>(
        FullState<ConfigT, StateT, ActionT, PlayerT> fullState,
        List<PlayerStateEntry<PlayerT, PlayerStateT>> playerStates) {
    public InitBundle {
        Objects.requireNonNull(fullState);
        Objects.requireNonNull(playerStates);
    }
}
