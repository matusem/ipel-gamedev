package sk.upjs.gdd.game;

import com.fasterxml.jackson.databind.ObjectMapper;

/** Serialize/deserialize payloads for the WIT {@code buffer} fields. */
public interface GameSerde {

    /** Underlying Jackson mapper (for edge cases like Rust-externally-tagged enums). */
    ObjectMapper jacksonMapper();

    <ValueT> ValueT deserialize(Class<ValueT> type, byte[] data) throws SerializationException;

    byte[] serialize(Object value) throws SerializationException;

    final class SerializationException extends Exception {
        public SerializationException(String message, Throwable cause) {
            super(message, cause);
        }
    }
}
