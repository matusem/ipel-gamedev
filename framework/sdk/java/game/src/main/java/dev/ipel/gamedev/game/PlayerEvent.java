package dev.ipel.gamedev.game;

/** Per-player notification (mirrors Rust {@code PlayerEvent}). */
public sealed interface PlayerEvent<PlayerViewEventT, PlayerResultT>
        permits PlayerEvent.Visible, PlayerEvent.Terminal {

    record Visible<PlayerViewEventT, PlayerResultT>(PlayerViewEventT event)
            implements PlayerEvent<PlayerViewEventT, PlayerResultT> {}

    record Terminal<PlayerViewEventT, PlayerResultT>(PlayerResultT result)
            implements PlayerEvent<PlayerViewEventT, PlayerResultT> {}
}
