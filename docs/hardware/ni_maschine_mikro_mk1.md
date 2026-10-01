# NI Maschine Mikro Mk1 Hardware Reference

## Overview

| Property     | Value                                    |
| ------------ | ---------------------------------------- |
| Manufacturer | Native Instruments                       |
| Product      | Maschine Mikro Mk1                       |
| VID:PID      | `0x17cc:0x1110`                          |
| USB Speed    | Full Speed (12 Mbps)                     |
| Interfaces   | 1 (HID Control interface 0)              |
| Screens      | 1 (128×64 monochrome graphic LCD)        |

The Maschine Mikro Mk1 is a compact groove production controller featuring 16 velocity- and pressure-sensitive pads (monochrome backlighting), a 128×64 high-contrast monochrome graphic display, an endless rotary encoder with push action, and dedicated navigation and transport controls.

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
* **Output Packet:** 48 bytes controlling all button LEDs and single-color pad backlights.
* **Screen Protocol:** 128×64 1-bit monochrome display using custom NI frame header (`0xe0 0x00 0x00 0x00 0x00 0x80 0x00 0x02`).
