package sk.upjs.gdd.tictactoe;

/** Visible move (mirrors Rust domain {@code PlayerEvent} struct). */
public record MoveEvent(Player player, Position action) {}
