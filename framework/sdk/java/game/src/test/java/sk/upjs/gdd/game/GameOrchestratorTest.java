package sk.upjs.gdd.game;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import org.junit.jupiter.api.Test;

class GameOrchestratorTest {

    record Cfg(List<String> players, int max) {}

    static final class St {
        int turn; // 0 or 1
        int moves;
    }

    record Move() {}

    record Ev(int moves) {}

    record PR(String o) {}

    static final class Rules implements GameRules<Cfg, St, Move, String, St, Ev, Ev, String, PR> {
        @Override
        public Optional<byte[]> validateConfig(Cfg config) {
            if (config.max() < 1) {
                return Optional.of("bad".getBytes(StandardCharsets.UTF_8));
            }
            return Optional.empty();
        }

        @Override
        public Cfg copyConfig(Cfg config) {
            return new Cfg(List.copyOf(config.players()), config.max());
        }

        @Override
        public List<String> players(Cfg config) {
            return config.players();
        }

        @Override
        public St init(Cfg config) {
            return new St();
        }

        @Override
        public St initPlayerState(Cfg config, String player, St sharedGameState) {
            return sharedGameState;
        }

        @Override
        public Optional<byte[]> canTakeAction(St playerState, Move action) {
            return Optional.empty();
        }

        @Override
        public List<Ev> takeAction(St state, PlayerAction<String, Move> playerAction) {
            state.moves++;
            state.turn = 1 - state.turn;
            return List.of(new Ev(state.moves));
        }

        @Override
        public Optional<String> checkGameOver(St state) {
            if (state.moves >= 2) {
                return Optional.of("done");
            }
            return Optional.empty();
        }

        @Override
        public Optional<Ev> derivePlayerEvent(St state, String viewer, InGameEvent<String, Move, Ev> event) {
            return Optional.of(new Ev(state.moves));
        }

        @Override
        public PR derivePlayerResult(St state, String viewer, String result) {
            return new PR(viewer.equals("a") ? "win" : "loss");
        }

        @Override
        public void applyPlayerEvent(St playerState, Ev event) {}
    }

    @Test
    void tryInitAndApply() {
        Rules rules = new Rules();
        Cfg cfg = new Cfg(List.of("a", "b"), 3);
        var init = GameOrchestrator.tryInitWithPlayers(cfg, rules);
        assertTrue(init.isOk());
        var bundle = init.ok();
        assertEquals(2, bundle.playerStates().size());
        St psa = bundle.playerStates().get(0).state();
        var pa = new PlayerAction<>("a", new Move());
        var applied = GameOrchestrator.applyAction(bundle.fullState(), pa, psa, rules);
        assertTrue(applied.isOk());
    }
}
