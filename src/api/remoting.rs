//! RPC protocol types and serialization

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Cursor, Read, Write};

use crate::api::{Action, Channel, Contact, Peer, Profile};
use crate::error::{RatNetError, Result};

/// Remote procedure call structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteCall {
    pub action: Action,
    pub args: Vec<Value>,
}

/// Response from a remote procedure call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteResponse {
    pub error: Option<String>,
    pub value: Option<Value>,
}

impl RemoteResponse {
    /// Create a successful response
    pub fn success(value: Value) -> Self {
        Self {
            error: None,
            value: Some(value),
        }
    }

    /// Create an error response
    pub fn error(error: String) -> Self {
        Self {
            error: Some(error),
            value: None,
        }
    }

    /// Check if this response is nil
    pub fn is_nil(&self) -> bool {
        self.value.is_none()
    }

    /// Check if this response is an error
    pub fn is_err(&self) -> bool {
        self.error.is_some()
    }
}

/// API parameter data types for serialization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum APIType {
    Invalid = 0x0,
    Nil = 0x1,
    Int64 = 0x2,
    Uint64 = 0x3,
    String = 0x4,
    Bytes = 0x5,
    BytesBytes = 0x6,
    InterfaceArray = 0x7,

    PubKeyECC = 0x10,
    PubKeyRSA = 0x11,

    ContactArray = 0x20,
    ChannelArray = 0x21,
    ProfileArray = 0x22,
    PeerArray = 0x23,

    Contact = 0x30,
    Channel = 0x31,
    Profile = 0x32,
    Peer = 0x33,

    Bundle = 0x40,
}

impl From<u8> for APIType {
    fn from(value: u8) -> Self {
        match value {
            0x0 => APIType::Invalid,
            0x1 => APIType::Nil,
            0x2 => APIType::Int64,
            0x3 => APIType::Uint64,
            0x4 => APIType::String,
            0x5 => APIType::Bytes,
            0x6 => APIType::BytesBytes,
            0x7 => APIType::InterfaceArray,
            0x10 => APIType::PubKeyECC,
            0x11 => APIType::PubKeyRSA,
            0x20 => APIType::ContactArray,
            0x21 => APIType::ChannelArray,
            0x22 => APIType::ProfileArray,
            0x23 => APIType::PeerArray,
            0x30 => APIType::Contact,
            0x31 => APIType::Channel,
            0x32 => APIType::Profile,
            0x33 => APIType::Peer,
            0x40 => APIType::Bundle,
            _ => APIType::Invalid,
        }
    }
}

/// Convert arguments to bytes for transport using binary serialization
pub fn args_to_bytes(args: &[Value]) -> Result<Bytes> {
    let mut buffer = Vec::new();
    serialize(&mut buffer, &Value::Array(args.to_vec()))?;
    Ok(Bytes::from(buffer))
}

/// Convert bytes back to arguments using binary deserialization
pub fn args_from_bytes(data: &[u8]) -> Result<Vec<Value>> {
    let mut cursor = Cursor::new(data);
    let result = deserialize(&mut cursor)?;
    match result {
        Value::Array(arr) => Ok(arr),
        _ => Err(RatNetError::Serialization("Expected array".to_string())),
    }
}

/// Convert a RemoteCall to bytes
pub fn remote_call_to_bytes(call: &RemoteCall) -> Result<Bytes> {
    let mut buffer = Vec::new();

    // Action - first byte
    buffer.write_u8(call.action as u8)?;

    // Args - serialize and append
    let args_bytes = args_to_bytes(&call.args)?;
    buffer.extend_from_slice(&args_bytes);

    Ok(Bytes::from(buffer))
}

/// Convert bytes to a RemoteCall
pub fn remote_call_from_bytes(data: &[u8]) -> Result<RemoteCall> {
    if data.len() < 2 {
        return Err(RatNetError::Serialization("Input too short".to_string()));
    }

    let action = Action::from(data[0]);
    let args = if data.len() > 1 {
        args_from_bytes(&data[1..])?
    } else {
        Vec::new()
    };

    Ok(RemoteCall { action, args })
}

