# NI Maschine Mikro Mk2 Hardware Reference

## Overview

| Property     | Value                                    |
| ------------ | ---------------------------------------- |
| Manufacturer | Native Instruments                       |
| Product      | Maschine Mikro Mk2                       |
| VID:PID      | `0x17cc:0x1200`                          |
| USB Speed    | Full Speed (12 Mbps)                     |
| Interfaces   | 1 (HID Control interface 0)              |
| Screens      | 1 (128×64 monochrome graphic LCD)        |

The Maschine Mikro Mk2 upgrades the Mikro platform with full multi-color RGB backlighting for all 16 velocity- and pressure-sensitive pads, enhanced button backlighting, a 128×64 high-contrast monochrome graphic display, and an endless push-rotary encoder.

---

## Physical Layout

```
┌──────────────────────────────────────────────┐
| [ 128x64 MONO LCD ]       ( Encoder / Push ) |
|                                              |
| [F1] [F2] [F3] [Group] [Browse] [Sample] [Step] |
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
| [Restart] [Play] [Rec] [Erase] [Grid] [Note] |
| [Shift]   [Nav]  [View]                      |
└──────────────────────────────────────────────┘
```

---

## USB Endpoints & Reports

* **Interface 0 (Control):** Interrupt IN `0x81`, Interrupt OUT `0x01`
* **Input Packets:**
  - `buttons` (16 bytes): Rotary encoder and 28 function/transport buttons.
  - `pads` (64 bytes): Velocity and polyphonic pressure data for the 16 pads.
* **Output Packet:** 80 bytes controlling all 16 RGB pad backlights (via 3-byte RGB offsets) and function button LEDs.
* **Screen Protocol:** 128×64 1-bit monochrome display using custom NI frame header (`0xe0 0x00 0x00 0x00 0x00 0x80 0x00 0x02`).
