package dev.ipel.gamedev.tictactoe;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonProperty;

/** Per-player view (mirrors Rust {@code PlayerState}); holds a copy of {@link State}. */
public final class TttPlayerState {
    public final Player player;
    public State state;

    @JsonCreator
    public TttPlayerState(@JsonProperty("player") Player player, @JsonProperty("state") State state) {
        this.player = player;
        this.state = state;
    }
}
