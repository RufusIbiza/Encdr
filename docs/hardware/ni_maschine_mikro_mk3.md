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
  - Report `0x01` (64 bytes): 40 button bits in bytes 1..5; encoder touch at byte 6 bit 0; encoder position modulo 16 in byte 7's low nibble.
  - The same report contains Smart Strip positions in bytes 10..11 and 12..13 (little endian). Zero means no contact; positions are normalized by 200. Encdr exposes `touchstrip` / `touchstrip_touch` and `touchstrip_dual` / `touchstrip_dual_touch`. In the tested two-finger gestures, positions follow left-to-right order, not finger identity.
  - Report `0x02` (64 bytes): 16-pad velocity and real-time polyphonic aftertouch pressure stream, decoded by the existing event-tuple parser. Raw pad indices start at the top row; `pad_1` is bottom-left and `pad_13` top-left.
* **Output Packets:**
  - Report `0x80` (81 bytes including report ID): all function-button, pad and Smart Strip LEDs share this report. Descriptor offsets count payload bytes: buttons 0..38, pads 39..54 (top row first), Smart Strip 55..79 (left to right). The encoder press/touch have no LEDs; there is no separate `0x81` pad LED report in this mapping.
  - Function-button levels used by this descriptor:
    - `0x00`: **Off** (`LedValue::Off`)
    - `0x7C`: **Low** (`LedValue::Backlight`)
    - `0x7E`: **Medium** (`LedValue::Dim`)
    - `0x7F`: **Bright** (`LedValue::Bright`)
    - These are discrete levels; the default NHL2 percentage conversion does not apply to this device.
  - Pad LEDs use the NI packed palette format (`(color_id << 2) | (intensity & 0x03)`). Each Smart Strip segment has its own byte; full white `71` was used for the position check. Arbitrary strip colors and the full set of brightness values remain unverified.
* **Screen Protocol:** 128×32 1-bit monochrome OLED, using the `ni_mikro_mk3` protocol. Each frame is two 265-byte transfers: headers `e0 00 00 00 00 80 00 02 00` and `e0 00 00 02 00 80 00 02 00`, each followed by 256 page-formatted bytes. A page contains 128 columns of eight vertical pixels, least significant bit at the top. Wire pixels are inverted (zero lights a pixel); Encdr converts ordinary row-major `PixelFormat::Mono` frames.