/// Convert a RemoteResponse to bytes
pub fn remote_response_to_bytes(response: &RemoteResponse) -> Result<Bytes> {
    let mut buffer = Vec::new();
    serialize(
        &mut buffer,
        &response
            .error
            .as_ref()
            .map(|s| Value::String(s.clone()))
            .unwrap_or(Value::Null),
    )?;
    serialize(
        &mut buffer,
        &response.value.as_ref().unwrap_or(&Value::Null),
    )?;
    Ok(Bytes::from(buffer))
}

/// Convert bytes to a RemoteResponse
pub fn remote_response_from_bytes(data: &[u8]) -> Result<RemoteResponse> {
    let mut cursor = Cursor::new(data);

    // Read error string
    let error = deserialize(&mut cursor)?;
    let error = if error == Value::Null {
        None
    } else {
        Some(error.as_str().unwrap_or("").to_string())
    };

    // Read value
    let value = deserialize(&mut cursor)?;
    let value = if value == Value::Null {
        None
    } else {
        Some(value)
    };

    Ok(RemoteResponse { error, value })
}

/// Convert array of byte arrays to bytes
pub fn bytes_bytes_to_bytes(bba: &[Vec<u8>]) -> Result<Bytes> {
    let mut buffer = Vec::new();
    let bba_value = Value::Array(
        bba.iter()
            .map(|bytes| Value::String(base64::encode(bytes)))
            .collect(),
    );
    serialize(&mut buffer, &bba_value)?;
    Ok(Bytes::from(buffer))
}

/// Convert bytes to array of byte arrays
pub fn bytes_bytes_from_bytes(data: &[u8]) -> Result<Vec<Vec<u8>>> {
    let mut cursor = Cursor::new(data);
    let result = deserialize(&mut cursor)?;
    match result {
        Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                if let Some(bytes) = item.as_str().map(|s| base64::decode(s).unwrap_or_default()) {
                    result.push(bytes);
                }
            }
            Ok(result)
        }
        _ => Err(RatNetError::Serialization("Expected array".to_string())),
    }
}

/// Serialize a value to binary format
fn serialize(w: &mut Vec<u8>, v: &Value) -> Result<()> {
    match v {
        Value::Null => {
            w.write_u8(APIType::Nil as u8)?;
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                w.write_u8(APIType::Int64 as u8)?;
                w.write_i64::<BigEndian>(i)?;
            } else if let Some(u) = n.as_u64() {
                w.write_u8(APIType::Uint64 as u8)?;
                w.write_u64::<BigEndian>(u)?;
            } else {
                return Err(RatNetError::Serialization(
                    "Unsupported number type".to_string(),
                ));
            }
        }
        Value::String(s) => {
            w.write_u8(APIType::String as u8)?;
            write_lv(w, s.as_bytes())?;
        }
        Value::Array(arr) => {
            w.write_u8(APIType::InterfaceArray as u8)?;
            let mut inner_buffer = Vec::new();
            let len_buf = varint_encode(arr.len() as u64)?;
            inner_buffer.extend_from_slice(&len_buf);
            for item in arr {
                serialize(&mut inner_buffer, item)?;
            }
            write_lv(w, &inner_buffer)?;
        }
        _ => {
            // For other types, serialize as JSON string
            let json_str = serde_json::to_string(v).map_err(|e| {
                RatNetError::Serialization(format!("JSON serialization failed: {}", e))
            })?;
            w.write_u8(APIType::String as u8)?;
            write_lv(w, json_str.as_bytes())?;
        }
    }
    Ok(())
}

