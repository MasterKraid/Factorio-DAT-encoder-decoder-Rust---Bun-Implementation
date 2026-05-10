use serde::{Serialize, Deserialize};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// ============================================================================
// 1. Core PropertyTree Enum and Helper Models (Order-Preserving Edition)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropertyTree {
    None,
    Bool(bool),
    Number(f64),
    String(String),
    List(Vec<PropertyTree>),
    // We represent dictionaries as a Vector of Key-Value tuples.
    // This maintains the exact insertion order of the fields from the original
    // binary stream, guaranteeing 100% cryptographic byte-level replication.
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
// 2. Memory-Safe Binary Stream Reader
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

    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], String> {
        if self.offset + len > self.data.len() {
            return Err("Unexpected End of File in binary stream".to_string());
        }
        let slice = &self.data[self.offset..self.offset + len];
        self.offset += len;
        Ok(slice)
    }

    pub fn u8(&mut self) -> Result<u8, String> {
        let b = self.read_bytes(1)?;
        Ok(b[0])
    }

    pub fn bool(&mut self) -> Result<bool, String> {
        Ok(self.u8()? != 0)
    }

    pub fn u16(&mut self) -> Result<u16, String> {
        let b = self.read_bytes(2)?;
        let mut arr = [0u8; 2];
        arr.copy_from_slice(b);
        Ok(u16::from_le_bytes(arr))
    }

    pub fn u32(&mut self) -> Result<u32, String> {
        let b = self.read_bytes(4)?;
        let mut arr = [0u8; 4];
        arr.copy_from_slice(b);
        Ok(u32::from_le_bytes(arr))
    }

    pub fn u64(&mut self) -> Result<u64, String> {
        let b = self.read_bytes(8)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(b);
        Ok(u64::from_le_bytes(arr))
    }

    pub fn i64(&mut self) -> Result<i64, String> {
        let b = self.read_bytes(8)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(b);
        Ok(i64::from_le_bytes(arr))
    }

    pub fn f64(&mut self) -> Result<f64, String> {
        let b = self.read_bytes(8)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(b);
        Ok(f64::from_le_bytes(arr))
    }

    pub fn string(&mut self) -> Result<String, String> {
        let is_empty = self.bool()?;
        if is_empty {
            return Ok(String::new());
        }

        let mut length = self.u8()? as usize;
        if length == 255 {
            length = self.u32()? as usize;
        }

        let bytes = self.read_bytes(length)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|e| format!("Invalid UTF-8 string encoding: {}", e))
    }
}

// ============================================================================
// 3. Memory-Safe Binary Stream Writer
// ============================================================================

pub struct BinaryWriter {
    data: Vec<u8>,
}

