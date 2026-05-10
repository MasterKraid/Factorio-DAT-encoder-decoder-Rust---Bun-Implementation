use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use thiserror::Error;

// ============================================================================
// 1. Core Error System
// ============================================================================

#[derive(Error, Debug)]
pub enum FactorioError {
    #[error("Unexpected end of file in binary stream")]
    Eof,
    #[error("Invalid UTF-8 string encoding: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error("Unknown PropertyTree type ID: {0}")]
    UnknownTypeId(u8),
    #[error("Factorio version identifier format must match 'X.Y.Z.W'")]
    InvalidVersionFormat,
    #[error("JSON parsing failure: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Root property tree node must be a Dictionary type")]
    InvalidRootNode,
    #[error("Parse version integer failure: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
}

// ============================================================================
// 2. Core PropertyTree Enum and Helper Models
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropertyTree {
    None,
    Bool(bool),
    Number(f64),
    String(String),
    List(Vec<PropertyTree>),
    Dictionary(Vec<(String, PropertyTree)>),
    SignedInt(i64),
    UnsignedInt(u64),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FactorioSettingsPayload {
    pub factorio_version: String,
    pub header_bool_flag: u8,
    pub settings: serde_json::Map<String, serde_json::Value>,
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

// ============================================================================
// 3. Memory-Safe Binary Stream Reader
// ============================================================================

pub struct BinaryReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> BinaryReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    pub fn is_eof(&self) -> bool {
        self.offset >= self.data.len()
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], FactorioError> {
        if self.offset + len > self.data.len() {
            return Err(FactorioError::Eof);
        }
        let slice = &self.data[self.offset..self.offset + len];
        self.offset += len;
        Ok(slice)
    }

    pub fn u8(&mut self) -> Result<u8, FactorioError> {
        let b = self.read_bytes(1)?;
        Ok(b[0])
    }

    pub fn bool(&mut self) -> Result<bool, FactorioError> {
        Ok(self.u8()? != 0)
    }

    pub fn u16(&mut self) -> Result<u16, FactorioError> {
        let b = self.read_bytes(2)?;
        let mut arr = [0u8; 2];
        arr.copy_from_slice(b);
        Ok(u16::from_le_bytes(arr))
    }

    pub fn u32(&mut self) -> Result<u32, FactorioError> {
        let b = self.read_bytes(4)?;
        let mut arr = [0u8; 4];
        arr.copy_from_slice(b);
        Ok(u32::from_le_bytes(arr))
    }

    pub fn u64(&mut self) -> Result<u64, FactorioError> {
        let b = self.read_bytes(8)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(b);
        Ok(u64::from_le_bytes(arr))
    }

    pub fn i64(&mut self) -> Result<i64, FactorioError> {
        let b = self.read_bytes(8)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(b);
        Ok(i64::from_le_bytes(arr))
    }

    pub fn f64(&mut self) -> Result<f64, FactorioError> {
        let b = self.read_bytes(8)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(b);
        Ok(f64::from_le_bytes(arr))
    }

    pub fn string(&mut self) -> Result<String, FactorioError> {
        let is_empty = self.bool()?;
        if is_empty {
            return Ok(String::new());
        }

        let mut length = self.u8()? as usize;
        if length == 255 {
            length = self.u32()? as usize;
        }

        let bytes = self.read_bytes(length)?;
        std::str::from_utf8(bytes)
            .map(|s| s.to_string())
            .map_err(FactorioError::from)
    }
}

// ============================================================================
// 4. Memory-Safe Binary Stream Writer
// ============================================================================

#[derive(Default)]
pub struct BinaryWriter {
    data: Vec<u8>,
}

impl BinaryWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    pub fn u8(&mut self, val: u8) {
        self.data.push(val);
    }

    pub fn bool(&mut self, val: bool) {
        self.u8(if val { 1 } else { 0 });
    }

    pub fn u16(&mut self, val: u16) {
        self.write_bytes(&val.to_le_bytes());
    }

    pub fn u32(&mut self, val: u32) {
        self.write_bytes(&val.to_le_bytes());
    }

    pub fn u64(&mut self, val: u64) {
        self.write_bytes(&val.to_le_bytes());
    }

    pub fn i64(&mut self, val: i64) {
        self.write_bytes(&val.to_le_bytes());
    }

    pub fn f64(&mut self, val: f64) {
        self.write_bytes(&val.to_le_bytes());
    }

    pub fn string(&mut self, val: &str) {
        self.u8(0);
        let bytes = val.as_bytes();
        if bytes.len() < 255 {
            self.u8(bytes.len() as u8);
        } else {
            self.u8(255);
            self.u32(bytes.len() as u32);
        }
        self.write_bytes(bytes);
    }

    pub fn bytes(self) -> Vec<u8> {
        self.data
    }
}

