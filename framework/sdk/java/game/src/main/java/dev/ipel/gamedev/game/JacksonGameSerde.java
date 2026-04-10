package dev.ipel.gamedev.game;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;

final class JacksonGameSerde implements GameSerde {
    private final ObjectMapper mapper;

    JacksonGameSerde(ObjectMapper mapper) {
        this.mapper = mapper;
    }

    @Override
    public ObjectMapper jacksonMapper() {
        return mapper;
    }

    @Override
    public <ValueT> ValueT deserialize(Class<ValueT> type, byte[] data) throws SerializationException {
        try {
            return mapper.readValue(data, type);
        } catch (Exception e) {
            throw new SerializationException(e.getMessage(), e);
        }
    }

    @Override
    public byte[] serialize(Object value) throws SerializationException {
        try {
            return mapper.writeValueAsBytes(value);
        } catch (JsonProcessingException e) {
            throw new SerializationException(e.getMessage(), e);
        }
    }
}
