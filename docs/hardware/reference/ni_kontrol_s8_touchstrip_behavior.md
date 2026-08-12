# NI Kontrol S8 Touchstrip Behavioral Logic

This document details the logic Traktor uses to calculate and update the touchstrip LEDs on the NI Kontrol S8 based on track state (position, phase, etc.).

---

## 1. Track State Input
The touchstrip LEDs are updated by `sub_1406a1b10`, which receives a 32-byte (`int128_t* arg2`) structure containing the current deck state.

### Structure Analysis
*   **Byte 9:** Control flags / count (number of active points?).
*   **Bytes 12-13:** Primary position/phase value (16-bit).
*   **Bytes 14-15:** Secondary position/phase value (16-bit).

---

## 2. LED Calculation Logic (`sub_1406a13c0`)
This function transforms the track state into a bitmask or set of indices for the 25 LEDs.

### Key Operations:
1.  **Median Filtering:** The function maintains a window of recent position values (defined during initialization) to smooth out the LED movement.
    -   Window size is checked to be odd and <= 16.
2.  **Mapping to 25 LEDs:**
    -   The 16-bit position values are scaled to the 0-24 range.
    -   The function handles "center," "left," and "right" display modes (as seen in the "to the center" strings in `sub_140695880`).
3.  **Phase Offset Display:**
    -   When in phase mode, the LEDs likely represent the offset from the master clock, with the center LED (index 12) representing perfect sync.

---

## 3. LED Buffer Update (`sub_1406a1b10`)
Once the LED states are calculated, they are passed to the port's LED update routine.

```cpp
// Final update call in sub_1406a1b10
int64_t result = (*(uint64_t*)(*(uint64_t*)rcx_17 + 0x10))(rcx_17, rdx_15);
```

- `rcx_17`: Pointer to the LED range object (registered via `sub_141765d30`).
- `rdx_15`: Pointer to the calculated LED states (25 bytes/bits).

---

## 4. Hardware Mapping Recap

| Deck  | Start Index (Internal) | Physical LED Indices | Buffer Report (Prefix) |
| ----- | ---------------------- | -------------------- | ---------------------- |
| Left  | `0x5d` (93)            | 93 - 117             | `0x80`                 |
| Right | `0xb7` (183)*          | 211 - 235            | `0x81`                 |

*\*Note: The right deck start index `0xb7` is often added to a base offset of 118, resulting in 301, but the registration code `sub_141765d30` suggests it handles the offset internally.*

---

## 5. Potential Issues with Third-Party Implementation

1.  **Report Length:** Traktor likely sends the full 118-byte (or 236-byte) segment even if only a few LEDs change. If the sent report is too short, the hardware may ignore it.
2.  **Filtering Logic:** The hardware might expect a specific "refresh" rate or a specific sequence of prefix reports to enable the touchstrip display mode.
3.  **Input/Output Linkage:** On some NI controllers, the touchstrip LEDs only active if the touch sensor (byte 16/32) is currently reporting a touch, OR if the deck is in a specific software state (e.g., "Loop" mode).
