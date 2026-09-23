# Native Instruments Maschine Plus (Controller Mode)

Hardware reference for the **Native Instruments Maschine Plus** (`0x17cc:0x1820`) running in USB Controller Mode.

## Overview

The Maschine Plus is an embedded standalone Linux music production workstation. When connected to a host computer via USB and switched into **Controller Mode** (via `Settings -> Controller Mode`), its USB OTG controller presents an interface identical to the **Maschine Mk3** (`0x17cc:0x1600`).

- **Vendor ID:** `0x17cc`
- **Product ID:** `0x1820`
- **Architecture:** Dual $480 \times 272$ color LCD displays, 16 RGB velocity/aftertouch pads, 8 rotary encoders with capacitive touch, 4D encoder, Smart Strip, and illuminated buttons.

## USB Interfaces & Endpoints

| Interface | Type | Direction | Endpoint | Transfer | Description |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Interface 4** | Control / HID | IN | `0x83` | Interrupt (64B) | Buttons, 4D encoder, knob touches, Smart Strip, pad reports |
| | Control / HID | OUT | `0x03` | Interrupt (64B) | Button backlights, RGB pads, Smart Strip LEDs |
| **Interface 5** | Display | OUT | `0x04` | Bulk (512B) | Screen blit commands for left and right displays |

## Input Reports

### Report `0x01`: Buttons & Encoders (42 bytes)
Contains the state of all 63 buttons, 4D encoder rotary and directional clicks, and knob touch capacitive sensors.

### Report `0x02`: Smart Strip (63 bytes)
Reports capacitive position, pressure, and gesture dynamics across the touchstrip.

### Report `0x20`: Velocity & Pressure Pads (128 bytes)
Uses the verified double-pumped 128-byte tuple protocol reverse-engineered in Encdr:
- Two 64-byte chunks (`Set A` and `Set B`) per USB transfer.
- Up to 21 3-byte tuples: `[pad_idx, flags_and_high_nibble, low_byte]`.
- 12-bit pressure resolution ($0..4095$).

## Output Reports & LEDs

- **Report `0x80` (`buttons`)**: 62 bytes controlling button backlights (white single-color brightness $0..255$).
- **Report `0x81` (`pad_leds`)**: 41 bytes controlling 16 RGB pad LEDs via Native Instruments indexed color palette.

## Dual Color Screens

- **Dimensions:** Dual $480 \times 272$ pixels (960 total width across both panels).
- **Pixel Format:** 16-bit BGR565 big-endian (`PixelFormat::Bgr565Be`).
- **Framing:** Bulk OUT transfer to Endpoint `0x04`:
  - **Left Screen Full Blit Header:** `0x84, 0x00, 0x00, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xe0, 0x01, 0x10, 0x00, 0x00, 0xff, 0x00`
  - **Right Screen Full Blit Header:** `0x84, 0x00, 0x01, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xe0, 0x01, 0x10, 0x00, 0x00, 0xff, 0x00`
  - **Footer:** `0x03, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00` (Left) / `...0x01, 0x00` (Right)
- **Partial Blit:** Fully supported with 4-pixel X-alignment and 2-pixel Y-alignment.
