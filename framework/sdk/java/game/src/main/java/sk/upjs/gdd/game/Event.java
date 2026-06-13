package sk.upjs.gdd.game;

/** Game-level event stream entry (mirrors Rust {@code Event}). */
public sealed interface Event<PlayerT, ActionT, GameEventT, GameResultT>
        permits Event.InGame, Event.GameOver {

    record InGame<PlayerT, ActionT, GameEventT, GameResultT>(InGameEvent<PlayerT, ActionT, GameEventT> event)
            implements Event<PlayerT, ActionT, GameEventT, GameResultT> {}

    record GameOver<PlayerT, ActionT, GameEventT, GameResultT>(GameResultT result)
            implements Event<PlayerT, ActionT, GameEventT, GameResultT> {}
}
