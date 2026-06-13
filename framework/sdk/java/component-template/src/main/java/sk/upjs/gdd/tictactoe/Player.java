package sk.upjs.gdd.tictactoe;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonValue;

public enum Player {
    X,
    O;

    @JsonValue
    public String toWire() {
        return name();
    }

    @JsonCreator
    public static Player fromWire(String s) {
        return Player.valueOf(s);
    }

    public Player other() {
        return this == X ? O : X;
    }
}
