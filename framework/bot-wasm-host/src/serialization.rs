use crate::bindings::SerializationFormat;
use rmp_serde::{Deserializer as RmpDeserializer, Serializer as RmpSerializer};

pub fn rmp_serialize<T: serde::Serialize>(data: &T) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    let mut serializer = RmpSerializer::new(&mut buffer);
    data.serialize(&mut serializer)
        .map_err(|error| format!("Serialization to MessagePack failed: {}", error))?;
    Ok(buffer)
}

pub fn json_serialize<T: serde::Serialize>(data: &T) -> Result<Vec<u8>, String> {
    let json_string = serde_json::to_string(data)
        .map_err(|error| format!("Serialization to JSON failed: {}", error))?;
    Ok(json_string.into_bytes())
}

pub fn get_serialize<T: serde::Serialize>(
    serialization_format: SerializationFormat,
) -> impl Fn(&T) -> Result<Vec<u8>, String> {
    match serialization_format {
        SerializationFormat::MessagePack => rmp_serialize::<T>,
        SerializationFormat::Json => json_serialize::<T>,
    }
}

pub fn rmp_deserialize<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    let mut deserializer = RmpDeserializer::new(data);
    T::deserialize(&mut deserializer)
        .map_err(|error| format!("Deserialization from MessagePack failed: {}", error))
}

pub fn json_deserialize<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    serde_json::from_slice(data)
        .map_err(|error| format!("Deserialization from JSON failed: {}", error))
}

pub fn get_deserialize<T: serde::de::DeserializeOwned>(
    serialization_format: SerializationFormat,
) -> impl Fn(&[u8]) -> Result<T, String> {
    match serialization_format {
        SerializationFormat::MessagePack => rmp_deserialize::<T>,
        SerializationFormat::Json => json_deserialize::<T>,
    }
}
