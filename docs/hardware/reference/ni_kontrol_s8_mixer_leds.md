# NI Kontrol S8 Mixer LED Addressing Reference

This document details the LED addresses, prefixes, and addressing logic for the Native Instruments Kontrol S8 mixer section.

---

## 1. Global Enable / Mode Switch

The user-identified prefix `0xf3` with offset `1` acts as a master switch. 

*   **Prefix:** `0xf3`
*   **Data:** `[0x01]` (Enable Computer Control) / `[0x00]` (Standalone Mode)

In Traktor, this is likely sent during device handshake to tell the hardware to allow the PC to override the state of the "Global" mixer LEDs (which otherwise operate in hardware-only mode for standalone mixing).

---

## 2. LED Addressing Protocol

The S8 uses a segmented LED buffer (309 LEDs total). Updates are sent using three primary prefixes:

| Prefix   | LED Range | Description                           |
| -------- | --------- | ------------------------------------- |
| **0x80** | 0 - 117   | Deck A (Left) and Mixer Channels A/B  |
| **0x81** | 118 - 235 | Deck B (Right) and Mixer Channels C/D |
| **0x82** | 236 - 308 | Global Mixer Controls & Master VU     |

---

## 3. Mixer LED Mapping (Per Channel)

Indices are relative to the start of the entire 309-LED buffer.

| Channel | Deck Index | Cue LED | Filter On LED | FX Assign L | FX Assign R | Level Meter (Start) | Count |
| ------- | ---------- | ------- | ------------- | ----------- | ----------- | ------------------- | ----- |
| **A**   | 0          | `0x19`  | `0xda`        | `0x4c`?     | `0x4d`?     | `0x1b`              | 6     |
| **B**   | 1          | `0x1a`  | `0xdb`        | `0x4e`      | `0x4f`      | `0x21`              | 6     |
| **C**   | 2          | `0x38`  | `0xdc`        | `0x50`?     | `0x51`?     | `0x70`              | 15    |
| **D**   | 3          | `0x3c`  | `0xdd`        | `0x52`?     | `0x53`?     | `0x52`              | 15    |

*Note: Level meter segments for A/B are 6 LEDs high, while C/D are 15 LEDs high (flagship channels).*

---

## 4. Global Mixer LEDs

| Element             | LED Index    | Note                |
| ------------------- | ------------ | ------------------- |
| **Snap**            | `0xd4` (212) | Prefix `0x81`       |
| **Quantize**        | `0xd5` (213) | Prefix `0x81`       |
| **Master VU Left**  | `0x9a` (154) | 9-segment sequence  |
| **Master VU Right** | `0xa3` (163) | 9-segment sequence  |
| **Mic 1 On**        | `0xe0`?      | Under investigation |
| **Mic 2 On**        | `0xe1`?      | Under investigation |

---

## 5. Source Code: Level Meter Registration (`sub_140690d50`)

This function is used to register multi-segment level meters (VUs).

```cpp
void** sub_140690d50(void* port, int64_t* name, int32_t start_index, int64_t count)
{
    // Sets up a sequence of 'count' LEDs starting at 'start_index'
    // Used for Mixer Channels (count 6 or 15) and Master VU (count 9)
    
    int32_t current_index = start_index;
    for (int i = 0; i < count; ++i) {
        // Registers individual LED components into the segment
        register_led_index(current_index++);
    }
}
```

---

## 6. Implementation Notes for VU Meters

1.  **Addressing:** For 15-segment meters (Channels C/D), the value sent to the start index usually propagates upwards.
2.  **Brightness:** NI controllers often use a single byte per mono LED (0-127). VU meters are usually handled as a sequence of these mono LEDs.
3.  **Prefix 0x82:** If sending to the master VU meters, ensure you are using the `0x82` prefix if the index is > 235, or check if they are mirrored in the `0x80/0x81` blocks. Traktor source suggests `0x9a` and `0xa3` which are within the `0x81` prefix range (118-235).
