# NI Kontrol F1 Hardware Reference

## Overview

| Property     | Value                                    |
| ------------ | ---------------------------------------- |
| Manufacturer | Native Instruments                       |
| Product      | Traktor Kontrol F1                       |
| VID:PID      | `0x17cc:0x1120`                          |
| USB Speed    | Full Speed (12 Mbps)                     |
| Interfaces   | 1 (HID Control interface 0)              |
| Screens      | None (LED-driven control surface)        |

The Traktor Kontrol F1 is a performance controller designed for Traktor's Remix Decks, featuring a 4x4 matrix of 16 multi-color RGB pads, 4 dedicated channel volume faders, 4 filter/FX potentiometers, a 2-digit 7-segment display, a push-action browse encoder, and dedicated mode/transport buttons.

---

## Physical Layout

```
┌──────────────────────────────────────────────┐
| [Size] [Type] [Rev] [Quant] [Capture]        |
|                                              |
|  [ 7-SEG ]           ( Browse / Push )       |
|                                              |
|  (Knob 1)   (Knob 2)   (Knob 3)   (Knob 4)   |
|                                              |
|  │  ▲  │    │  ▲  │    │  ▲  │    │  ▲  │    |
|  │  █  │    │  █  │    │  █  │    │  █  │    |
|  │  ▼  │    │  ▼  │    │  ▼  │    │  ▼  │    |
|  Fader 1    Fader 2    Fader 3    Fader 4    |
|                                              |
| ┌──────┬──────┬──────┬──────┐                |
| │Pad 1 │Pad 2 │Pad 3 │Pad 4 │                |
| ├──────┼──────┼──────┼──────┤                |
| │Pad 5 │Pad 6 │Pad 7 │Pad 8 │                |
| ├──────┼──────┼──────┼──────┤                |
| │Pad 9 │Pad 10│Pad 11│Pad 12│                |
| ├──────┼──────┼──────┼──────┤                |
| │Pad 13│Pad 14│Pad 15│Pad 16│                |
| └──────┴──────┴──────┴──────┘                |
|                                              |
| [Stop 1]   [Stop 2]   [Stop 3]   [Stop 4]    |
| [Sync 1]   [Sync 2]   [Sync 3]   [Sync 4]    |
|                                     [Shift]  |
└──────────────────────────────────────────────┘
```

---

## USB Endpoints & Reports

* **Interface 0 (Control):** Interrupt IN `0x81`, Interrupt OUT `0x01`
* **Input Packet:** 22 bytes (`controls` report) containing all 4 faders, 4 knobs, rotary encoder, function buttons, and the 16 RGB pads.
* **Output Packet:** 80 bytes (`all_leds` report) controlling the 16 RGB pad backlights, 4 stop/mute single LEDs, dual 7-segment display digits, and mode buttons.

---

## 7-Segment / 8-Segment Digital Display

The Kontrol F1 features a central 2-digit digital readout driven via the LED Output Report (`all_leds`):
* **Byte Offset 8:** `display_left` (Left digit)
* **Byte Offset 9:** `display_right` (Right digit)

### 7-Segment + Dot Bitmask Architecture
Each digit is an 8-bit byte containing bit-mapped segment flags (7 segments + decimal point):

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

### Encdr Code Example

```rust
use encdr::{Encdr, EncdrConfig, SevenSegment};

let encdr = Encdr::new(EncdrConfig::default())?;
let f1 = encdr.scan()?[0];

// 1. Display numeric digits using SevenSegment::from_digit
encdr.set_seven_segment(f1, "display_left", SevenSegment::from_digit(4));
encdr.set_seven_segment(f1, "display_right", SevenSegment::from_digit(2));

// 2. Display text with automatic decimal point merging (e.g. "A.1")
encdr.set_seven_segment_str(f1, &["display_left", "display_right"], "A.1");
```
