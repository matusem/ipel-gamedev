use common::SerializationFormat;
use rmp_serde::{Deserializer as RmpDeserializer, Serializer as RmpSerializer};

pub fn write_buffer<T: serde::Serialize>(
    data: &T,
    output_pointer: *mut u32,
    serialize: impl FnOnce(&T, &mut Vec<u8>) -> Result<(), String>,
) -> usize {
    let mut buffer = Vec::new();
    let _ = serialize(data, &mut buffer);
    let pointer = buffer.as_mut_ptr();
    let length = buffer.len();
    std::mem::forget(buffer);

    unsafe {
        *(output_pointer) = pointer as u32;
    }

    length
}

pub fn rmp_serialize<T: serde::Serialize>(data: &T, buffer: &mut Vec<u8>) -> Result<(), String> {
    let mut serializer = RmpSerializer::new(buffer);
    data.serialize(&mut serializer)
        .map_err(|e| format!("Serialization to MessagePack failed: {}", e))
}

pub fn json_serialize<T: serde::Serialize>(data: &T, buffer: &mut Vec<u8>) -> Result<(), String> {
    let json_string =
        serde_json::to_string(data).map_err(|e| format!("Serialization to JSON failed: {}", e))?;
    buffer.extend_from_slice(json_string.as_bytes());
    Ok(())
}

pub fn get_serialize<T: serde::Serialize>(
    serialization_format: SerializationFormat,
) -> impl FnOnce(&T, &mut Vec<u8>) -> Result<(), String> {
    match serialization_format {
        SerializationFormat::Rmp => rmp_serialize::<T>,
        SerializationFormat::Json => json_serialize::<T>,
    }
}

pub fn read_buffer(input_pointer: *const u8, length: usize) -> Result<Vec<u8>, String> {
    if input_pointer.is_null() || length == 0 {
        return Err("Input pointer is null or length is zero".to_string());
    }

    let data = unsafe { std::slice::from_raw_parts(input_pointer, length) };
    Ok(data.to_vec())
}

pub fn deserialize_buffer<T: serde::de::DeserializeOwned>(
    input_pointer: *const u8,
    length: usize,
    deserialize: impl FnOnce(&[u8]) -> Result<T, String>,
) -> Result<T, String> {
    let data = read_buffer(input_pointer, length)?;
    deserialize(&data)
}

pub fn rmp_deserialize<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    let mut deserializer = RmpDeserializer::new(data);
    T::deserialize(&mut deserializer)
        .map_err(|e| format!("Deserialization from MessagePack failed: {}", e))
}

pub fn json_deserialize<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    serde_json::from_slice(data).map_err(|e| format!("Deserialization from JSON failed: {}", e))
}

pub fn get_deserialize<T: serde::de::DeserializeOwned>(
    serialization_format: SerializationFormat,
) -> impl FnOnce(&[u8]) -> Result<T, String> {
    match serialization_format {
        SerializationFormat::Rmp => rmp_deserialize::<T>,
        SerializationFormat::Json => json_deserialize::<T>,
    }
}
