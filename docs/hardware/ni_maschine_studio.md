# Native Instruments Maschine Studio

Hardware reference for the **Native Instruments Maschine Studio** (`0x17cc:0x1300`).

## Overview

The Maschine Studio was Native Instruments' flagship production controller, featuring an expanded control surface with dual high-resolution color displays, a large optical jogwheel with a 32-segment LED ring, dedicated stereo audio level meters, an edit section, and 16 RGB velocity/aftertouch pads.

- **Vendor ID:** `0x17cc`
- **Product ID:** `0x1300`
- **Architecture:** Pure USB controller (no integrated audio interface). Dual $480 \times 272$ LCD displays, 10 rotary encoders, 64 buttons, 16 RGB pads, 4-report LED engine.

## USB Interfaces & Endpoints

| Interface | Type | Direction | Endpoint | Transfer | Description |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Interface 0** | Control / HID | IN | `0x81` | Interrupt (64B) | Button matrix, encoders, jogwheel, pads |
| | Control / HID | OUT | `0x01` | Interrupt (64B) | 4-report LED updates (`0x80`, `0x81`, `0x82`, `0x83`) |
| **Interface 1** | Display | OUT | `0x02` | Bulk (512B) | Screen blit commands for left and right displays |

## Input Reports

### Report `0x01`: Buttons, Jogwheel & Knobs (42 bytes)
The button matrix spans 16 bytes:
- **Audio Routing & Master:** `in_1`, `in_2`, `in_3`, `in_4`, `master`, `group`, `sound`, `cue`
- **Knob Touches:** `knob_touch_1` through `knob_touch_5` capacitive sensing
- **Top Display Buttons:** `top_1` through `top_8` above the screens
- **Edit Cluster:** `copy`, `paste`, `note`, `nudge`, `undo`, `redo`, `quantize`, `clear`
- **Jogwheel Navigation:** `jogwheel_press`, `back`, `nav_left`, `nav_right`, `enter`
- **Group Selectors:** `group_a` through `group_h`
- **View Modes:** `channel`, `plugin`, `arrange`, `mix`, `browse`, `sampling`, `all`, `auto`
- **Transport & Performance:** `loop`, `metro`, `select`, `grid`, `play`, `rec`, `erase`, `tap`, `snap`, `macro`, `note_repeat`
- **Pad Modes:** `scene`, `pattern`, `pad_mode`, `navigate`, `duplicate`, `pad_select`, `solo`, `mute`
- **Encoders:**
  - `jogwheel`: 16-bit wide wraparound (`wrap16_wide`) with 1000.0 scale factor.
  - `level`: 4-bit wraparound master level encoder (`wrap16`).
  - `knob_1` through `knob_8`: Signed 16-bit delta display encoders.

### Report `0x20`: Velocity & Pressure Pads
Reports 16 analog channels corresponding to velocity and pressure on the 16 physical pads.

## Output Reports & LED Engine

The Maschine Studio divides its extensive LED hardware across **4 distinct output reports**:

### 1. Button LEDs (Report `0x80`, 63 bytes)
Controls backlighting for transport controls, edit functions, sequencer modes, and view buttons.

### 2. Pad & Group RGB LEDs (Report `0x81`, 45 bytes)
- **Group Buttons:** Full RGB backlights for `group_a` through `group_h` (`offsets: { r, g, b }`).
- **Pads:** 16 RGB pads (`pad_1` through `pad_16`) using Native Instruments indexed color palette.

### 3. Master Section & Audio Level Meters (Report `0x82`, 53 bytes)
- **Master Buttons:** Backlights for `in_1`..`in_4`, `master`, `group`, `sound`, `cue`.
- **Top Display Buttons:** `top_1`..`top_8`.
- **Level Meters:**
  - `meter_left`: 16-segment stereo level ladder (Left channel).
  - `meter_right`: 16-segment stereo level ladder (Right channel).

```rust
// Setting audio peak meter ladders:
let mut left = vec![0u8; 16];
left[..12].fill(255); // 12 of 16 segments
encdr.set_led_strip(device_id, "meter_left", &left);
```

### 4. Jogwheel LED Ring (Report `0x83`, 56 bytes)
- `jogwheel_ring`: 32-segment circular LED ring around the perimeter of the jogwheel.

```rust
// Illuminating a position marker on the jogwheel ring:
let mut ring = vec![0u8; 32];
ring[16] = 255;
encdr.set_led_strip(device_id, "jogwheel_ring", &ring);
```

## Dual Color Screens

- **Dimensions:** Dual $480 \times 272$ pixels per panel.
- **Pixel Format:** 16-bit BGR565 big-endian (`PixelFormat::Bgr565Be`).
- **Framing:** Bulk OUT transfers to Interface 1, Endpoint `0x02`.
- **Full Blit Headers:**
  - Left: `0x84,0x00,0x00,0x60,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x01,0xe0,0x01,0x10,0x00,0x00,0xff,0x00`
  - Right: `0x84,0x00,0x01,0x60,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x01,0xe0,0x01,0x10,0x00,0x00,0xff,0x00`
- **Partial Blit:** Fully supported with 4-pixel horizontal alignment and 2-pixel vertical alignment.
