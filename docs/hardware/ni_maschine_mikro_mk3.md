# NI Maschine Mikro Mk3 Hardware Reference

## Overview

| Property     | Value                                    |
| ------------ | ---------------------------------------- |
| Manufacturer | Native Instruments                       |
| Product      | Maschine Mikro Mk3                       |
| VID:PID      | `0x17cc:0x1700`                          |
| USB Speed    | Full Speed (12 Mbps)                     |
| Interfaces   | 1 (HID Control interface 0)              |
| Screens      | 1 (128×32 monochrome OLED)               |

The Maschine Mikro Mk3 is a streamlined production controller featuring 16 large multi-color RGB velocity- and polyphonic aftertouch-sensitive pads, a dual-touch Smart Strip with a 25-segment LED meter bar, a compact 128×32 monochrome OLED display, a push-action encoder, and dedicated performance mode selectors.

---

## Physical Layout

```
┌──────────────────────────────────────────────┐
| [ 128x32 OLED ]           ( Encoder / Push ) |
|                                              |
| [Group] [Auto] [Lock] [Sample] [Pitch] [Mod] [Perform] [Notes] |
|                                              |
|  ═══════════ 25-LED SMART STRIP ═══════════  |
|                                              |
| ┌──────┬──────┬──────┬──────┐                |
| │Pad 13│Pad 14│Pad 15│Pad 16│                |
| ├──────┼──────┼──────┼──────┤                |
| │Pad 9 │Pad 10│Pad 11│Pad 12│                |
| ├──────┼──────┼──────┼──────┤                |
| │Pad 5 │Pad 6 │Pad 7 │Pad 8 │                |
| ├──────┼──────┼──────┼──────┤                |
| │Pad 1 │Pad 2 │Pad 3 │Pad 4 │                |
| └──────┴──────┴──────┴──────┘                |
|                                              |
| [Pad Mode] [Keyboard] [Chords] [Step]        |
| [Scene] [Pattern] [Events] [Variation]       |
| [Duplicate] [Select] [Solo] [Mute]           |
| [Shift] [Erase] [Restart] [Play] [Rec]       |
└──────────────────────────────────────────────┘
```

---

## USB Endpoints & Reports

* **Interface 0 (Control):** Interrupt IN `0x81`, Interrupt OUT `0x01`
* **Input Packets:**
  - `buttons` (17 bytes): Rotary encoder, navigation, transport, and mode buttons.
  - `touchstrip` (8 bytes): 10-bit touchstrip slider position and touch contact detection.
  - `pads` (64 bytes): 16-pad velocity and real-time polyphonic aftertouch pressure stream.
* **Output Packets:**
  - Report `0x80` (48 bytes): Single-color function buttons (offsets 1..27) and 25-segment Smart Strip LED bar (offsets 28..52). Function buttons use Native Instruments' 4-level discrete duty cycle protocol:
    - `0x00`: **Off** (`LedValue::Off` / `LedValue::OFF`)
    - `0xE4` (228): **Dim / Half-brightness** (`LedValue::Dim` / `LedValue::DIM`) — idle / half-brightness
    - `0x9E` (158): **Bright / Active** (`LedValue::Bright` / `LedValue::BRIGHT`) — active state
    - `0xFF` (255): **Max drive** (`LedValue::Single(255)` / `LedValue::MAX`)
    - Arbitrary duty cycle percentages ($0..100\%$) encode as `0x80 | pct` via `LedValue::single_percent(pct)`.
  - Report `0x81` (20 bytes): 16 RGB pads driven via Native Instruments packed palette byte format (`(color_id << 2) | (intensity & 0x03)`).
* **Screen Protocol:** 128×32 1-bit monochrome OLED display using custom NI frame header (`0xe0 0x00 0x00 0x00 0x00 0x80 0x00 0x02`).