// ============================================================================
// 5. PropertyTree Recursive Serializers
// ============================================================================

pub fn decode_property_tree(reader: &mut BinaryReader) -> Result<PropertyTree, FactorioError> {
    let type_id = reader.u8()?;
    let _any_flag = reader.bool()?;

    match type_id {
        0 => Ok(PropertyTree::None),
        1 => {
            let val = reader.bool()?;
            Ok(PropertyTree::Bool(val))
        }
        2 => {
            let val = reader.f64()?;
            Ok(PropertyTree::Number(val))
        }
        3 => {
            let val = reader.string()?;
            Ok(PropertyTree::String(val))
        }
        4 => {
            let count = reader.u32()?;
            let mut list = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let _key = reader.string()?;
                let item = decode_property_tree(reader)?;
                list.push(item);
            }
            Ok(PropertyTree::List(list))
        }
        5 => {
            let count = reader.u32()?;
            let mut map = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let key = reader.string()?;
                let item = decode_property_tree(reader)?;
                map.push((key, item));
            }
            Ok(PropertyTree::Dictionary(map))
        }
        6 => {
            let val = reader.i64()?;
            Ok(PropertyTree::SignedInt(val))
        }
        7 => {
            let val = reader.u64()?;
            Ok(PropertyTree::UnsignedInt(val))
        }
        _ => Err(FactorioError::UnknownTypeId(type_id)),
    }
}

pub fn encode_property_tree(writer: &mut BinaryWriter, node: &PropertyTree) {
    match node {
        PropertyTree::None => {
            writer.u8(0);
            writer.bool(false);
        }
        PropertyTree::Bool(b) => {
            writer.u8(1);
            writer.bool(false);
            writer.bool(*b);
        }
        PropertyTree::Number(f) => {
            writer.u8(2);
            writer.bool(false);
            writer.f64(*f);
        }
        PropertyTree::String(s) => {
            writer.u8(3);
            writer.bool(false);
            writer.string(s);
        }
        PropertyTree::List(items) => {
            writer.u8(4);
            writer.bool(false);
            writer.u32(items.len() as u32);
            for item in items {
                writer.string("");
                encode_property_tree(writer, item);
            }
        }
        PropertyTree::Dictionary(map) => {
            writer.u8(5);
            writer.bool(false);
            writer.u32(map.len() as u32);
            for (k, v) in map {
                writer.string(k);
                encode_property_tree(writer, v);
            }
        }
        PropertyTree::SignedInt(i) => {
            writer.u8(6);
            writer.bool(false);
            writer.i64(*i);
        }
        PropertyTree::UnsignedInt(u) => {
            writer.u8(7);
            writer.bool(false);
            writer.u64(*u);
        }
    }
}

// ============================================================================
// 6. Parallel JSON Flat-Schema Formatting Algorithms
// ============================================================================

