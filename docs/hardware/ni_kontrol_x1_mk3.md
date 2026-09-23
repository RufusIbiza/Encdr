# NI Traktor Kontrol X1 MK3 Hardware Reference

## Overview

| Property     | Value                                                              |
| ------------ | ------------------------------------------------------------------ |
| Manufacturer | Native Instruments                                                 |
| Product      | Traktor Kontrol X1 MK3                                             |
| VID:PID      | `0x17cc:0x2200`                                                    |
| USB Speed    | Full Speed (12 Mbps) / High Speed                                  |
| Interfaces   | 1 (Combined HID Control & OLED Display interface)                  |
| Screens      | 5x 128 x 64 Monochrome OLED Displays (Left/Right FX, Loops, Mode)  |

The Traktor Kontrol X1 MK3 is a modular performance DJ controller featuring 5 high-contrast monochrome OLED displays, 4 continuous rotary push encoders, 8 analog potentiometers for dual FX units, 21 tactile transport and performance buttons, RGB backlit hotcue buttons, and customizable RGB underglow/ambient light bars.

---

## Getting Started

### Linux Requirements

The Linux kernel `hidraw` driver automatically binds to the Traktor Kontrol X1 MK3. `encdr` utilizes the `detach_kernel_driver` quirk to claim the interface.

1. **Udev Rules (Required):** Create or update `/etc/udev/rules.d/99-ni-controllers.rules`:
   ```bash
   SUBSYSTEM=="usb", ATTR{idVendor}=="17cc", ATTR{idProduct}=="2200", MODE="0666"
   SUBSYSTEM=="hidraw", ATTRS{idVendor}=="17cc", ATTRS{idProduct}=="2200", MODE="0666"
   ```
2. **Reload Rules:**
   ```bash
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```

### Running the Example

An interactive test application is provided in the repository demonstrating real-time input parsing, dynamic LED/RGB feedback, and concurrent rendering across all 5 monochrome OLED screens:

```bash
cargo run -p encdr-examples --bin x1_mk3_test
```

---

## Physical Layout

```
┌─────────────────────────────────────────────────────────────┐
│                      NI KONTROL X1 MK3                      │
│                                                             │
│   ┌──────────────┐                       ┌──────────────┐   │
│   │   LEFT FX    │        [ MODE ]       │   RIGHT FX   │   │
│   │   128 x 64   │      ┌──────────┐     │   128 x 64   │   │
│   └──────────────┘      │   MODE   │     └──────────────┘   │
│   (FX1) (FX2) (FX3)(FX4)│ 128 x 64 │     (FX5) (FX6) (FX7)(FX8)
│   [ 1 ] [ 2 ] [ 3 ][ 4 ]└──────────┘     [ 1 ] [ 2 ] [ 3 ][ 4 ]
│                                                             │
│   ┌──────────────┐                       ┌──────────────┐   │
│   │  LEFT LOOP   │                       │  RIGHT LOOP  │   │
│   │   128 x 64   │                       │   128 x 64   │   │
│   └──────────────┘                       └──────────────┘   │
│    (LEFT LOOP)                            (RIGHT LOOP)      │
│   (LEFT BROWSE)                          (RIGHT BROWSE)     │
│                                                             │
│   [ Hotcue 1 ] [ Hotcue 2 ]              [ Hotcue 1 ] [ Hotcue 2 ]
│   [ Hotcue 3 ] [ Hotcue 4 ]              [ Hotcue 3 ] [ Hotcue 4 ]
│                                                             │
│   [ ◄ Nudge ]  [ Nudge ► ]               [ ◄ Nudge ]  [ Nudge ► ]
│   [ Assign L ] [ Assign R ]              [ Assign L ] [ Assign R ]
│                                                             │
│   [ REV ]      [ SYNC ]     [ SHIFT ]    [ REV ]      [ SYNC ]
│   [ CUE ]      [ PLAY ]                  [ CUE ]      [ PLAY ]
│                                                             │
│   ═══════════════════════════════════════════════════════   │
│   [ Underglow Ambient Light Guide L ] [ Underglow R ]       │
└─────────────────────────────────────────────────────────────┘
```

---

## USB Protocol Specification

### Protocol String

```
I-1 b30 n4 W8 O-80 B31 O-A0 B2 O-E0 W4 B100 O-E1 W4 B100 O-E2 W4 B100 O-E3 W4 B100 O-E4 W4 B100 F-D0 D1 B1C F-D8 D2 W4 B10 F-F8 W2 B6 F-F9 W2 B6 F-FA W2 B6 F-FB W2 B6 F-FC W2 B6
```

### Protocol Breakdown

* `I-1`: Interrupt IN Report ID `0x01`.
  * `b30`: 48 bits (6 bytes) button bitmask.
  * `n4`: 4 nibbles (2 bytes) rotary encoders (`wrap16`).
  * `W8`: 8 16-bit analog words (potentiometers).
* `O-80 B31`: Interrupt OUT Report ID `0x80`, 49 bytes payload (`B31`) for button backlights, RGB hotcues, and underglow.
* `O-A0 B2`: Interrupt OUT Report ID `0xA0`, 2 bytes configuration.
* `O-E0 W4 B100` – `O-E4 W4 B100`: Interrupt OUT Reports `0xE0`..`0xE4`, 1,024 bytes (4 slices of 256 bytes) per OLED screen.
* `F-F8 W2 B6` – `F-FC W2 B6`: Feature reports for screens 1 through 5 initialization/power.

