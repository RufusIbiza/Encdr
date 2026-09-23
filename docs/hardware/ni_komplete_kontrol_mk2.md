# NI Komplete Kontrol S-Series Mk2 Hardware Reference

## Overview

| Property     | Value                                                              |
| ------------ | ------------------------------------------------------------------ |
| Manufacturer | Native Instruments                                                 |
| Product      | Komplete Kontrol S49 Mk2 / S61 Mk2 / S88 Mk2                       |
| VID:PID      | `0x17cc:0x1610` (S49), `0x17cc:0x1620` (S61), `0x17cc:0x1630` (S88)|
| USB Speed    | High Speed (480 Mbps)                                              |
| Interfaces   | 4 total (Audio/MIDI: 0 & 1, HID Control: 2, Screens: 3)            |
| Screens      | 2x 480 x 272, BGR565 big-endian (left and right)                   |

The Native Instruments Komplete Kontrol S-Series Mk2 is a premium smart keyboard controller featuring two 4.3" high-resolution color displays, eight touch-sensitive rotary encoders, a 4D push encoder with directional joystick navigation, an RGB Light Guide above each key, dedicated DAW transport and navigation controls, pitch/modulation wheels, and a multi-touch expressive touchstrip.

---

## Physical Layout

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                        │
│  [Pitch] [Mod]  [Scale] [Arp]      [Top1][Top2][Top3][Top4][Top5][Top6][Top7][Top8]    │
│                 [Undo ] [Redo]                                                         │
│                 [Quant] [Auto]   ┌───────────────────────┐  ┌───────────────────────┐  │
│                                  │                       │  │                       │  │
│  [Loop ] [Metro] [Tempo]         │     LEFT SCREEN       │  │     RIGHT SCREEN      │  │
│  [Play ] [Rec  ] [Stop ]         │      480 x 272        │  │      480 x 272        │  │
│                                  │                       │  │                       │  │
│  [Track] [Plugin][Mixer]         └───────────────────────┘  └───────────────────────┘  │
│  [Scene] [Mute ] [Solo ]         (enc1) (enc2) (enc3) (enc4) (enc5) (enc6) (enc7) (enc8│
│                                                                                        │
│  [Preset▲] [Page◄] [Page►]                                               [▲]           │
│  [Preset▼] [MIDI ] [Setup]                                           [◄] (4D) [►]      │
│                                                                          [▼]           │
│  ════════════════ EXPRESSIVE TOUCHSTRIP ════════════════                               │
│                                                                                        │
│  ▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼ RGB LIGHT GUIDE ▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼                       │
│  | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | |                     │
│  | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | | |                     │
│  |_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|_|                     │
│                                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Getting Started

### Linux Requirements

Under Linux, the kernel's generic USB HID driver automatically claims Interface 2 (HID Control). `encdr` requires a `udev` rule to grant non-root access and uses its built-in kernel driver detachment quirk.

1. **Udev Rules:**
   Create `/etc/udev/rules.d/99-ni-controllers.rules`:
   ```bash
   SUBSYSTEM=="usb", ATTR{idVendor}=="17cc", MODE="0666"
   ```
   Then reload `udev`:
   ```bash
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```

2. **Automatic Detachment:**
   `encdr` automatically sets the `detach_kernel_driver` and `dual_handle` quirks for Komplete Kontrol Mk2 devices, allowing simultaneous access to Interface 2 (Control) and Interface 3 (Dual Displays).

---

## USB Interfaces & Endpoints

| Interface | ID        | Number | Endpoint In        | Endpoint Out       | Type               | Description                        |
| --------- | --------- | ------ | ------------------ | ------------------ | ------------------ | ---------------------------------- |
| Audio/MIDI| `midi`    | 0 & 1  | `0x81` (bulk)      | `0x01` (bulk)      | USB Audio Class 2  | Piano key presses (standard MIDI)  |
| Control   | `control` | 2      | `0x82` (interrupt) | `0x02` (interrupt) | HID Control        | Buttons, 4D encoder, knobs, LEDs   |
| Screen    | `screen`  | 3      | —                  | `0x03` (bulk)      | Vendor Specific    | Dual 480x272 BGR565 display frames |
| DFU       | `dfu`     | 4      | —                  | —                  | Application DFU    | Firmware upgrade                   |

---

## Input Reports (Interface 2, Endpoint `0x82`)

### Buttons Packet (Report `0x01`, 32 bytes)

Sent on interrupt IN endpoint `0x82` when any button or 4D navigation element is pressed, released, or rotated.

#### Top Screen Buttons (8)

| Name    | Byte | Mask   | Description                          |
| ------- | ---- | ------ | ------------------------------------ |
| `top_1` | 1    | `0x10` | Screen 1 Button 1 (leftmost)         |
| `top_2` | 1    | `0x20` | Screen 1 Button 2                    |
| `top_3` | 1    | `0x40` | Screen 1 Button 3                    |
| `top_4` | 1    | `0x80` | Screen 1 Button 4                    |
| `top_5` | 1    | `0x01` | Screen 2 Button 1                    |
| `top_6` | 1    | `0x02` | Screen 2 Button 2                    |
| `top_7` | 1    | `0x04` | Screen 2 Button 3                    |
| `top_8` | 1    | `0x08` | Screen 2 Button 4 (rightmost)        |

#### Transport & Mode Buttons

| Name          | Byte | Mask   | Description                         |
| ------------- | ---- | ------ | ----------------------------------- |
| `play`        | 2    | `0x10` | Play button                         |
| `restart`     | 2    | `0x20` | Restart / Loop button               |
| `stop`        | 3    | `0x01` | Stop button                         |
| `rec`         | 3    | `0x02` | Record button                       |
| `metro`       | 3    | `0x08` | Metronome button                    |
| `preset_up`   | 3    | `0x10` | Preset Next button                  |
| `page_right`  | 3    | `0x20` | Page Right (>) button               |
| `preset_down` | 3    | `0x40` | Preset Previous button              |
| `page_left`   | 3    | `0x80` | Page Left (<) button                |
| `mute`        | 4    | `0x01` | Track Mute button                   |
| `solo`        | 4    | `0x02` | Track Solo button                   |
| `scene`       | 4    | `0x04` | Scene selector                      |
| `clear`       | 4    | `0x20` | Clear / Pattern reset               |
| `plugin`      | 5    | `0x02` | Plugin mode switch                  |
| `setup`       | 5    | `0x08` | Hardware Setup menu                 |
| `midi`        | 5    | `0x20` | MIDI Controller mode switch         |

#### 4D Push Encoder Navigation

The 4D push encoder operates as a directional joystick, push button, and rotary notched dial:

| Name                 | Byte | Bits / Mask | Encoding   | Description               |
| -------------------- | ---- | ----------- | ---------- | ------------------------- |
| `encoder_main_press` | 6    | `0x08`      | Button     | Main 4D encoder push      |
| `encoder_main_left`  | 6    | `0x10`      | Button     | Main 4D joystick left     |
| `encoder_main_up`    | 6    | `0x20`      | Button     | Main 4D joystick up       |
| `encoder_main_down`  | 6    | `0x40`      | Button     | Main 4D joystick down     |
| `encoder_main_right` | 6    | `0x80`      | Button     | Main 4D joystick right    |
| `encoder_main`       | 30   | 4-bit       | `wrap16`   | Rotary notched jog counter|

### Continuous Encoders (Report `0xAA`, 51 bytes)

The 8 rotary encoders underneath the dual displays send values in Report `0xAA`:
- Encoder 1..8 values are located at `DATA_IN[17 + (i * 2)]`.

---

## Output Reports & LED Control (Interface 2, Endpoint `0x02`)

### 1. Initialization Handshake

Before the hardware accepts Light Guide updates, it must receive an activation handshake on the control interface interrupt OUT endpoint:
```
[0xA0, 0x00, 0x00]
```

### 2. Button LEDs (Report `0x80`, 80 bytes)

Button backlights are controlled via Report ID `0x80` (followed by up to 79 brightness bytes, where 0 is off and 255 is maximum intensity):

| Name          | Offset | Description                   |
| ------------- | ------ | ----------------------------- |
| `top_1`       | 3      | Display button 1 (leftmost)   |
| `top_2`       | 4      | Display button 2              |
| `top_3`       | 5      | Display button 3              |
| `top_4`       | 6      | Display button 4              |
| `top_5`       | 7      | Display button 5              |
| `top_6`       | 8      | Display button 6              |
| `top_7`       | 9      | Display button 7              |
| `top_8`       | 10     | Display button 8 (rightmost)  |
| `shift`       | 15     | Shift button                  |
| `page_left`   | 23     | Page Left (<) button          |
| `mute`        | 25     | Mute button                   |
| `solo`        | 26     | Solo button                   |
| `page_right`  | 28     | Page Right (>) button         |
| `play`        | 30     | Play button                   |
| `rec`         | 31     | Record button                 |
| `stop`        | 32     | Stop button                   |
| `preset_up`   | 33     | Preset Up button              |
| `preset_down` | 34     | Preset Down button            |
| `plugin`      | 37     | Plugin mode button            |
| `midi`        | 40     | MIDI mode button              |

### 3. Key Light Guide (Report `0x81`, 250 bytes)

The Light Guide consists of individual RGB LEDs positioned directly above every piano key.
- **Prefix byte:** `0x81`
- **Payload:** 249 bytes (1 byte per key LED)

#### Native Instruments Palette Encoding (1 Byte per Key)

Each key LED is encoded in a **single byte**:
- `0x00`: Key LED Off
- **Bits 7..2:** Color palette index (`1..17`)
- **Bits 1..0:** Intensity level (`0..3`, where 0 is dimmest and 3 is full brightness)

Formula: `packed_byte = (color_id << 2) | (intensity & 0x03)`

##### Hardware Color Palette:

| Color ID | Name                 | Color ID | Name           |
| -------- | -------------------- | -------- | -------------- |
| 1        | Red                  | 10       | Sky Blue       |
| 2        | Orange-Red           | 11       | Blue           |
| 3        | Orange               | 12       | Indigo         |
| 4        | Warm Yellow / Amber  | 13       | Violet / Purple|
| 5        | Yellow               | 14       | Magenta / Pink |
| 6        | Lime                 | 15       | Rose           |
| 7        | Green                | 16       | Crimson        |
| 8        | Mint                 | 17       | White          |
| 9        | Cyan                 |          |                |

#### Key Offsets by Model

| Model | Keys | Lowest Note      | Buffer Range |
| ----- | ---- | ---------------- | ------------ |
| S49   | 49   | C1 (MIDI note 36)| Offsets 0..48|
| S61   | 61   | C1 (MIDI note 36)| Offsets 0..60|
| S88   | 88   | A-1 (MIDI note 21)| Offsets 0..87|

---

## Dual Color Displays (Interface 3, Endpoint `0x03` Bulk)

| Property               | Value                                     |
| ---------------------- | ----------------------------------------- |
| Count                  | 2 (left and right)                        |
| Resolution             | 480 x 272 pixels each                     |
| Pixel Format           | BGR565 big-endian                         |
| Full frame payload     | 261,120 bytes (480 * 272 * 2) per screen  |
| Interface              | `screen` (Interface 3)                    |
| Endpoint               | `0x03` (bulk out)                         |
| Partial Update Support | Yes (aligned to 4px horizontal, 2px vert) |

The screen protocol is identical to the Maschine Mk3 display protocol, selecting the target display via byte `[2]` in the 20-byte command header:
- `0x00`: Left Screen
- `0x01`: Right Screen

### Full Blit Transfer Format

A screen blit frame consists of:
```
[20-byte header] [261,120 bytes BGR565 pixel data] [8-byte footer]
```

- **Header (Left Screen):**
  `84 00 00 60 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00`
- **Header (Right Screen):**
  `84 00 01 60 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00`
- **Footer:**
  `03 00 00 00 40 00 00 00`
