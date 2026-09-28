# Native Instruments Traktor Kontrol S4 MK3 Hardware Protocol

- **Vendor ID**: `0x17cc`
- **Product ID**: `0x1720`
- **Class**: USB Composite (Audio + HID + Bulk Vendor Screen Interface)

---

## 1. USB Interface Architecture

| Interface | Type | Direction | Endpoint | Usage |
|---|---|---|---|---|
| **0, 1, 2** | Audio / MIDI | IN / OUT | — | Multi-channel audio interface |
| **3** | HID / Control | IN | `0x83` (Interrupt) | Button presses, encoders, faders, jogwheels |
| **3** | HID / Control | OUT | `0x02` (Interrupt) | LEDs, VU meters, motorized jogwheel torque |
| **4** | Bulk / Screen | OUT | `0x03` (Bulk) | Dual color deck display framebuffers |

---

## 2. Input Reports (Interface 3, EP 0x83)

### Report 1: Buttons, Deck Switches & Encoders (25 bytes)
- **Byte 0**: Deck left master, browse press, star, play
- **Byte 1**: Deck left view, playlist, FX 1..3, reverse, flux, FX on
- **Byte 2**: Mixer FX unit assign (Channels 1–4 to FX 1/2)
- **Byte 3**: Left deck 8 RGB performance pads (bits 0..7)
- **Byte 4**: Left deck transport (play, cue, pad modes: hotcue, record, samples, mute)
- **Byte 5**: Left deck stems mode, shift, deck switch A/C, jog mode, TT mode, grid, sync
- **Byte 6**: Left deck encoder presses (loop encoder bit 2, move encoder bit 5)
- **Byte 7**: Mixer PFL cue buttons and FX select buttons
- **Byte 8**: Mixer QuickEffect preset buttons (Filter, 1..4)
- **Byte 9**: Right deck view, star, play, playlist, FX 1..3, FX on
- **Byte 10**: Right deck master, browse press, reverse, flux
- **Byte 11**: Mixer quantize (bit 6), Ch4 FX2 (bit 7)
- **Byte 12**: Right deck transport (play, stems, hotcue, record, samples, mute)
- **Byte 13**: Right deck 8 RGB performance pads (bits 0..7)
- **Byte 14**: Right deck jog mode, shift, deck switch B/D, sync, cue, TT mode, grid
- **Byte 15**: Right deck encoder presses (loop encoder bit 2, move encoder bit 5)
- **Byte 16**: Jogwheel top capacitive touch plates (Left bit 4, Right bit 5)
- **Bytes 19..21**: 4-bit nibble encoders (Loop, Move, Browse for Left & Right decks)

### Report 2: Potentiometers & Faders (78 bytes, 16-bit uints)
- **Bytes 0..1**: Crossfader
- **Bytes 2..9**: Channel volume faders 1–4
- **Bytes 10..13**: Left and right tempo pitch faders
- **Bytes 14..21**: Channel gain pots
- **Bytes 22..29**: Master, Booth, Headphone Mix pots
- **Bytes 30..37**: FX Unit 1 dry/wet and knobs 1–3
- **Bytes 38..61**: 3-band EQs for Channels 1–4
- **Bytes 62..69**: QuickEffect knobs for Channels 1–4
- **Bytes 70..77**: FX Unit 2 dry/wet and knobs 1–3

### Report 3: Jog Wheel Position & Velocity (48 bytes)
- **Bytes 11..12**: Left wheel relative delta counter (16-bit)
- **Bytes 15..16**: Left wheel absolute position (0..2879 steps per revolution)
- **Bytes 39..40**: Right wheel relative delta counter (16-bit)
- **Bytes 43..44**: Right wheel absolute position (0..2879 steps per revolution)

---

## 3. Output Reports (Interface 3, EP 0x02)

### Report 0x80 (128): Buttons & RGB Pad LEDs (94 bytes)
- **Bytes 0..7**: Left deck pad colors (NI indexed palette 0..127)
- **Bytes 8..22**: Left deck button backlight LEDs
- **Bytes 23..30**: Right deck pad colors (NI indexed palette 0..127)
- **Bytes 31..45**: Right deck button backlight LEDs
- **Bytes 46..49**: Mixer FX select LEDs
- **Bytes 55..76**: Transport, shift, flux, and FX LEDs
- **Bytes 77..93**: Mixer PFL cue, FX assign, and quantize LEDs

### Report 0x81 (129): Mixer VU Level Meters (78 bytes)
- 14 segments per channel (13 level segments + 1 peak clip LED)
- Brightness levels: 0 (off), 125 (dim), 127 (full)

### Report 0x31 (49): Motorized Jogwheel Haptic Drive (10 bytes)
Direct motor torque and velocity feedback:
- **Bytes 0..4**: Left deck motor packet:
  - `[0]`: `0x01` (Command header)
  - `[1]`: `0x20` (Forward direction) / `0xe0` (Reverse direction)
  - `[2]`: `0x01` (Forward) / `0xfe` (Reverse)
  - `[3]`: Velocity / Force LSB (`velocity & 0xFF`)
  - `[4]`: Velocity / Force MSB (`velocity >> 8`)
- **Bytes 5..9**: Right deck motor packet (same structure for right motor)

### Report 0x32 (50): Jog Wheel LED Ring (40 bytes)
- **Byte 0**: Deck index (`0` for Left, `1` for Right)
- **Byte 1**: LED Mode:
  - `0`: Off
  - `1`: Dim flash
  - `2`: Needle position spot
  - `3`: Ring flash
  - `4`: Dim spot
  - `5`: Individually addressable LEDs
- **Bytes 2..3**: Needle position (0..2879, little-endian)
- **Byte 4**: Base color + brightness
- **Bytes 8..39**: (In mode 5) Individual RGB values for LEDs around the ring

---

## 4. Dual Screen Blit Protocol (Interface 4, EP Bulk 0x03)

- **Resolution**: 320 x 240 pixels each (Left deck & Right deck)
- **Pixel Format**: RGB565 / BGR565 Big-Endian
- **Header (20 bytes)**:
  - Left screen: `0x84, 0x00, 0x00, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x40, 0x00, 0xf0, 0x00, 0x00, 0xff, 0x00`
  - Right screen: `0x84, 0x00, 0x01, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x40, 0x00, 0xf0, 0x00, 0x00, 0xff, 0x00`
- **Payload**: 320 * 240 * 2 = 153,600 bytes
- **Footer (8 bytes)**:
  - Left screen: `0x03, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00`
  - Right screen: `0x03, 0x00, 0x00, 0x00, 0x40, 0x00, 0x01, 0x00`
