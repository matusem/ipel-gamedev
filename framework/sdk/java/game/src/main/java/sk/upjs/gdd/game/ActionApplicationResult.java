package sk.upjs.gdd.game;

import java.util.List;
import java.util.Map;
import java.util.Objects;

public record ActionApplicationResult<
                PlayerT,
                PlayerViewEventT,
                PlayerResultT,
                SpectatorViewEventT,
                SpectatorResultT>(
        Map<PlayerT, List<PlayerEvent<PlayerViewEventT, PlayerResultT>>> playerEvents,
        List<SpectatorEvent<SpectatorViewEventT, SpectatorResultT>> spectatorEvents) {
    public ActionApplicationResult {
        Objects.requireNonNull(playerEvents);
        Objects.requireNonNull(spectatorEvents);
    }
}

