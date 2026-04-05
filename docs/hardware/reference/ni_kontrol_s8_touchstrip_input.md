# NI Kontrol S8 Touchstrip Input Analysis

This document details the analysis of the input data (touch and position) coming from the touchstrips on the Native Instruments Kontrol S8.

---

## 1. HID Report Structure Analysis

The S8 uses two main input reports on Interface 0 (HID):
- **Buttons Report (46 bytes):** Contains digital buttons and capacitive touch bits.
- **Sliders Report (176 bytes):** Contains high-resolution (16-bit) data for faders, knobs, and touchstrip positions.

### Report Segmentation Hypothesis
Based on working offsets for the right deck and the symmetrical layout of the S8, the reports appear to be segmented into 16-byte blocks:

| Block | Range (Bytes) | Description |
|-------|---------------|-------------|
| **Deck A (Left)** | 1 - 16 | Buttons, Touch Bits, Slider data for Deck A |
| **Mixer** | 17 - 32 | Mixer Channel faders, EQs, and Global buttons |
| **Deck B (Right)** | 33 - 48 | Buttons, Touch Bits, Slider data for Deck B |

---

## 2. Touchstrip Data Mapping

### Position Data (Sliders Report)
The position is a 16-bit value (only 10 bits used, range 0-1024).

| Deck | Bytes (Offsets) | Note |
|------|-----------------|------|
| **Left** | `[2, 3]` | Symmetrically matches the right deck offset. |
| **Right** | `[34, 35]` | Confirmed working by user. |

### Touch State (Buttons Report)
The capacitive touch bit indicates if a finger is currently on the strip.

| Deck | Byte | Mask | Note |
|------|------|------|------|
| **Left** | 16 | `0x01` | Bit 0 of the last byte in the Deck A block. |
| **Right** | 32 | `0x01` | Bit 0 of the last byte in the Deck B block (confirmed). |

---

## 3. Relevant Traktor Functions

### `sub_140695880`: Touchstrip Initialization
Registers the touchstrip element and sets up its behavior.

```cpp
// S8 call sites
sub_140695880(result, &s_1, 0, 0x44800000, 1); // Left (Deck 0)
sub_140695880(result, &var_b38, 1, 0x44800000, 1); // Right (Deck 1)
```

### `sub_1406dc770`: Input Update Logic
This method of `NHLTouchstrip` is called when new HID data arrives. It dispatches the raw buffer values to the high-level mapping layer.

```cpp
int64_t* sub_1406dc770(void* touchstrip_obj) {
    // Reads from captured lambdas that link to report offsets
    // Lambda 1: Handles capacitive touch bit
    // Lambda 2: Handles 16-bit position data
    // ...
}
```

### `sub_1406a13c0`: Position Filtering
The raw position data is passed through this filter (often called the `S8TouchstripFilter`) to smooth out jitter and handle track phase offsets.

---

## 4. Commentary & Interpretation

1.  **Resolution:** While the data is sent as 16-bit, the hardware sensor resolution is 10-bit (`max_value: 1024`). Reading it as a full 16-bit value without scaling or bit-masking might result in values up to 65535 if the hardware doesn't clear the lower bits, which explains the user's previous "wrong" data.
2.  **Symmetry:** The 16-byte (8-slider) offset between Deck A and Deck B is consistent across both the button and slider reports. This confirms the S8's internal design as a "Mixer with two D2-like controllers attached" at the protocol level.
3.  **Touch bit vs Position:** The touch bit must be checked before interpreting position. When the touch bit is `0`, the position value should be ignored as it may contain stale data.