/// Deserialize a value from binary format
fn deserialize(r: &mut Cursor<&[u8]>) -> Result<Value> {
    let typ = r.read_u8()?;
    let api_type = APIType::from(typ);

    match api_type {
        APIType::Nil => Ok(Value::Null),
        APIType::Int64 => {
            let value = r.read_i64::<BigEndian>()?;
            Ok(Value::Number(serde_json::Number::from(value)))
        }
        APIType::Uint64 => {
            let value = r.read_u64::<BigEndian>()?;
            Ok(Value::Number(serde_json::Number::from(value)))
        }
        APIType::String => {
            let data = read_lv(r)?;
            let s = String::from_utf8(data)
                .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
            Ok(Value::String(s))
        }
        APIType::Bytes => {
            let data = read_lv(r)?;
            Ok(Value::String(base64::encode(data)))
        }
        APIType::BytesBytes => {
            let data = read_lv(r)?;
            let mut cursor = Cursor::new(data.as_slice());
            let (len, _) = varint_decode(&mut cursor)?;
            let mut result = Vec::new();
            for _ in 0..len {
                let item_data = read_lv(&mut cursor)?;
                result.push(Value::String(base64::encode(item_data)));
            }
            Ok(Value::Array(result))
        }
        APIType::InterfaceArray => {
            let data = read_lv(r)?;
            let mut cursor = Cursor::new(data.as_slice());
            let (len, _) = varint_decode(&mut cursor)?;
            let mut result = Vec::new();
            for _ in 0..len {
                let item = deserialize(&mut cursor)?;
                result.push(item);
            }
            Ok(Value::Array(result))
        }
        APIType::Contact => {
            let data = read_lv(r)?;
            let mut cursor = Cursor::new(data.as_slice());
            let name_data = read_lv(&mut cursor)?;
            let pubkey_data = read_lv(&mut cursor)?;
            let name = String::from_utf8(name_data)
                .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
            let pubkey = String::from_utf8(pubkey_data)
                .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;

            let mut contact = serde_json::Map::new();
            contact.insert("name".to_string(), Value::String(name));
            contact.insert("pubkey".to_string(), Value::String(pubkey));
            Ok(Value::Object(contact))
        }
        APIType::ContactArray => {
            let data = read_lv(r)?;
            let mut cursor = Cursor::new(data.as_slice());
            let (len, _) = varint_decode(&mut cursor)?;
            let mut result = Vec::new();
            for _ in 0..len {
                let name_data = read_lv(&mut cursor)?;
                let pubkey_data = read_lv(&mut cursor)?;
                let name = String::from_utf8(name_data)
                    .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
                let pubkey = String::from_utf8(pubkey_data)
                    .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;

                let mut contact = serde_json::Map::new();
                contact.insert("name".to_string(), Value::String(name));
                contact.insert("pubkey".to_string(), Value::String(pubkey));
                result.push(Value::Object(contact));
            }
            Ok(Value::Array(result))
        }
        APIType::Channel => {
            let data = read_lv(r)?;
            let mut cursor = Cursor::new(data.as_slice());
            let name_data = read_lv(&mut cursor)?;
            let pubkey_data = read_lv(&mut cursor)?;
            let name = String::from_utf8(name_data)
                .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
            let pubkey = String::from_utf8(pubkey_data)
                .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;

            let mut channel = serde_json::Map::new();
            channel.insert("name".to_string(), Value::String(name));
            channel.insert("pubkey".to_string(), Value::String(pubkey));
            Ok(Value::Object(channel))
        }
        APIType::ChannelArray => {
            let data = read_lv(r)?;
            let mut cursor = Cursor::new(data.as_slice());
            let (len, _) = varint_decode(&mut cursor)?;
            let mut result = Vec::new();
            for _ in 0..len {
                let name_data = read_lv(&mut cursor)?;
                let pubkey_data = read_lv(&mut cursor)?;
                let name = String::from_utf8(name_data)
                    .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
                let pubkey = String::from_utf8(pubkey_data)
                    .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;

                let mut channel = serde_json::Map::new();
                channel.insert("name".to_string(), Value::String(name));
                channel.insert("pubkey".to_string(), Value::String(pubkey));
                result.push(Value::Object(channel));
            }
            Ok(Value::Array(result))
        }
        APIType::Profile => {
            let data = read_lv(r)?;
            let mut cursor = Cursor::new(data.as_slice());
            let name_data = read_lv(&mut cursor)?;
            let pubkey_data = read_lv(&mut cursor)?;
            let enabled_byte = cursor.read_u8()?;

            let name = String::from_utf8(name_data)
                .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
            let pubkey = String::from_utf8(pubkey_data)
                .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
            let enabled = enabled_byte == 1;

            let mut profile = serde_json::Map::new();
            profile.insert("name".to_string(), Value::String(name));
            profile.insert("pubkey".to_string(), Value::String(pubkey));
            profile.insert("enabled".to_string(), Value::Bool(enabled));
            Ok(Value::Object(profile))
        }
        APIType::ProfileArray => {
            let data = read_lv(r)?;
            let mut cursor = Cursor::new(data.as_slice());
            let (len, _) = varint_decode(&mut cursor)?;
            let mut result = Vec::new();
            for _ in 0..len {
                let name_data = read_lv(&mut cursor)?;
                let pubkey_data = read_lv(&mut cursor)?;
                let enabled_byte = cursor.read_u8()?;

                let name = String::from_utf8(name_data)
                    .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
                let pubkey = String::from_utf8(pubkey_data)
                    .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
                let enabled = enabled_byte == 1;

                let mut profile = serde_json::Map::new();
                profile.insert("name".to_string(), Value::String(name));
                profile.insert("pubkey".to_string(), Value::String(pubkey));
                profile.insert("enabled".to_string(), Value::Bool(enabled));
                result.push(Value::Object(profile));
            }
            Ok(Value::Array(result))
        }
        APIType::Peer => {
            let data = read_lv(r)?;
            let mut cursor = Cursor::new(data.as_slice());
            let name_data = read_lv(&mut cursor)?;
            let group_data = read_lv(&mut cursor)?;
            let uri_data = read_lv(&mut cursor)?;
            let enabled_byte = cursor.read_u8()?;

            let name = String::from_utf8(name_data)
                .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
            let group = String::from_utf8(group_data)
                .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
            let uri = String::from_utf8(uri_data)
                .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
            let enabled = enabled_byte == 1;

            let mut peer = serde_json::Map::new();
            peer.insert("name".to_string(), Value::String(name));
            peer.insert("group".to_string(), Value::String(group));
            peer.insert("uri".to_string(), Value::String(uri));
            peer.insert("enabled".to_string(), Value::Bool(enabled));
            Ok(Value::Object(peer))
        }
        APIType::PeerArray => {
            let data = read_lv(r)?;
            let mut cursor = Cursor::new(data.as_slice());
            let (len, _) = varint_decode(&mut cursor)?;
            let mut result = Vec::new();
            for _ in 0..len {
                let name_data = read_lv(&mut cursor)?;
                let group_data = read_lv(&mut cursor)?;
                let uri_data = read_lv(&mut cursor)?;
                let enabled_byte = cursor.read_u8()?;

                let name = String::from_utf8(name_data)
                    .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
                let group = String::from_utf8(group_data)
                    .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
                let uri = String::from_utf8(uri_data)
                    .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
                let enabled = enabled_byte == 1;

                let mut peer = serde_json::Map::new();
                peer.insert("name".to_string(), Value::String(name));
                peer.insert("group".to_string(), Value::String(group));
                peer.insert("uri".to_string(), Value::String(uri));
                peer.insert("enabled".to_string(), Value::Bool(enabled));
                result.push(Value::Object(peer));
            }
            Ok(Value::Array(result))
        }
        APIType::Bundle => {
            let data = read_lv(r)?;
            let mut cursor = Cursor::new(data.as_slice());
            let bundle_data = read_lv(&mut cursor)?;
            let time = cursor.read_i64::<BigEndian>()?;

            let mut bundle = serde_json::Map::new();
            bundle.insert(
                "data".to_string(),
                Value::String(base64::encode(bundle_data)),
            );
            bundle.insert(
                "time".to_string(),
                Value::Number(serde_json::Number::from(time)),
            );
            Ok(Value::Object(bundle))
        }
        _ => {
            // For unsupported types, try to read as string
            let data = read_lv(r)?;
            let s = String::from_utf8(data)
                .map_err(|e| RatNetError::Serialization(format!("Invalid UTF-8: {}", e)))?;
            Ok(Value::String(s))
        }
    }
}

