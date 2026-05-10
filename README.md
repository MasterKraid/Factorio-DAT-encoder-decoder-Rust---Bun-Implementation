# Factorio Mod Settings Serialization Library

> [!NOTE]
> This library implements and complements the technical specifications detailed in the [Original Binary Format Documentation](https://github.com/MasterKraid/Factorio-DAT-file-analysis/blob/main/DOCUMENTATION.md).

This repository contains a high-performance, memory-safe, and cryptographically bit-perfect Rust implementation for parsing and compiling Factorio `mod-settings.dat` configuration files. Designed specifically for integration with a Tauri-based desktop application using the Bun runtime, this library exposes a robust, C-compatible FFI interface while maintaining complete, byte-level exactness to Factorio's native C++ serialization logic.

## Technical Accomplishments

*   **Order-Preserving Vector Association Lists:** Factorio's binary format expects dictionary keys to be serialized in their original insertion order as declared by the active mod configuration. To bypass the arbitrary ordering of standard hash maps without adding heavy dependencies, this library implements a zero-dependency association list using `Vec<(String, PropertyTree)>`.
*   **Zero-Copy Parse Validations:** String parsers inside the binary reader operate directly on validated slices, avoiding unnecessary intermediate memory allocations or vector cloning.
*   **Ownership-Driven Optimizations:** Recursive formatting algorithms consume tree nodes by value using `into_iter()`. This eliminates the overhead of cloning complex nesting structures during payload conversions.
*   **Structured Error Typing:** Replaced generic string-based errors with a comprehensive custom error enum utilizing the industry-standard `thiserror` library.
*   **FFI Panic Boundaries:** Native FFI entry points implement strict guard boundaries to catch internal errors and gracefully map them to C-compatible string results, ensuring that a panic never crosses the ABI boundary to crash the host application.

---

## Compilation

The workspace compiles both a standalone CLI binary and a dynamic link library (`.dll`). Compile with a thread count constraint to manage host CPU thresholds:

```powershell
cargo build --release -j 10
```

*   **CLI Executable:** `target/release/factorio_settings.exe`
*   **Dynamic Link Library:** `target/release/factorio_settings.dll`

---

## Command Line Interface (CLI)

The CLI binary provides standard conversion operations directly from the shell:

```powershell
# Decode binary configuration to a parallel, flat JSON file
./target/release/factorio_settings.exe decode mod-settings.dat decoded_settings.json

# Compile flat JSON back into a bit-perfect Factorio binary
./target/release/factorio_settings.exe encode decoded_settings.json test_encoded.dat
```

---

## JavaScript / Bun FFI Bindings

The dynamic library exposes standard `extern "C"` endpoints to allow low-overhead, memory-safe interactions inside the Bun JavaScript environment.

### symbol Declarations

```javascript
import { dlopen, ptr, toBuffer, CString } from "bun:ffi";

const { symbols: lib } = dlopen("target/release/factorio_settings.dll", {
  decode_settings_dat: {
    args: ["ptr", "usize"],
    returns: "ptr",
  },
  encode_settings_dat: {
    args: ["ptr", "ptr"],
    returns: "ptr",
  },
  free_string: {
    args: ["ptr"],
    returns: "void",
  },
  free_bytes: {
    args: ["ptr", "usize"],
    returns: "void",
  },
});
```

### Full Roundtrip Execution

```javascript
import { readFileSync, writeFileSync } from "fs";

// 1. Load and decode binary settings
const datBuffer = readFileSync("mod-settings.dat");
const jsonPtr = lib.decode_settings_dat(ptr(datBuffer), datBuffer.length);
const jsonStr = new CString(jsonPtr).toString();
lib.free_string(jsonPtr); // Free C-string memory in Rust heap

// 2. Manipulate configurations in standard JavaScript context
const payload = JSON.parse(jsonStr);
payload.settings["runtime-global"]["shadow-end-animation-speed-jet"] = 1.5;

// 3. Re-compile JSON to cryptographically bit-perfect binary
const modifiedJsonStr = JSON.stringify(payload, null, 2);
const jsonUtf8 = Buffer.from(modifiedJsonStr + "\0"); // C-string null terminator
const outLenBuf = new Uint32Array(2); // Container to capture return length pointer

const outBytesPtr = lib.encode_settings_dat(ptr(jsonUtf8), ptr(outLenBuf));
const compiledLen = outLenBuf[0];

// 4. Save and free native buffer
const compiledBuffer = Buffer.from(toBuffer(outBytesPtr, 0, compiledLen));
writeFileSync("mod-settings-modified.dat", compiledBuffer);
lib.free_bytes(outBytesPtr, compiledLen); // Free compiled bytes in Rust heap
```


## Rust Native Usage

You can use this directly in your Rust applications by adding it to your `Cargo.toml`:
```bash
cargo add factorio_settings
```

```rust
use std::fs;
use factorio_settings::{decode_dat_to_json, encode_json_to_dat};

fn main() {
    // Decode
    let dat_bytes = fs::read("mod-settings.dat").unwrap();
    let json_string = decode_dat_to_json(&dat_bytes).unwrap();

    // Encode
    let new_dat_bytes = encode_json_to_dat(&json_string).unwrap();
    fs::write("encoded-settings.dat", new_dat_bytes).unwrap();
}
```