fn property_tree_to_json_and_type(node: PropertyTree) -> (serde_json::Value, String) {
    match node {
        PropertyTree::None => (serde_json::Value::Null, "none".to_string()),
        PropertyTree::Bool(b) => (serde_json::Value::Bool(b), "bool".to_string()),
        PropertyTree::Number(f) => (serde_json::json!(f), "number".to_string()),
        PropertyTree::String(s) => (serde_json::Value::String(s), "string".to_string()),
        PropertyTree::SignedInt(i) => (serde_json::json!(i), "signed_int".to_string()),
        PropertyTree::UnsignedInt(u) => (serde_json::json!(u), "unsigned_int".to_string()),
        PropertyTree::List(items) => {
            let arr = items
                .into_iter()
                .map(|item| {
                    let (v, _) = property_tree_to_json_and_type(item);
                    v
                })
                .collect();
            (serde_json::Value::Array(arr), "list".to_string())
        }
        PropertyTree::Dictionary(dict) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in dict.into_iter() {
                let (val, _) = property_tree_to_json_and_type(v);
                obj.insert(k, val);
            }
            (serde_json::Value::Object(obj), "dictionary".to_string())
        }
    }
}

fn infer_json_value_type(val: serde_json::Value) -> PropertyTree {
    match val {
        serde_json::Value::Null => PropertyTree::None,
        serde_json::Value::Bool(b) => PropertyTree::Bool(b),
        serde_json::Value::Number(n) => PropertyTree::Number(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => PropertyTree::String(s),
        serde_json::Value::Array(arr) => {
            let items = arr.into_iter().map(infer_json_value_type).collect();
            PropertyTree::List(items)
        }
        serde_json::Value::Object(obj) => {
            let mut map = Vec::new();
            for (k, v) in obj.into_iter() {
                map.push((k, infer_json_value_type(v)));
            }
            PropertyTree::Dictionary(map)
        }
    }
}

fn json_value_to_property_tree(val: serde_json::Value, type_str: &str) -> PropertyTree {
    match type_str {
        "bool" => PropertyTree::Bool(val.as_bool().unwrap_or(false)),
        "number" => PropertyTree::Number(val.as_f64().unwrap_or(0.0)),
        "string" => match val {
            serde_json::Value::String(s) => PropertyTree::String(s),
            _ => PropertyTree::String(val.to_string()),
        },
        "signed_int" => {
            if let Some(i) = val.as_i64() {
                PropertyTree::SignedInt(i)
            } else if let Some(f) = val.as_f64() {
                PropertyTree::SignedInt(f as i64)
            } else {
                PropertyTree::SignedInt(0)
            }
        }
        "unsigned_int" => {
            if let Some(u) = val.as_u64() {
                PropertyTree::UnsignedInt(u)
            } else if let Some(f) = val.as_f64() {
                PropertyTree::UnsignedInt(f as u64)
            } else {
                PropertyTree::UnsignedInt(0)
            }
        }
        "list" => {
            if let serde_json::Value::Array(arr) = val {
                let items = arr.into_iter().map(infer_json_value_type).collect();
                PropertyTree::List(items)
            } else {
                PropertyTree::List(Vec::new())
            }
        }
        "dictionary" => {
            if let serde_json::Value::Object(obj) = val {
                let mut map = Vec::new();
                for (k, v) in obj.into_iter() {
                    map.push((k, infer_json_value_type(v)));
                }
                PropertyTree::Dictionary(map)
            } else {
                PropertyTree::Dictionary(Vec::new())
            }
        }
        _ => PropertyTree::None,
    }
}

pub fn property_tree_to_payload(
    version_str: String,
    header_flag: u8,
    root: PropertyTree,
) -> Result<FactorioSettingsPayload, FactorioError> {
    let root_dict = match root {
        PropertyTree::Dictionary(dict) => dict,
        _ => return Err(FactorioError::InvalidRootNode),
    };

    let mut settings = serde_json::Map::new();
    let mut metadata = serde_json::Map::new();

    for (section_name, section_node) in root_dict.into_iter() {
        let sec_dict = match section_node {
            PropertyTree::Dictionary(dict) => dict,
            _ => continue,
        };

        let mut section_settings = serde_json::Map::new();
        let mut section_metadata = serde_json::Map::new();

        for (setting_name, setting_node) in sec_dict.into_iter() {
            let setting_dict = match setting_node {
                PropertyTree::Dictionary(dict) => dict,
                _ => continue,
            };

            if let Some((_, val_node)) = setting_dict.into_iter().find(|(k, _)| k == "value") {
                let (json_val, type_str) = property_tree_to_json_and_type(val_node);
                section_settings.insert(setting_name.clone(), json_val);
                section_metadata.insert(setting_name, serde_json::Value::String(type_str));
            }
        }

        settings.insert(
            section_name.clone(),
            serde_json::Value::Object(section_settings),
        );
        metadata.insert(section_name, serde_json::Value::Object(section_metadata));
    }

    Ok(FactorioSettingsPayload {
        factorio_version: version_str,
        header_bool_flag: header_flag,
        settings,
        metadata,
    })
}

pub fn payload_to_property_tree(
    payload: FactorioSettingsPayload,
) -> Result<PropertyTree, FactorioError> {
    let mut root_dict = Vec::new();

    for (section_name, section_settings_val) in payload.settings.into_iter() {
        let section_settings = match section_settings_val {
            serde_json::Value::Object(obj) => obj,
            _ => continue,
        };

        let mut section_metadata = payload
            .metadata
            .get(&section_name)
            .and_then(|v| v.as_object().cloned());

        let mut section_dict = Vec::new();

        for (setting_name, val) in section_settings.into_iter() {
            let mut setting_container = Vec::new();

            let type_str = if let Some(ref mut meta) = section_metadata {
                meta.remove(&setting_name)
                    .and_then(|v| {
                        if let serde_json::Value::String(s) = v {
                            Some(s)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "string".to_string())
            } else {
                "string".to_string()
            };

            let pt_val = json_value_to_property_tree(val, &type_str);
            setting_container.push(("value".to_string(), pt_val));

            section_dict.push((setting_name, PropertyTree::Dictionary(setting_container)));
        }

        root_dict.push((section_name, PropertyTree::Dictionary(section_dict)));
    }

    Ok(PropertyTree::Dictionary(root_dict))
}

// ============================================================================
// 7. Primary Library Endpoints (JSON File <=> DAT Binary)
// ============================================================================

pub fn decode_dat_to_json(dat_bytes: &[u8]) -> Result<String, FactorioError> {
    let mut reader = BinaryReader::new(dat_bytes);

    let major = reader.u16()?;
    let minor = reader.u16()?;
    let patch = reader.u16()?;
    let build = reader.u16()?;
    let version_str = format!("{}.{}.{}.{}", major, minor, patch, build);

    let flag = reader.u8()?;

    let root = decode_property_tree(&mut reader)?;

    let payload = property_tree_to_payload(version_str, flag, root)?;

    serde_json::to_string_pretty(&payload).map_err(FactorioError::from)
}

pub fn encode_json_to_dat(json_str: &str) -> Result<Vec<u8>, FactorioError> {
    let payload: FactorioSettingsPayload = serde_json::from_str(json_str)?;

    let mut writer = BinaryWriter::new();

    let parts: Vec<&str> = payload.factorio_version.split('.').collect();
    if parts.len() != 4 {
        return Err(FactorioError::InvalidVersionFormat);
    }

    let major = parts[0].parse::<u16>()?;
    let minor = parts[1].parse::<u16>()?;
    let patch = parts[2].parse::<u16>()?;
    let build = parts[3].parse::<u16>()?;

    writer.u16(major);
    writer.u16(minor);
    writer.u16(patch);
    writer.u16(build);
    writer.u8(payload.header_bool_flag);

    let root_tree = payload_to_property_tree(payload)?;
    encode_property_tree(&mut writer, &root_tree);

    Ok(writer.bytes())
}

// ============================================================================
// 8. C-Compatible FFI Interface Exports (Perfect for Bun FFI)
// ============================================================================

/// Deserializes a binary `mod-settings.dat` memory buffer into a JSON string payload.
///
/// # Safety
///
/// This function dereferences raw pointers. The caller must guarantee that:
/// * `dat_ptr` is a valid, initialized pointer to a byte buffer of at least `dat_len` bytes.
/// * The memory referenced by `dat_ptr` remains immutable during execution.
#[no_mangle]
pub unsafe extern "C" fn decode_settings_dat(dat_ptr: *const u8, dat_len: usize) -> *mut c_char {
    if dat_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let data = std::slice::from_raw_parts(dat_ptr, dat_len);
    match decode_dat_to_json(data) {
        Ok(json_str) => match CString::new(json_str) {
            Ok(c_str) => c_str.into_raw(),
            Err(_) => CString::new("ERROR: Null byte found in generated JSON string")
                .unwrap()
                .into_raw(),
        },
        Err(err) => {
            let err_msg = format!("ERROR: {}", err);
            match CString::new(err_msg) {
                Ok(c_str) => c_str.into_raw(),
                Err(_) => CString::new("ERROR: Null byte found in error message")
                    .unwrap()
                    .into_raw(),
            }
        }
    }
}

/// Serializes a JSON string configuration back into an order-preserving binary buffer.
///
/// # Safety
///
/// This function dereferences raw pointers. The caller must guarantee that:
/// * `json_ptr` is a valid, null-terminated C-string pointer.
/// * `out_len` is a valid, writable pointer to a `usize` value.
#[no_mangle]
pub unsafe extern "C" fn encode_settings_dat(json_ptr: *const c_char, out_len: *mut usize) -> *mut u8 {
    if json_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = CStr::from_ptr(json_ptr);
    let json_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => {
            *out_len = 0;
            return std::ptr::null_mut();
        }
    };

    match encode_json_to_dat(json_str) {
        Ok(bytes) => {
            *out_len = bytes.len();
            let boxed = bytes.into_boxed_slice();
            Box::into_raw(boxed) as *mut u8
        }
        Err(err) => {
            eprintln!("Rust FFI serialization failure: {}", err);
            *out_len = 0;
            std::ptr::null_mut()
        }
    }
}

/// Frees C-string allocations generated on the Rust heap.
///
/// # Safety
///
/// This function dereferences raw pointers. The caller must guarantee that:
/// * `ptr` is a valid raw pointer allocated previously by `decode_settings_dat`.
/// * This pointer has not been freed or modified previously.
#[no_mangle]
pub unsafe extern "C" fn free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

/// Frees raw compiled binary allocations generated on the Rust heap.
///
/// # Safety
///
/// This function dereferences raw pointers. The caller must guarantee that:
/// * `ptr` is a valid raw pointer allocated previously by `encode_settings_dat`.
/// * `len` matches the exact buffer size written during allocation.
/// * This pointer has not been freed or modified previously.
#[no_mangle]
pub unsafe extern "C" fn free_bytes(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        let fat_ptr = std::ptr::slice_from_raw_parts_mut(ptr, len);
        let _ = Box::from_raw(fat_ptr);
    }
}

// ============================================================================
// 9. Automated Testing
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_json() {
        let json_input = r#"{
          "factorio_version": "2.0.76.0",
          "header_bool_flag": 0,
          "settings": {
            "startup": {
              "test-setting": "value"
            }
          },
          "metadata": {
            "startup": {
              "test-setting": "string"
            }
          }
        }"#;

        let bytes = encode_json_to_dat(json_input).expect("Encoding failed");
        let decoded_json = decode_dat_to_json(&bytes).expect("Decoding failed");

        let v1: serde_json::Value = serde_json::from_str(json_input).unwrap();
        let v2: serde_json::Value = serde_json::from_str(&decoded_json).unwrap();
        assert_eq!(v1, v2);
    }
}
