package wit.worlds;

import sk.upjs.gdd.game.PlayerEvent;
import sk.upjs.gdd.game.SpectatorEvent;
import sk.upjs.gdd.tictactoe.GameOutcome;
import sk.upjs.gdd.tictactoe.MoveEvent;
import sk.upjs.gdd.tictactoe.PlayerOutcome;

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
