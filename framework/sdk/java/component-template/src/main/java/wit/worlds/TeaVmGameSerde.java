package wit.worlds;

import com.fasterxml.jackson.databind.ObjectMapper;
import dev.ipel.gamedev.game.GameSerde;
import dev.ipel.gamedev.game.SerializationFormat;

/**
 * TeaVM-safe serde: Jackson/msgpack libraries trigger {@code UnusedFunctionElimination} NPE when
 * {@code wit.worlds.GameCore} WIT exports are preserved (see sdk/java/README.md).
 */
public final class TeaVmGameSerde implements GameSerde {

    private final SerializationFormat format;

    private TeaVmGameSerde(SerializationFormat format) {
        this.format = format;
    }

    public static GameSerde forFormat(SerializationFormat format) {
        return new TeaVmGameSerde(format);
    }

    @Override
    public ObjectMapper jacksonMapper() {
        throw new UnsupportedOperationException("TeaVM guest does not use Jackson");
    }

    @Override
    public <ValueT> ValueT deserialize(Class<ValueT> type, byte[] data) throws SerializationException {
        if (format != SerializationFormat.JSON) {
            throw new SerializationException("MessagePack is not supported in the TeaVM guest", null);
        }
        try {
            return TeaVmJson.deserialize(type, data);
        } catch (RuntimeException e) {
            throw new SerializationException(e.getMessage(), e);
        }
    }

    @Override
    public byte[] serialize(Object value) throws SerializationException {
        if (format != SerializationFormat.JSON) {
            throw new SerializationException("MessagePack is not supported in the TeaVM guest", null);
        }
        try {
            return TeaVmJson.serialize(value);
        } catch (RuntimeException e) {
            throw new SerializationException(e.getMessage(), e);
        }
    }
}
