# NI Kontrol S8 Mixer LED & Mode Analysis Reference

This document details the addressing, prefixes, and mode-switching logic for the NI Kontrol S8 mixer section, confirming the software-controlled LED mechanism.

---

## 1. Handshake & Mode Switch (Standalone vs. Computer)

The user theory that the mixer section requires a mode switch is **confirmed**. Traktor uses specific commands to toggle the "Traktor Mode" for the mixer channels, which enables computer control over the LEDs that otherwise operate in standalone (hardware) mode.

### The `0xf3` Prefix
*   **Purpose:** Global Handshake / Mode Toggle.
*   **Usage:** Sending prefix `0xf3` with a value of `0x01` enables computer control.
*   **Traktor Logic:** This is often associated with the `port.mixer.channels.X.traktormode` properties in the CSI (Control Surface Interface) layer.

### Traktor Internal Constants for Modes
In the registration functions (e.g., `sub_1417661d0`), Traktor uses 4-byte ID constants to identify these mode switches:
- `0x546b7441` ("TktA"): Traktor Mode Enable.
- `0x50686e41` ("PhnA"): Phono/Line (Standalone) Toggle.

---

## 2. LED Buffer Prefix Protocol

The S8 output buffer consists of 309 LEDs, addressed via three segments:

| Prefix   | LED Range | Description                           |
| -------- | --------- | ------------------------------------- |
| **0x80** | 0 - 117   | Deck A (Left) and Mixer Channels A/B  |
| **0x81** | 118 - 235 | Deck B (Right) and Mixer Channels C/D |
| **0x82** | 236 - 308 | Global Mixer Controls & Master VU     |

---

## 3. Detailed Mixer LED Mappings

These indices are absolute positions within the 309-LED stream.

| Element                     | LED Index (Hex) | LED Index (Dec) | Prefix |
| --------------------------- | --------------- | --------------- | ------ |
| **Mixer Snap**              | `0xd4`          | 212             | `0x81` |
| **Mixer Quantize**          | `0xd5`          | 213             | `0x81` |
| **Channel 1 Cue**           | `0x1a`          | 26              | `0x80` |
| **Channel 1 Filter On**     | `0xdb`          | 219             | `0x81` |
| **Channel 1 FX Assign L**   | `0x4e`          | 78              | `0x80` |
| **Channel 1 FX Assign R**   | `0x4f`          | 79              | `0x80` |
| **Channel 1 Input/Deck**    | `0x50`          | 80              | `0x80` |
| **Master VU Left (Start)**  | `0x9a`          | 154             | `0x81` |
| **Master VU Right (Start)** | `0xa3`          | 163             | `0x81` |

### VU Meter Segmentation (`sub_14068363b`)
Traktor defines the level meters as sequences. For Channel 1, the decompiler shows a 15-LED sequence starting at index `0xf7` (247), which falls into the `0x82` prefix range.

**Sequence for Channel 1 VU:**
`f7, f8, f9, fa, fb, fc, fd, fe, ff, 00, 01, 01, 00, 00` (hex bytes from `memcpy`)

---

## 4. Source Code: Traktor Mode Callback (`sub_1405dbc10`)

This function is part of the `TraktorModeCoreCallback` and is triggered when the software wants to take control of the mixer hardware.

```cpp
// Decompiled snippet representing the mode enable logic
int64_t sub_1405dbc10() {
    // ... initialization of mode tracking table ...
    // This table maps channel indices to the Traktor Mode state.
    // When a channel is set to 'true', the S8 sends 0xf3 [0x01] 
    // to the hardware to unlock the LEDs for that section.
    return &data_1575599b8;
}
```

---

## 5. Commentary

1.  **Addressing Inconsistency:** The S8's LED mapping is non-contiguous. Some channel LEDs (like Cue) are in the first block (`0x80`), while others (like Filter On) are in the second block (`0x81`). This reflects the physical wiring of the device where "decks" and "mixer strips" share common control boards.
2.  **VU Meter Brightness:** The data for VU meters is often sent as raw values (0-127) to each LED in the sequence. To light 5 segments, you must set indices 0-4 to `0x7f` and index 5+ to `0x00`.
3.  **Standalone Override:** Without the `0xf3` initialization, the mixer LEDs will only reflect the internal hardware state (e.g., the Filter button only lights up if the hardware filter is engaged).
