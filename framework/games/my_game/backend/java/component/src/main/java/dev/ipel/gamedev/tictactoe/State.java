package dev.ipel.gamedev.tictactoe;

import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.ArrayList;

public final class State implements Cloneable {
    public Config config;
    @JsonProperty("current_player")
    public Player currentPlayer;
    /** Length {@code sideLength * sideLength}; {@code null} entries are empty cells. */
    public ArrayList<Player> board = new ArrayList<>();

    /** For Jackson */
    public State() {}

    public static State initial(Config cfg) {
        State s = new State();
        s.config = cfg.clone();
        s.currentPlayer = Player.X;
        int n = cfg.sideLength * cfg.sideLength;
        s.board = new ArrayList<>(n);
        for (int i = 0; i < n; i++) {
            s.board.add(null);
        }
        return s;
    }

    public Player getCell(Position pos) {
        int i = pos.toIndex(config.sideLength);
        if (i < 0 || i >= board.size()) {
            return null;
        }
        return board.get(i);
    }

    public void setCell(Position pos, Player p) {
        int i = pos.toIndex(config.sideLength);
        if (i >= 0 && i < board.size()) {
            board.set(i, p);
        }
    }

    public boolean isFull() {
        for (Player c : board) {
            if (c == null) {
                return false;
            }
        }
        return true;
    }

    @Override
    public State clone() {
        State s = new State();
        s.config = config != null ? config.clone() : null;
        s.currentPlayer = currentPlayer;
        s.board = new ArrayList<>(board);
        return s;
    }
}
