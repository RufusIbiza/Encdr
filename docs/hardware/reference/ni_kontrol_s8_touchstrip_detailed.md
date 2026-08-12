# NI Kontrol S8 Touchstrip Detailed Analysis (1:1 Tracking)

This document details the exact addressing and interpretation logic for the NI Kontrol S8 touchstrips, specifically focusing on the 1:1 positional tracking used for track seeking.

---

## 1. Input Mapping Summary

The S8 touchstrip data is split across two reports. While the right deck is well-understood, the left deck uses symmetrical offsets within the same report structures.

| Deck      | Control               | Report         | Address                | Type                       | Max Value |
| --------- | --------------------- | -------------- | ---------------------- | -------------------------- | --------- |
| **Left**  | **Absolute Position** | Sliders (176b) | **Bytes 2, 3**         | 16-bit (10-bit resolution) | 1024      |
| **Left**  | **Delta / Relative**  | Sliders (176b) | **Byte 28**            | 8-bit Signed               | -         |
| **Left**  | **Touch State**       | Buttons (46b)  | **Byte 16, Mask 0x01** | Bit                        | -         |
| **Right** | **Absolute Position** | Sliders (176b) | **Bytes 34, 35**       | 16-bit (10-bit resolution) | 1024      |
| **Right** | **Delta / Relative**  | Sliders (176b) | **Byte 44**?           | 8-bit Signed               | -         |
| **Right** | **Touch State**       | Buttons (46b)  | **Byte 32, Mask 0x01** | Bit                        | -         |

---

## 2. Positional Logic: 1:1 Tracking (`sub_140721470`)

When "Shift" is held down, Traktor switches to a 1:1 tracking mode for the touchstrip. This bypasses the standard relative "jog" behavior and uses the absolute capacitive position.

### Coordinate Mapping
The raw position value (0-1024) is normalized to a 0.0 to 1.0 float range.
- **Start of Strip:** 0
- **End of Strip:** 1024

Traktor applies a small deadzone at the edges:
```cpp
// Decompiler observation in sub_140721470
zmm1 = (*(uint32_t*)raw_position) * 1.00999999f; // Scaling factor
arg4 = zmm1 + 0.0700000077f; // Offset for deadzone/edge alignment
```

### Interpretation of Delta (Byte 28)
Byte 28 (Left) and its Right counterpart provide relative movement. This is used for "nudging" or "pitch bending" when not in seek mode.
- `> 0`: Clockwise / Forward movement.
- `< 0`: Counter-clockwise / Backward movement.

---

## 3. Touch Interpretation (Byte 29)

The user noted that **Byte 29** is "touched state" (<1 = not touched, 1 = touched). 
- In the **46-byte report**, bytes 16, 29, 32, and 45 appear to be dedicated "Status" bytes for the capacitive sensors.
- **Left Deck Status:** Byte 16 (Touch Bit) and Byte 29 (Touch Pressure/State).
- **Right Deck Status:** Byte 32 (Touch Bit) and Byte 45 (Touch Pressure/State).

---

## 4. Relevant Functions for Review

### `sub_14070d9c0`: `TouchstripTrackSeek` Constructor
Sets up the pins for "fingers_touching" and "position".

### `sub_140721470`: `onPositionChanged`
Handles the 1:1 tracking logic when the finger moves across the strip. It calculates the seek position within the track by multiplying the normalized touchstrip position (0.0 - 1.0) by the total track duration.

### `sub_1406dc770`: `NHLTouchstrip::process`
The primary dispatcher that reads the HID buffer and updates the internal touchstrip state before passing it to the filters and adapters.
