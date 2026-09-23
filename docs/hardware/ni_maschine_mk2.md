# NI Maschine Mk2 Hardware Reference

## Overview

| Property     | Value                                                    |
| ------------ | -------------------------------------------------------- |
| Manufacturer | Native Instruments                                       |
| Product      | Maschine Mk2                                             |
| VID:PID      | `0x17cc:0x1140`                                          |
| USB Speed    | Full Speed (12 Mbps) / High Speed                        |
| Interfaces   | 1 (Combined HID Control & Screen interface)              |
| Screens      | 2x 256 x 64 Monochrome LCD Displays (Left and Right)     |

The Maschine Mk2 is a pad-based groovebox production controller featuring dual 256x64 monochrome LCD displays, 8 continuous rotary encoders, 16 velocity/pressure-sensitive RGB pads arranged in a 4x4 grid, 8 RGB Group buttons (A-H), transport controls, and function/mode navigation buttons (top_1-top_8, Control, Step, Browser, Sampling, etc.).

---

## Getting Started

### Linux Requirements

The Linux kernel `hidraw` driver automatically binds to the Maschine Mk2. `encdr` utilizes the `detach_kernel_driver` quirk to claim the interface.

1. **Udev Rules (Required):** Create or update `/etc/udev/rules.d/99-ni-controllers.rules`:
   ```bash
   SUBSYSTEM=="usb", ATTR{idVendor}=="17cc", MODE="0666"
   SUBSYSTEM=="hidraw", ATTRS{idVendor}=="17cc", ATTRS{idProduct}=="1140", MODE="0666"
   ```
2. **Reload Rules:**
   ```bash
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```

---

## Physical Layout

```
┌──────────────────────────────────────────────────────────────────────────┐
│                                                                          │
│  [Control] [Step]  [top_1][top_2][top_3][top_4][top_5][top_6][top_7][top_8]│
│  [Browser] [Sampling]                                                    │
│  [◄ Page ] [Page ►]                                                      │
│                                                                          │
│  ┌───────────────────────┐  ┌───────────────────────┐                    │
│  │                       │  │                       │   [Volume][Tempo]  │
│  │     LEFT SCREEN       │  │     RIGHT SCREEN      │   [Swing]          │
│  │       256 x 64        │  │       256 x 64        │   [Nav  ] [Enter]  │
│  │                       │  │                       │   [◄ Nav] [Nav ►]  │
│  └───────────────────────┘  └───────────────────────┘   (Volume Encoder) │
│  (enc1) (enc2) (enc3) (enc4) (enc5) (enc6) (enc7) (enc8)                │
│                                                                          │
│  [Grp A] [Grp B] [Grp C] [Grp D]   [Scene]  [Pattern] [Pad Mode]           │
│  [Grp E] [Grp F] [Grp G] [Grp H]   [Navigate][Duplicate][Select]         │
│                                    [Solo]   [Mute]                       │
│  [Pad13] [Pad14] [Pad15] [Pad16]                                         │
│  [Pad 9] [Pad10] [Pad11] [Pad12]   [Restart][Step ◄][Step ►][Grid]      │
│  [Pad 5] [Pad 6] [Pad 7] [Pad 8]   [Play]   [Rec]   [Erase] [Shift]     │
│  [Pad 1] [Pad 2] [Pad 3] [Pad 4]                                         │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## USB Protocol Specification

### Input Packets

#### 1. Button & Encoder Report (`0x01`)
Report ID `0x01` is 24 bytes long and reports physical button presses and rotary encoder relative movements. Standardized names matching the Mk3 descriptor are used across controls:

* **Bytes 1–6 (Button Bitmask):**
  * `Byte 1`: `top_8` (`0x01`), `top_7` (`0x02`), `top_6` (`0x04`), `top_5` (`0x08`), `top_4` (`0x10`), `top_3` (`0x20`), `top_2` (`0x40`), `top_1` (`0x80`)
  * `Byte 2`: `auto` (`0x01`), `all` (`0x02`), `arrow_left` (`0x04`), `arrow_right` (`0x08`), `sampling` (`0x10`), `browser` (`0x20`), `step` (`0x40`), `control` (`0x80`)
  * `Byte 3`: `navigate` (`0x01`), `note_repeat` (`0x02`), `enter` (`0x04`), `nav_right` (`0x08`), `nav_left` (`0x10`), `tempo` (`0x20`), `swing` (`0x40`), `volume` (`0x80`)
  * `Byte 4`: `group_h` (`0x01`), `group_g` (`0x02`), `group_f` (`0x04`), `group_e` (`0x08`), `group_d` (`0x10`), `group_c` (`0x20`), `group_b` (`0x40`), `group_a` (`0x80`)
  * `Byte 5`: `shift` (`0x01`), `erase` (`0x02`), `rec` (`0x04`), `play` (`0x08`), `grid` (`0x10`), `step_right` (`0x20`), `step_left` (`0x40`), `restart` (`0x80`)
  * `Byte 6`: `mute` (`0x01`), `solo` (`0x02`), `select` (`0x04`), `duplicate` (`0x08`), `nav_mode` (`0x10`), `pad_mode` (`0x20`), `pattern` (`0x40`), `scene` (`0x80`)

* **Bytes 8–23 (Relative Encoders):**
  `screen_encoder_1` through `screen_encoder_8` output relative step deltas across even byte offsets `8, 10, 12, 14, 16, 18, 20, 22`.

#### 2. Pad Pressure Report (`0x20`)
Report ID `0x20` is 32 bytes long, containing 16x 12-bit pad pressure readings (`pad_1` through `pad_16`).

---

### Output Packets (LED Control)

The Maschine Mk2 uses three output HID reports to update controller LEDs:

1. **Pad RGB LEDs (Report `0x80`):**
   * Length: 49 bytes (`0x80` prefix byte + 16x 3-byte RGB triplets for `pad_1`..`pad_16`).

2. **Monochrome Button LEDs (Report `0x82`):**
   * Length: 32 bytes (`0x82` prefix byte + 31 monochrome button brightness bytes).
   * Controls `top_1`..`top_8`, `control`, `step`, `browser`, `sampling`, `arrow_left`/`arrow_right`, `all`, `auto`, `note_repeat`, `navigate`, etc.

3. **Group RGB & Transport LEDs (Report `0x81`):**
   * Length: 57 bytes (`0x81` prefix byte + 8x RGB triplets for `group_a`..`group_h` plus single bytes for transport controls).

---

### Screen Output Protocol

The Mk2 dual 256x64 monochrome LCD screens are written using band-sliced transfer reports:
* **Left Display:** Report ID `0xE0`
* **Right Display:** Report ID `0xE1`

Each screen is updated in 8 vertical bands of 256 bytes (8 rows x 32 bytes = 256 bytes per band).
