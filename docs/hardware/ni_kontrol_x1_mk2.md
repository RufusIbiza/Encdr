# NI Traktor Kontrol X1 Mk2 Hardware Reference

## Overview

| Property     | Value                                    |
| ------------ | ---------------------------------------- |
| Manufacturer | Native Instruments                       |
| Product      | Traktor Kontrol X1 Mk2                   |
| VID:PID      | `0x17cc:0x1220`                          |
| USB Speed    | Full Speed (12 Mbps)                     |
| Interfaces   | 1 (HID Control interface 0)              |
| Screens      | 2 dual 7-segment LED numeric displays    |

The Traktor Kontrol X1 Mk2 expands on the original X1 with multi-purpose capacitive touchstrips (with 11-segment LED progress/meter indicators), RGB backlighting for cue buttons, touch-sensitive rotary encoders, and dual 7-segment digital loop displays.

---

## Physical Layout

```
┌──────────────────────────────────────────────┐
| [FX1 On]  [ 1 ] [ 2 ] [ 3 ]   [FX2 On] [ 1 ] [ 2 ] [ 3 ] |
| (Knob 1) (Knob 2)(Knob 3)(Knob 4) (Knob 1)(Knob 2)(Knob 3)(Knob 4) |
|                                              |
|  [ 7-SEG ]                     [ 7-SEG ]     |
| (Loop Enc) (Browse Enc)       (Loop Enc)(Browse Enc) |
|                                              |
|  ══════ TOUCHSTRIP LEFT ══════ ══════ TOUCHSTRIP RIGHT ══════ |
|                                              |
| [Cue 1] [Cue 2] [Cue 3] [Cue 4]  [Cue 1] [Cue 2] [Cue 3] [Cue 4] |
| [Flux]  [Sync]  [Cue]   [Play]   [Flux]  [Sync]  [Cue]   [Play]  |
|                                        [Shift] |
└──────────────────────────────────────────────┘
```

---

## USB Endpoints & Reports

* **Interface 0 (Control):** Interrupt IN `0x81`, Interrupt OUT `0x01`
* **Input Packet:** 32 bytes containing 8 analog knobs, 2 touchstrips (10-bit position + touch detection), 4 capacitive encoders with push detection, and 31 buttons.
* **Output Packet:** 96 bytes controlling 8 RGB hotcue pads, 22 touchstrip LEDs, 4 numeric 7-segment display digits, and transport indicators.

---

## 7-Segment / 8-Segment Loop Displays

The Kontrol X1 Mk2 features two dual 8-segment (7 segments + decimal point) LED displays for Deck A/C (left) and Deck B/D (right), driven through the LED Output Report:
* **Byte Offset 9:** `display_left_1` (Left deck, tens / sub-beat digit)
* **Byte Offset 10:** `display_left_2` (Left deck, units digit)
* **Byte Offset 11:** `display_right_1` (Right deck, tens / sub-beat digit)
* **Byte Offset 12:** `display_right_2` (Right deck, units digit)

### 7-Segment + Dot Bitmask Architecture

```text
      -- a (bit 0, 0x01) --
     |                     |
  f (bit 5, 0x20)       b (bit 1, 0x02)
     |                     |
      -- g (bit 6, 0x40) --
     |                     |
  e (bit 4, 0x10)       c (bit 2, 0x04)
     |                     |
      -- d (bit 3, 0x08) --     * dp (bit 7, 0x80)
```

| Bit | Hex | Segment | Description |
|---|---|---|---|
| **0** | `0x01` | `SEG_A` | Top horizontal bar |
| **1** | `0x02` | `SEG_B` | Top-right vertical bar |
| **2** | `0x04` | `SEG_C` | Bottom-right vertical bar |
| **3** | `0x08` | `SEG_D` | Bottom horizontal bar |
| **4** | `0x10` | `SEG_E` | Bottom-left vertical bar |
| **5** | `0x20` | `SEG_F` | Top-left vertical bar |
| **6** | `0x40` | `SEG_G` | Center horizontal bar |
| **7** | `0x80` | `SEG_DP`| Decimal point dot |

### Traktor Loop Size Display Notation

| Loop Length | Left Digit (`display_left_1`) | Right Digit (`display_left_2`) | Rendered Readout |
|---|---|---|---|
| **32 beats** | `'3'` (`0x4F`) | `'2'` (`0x5B`) | `32` |
| **16 beats** | `'1'` (`0x06`) | `'6'` (`0x7D`) | `16` |
| **8 beats**  | Blank (`0x00`) | `'8'` (`0x7F`) | ` 8` |
| **4 beats**  | Blank (`0x00`) | `'4'` (`0x66`) | ` 4` |
| **2 beats**  | Blank (`0x00`) | `'2'` (`0x5B`) | ` 2` |
| **1 beat**   | Blank (`0x00`) | `'1'` (`0x06`) | ` 1` |
| **1/2 beat** | `.` (`0x80`)   | `'2'` (`0x5B`) | `.2` |
| **1/4 beat** | `.` (`0x80`)   | `'4'` (`0x66`) | `.4` |
| **1/8 beat** | `.` (`0x80`)   | `'8'` (`0x7F`) | `.8` |
| **1/16 beat**| `'1.'` (`0x86`)| `'6'` (`0x7D`) | `1.6` |
| **1/32 beat**| `'3.'` (`0xCF`)| `'2'` (`0x5B`) | `3.2` |

When a loop is active, the right digit illuminates its decimal point (`SEG_DP`, `0x80`), displaying e.g. `.4.` for an active 1/4 beat loop.

### Encdr Code Example

```rust
use encdr::{Encdr, EncdrConfig, SevenSegment};

let encdr = Encdr::new(EncdrConfig::default())?;
let x1 = encdr.scan()?[0];

// Display an active 1/4 beat loop on Left Deck
encdr.set_loop_display(x1, "display_left_1", "display_left_2", 0.25, true);

// Display a 16 beat loop on Right Deck
encdr.set_loop_display(x1, "display_right_1", "display_right_2", 16.0, false);
```
