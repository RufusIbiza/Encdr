# NI Traktor Kontrol Z2 Hardware Reference

## Overview

| Property     | Value                                    |
| ------------ | ---------------------------------------- |
| Manufacturer | Native Instruments                       |
| Product      | Traktor Kontrol Z2                       |
| VID:PID      | `0x17cc:0x1230`                          |
| USB Speed    | Full Speed (12 Mbps)                     |
| Interfaces   | 1 (HID Control interface 0)              |
| Screens      | 2 dual 7-segment loop displays           |

The Traktor Kontrol Z2 is a 2+2 channel DJ mixer and control surface featuring Innofader technology, multi-color RGB cue/remix pads, dedicated loop encoders with 7-segment displays, stereo master/channel VU meters, and full transport and FX controls.

---

## Physical Layout

```
┌──────────────────────────────────────────────┐
| [FX 1 Panel]         [Main / Booth]     [FX 2 Panel] |
| (Gain A) (High A)    (Gain B) (High B) |
| (Mid A)  (Low A)     (Mid B)  (Low B)  |
| (Filter A)           (Filter B)        |
|                                              |
|  [ 7-SEG ]                              [ 7-SEG ]  |
| (Loop A) (Browse A)                     (Loop B) (Browse B) |
|                                              |
| ┌──────┬──────┐                         ┌──────┬──────┐ |
| │Pad A1│Pad A2│                         │Pad B1│Pad B2│ |
| ├──────┼──────┤                         ├──────┼──────┤ |
| │Pad A3│Pad A4│                         │Pad B3│Pad B4│ |
| └──────┴──────┘                         └──────┴──────┘ |
|                                              |
| [Flux] [Sync] [Cue] [Play]             [Flux] [Sync] [Cue] [Play] |
|                                              |
|         │  ▲  │                 │  ▲  │      |
|         │  █  │                 │  █  │      |
|         │  ▼  │                 │  ▼  │      |
|         Fader A                 Fader B      |
|                                              |
|                  ◄─── Crossfader ───►        |
└──────────────────────────────────────────────┘
```

---

## USB Endpoints & Reports

* **Interface 0 (Control):** Interrupt IN `0x81`, Interrupt OUT `0x01`
* **Input Packet:** 36 bytes containing all analog faders, EQ knobs, rotary encoders, 8 RGB cue pads, and navigation/transport buttons.
* **Output Packet:** 80 bytes controlling 8 RGB cue pads, dual 7-segment loop displays, stereo channel VU meters, and transport LEDs.

---

## 7-Segment / 8-Segment Loop Displays

The Kontrol Z2 features two dual 8-segment (7 segments + decimal point) LED displays for Deck A (left) and Deck B (right), driven through the LED Output Report (`all_leds`):
* **Byte Offset 1:** `display_a_1` (Deck A, tens / sub-beat digit)
* **Byte Offset 2:** `display_a_2` (Deck A, units digit)
* **Byte Offset 3:** `display_b_1` (Deck B, tens / sub-beat digit)
* **Byte Offset 4:** `display_b_2` (Deck B, units digit)

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

| Loop Length | Left Digit (`display_a_1`) | Right Digit (`display_a_2`) | Rendered Readout |
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
let z2 = encdr.scan()?[0];

// Display an active 1/4 beat loop on Deck A
encdr.set_loop_display(z2, "display_a_1", "display_a_2", 0.25, true);

// Display a 16 beat loop on Deck B
encdr.set_loop_display(z2, "display_b_1", "display_b_2", 16.0, false);
```
