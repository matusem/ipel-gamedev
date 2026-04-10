package dev.ipel.gamedev.game;

import java.util.Objects;

public record PlayerStateEntry<PlayerT, PlayerStateT>(PlayerT player, PlayerStateT state) {
    public PlayerStateEntry {
        Objects.requireNonNull(player);
        Objects.requireNonNull(state);
    }
}
