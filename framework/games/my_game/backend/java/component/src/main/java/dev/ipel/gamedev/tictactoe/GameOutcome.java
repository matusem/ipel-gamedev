package dev.ipel.gamedev.tictactoe;

/** Authoritative end state (mirrors Rust {@code GameOutcome}). */
public sealed interface GameOutcome permits GameOutcome.Win, GameOutcome.Draw {

    record Win(Player winner) implements GameOutcome {}

    record Draw() implements GameOutcome {}
}