/// Write TLV (Type-Length-Value) format
fn write_tlv(w: &mut Vec<u8>, typ: APIType, value: &[u8]) -> Result<()> {
    w.write_u8(typ as u8)?;
    if typ != APIType::Nil {
        write_lv(w, value)?;
    }
    Ok(())
}

/// Write Length-Value format
fn write_lv(w: &mut Vec<u8>, value: &[u8]) -> Result<()> {
    let len_buf = varint_encode(value.len() as u64)?;
    w.extend_from_slice(&len_buf);
    w.extend_from_slice(value);
    Ok(())
}

/// Read Length-Value format
fn read_lv(r: &mut Cursor<&[u8]>) -> Result<Vec<u8>> {
    let (len, _) = varint_decode(r)?;
    if len == 0 {
        return Ok(Vec::new());
    }
    let mut v = vec![0u8; len as usize];
    r.read_exact(&mut v)?;
    Ok(v)
}

/// Encode u64 as varint
fn varint_encode(mut value: u64) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    while value >= 0x80 {
        buf.push((value as u8) | 0x80);
        value >>= 7;
    }
    buf.push(value as u8);
    Ok(buf)
}

/// Decode varint to u64
fn varint_decode(r: &mut Cursor<&[u8]>) -> Result<(u64, usize)> {
    let mut result = 0u64;
    let mut shift = 0u32;
    let mut bytes_read = 0usize;

    loop {
        let byte = r.read_u8()?;
        bytes_read += 1;

        if shift >= 64 {
            return Err(RatNetError::Serialization("Varint overflow".to_string()));
        }

        result |= ((byte & 0x7F) as u64) << shift;

        if (byte & 0x80) == 0 {
            break;
        }

        shift += 7;
    }

    Ok((result, bytes_read))
}

