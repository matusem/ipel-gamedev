package sk.upjs.gdd.tictactoe;

import com.fasterxml.jackson.annotation.JsonFormat;

/** Tuple (row, col); serde matches Rust {@code Position} as JSON array {@code [row, col]}. */
@JsonFormat(shape = JsonFormat.Shape.ARRAY)
public record Position(int row, int col) {
    public int toIndex(int side) {
        return row * side + col;
    }

    public boolean inBounds(int side) {
        return row >= 0 && col >= 0 && row < side && col < side;
    }
}
