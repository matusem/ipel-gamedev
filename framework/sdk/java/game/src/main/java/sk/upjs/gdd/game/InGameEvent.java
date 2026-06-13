package sk.upjs.gdd.game;

/** In-game event: either the action line or a domain event (mirrors Rust {@code InGameEvent}). */
public sealed interface InGameEvent<PlayerT, ActionT, GameEventT>
        permits InGameEvent.PlayerActionEvent, InGameEvent.DomainEvent {

    record PlayerActionEvent<PlayerT, ActionT, GameEventT>(PlayerAction<PlayerT, ActionT> action)
            implements InGameEvent<PlayerT, ActionT, GameEventT> {}

    record DomainEvent<PlayerT, ActionT, GameEventT>(GameEventT event)
            implements InGameEvent<PlayerT, ActionT, GameEventT> {}
}
