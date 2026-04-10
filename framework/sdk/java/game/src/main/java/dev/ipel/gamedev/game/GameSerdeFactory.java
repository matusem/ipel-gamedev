package dev.ipel.gamedev.game;

import com.fasterxml.jackson.databind.DeserializationFeature;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.json.JsonMapper;
import com.fasterxml.jackson.databind.PropertyNamingStrategies;
import org.msgpack.jackson.dataformat.MessagePackFactory;

/** Matches Rust serde_json / rmp_serde naming ({@code snake_case} fields). */
public final class GameSerdeFactory {

    private GameSerdeFactory() {}

    public static GameSerde forFormat(SerializationFormat format) {
        return switch (format) {
            case JSON -> new JacksonGameSerde(jsonMapper());
            case MESSAGE_PACK -> new JacksonGameSerde(msgpackMapper());
        };
    }

    public static ObjectMapper jsonMapper() {
        return JsonMapper.builder()
                .propertyNamingStrategy(PropertyNamingStrategies.SNAKE_CASE)
                .configure(DeserializationFeature.FAIL_ON_UNKNOWN_PROPERTIES, false)
                .build();
    }

    public static ObjectMapper msgpackMapper() {
        ObjectMapper m = new ObjectMapper(new MessagePackFactory());
        m.setPropertyNamingStrategy(PropertyNamingStrategies.SNAKE_CASE);
        m.configure(DeserializationFeature.FAIL_ON_UNKNOWN_PROPERTIES, false);
        return m;
    }
}