/// Read a serialized buffer from the wire
pub fn read_buffer(reader: &mut dyn Read) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 10];
    let mut len_bytes = 0;

    // Read varint length
    loop {
        let byte = reader.read_u8()?;
        len_buf[len_bytes] = byte;
        len_bytes += 1;

        if (byte & 0x80) == 0 {
            break;
        }

        if len_bytes >= 10 {
            return Err(RatNetError::Serialization("Varint too long".to_string()));
        }
    }

    let mut cursor = Cursor::new(&len_buf[..len_bytes]);
    let (rlen, _) = varint_decode(&mut cursor)?;

    // Read the actual data
    let mut buf = vec![0u8; rlen as usize];
    reader.read_exact(&mut buf)?;

    Ok(buf)
}

/// Write a serialized buffer to the wire
pub fn write_buffer(writer: &mut dyn Write, data: &[u8]) -> Result<()> {
    let len_buf = varint_encode(data.len() as u64)?;
    writer.write_all(&len_buf)?;
    writer.write_all(data)?;
    Ok(())
}

/// Helper trait for JSON serialization
pub trait JSON {
    fn to_json(&self) -> Result<String>;
    fn from_json(json: &str) -> Result<Self>
    where
        Self: Sized;
}

/// Implement JSON trait for common types
impl JSON for Value {
    fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(Into::into)
    }

    fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(Into::into)
    }
}

impl JSON for Contact {
    fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(Into::into)
    }

    fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(Into::into)
    }
}

impl JSON for Channel {
    fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(Into::into)
    }

    fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(Into::into)
    }
}

impl JSON for Profile {
    fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(Into::into)
    }

    fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(Into::into)
    }
}

impl JSON for Peer {
    fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(Into::into)
    }

    fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(Into::into)
    }
}
