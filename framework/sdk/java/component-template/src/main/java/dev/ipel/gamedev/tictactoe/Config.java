package dev.ipel.gamedev.tictactoe;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;
import java.nio.charset.StandardCharsets;
import java.util.Optional;

@JsonIgnoreProperties(ignoreUnknown = true)
public class Config implements Cloneable {
    @JsonProperty("side_length")
    public int sideLength = 3;

    @JsonProperty("win_length")
    public int winLength = 3;

    @Override
    public Config clone() {
        try {
            return (Config) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new IllegalStateException(e);
        }
    }

    public Optional<byte[]> validate() {
        final int maxSide = 20;
        if (sideLength < 2) {
            return Optional.of("side_length must be at least 2".getBytes(StandardCharsets.UTF_8));
        }
        if (sideLength > maxSide) {
            return Optional.of(("side_length must be at most " + maxSide).getBytes(StandardCharsets.UTF_8));
        }
        if (winLength < 2) {
            return Optional.of("win_length must be at least 2".getBytes(StandardCharsets.UTF_8));
        }
        if (winLength > sideLength) {
            return Optional.of("win_length cannot exceed side_length".getBytes(StandardCharsets.UTF_8));
        }
        return Optional.empty();
    }
}
