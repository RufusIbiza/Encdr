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
  - Report `0x80` (48 bytes): Single-color function buttons and 25-segment Smart Strip LED bar.
  - Report `0x81` (20 bytes): 16 RGB pads driven via Native Instruments packed palette byte format.
* **Screen Protocol:** 128×32 1-bit monochrome OLED display using custom NI frame header (`0xe0 0x00 0x00 0x00 0x00 0x80 0x00 0x02`).