---

### Input Packets

#### Report ID `0x01` (Controls, 25 bytes)

* **Byte 0**: Report ID (`0x01`).
* **Byte 1 (System / Navigation Buttons)**:
  * `0x01`: `shift`
  * `0x02`: `mode`
  * `0x04`: `left_browse_press`
  * `0x08`: `left_loop_press`
  * `0x10`: `right_browse_press`
  * `0x20`: `right_loop_press`
* **Byte 2 (FX Buttons)**:
  * `0x01`: `left_fx_1`
  * `0x02`: `left_fx_2`
  * `0x04`: `left_fx_3`
  * `0x08`: `left_fx_4`
  * `0x10`: `right_fx_1`
  * `0x20`: `right_fx_2`
  * `0x40`: `right_fx_3`
  * `0x80`: `right_fx_4`
* **Byte 3 (Hotcue Buttons)**:
  * `0x01`: `left_hotcue_1`
  * `0x02`: `left_hotcue_2`
  * `0x04`: `left_hotcue_3`
  * `0x08`: `left_hotcue_4`
  * `0x10`: `right_hotcue_1`
  * `0x20`: `right_hotcue_2`
  * `0x40`: `right_hotcue_3`
  * `0x80`: `right_hotcue_4`
* **Byte 4 (Transport Buttons)**:
  * `0x01`: `left_play`
  * `0x02`: `left_cue`
  * `0x04`: `left_rev`
  * `0x08`: `left_sync`
  * `0x10`: `right_play`
  * `0x20`: `right_cue`
  * `0x40`: `right_rev`
  * `0x80`: `right_sync`
* **Byte 5 (Assign & Nudge Buttons)**:
  * `0x01`: `left_assign_left`
  * `0x02`: `left_assign_right`
  * `0x04`: `left_nudge_slow`
  * `0x08`: `left_nudge_fast`
  * `0x10`: `right_assign_left`
  * `0x20`: `right_assign_right`
  * `0x40`: `right_nudge_slow`
  * `0x80`: `right_nudge_fast`
* **Byte 7 (Left Encoders, 4-bit `wrap16`)**:
  * Bits 7..4: `left_browse`
  * Bits 3..0: `left_loop`
* **Byte 8 (Right Encoders, 4-bit `wrap16`)**:
  * Bits 7..4: `right_browse`
  * Bits 3..0: `right_loop`
* **Bytes 9–24 (Analog Potentiometers, 12-bit little-endian)**:
  * Bytes 9–10: `left_fx_knob_1`
  * Bytes 11–12: `left_fx_knob_2`
  * Bytes 13–14: `left_fx_knob_3`
  * Bytes 15–16: `left_fx_knob_4`
  * Bytes 17–18: `right_fx_knob_1`
  * Bytes 19–20: `right_fx_knob_2`
  * Bytes 21–22: `right_fx_knob_3`
  * Bytes 23–24: `right_fx_knob_4`

---

### Output Packets (LED Control)

#### Report ID `0x80` (LEDs, 49 bytes)

* **Byte 0**: Report ID (`0x80`).
* **Bytes 1–10 (Single Brightness 0–127)**:
  * `shift` (1), `mode` (2), `left_fx_1..4` (3..6), `right_fx_1..4` (7..10).
* **Bytes 11–34 (RGB Triplets 0–255)**:
  * `left_hotcue_1..4`: Offsets 11..22.
  * `right_hotcue_1..4`: Offsets 23..34.
* **Bytes 35–42 (Transport Single Brightness 0–127)**:
  * `left_play` (35), `left_cue` (36), `left_rev` (37), `left_sync` (38).
  * `right_play` (39), `right_cue` (40), `right_rev` (41), `right_sync` (42).
* **Bytes 43–48 (RGB Underglow Light Bars)**:
  * `underglow_left`: Offsets 43..45 (R, G, B).
  * `underglow_right`: Offsets 46..48 (R, G, B).

---

### Screen Protocol

The X1 MK3 features 5 identical 128x64 monochrome OLED displays:

| Screen Name   | Report ID | Resolution | Frame Bytes | Description         |
| ------------- | --------- | ---------- | ----------- | ------------------- |
| `left_fx`     | `0xE0`    | 128 x 64   | 1,024       | Left FX Unit        |
| `left_loop`   | `0xE1`    | 128 x 64   | 1,024       | Deck A Loop / Beat  |
| `center_mode` | `0xE2`    | 128 x 64   | 1,024       | Center Mode Status  |
| `right_loop`  | `0xE3`    | 128 x 64   | 1,024       | Deck B Loop / Beat  |
| `right_fx`    | `0xE4`    | 128 x 64   | 1,024       | Right FX Unit       |

* **Pixel Format**: 1-bit monochrome (`mono`), MSB-first row-major.
* **Buffer Size**: 128 / 8 = 16 bytes per row $\times$ 64 rows = 1,024 bytes per frame.
* **Blit Transfer**: 1,025 bytes submitted via Interrupt OUT Endpoint `0x01` (`[Report ID] + [1024 bytes bitmap]`).
