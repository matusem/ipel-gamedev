package sk.upjs.gdd.tictactoe;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonValue;

/** Per-player end view; serde matches Rust (string in {@code {"GameOver":"Win"}}). */
public enum PlayerOutcome {
    Win,
    Loss,
    Draw;

    @JsonValue
    public String toWire() {
        return name();
    }

    @JsonCreator
    public static PlayerOutcome fromWire(String s) {
        return PlayerOutcome.valueOf(s);
    }
}
