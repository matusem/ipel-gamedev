package wit.worlds;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;
import dev.ipel.gamedev.game.PlayerEvent;
import dev.ipel.gamedev.tictactoe.MoveEvent;
import dev.ipel.gamedev.tictactoe.PlayerOutcome;

/** Serde JSON matching Rust {@code PlayerEvent} (externally tagged {@code Event} / {@code GameOver}). */
final class EventEncoding {

    private EventEncoding() {}

    static byte[] playerEvent(ObjectMapper mapper, PlayerEvent<MoveEvent, PlayerOutcome> pe) {
        try {
            ObjectNode root = mapper.createObjectNode();
            if (pe instanceof PlayerEvent.Visible<MoveEvent, PlayerOutcome> v) {
                root.set("Event", mapper.valueToTree(v.event()));
            } else if (pe instanceof PlayerEvent.Terminal<MoveEvent, PlayerOutcome> t) {
                root.set("GameOver", mapper.valueToTree(t.result()));
            }
            return mapper.writeValueAsBytes(root);
        } catch (com.fasterxml.jackson.core.JsonProcessingException e) {
            throw new IllegalStateException(e);
        }
    }
}
