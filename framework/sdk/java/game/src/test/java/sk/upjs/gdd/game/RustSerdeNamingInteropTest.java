package sk.upjs.gdd.game;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;

/**
 * Locks JSON field naming to serde's default {@code snake_case} for simple structs (see
 * {@code games/my_game} {@code Config} in Rust).
 */
class RustSerdeNamingInteropTest {

    record TicTacToeConfig(int sideLength, int winLength) {}

    @Test
    void ticTacToeDefaultConfigJsonMatchesSerde() throws Exception {
        ObjectMapper m = GameSerdeFactory.jsonMapper();
        TicTacToeConfig cfg = new TicTacToeConfig(3, 3);
        byte[] bytes = m.writeValueAsBytes(cfg);
        // serde_json for Config { side_length: 3, win_length: 3 }
        assertArrayEquals("{\"side_length\":3,\"win_length\":3}".getBytes(java.nio.charset.StandardCharsets.UTF_8), bytes);
        TicTacToeConfig round = m.readValue(bytes, TicTacToeConfig.class);
        assertEquals(3, round.sideLength());
        assertEquals(3, round.winLength());
    }

    @Test
    void messagePackRoundTrip() throws Exception {
        GameSerde serde = GameSerdeFactory.forFormat(SerializationFormat.MESSAGE_PACK);
        TicTacToeConfig cfg = new TicTacToeConfig(3, 3);
        byte[] packed = serde.serialize(cfg);
        TicTacToeConfig back = serde.deserialize(TicTacToeConfig.class, packed);
        assertEquals(3, back.sideLength());
        assertEquals(3, back.winLength());
    }
}
