package wit.worlds;

import dev.ipel.gamedev.tictactoe.Player;
import dev.ipel.gamedev.tictactoe.Position;

public record PlayerActionWire(Player player, Position action) {}
