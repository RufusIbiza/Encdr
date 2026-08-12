# NI Kontrol S8 Mixer LED Discovery & Mode Reference

This document summarizes the findings regarding the NI Kontrol S8 mixer LEDs, including the handshake/mode switch and specific LED indices.

---

## 1. Handshake / Mode Toggle (The `0xf3` Command)

The Kontrol S8 mixer LEDs are unique because they can operate in standalone mode (controlled by hardware) or software mode (controlled by Traktor). 

To enable computer control over the mixer LEDs, a specific "handshake" command must be sent to the device via the HID control interface (Interface 5).

*   **Command Prefix:** `0xf3`
*   **Data Offset 1:** `0x01` (Enable) / `0x00` (Disable)
*   **Implementation Note:** Sending `0xf3 0x01` "unlocks" the mixer section for external LED control.

---

## 2. LED Prefix Segmentation

The S8 uses a 309-LED buffer sent in 310-byte reports (Report ID + 309 bytes). The prefix determines which segment of the hardware is being addressed.

| Prefix   | LED Stream Range | Target Section                              |
| -------- | ---------------- | ------------------------------------------- |
| **0x80** | 0 - 117          | Deck A (Left) & Mixer A/B buttons           |
| **0x81** | 118 - 235        | Deck B (Right) & Mixer C/D buttons + Global |
| **0x82** | 236 - 308        | Global Mixer & Master VU meters             |

---

## 3. Discovered LED Indices (Mixer Section)

These indices are absolute within the 309-LED stream. To address them via a specific prefix, subtract the base index of that prefix if the hardware expects relative offsets.

| Element           | absolute Index | Prefix | Notes             |
| ----------------- | -------------- | ------ | ----------------- |
| **Snap**          | 212 (`0xd4`)   | `0x81` | Global Mixer      |
| **Quantize**      | 213 (`0xd5`)   | `0x81` | Global Mixer      |
| **Cue A**         | 25 (`0x19`)    | `0x80` |                   |
| **Cue B**         | 26 (`0x1a`)    | `0x80` |                   |
| **Cue C**         | 56 (`0x38`)    | `0x80` |                   |
| **Cue D**         | 60 (`0x3c`)    | `0x80` |                   |
| **Filter On A**   | 218 (`0xda`)   | `0x81` |                   |
| **Filter On B**   | 219 (`0xdb`)   | `0x81` |                   |
| **Filter On C**   | 220 (`0xdc`)   | `0x81` |                   |
| **Filter On D**   | 221 (`0xdd`)   | `0x81` |                   |
| **FX Assign 1.1** | 282 (`0x11a`)  | `0x82` | Channel 1, Unit 1 |
| **FX Assign 1.2** | 283 (`0x11b`)  | `0x82` | Channel 1, Unit 2 |
| **Deck Input A**  | 80 (`0x50`)    | `0x80` |                   |
| **Master VU L**   | 154 (`0x9a`)   | `0x81` | 9-segment start   |
| **Master VU R**   | 163 (`0xa3`)   | `0x81` | 9-segment start   |

---

## 4. Source Code: Mode Enable Callback (`sub_1405dbc10`)

This function manages the "Traktor Mode" state for the mixer hardware.

```cpp
// Logic representing the S8 mode switch
int64_t sub_1405dbc10() {
    // ...Handshake sequence...
    // When a channel is mapped to a Traktor deck, 
    // it sends 0xf3 [0x01] to the hardware.
    // This allows the software to override individual LEDs like CUE and FILTER.
    return &data_1575599b8;
}
```

---

## 5. Source Code: Level Meter Registration (`sub_140690d50`)

Used by Traktor to define the range of LEDs used for the VU meters.

```cpp
void** sub_140690d50(void* port, int64_t* name, int32_t start_index, int64_t count)
{
    // For S8 Channel 1 (A), start_index is often 0x5e or 0xf7 
    // depending on the initialization pass.
    // The 'count' is 6 for micro-meters or 15 for flagship-meters.
    
    int32_t current_index = start_index;
    for (int i = 0; i < count; ++i) {
        register_led_index(current_index++);
    }
}
```
