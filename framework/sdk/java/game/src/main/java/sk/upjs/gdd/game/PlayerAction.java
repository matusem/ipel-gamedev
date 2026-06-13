package sk.upjs.gdd.game;

import java.util.Objects;

/** Player + action (mirrors Rust {@code PlayerAction}). */
public final class PlayerAction<PlayerT, ActionT> {
    public final PlayerT player;
    public final ActionT action;

    public PlayerAction(PlayerT player, ActionT action) {
        this.player = Objects.requireNonNull(player);
        this.action = Objects.requireNonNull(action);
    }

    public PlayerAction<PlayerT, ActionT> copy() {
        return new PlayerAction<>(player, action);
    }
}
