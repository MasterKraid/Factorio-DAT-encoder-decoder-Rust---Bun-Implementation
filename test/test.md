```powershell
# Step 1: Decode binary configuration to structured JSON
.\factorio_settings.exe decode mod-settings.dat decoded_settings.json

# Step 2: Encode structured JSON back to binary configuration
.\factorio_settings.exe encode decoded_settings.json test.dat
```

### Execution Logs
```text
[INFO] Reading binary file: mod-settings.dat...
[SUCCESS] Decoded configuration output written to: decoded_settings.json
[INFO] Reading JSON configuration: decoded_settings.json...
[SUCCESS] Compiled binary configuration written to: test.dat
```

---
MD5 Signatures
*   **Original File (`mod-settings.dat`):** `382F3E8812CF7820A0BDEC4EA4B18197`
*   **Compiled File (`test.dat`):** `382F3E8812CF7820A0BDEC4EA4B18197`
*   **Result:** **MATCH (Bit-Perfect)**

### SHA-256 Signatures
*   **Original File (`mod-settings.dat`):** `5D295D0459320352022FAE77EB7EE20A478112ADA3E00CE9F54DE122F4B761E4`
*   **Compiled File (`test.dat`):** `5D295D0459320352022FAE77EB7EE20A478112ADA3E00CE9F54DE122F4B761E4`
*   **Result:** **MATCH (Bit-Perfect)**