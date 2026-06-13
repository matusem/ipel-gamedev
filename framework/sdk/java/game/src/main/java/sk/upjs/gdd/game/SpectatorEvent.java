package sk.upjs.gdd.game;

/** Public observer notification (mirrors Rust {@code SpectatorEvent}). */
public sealed interface SpectatorEvent<SpectatorViewEventT, SpectatorResultT>
        permits SpectatorEvent.Visible, SpectatorEvent.Terminal {

    record Visible<SpectatorViewEventT, SpectatorResultT>(SpectatorViewEventT event)
            implements SpectatorEvent<SpectatorViewEventT, SpectatorResultT> {}

    record Terminal<SpectatorViewEventT, SpectatorResultT>(SpectatorResultT result)
            implements SpectatorEvent<SpectatorViewEventT, SpectatorResultT> {}
}

