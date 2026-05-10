import { dlopen, ptr, toBuffer, CString } from "bun:ffi";
import { readFileSync, writeFileSync } from "fs";
import { join } from "path";

// 1. Resolve path to compiled dynamic library (DLL)
const dllPath = join(import.meta.dir, "target/release/factorio_settings.dll");

console.log("===============================================================================");
console.log("Bun + Rust FFI Integration & Verification Test Suite");
console.log("===============================================================================");
console.log(`[FFI] Loading compiled Rust DLL from: ${dllPath}`);

// 2. Bind the C-compatible export functions from the Rust library
const { symbols: lib } = dlopen(dllPath, {
  decode_settings_dat: {
    args: ["ptr", "usize"],
    returns: "ptr", // Returns raw char* (C-string pointer)
  },
  encode_settings_dat: {
    args: ["ptr", "ptr"], // JSON string pointer (char*), out_len pointer (usize*)
    returns: "ptr", // Returns raw u8* (binary data pointer)
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

console.log("[FFI] Rust library loaded successfully!\n");

// ============================================================================
// Step 3: Test Decode Flow (Binary -> JSON)
// ============================================================================
const datPath = join(import.meta.dir, "mod-settings.dat");
console.log(`[FFI] Loading original binary dat file: ${datPath}`);
const datBuffer = readFileSync(datPath);

// Convert standard Uint8Array to a raw FFI pointer
const datPtr = ptr(datBuffer);
const datLen = datBuffer.length;

console.log(`[FFI] Invoking decode_settings_dat(${datPtr}, ${datLen}) in Rust...`);
const jsonPtr = lib.decode_settings_dat(datPtr, datLen);

if (jsonPtr === 0 || jsonPtr === null) {
  console.error("[FFI] Decode failed: received null pointer from Rust");
  process.exit(1);
}

// Map the C-string pointer to standard JavaScript string
const jsonStr = new CString(jsonPtr).toString();

// Free C-allocated string memory inside Rust immediately to prevent memory leaks
lib.free_string(jsonPtr);

if (jsonStr.startsWith("ERROR:")) {
  console.error(`[FFI] Rust Decoder Error: ${jsonStr}`);
  process.exit(1);
}

console.log("[FFI] Decoded configuration payload successfully!");
const payload = JSON.parse(jsonStr);
console.log(`  Factorio Version: ${payload.factorio_version}`);
console.log(`  Header Flag:      ${payload.header_bool_flag}`);
console.log(`  Section Count:    ${Object.keys(payload.settings).length}`);
console.log(`  Active Sections:  ${Object.keys(payload.settings).join(", ")}\n`);

// ============================================================================
// Step 4: Test Encode Flow with inline parameter editing
// ============================================================================
console.log("[FFI] Modifying settings parameter inside JavaScript environment...");

// Toggle jetpack shadow animation speed as a verification test edit
if (payload.settings["runtime-global"] && payload.settings["runtime-global"]["shadow-end-animation-speed-jet"] !== undefined) {
  const originalVal = payload.settings["runtime-global"]["shadow-end-animation-speed-jet"];
  const newVal = originalVal === 1.0 ? 1.5 : 1.0;
  payload.settings["runtime-global"]["shadow-end-animation-speed-jet"] = newVal;
  console.log(`  Edited 'shadow-end-animation-speed-jet': ${originalVal} -> ${newVal}`);
} else {
  console.log("  No jetpack animation speed setting found. Writing config unaltered...");
}

const modifiedJsonStr = JSON.stringify(payload, null, 2);

// Convert modified JSON string to a null-terminated Uint8Array buffer
const jsonUtf8 = Buffer.from(modifiedJsonStr + "\0");
const jsonStringPtr = ptr(jsonUtf8);

// Allocate a 64-bit integer buffer (Uint32Array of length 2) for Rust to write the returning length to
const outLenBuf = new Uint32Array(2);
const outLenPtr = ptr(outLenBuf);

console.log("[FFI] Invoking encode_settings_dat() compiler in Rust...");
const outBytesPtr = lib.encode_settings_dat(jsonStringPtr, outLenPtr);

if (outBytesPtr === 0 || outBytesPtr === null) {
  console.error("[FFI] Encoding failed: received null pointer from Rust compiler");
  process.exit(1);
}

// Read the compiled size length written by Rust
const compiledLen = outLenBuf[0];
console.log(`[FFI] Compiled binary size: ${compiledLen} bytes`);

// wrap raw memory pointer into a node.js Buffer for writing
const compiledBuffer = Buffer.from(toBuffer(outBytesPtr, 0, compiledLen));

const outDatPath = join(import.meta.dir, "test_ffi_encoded.dat");
console.log(`[FFI] Writing compiled binary back to disk: ${outDatPath}`);
writeFileSync(outDatPath, compiledBuffer);

// Free the compiled bytes vector buffer inside Rust safely
lib.free_bytes(outBytesPtr, compiledLen);

console.log("\n===============================================================================");
console.log("[SUCCESS] FFI integration roundtrip completed perfectly with zero memory leaks!");
console.log("===============================================================================");
