# NI Komplete Kontrol S-Series Mk1 Hardware Reference

## Overview

| Property     | Value                                                                             |
| ------------ | --------------------------------------------------------------------------------- |
| Manufacturer | Native Instruments                                                                |
| Product      | Komplete Kontrol S25 Mk1 / S49 Mk1 / S61 Mk1 / S88 Mk1                            |
| VID:PID      | `0x17cc:0x1340` (S25), `0x17cc:0x1350` (S49), `0x17cc:0x1360` (S61), `0x17cc:0x1410` (S88) |
| USB Speed    | Full Speed (12 Mbps)                                                              |
| Endpoints    | Interrupt In `0x81`, Interrupt Out `0x01`                                         |
| Screens      | 8x 128 x 32 monochrome OLED displays (`display_1` .. `display_8`)                |
| Light Guide  | Per-key RGB/single strip (25, 49, 61, or 88 elements)                             |

The Native Instruments Komplete Kontrol S-Series Mk1 keyboards combine Fatar keybeds with per-key Light Guide illumination, 8 capacitive rotary encoders with dedicated 128x32 OLED parameter displays, dual touch strips (pitch & mod), directional navigation, and dedicated transport/function controls.

---

## Physical Layout

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                        │
│  [Shift] [Scale] [Arp]                                                                 │
│                                                                                        │
│  [Oct-] [Oct+]   ┌────────┐┌────────┐┌────────┐┌────────┐┌────────┐┌────────┐┌────────┐┌────────┐│
│  [Pre-] [Pre+]   │ DISP 1 ││ DISP 2 ││ DISP 3 ││ DISP 4 ││ DISP 5 ││ DISP 6 ││ DISP 7 ││ DISP 8 ││
│                  └────────┘└────────┘└────────┘└────────┘└────────┘└────────┘└────────┘└────────┘│
│  [Play] [Rec ]    (enc 1)   (enc 2)   (enc 3)   (enc 4)   (enc 5)   (enc 6)   (enc 7)   (enc 8) │
│  [Stop] [Loop]                                                               [▲]       │
│  [Inst] [◄Page][Page►]                                                    [◄](Nav)[►]  │
│                                                                              [▼]       │
│  [Pitch Strip] [Mod Strip]                                                             │
│                                                                                        │
│  ▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼ LIGHT GUIDE ▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼                   │
│  | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | |                     │
│  | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | |                     │
│  |_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|                     │
│                                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Controls Mapping

### Buttons & Navigation (Report 32 bytes)

Buttons report on endpoint `0x81`:

| Byte | Bit Mask | Control Name |
| ---- | -------- | ------------ |
| 1    | `0x01`   | `shift`      |
| 1    | `0x02`   | `scale`      |
| 1    | `0x04`   | `arp`        |
| 1    | `0x10`   | `preset_up`  |
| 1    | `0x20`   | `preset_down`|
| 1    | `0x40`   | `octave_down`|
| 1    | `0x80`   | `octave_up`  |
| 2    | `0x01`   | `play`       |
| 2    | `0x02`   | `rec`        |
| 2    | `0x04`   | `stop`       |
| 2    | `0x08`   | `loop`       |
| 2    | `0x10`   | `instance`   |
| 2    | `0x20`   | `page_left`  |
| 2    | `0x40`   | `page_right` |
| 3    | `0x01`   | `nav_left`   |
| 3    | `0x02`   | `nav_right`  |
| 3    | `0x04`   | `nav_up`     |
| 3    | `0x08`   | `nav_down`   |
| 3    | `0x10`   | `nav_press`  |

### Touch Sensors & Encoders

Each of the 8 knobs provides capacitive touch sensing and rotary encoder ticks:

- **Touch Sensors (Byte 4)**: `touch_1` (`0x01`) through `touch_8` (`0x80`).
- **Encoders (Bytes 5-8)**: 4-bit nibble relative encoders (`wrap16`):
  - Byte 5: `encoder_1` (bits 0-3), `encoder_2` (bits 4-7)
  - Byte 6: `encoder_3` (bits 0-3), `encoder_4` (bits 4-7)
  - Byte 7: `encoder_5` (bits 0-3), `encoder_6` (bits 4-7)
  - Byte 8: `encoder_7` (bits 0-3), `encoder_8` (bits 4-7)

---

## LEDs & Light Guide

Outputs are transmitted via Interrupt Out `0x01`:

1. **Button LEDs (Prefix `0x80`)**:
   - Protocol: `linear_7bit` (0-127 brightness levels).
   - Offsets 0-13 map to `shift`, `scale`, `arp`, `preset_up`, `preset_down`, `octave_down`, `octave_up`, `play`, `rec`, `stop`, `loop`, `instance`, `page_left`, `page_right`.

2. **Light Guide (Prefix `0x81`)**:
   - Individual LED illumination above each key on the keybed.
   - S25: 25 LEDs (76-byte buffer)
   - S49: 49 LEDs (148-byte buffer)
   - S61: 61 LEDs (184-byte buffer)
   - S88: 88 LEDs (265-byte buffer)

---

## Parameter Displays

The Mk1 series features 8 separate monochrome OLED displays (128 x 32 pixels each) situated directly above the 8 rotary knobs:

- Displays are addressed as `display_1` through `display_8`.
- Blit command prefixes: `0xe0` for `display_1`, up to `0xe7` for `display_8`.
- Format: 1-bit monochrome (`PixelFormat::Mono`), 512 bytes per full frame blit.