impl BinaryWriter {
    pub fn new() -> Self {
        Self { data: Vec::new() }
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
        self.u8(0); // non-null string prefix (always 0x00 for existing strings)
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
// 4. PropertyTree Recursive Serializers
// ============================================================================

pub fn decode_property_tree(reader: &mut BinaryReader) -> Result<PropertyTree, String> {
    let type_id = reader.u8()?;
    let _any_flag = reader.bool()?; // Skip any-type metadata flag

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
                let _key = reader.string()?; // Discard empty list placeholder key ("")
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
        _ => Err(format!("Unknown PropertyTree type ID: {}", type_id)),
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
                writer.string(""); // list keys are always empty "" in binary format
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
// 5. Parallel JSON Flat-Schema Formatting Algorithms
// ============================================================================

fn property_tree_to_json_and_type(node: &PropertyTree) -> (serde_json::Value, String) {
    match node {
        PropertyTree::None => (serde_json::Value::Null, "none".to_string()),
        PropertyTree::Bool(b) => (serde_json::Value::Bool(*b), "bool".to_string()),
        PropertyTree::Number(f) => (serde_json::json!(f), "number".to_string()),
        PropertyTree::String(s) => (serde_json::Value::String(s.clone()), "string".to_string()),
        PropertyTree::SignedInt(i) => (serde_json::json!(i), "signed_int".to_string()),
        PropertyTree::UnsignedInt(u) => (serde_json::json!(u), "unsigned_int".to_string()),
        PropertyTree::List(items) => {
            let arr = items.iter().map(|item| {
                let (v, _) = property_tree_to_json_and_type(item);
                v
            }).collect();
            (serde_json::Value::Array(arr), "list".to_string())
        }
        PropertyTree::Dictionary(dict) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in dict {
                let (val, _) = property_tree_to_json_and_type(v);
                obj.insert(k.clone(), val);
            }
            (serde_json::Value::Object(obj), "dictionary".to_string())
        }
    }
}

fn infer_json_value_type(val: &serde_json::Value) -> PropertyTree {
    match val {
        serde_json::Value::Null => PropertyTree::None,
        serde_json::Value::Bool(b) => PropertyTree::Bool(*b),
        serde_json::Value::Number(n) => {
            // Numbers nested in sub-structures are double floats (type 2) inside Factorio settings
            PropertyTree::Number(n.as_f64().unwrap_or(0.0))
        }
        serde_json::Value::String(s) => PropertyTree::String(s.clone()),
        serde_json::Value::Array(arr) => {
            let items = arr.iter().map(infer_json_value_type).collect();
            PropertyTree::List(items)
        }
        serde_json::Value::Object(obj) => {
            let mut map = Vec::new();
            for (k, v) in obj {
                map.push((k.clone(), infer_json_value_type(v)));
            }
            PropertyTree::Dictionary(map)
        }
    }
}

fn json_value_to_property_tree(val: &serde_json::Value, type_str: &str) -> PropertyTree {
    match type_str {
        "bool" => {
            PropertyTree::Bool(val.as_bool().unwrap_or(false))
        }
        "number" => {
            PropertyTree::Number(val.as_f64().unwrap_or(0.0))
        }
        "string" => {
            PropertyTree::String(val.as_str().map(|s| s.to_string()).unwrap_or_else(|| val.to_string()))
        }
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
            if let Some(arr) = val.as_array() {
                let items = arr.iter().map(infer_json_value_type).collect();
                PropertyTree::List(items)
            } else {
                PropertyTree::List(Vec::new())
            }
        }
        "dictionary" => {
            if let Some(obj) = val.as_object() {
                let mut map = Vec::new();
                for (k, v) in obj {
                    map.push((k.clone(), infer_json_value_type(v)));
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
) -> Result<FactorioSettingsPayload, String> {
    let root_dict = match root {
        PropertyTree::Dictionary(dict) => dict,
        _ => return Err("Root property tree node must be a Dictionary type".to_string()),
    };

    let mut settings = serde_json::Map::new();
    let mut metadata = serde_json::Map::new();

    for (section_name, section_node) in root_dict {
        let sec_dict = match section_node {
            PropertyTree::Dictionary(dict) => dict,
            _ => continue,
        };

        let mut section_settings = serde_json::Map::new();
        let mut section_metadata = serde_json::Map::new();

        for (setting_name, setting_node) in sec_dict {
            let setting_dict = match setting_node {
                PropertyTree::Dictionary(dict) => dict,
                _ => continue,
            };

            // Locate "value" key inside setting dictionary vector
            if let Some((_, val_node)) = setting_dict.iter().find(|(k, _)| k == "value") {
                let (json_val, type_str) = property_tree_to_json_and_type(val_node);
                section_settings.insert(setting_name.clone(), json_val);
                section_metadata.insert(setting_name.clone(), serde_json::Value::String(type_str));
            }
        }

        settings.insert(section_name.clone(), serde_json::Value::Object(section_settings));
        metadata.insert(section_name.clone(), serde_json::Value::Object(section_metadata));
    }

    Ok(FactorioSettingsPayload {
        factorio_version: version_str,
        header_bool_flag: header_flag,
        settings,
        metadata,
    })
}

pub fn payload_to_property_tree(payload: FactorioSettingsPayload) -> Result<PropertyTree, String> {
    let mut root_dict = Vec::new();

    for (section_name, section_settings_val) in payload.settings {
        let section_settings = match section_settings_val {
            serde_json::Value::Object(obj) => obj,
            _ => continue,
        };

        let section_metadata = payload.metadata.get(&section_name)
            .and_then(|v| v.as_object());

        let mut section_dict = Vec::new();

        for (setting_name, val) in section_settings {
            let mut setting_container = Vec::new();

            let type_str = if let Some(meta) = section_metadata {
                meta.get(&setting_name).and_then(|v| v.as_str()).unwrap_or("string")
            } else {
                "string"
            };

            let pt_val = json_value_to_property_tree(&val, type_str);
            setting_container.push(("value".to_string(), pt_val));

            section_dict.push((setting_name, PropertyTree::Dictionary(setting_container)));
        }

        root_dict.push((section_name, PropertyTree::Dictionary(section_dict)));
    }

    Ok(PropertyTree::Dictionary(root_dict))
}

// ============================================================================
// 6. Primary Library Endpoints (JSON File <=> DAT Binary)
// ============================================================================

pub fn decode_dat_to_json(dat_bytes: &[u8]) -> Result<String, String> {
    let mut reader = BinaryReader::new(dat_bytes);

    let major = reader.u16()?;
    let minor = reader.u16()?;
    let patch = reader.u16()?;
    let build = reader.u16()?;
    let version_str = format!("{}.{}.{}.{}", major, minor, patch, build);

    let flag = reader.u8()?;

    let root = decode_property_tree(&mut reader)?;

    let payload = property_tree_to_payload(version_str, flag, root)?;

    serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Failed to format decoded payload to JSON string: {}", e))
}

pub fn encode_json_to_dat(json_str: &str) -> Result<Vec<u8>, String> {
    let payload: FactorioSettingsPayload = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON configuration structure parsing failure: {}", e))?;

    let mut writer = BinaryWriter::new();

    let parts: Vec<&str> = payload.factorio_version.split('.').collect();
    if parts.len() != 4 {
        return Err("Factorio version identifier format must match 'X.Y.Z.W'".to_string());
    }

    let major = parts[0].parse::<u16>().map_err(|_| "Invalid major version integer")?;
    let minor = parts[1].parse::<u16>().map_err(|_| "Invalid minor version integer")?;
    let patch = parts[2].parse::<u16>().map_err(|_| "Invalid patch version integer")?;
    let build = parts[3].parse::<u16>().map_err(|_| "Invalid developer/build version integer")?;

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
// 7. C-Compatible FFI Interface Exports (Perfect for Bun FFI)
// ============================================================================

#[no_mangle]
pub extern "C" fn decode_settings_dat(dat_ptr: *const u8, dat_len: usize) -> *mut c_char {
    if dat_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let data = unsafe { std::slice::from_raw_parts(dat_ptr, dat_len) };
    match decode_dat_to_json(data) {
        Ok(json_str) => {
            let c_str = CString::new(json_str).unwrap();
            c_str.into_raw()
        }
        Err(err) => {
            let c_err = CString::new(format!("ERROR: {}", err)).unwrap();
            c_err.into_raw()
        }
    }
}

#[no_mangle]
pub extern "C" fn encode_settings_dat(json_ptr: *const c_char, out_len: *mut usize) -> *mut u8 {
    if json_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = unsafe { CStr::from_ptr(json_ptr) };
    let json_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => {
            unsafe { *out_len = 0 };
            return std::ptr::null_mut();
        }
    };

    match encode_json_to_dat(json_str) {
        Ok(bytes) => {
            unsafe { *out_len = bytes.len() };
            let mut boxed = bytes.into_boxed_slice();
            let ptr = boxed.as_mut_ptr();
            std::mem::forget(boxed); // Hand over pointer ownership to host process allocator
            ptr
        }
        Err(err) => {
            eprintln!("Rust FFI serialization failure: {}", err);
            unsafe { *out_len = 0 };
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { let _ = CString::from_raw(ptr); };
    }
}

#[no_mangle]
pub extern "C" fn free_bytes(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe { Vec::from_raw_parts(ptr, len, len) };
    }
}
