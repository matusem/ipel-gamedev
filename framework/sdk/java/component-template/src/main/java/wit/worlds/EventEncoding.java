package wit.worlds;

import dev.ipel.gamedev.game.PlayerEvent;
import dev.ipel.gamedev.game.SpectatorEvent;
import dev.ipel.gamedev.tictactoe.GameOutcome;
import dev.ipel.gamedev.tictactoe.MoveEvent;
import dev.ipel.gamedev.tictactoe.PlayerOutcome;

/** Serde JSON matching Rust {@code PlayerEvent} (externally tagged {@code Event} / {@code GameOver}). */
final class EventEncoding {

    private EventEncoding() {}

    static byte[] playerEvent(PlayerEvent<MoveEvent, PlayerOutcome> pe) {
        return TeaVmJson.playerEvent(pe);
    }

    static byte[] spectatorEvent(SpectatorEvent<MoveEvent, GameOutcome> se) {
        return TeaVmJson.spectatorEvent(se);
    }
}
