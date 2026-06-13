package sk.upjs.gdd.game;

import java.util.List;
import java.util.Optional;

/**
 * Per-game rules (mirrors Rust {@code Config} + {@code GameCore} + {@code PlayerState} surface used by
 * {@code apply_action}).
 */
public interface GameRules<
        ConfigT,
        StateT,
        ActionT,
        PlayerT,
        PlayerStateT,
        GameEventT,
        PlayerViewEventT,
        GameResultT,
        PlayerResultT> {

    Optional<byte[]> validateConfig(ConfigT config);

    ConfigT copyConfig(ConfigT config);

    List<PlayerT> players(ConfigT config);

    StateT init(ConfigT config);

    /**
     * Per-player view state. {@code sharedGameState} is the same instance stored in {@link FullState#state}; games
     * with perfect information may keep a reference to it, while hidden-information games should copy.
     */
    PlayerStateT initPlayerState(ConfigT config, PlayerT player, StateT sharedGameState);

    /** Empty if the action is allowed; otherwise a serialized domain error (passed as {@code game-core} WIT variant). */
    Optional<byte[]> canTakeAction(PlayerStateT playerState, ActionT action);

    List<GameEventT> takeAction(StateT state, PlayerAction<PlayerT, ActionT> playerAction);

    Optional<GameResultT> checkGameOver(StateT state);

    Optional<PlayerViewEventT> derivePlayerEvent(
            StateT state, PlayerT viewer, InGameEvent<PlayerT, ActionT, GameEventT> event);

    PlayerResultT derivePlayerResult(StateT state, PlayerT viewer, GameResultT result);

    /** Apply a visible per-player event to that player's view state (mirrors Rust {@code PlayerState::apply_event}). */
    void applyPlayerEvent(PlayerStateT playerState, PlayerViewEventT event);
}
