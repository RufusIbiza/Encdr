# Native Instruments Traktor Kontrol S4 MK2 Hardware Protocol

- **Vendor ID**: `0x17cc`
- **Product ID**: `0x1310`
- **Class**: USB Composite (Audio + HID)

---

## 1. USB Interface Architecture

| Interface | Type | Direction | Endpoint | Usage |
|---|---|---|---|---|
| **0, 1, 2** | Audio / MIDI | IN / OUT | — | Multi-channel sound card & MIDI ports |
| **3** | HID / Control | IN | `0x83` (Interrupt) | Buttons, jog wheels (32-bit delta), faders, EQs |
| **3** | HID / Control | OUT | `0x02` (Interrupt) | RGB pad LEDs, button backlights, VU meters, 7-segment loop displays |

---

## 2. Input Packets (Interface 3, EP 0x83)

### Packet 1: Buttons & Jog Wheels (20 bytes)
- **Bytes 1..4**: Left jog wheel 32-bit position counter
- **Bytes 5..8**: Right jog wheel 32-bit position counter
- **Byte 10**: Right load, deck switch, cue B/D, quantize, snap
- **Byte 11**: Right remix slots 1..4, loop out/in, flux, reset
- **Byte 12**: Right hotcue 1..4, shift, sync, cue, play
- **Byte 13**: Left hotcue 1..4, shift, sync, cue, play
- **Byte 14**: Left remix slots 1..4, loop out/in, flux, reset
- **Byte 15**: Left load, deck switch, cue A/C, browse encoder press, jog wheel touch detection
- **Byte 16**: FX buttons, loop/tempo encoder presses
- **Byte 17**: Headphone cue buttons, mixer buttons

### Packet 2: Sliders, EQs & Potentiometers (80 bytes)
- 4-channel mixer strips (Gain, 3-band EQ, Filter, Fader)
- Crossfader & Pitch tempo faders (12-bit ADCs normalized to 4096)
- FX unit knobs 1–3 + Dry/Wet

---

## 3. Output Packets (Interface 3, EP 0x02)

### Report 0x80: Deck LEDs & RGB Pads (64 bytes)
- RGB pads for Decks A & B with 3-byte RGB offsets (R, G, B)
- Single-byte backlights for transport, cue, loop, and FX buttons

### Report 0x81: Mixer LEDs & VU Level Meters (64 bytes)
- Channel level meters (Left/Right)
- Clip indicators

### Report 0xd5: 7-Segment Loop Displays & Status Dots (32 bytes)
The S4 MK2 features two 2-digit 7-segment LED displays (Deck A/C loop length on the left, Deck B/D loop length on the right) and dedicated status dots.

| Byte Offset | Target Control | Description |
|---|---|---|
| **4** | `left_loop_digit_1` | Deck A (or C) Left Digit (7-segment bitmask) |
| **8** | `left_loop_digit_2` | Deck A (or C) Right Digit (7-segment bitmask) |
| **12** | `right_loop_digit_1` | Deck B (or D) Left Digit (7-segment bitmask) |
| **16** | `right_loop_digit_2` | Deck B (or D) Right Digit (7-segment bitmask) |
| **20** | `left_loop_dot` | Deck A (or C) dedicated status dot LED (0=Off, 255=On) |
| **24** | `right_loop_dot` | Deck B (or D) dedicated status dot LED (0=Off, 255=On) |

#### 7-Segment Bitmask Architecture:
```text
      -- a (bit 0, 0x01) --
     |                     |
  f (bit 5, 0x20)       b (bit 1, 0x02)
     |                     |
      -- g (bit 6, 0x40) --
     |                     |
  e (bit 4, 0x10)       c (bit 2, 0x04)
     |                     |
      -- d (bit 3, 0x08) --     * dp (bit 7, 0x80)
```

| Bit | Hex | Segment | Description |
|---|---|---|---|
| **0** | `0x01` | `SEG_A` | Top horizontal bar |
| **1** | `0x02` | `SEG_B` | Top-right vertical bar |
| **2** | `0x04` | `SEG_C` | Bottom-right vertical bar |
| **3** | `0x08` | `SEG_D` | Bottom horizontal bar |
| **4** | `0x10` | `SEG_E` | Bottom-left vertical bar |
| **5** | `0x20` | `SEG_F` | Top-left vertical bar |
| **6** | `0x40` | `SEG_G` | Center horizontal bar |
| **7** | `0x80` | `SEG_DP`| Decimal point |

#### Traktor Loop Size Display Notation:
Whole beats display the number directly across both digits. Fractional sub-beats run the counter in reverse with the decimal dot illuminated:

| Loop Length | Left Digit (`left_loop_digit_1`) | Right Digit (`left_loop_digit_2`) | Rendered Readout |
|---|---|---|---|
| **32 beats** | `'3'` (`0x4F`) | `'2'` (`0x5B`) | `32` |
| **16 beats** | `'1'` (`0x06`) | `'6'` (`0x7D`) | `16` |
| **8 beats**  | Blank (`0x00`) | `'8'` (`0x7F`) | ` 8` |
| **4 beats**  | Blank (`0x00`) | `'4'` (`0x66`) | ` 4` |
| **2 beats**  | Blank (`0x00`) | `'2'` (`0x5B`) | ` 2` |
| **1 beat**   | Blank (`0x00`) | `'1'` (`0x06`) | ` 1` |
| **1/2 beat** | `.` (`0x80`)   | `'2'` (`0x5B`) | `.2` |
| **1/4 beat** | `.` (`0x80`)   | `'4'` (`0x66`) | `.4` |
| **1/8 beat** | `.` (`0x80`)   | `'8'` (`0x7F`) | `.8` |
| **1/16 beat**| `'1.'` (`0x86`)| `'6'` (`0x7D`) | `1.6` |
| **1/32 beat**| `'3.'` (`0xCF`)| `'2'` (`0x5B`) | `3.2` |

When a loop is active:
- The right digit's decimal point (`SEG_DP`, `0x80`) is illuminated (e.g. `.4.` for active 1/4 beat).
- The dedicated status dot LED (`left_loop_dot` / `right_loop_dot`, byte offset 20 / 24) is illuminated (`255`).

#### Encdr Code Example:
```rust
use encdr::{Encdr, EncdrConfig, SevenSegment};

let encdr = Encdr::new(EncdrConfig::default())?;
let s4 = encdr.scan()?[0];

// Display a 1/4 beat active loop on Deck A
encdr.set_loop_display_with_dot(
    s4,
    "left_loop_digit_1",
    "left_loop_digit_2",
    Some("left_loop_dot"),
    0.25,
    true, // Active loop -> renders '.4.' and illuminates left_loop_dot
);

// Display arbitrary text (e.g. "1.6")
encdr.set_seven_segment_str(s4, "right_loop_digit_1", "right_loop_digit_2", "1.6");
```

#### Legacy MIDI Mapping:
In standard MIDI mode, the loop displays are addressed via Control Change messages:
- **Left Digit**: CC `78` (Channels 1–4 for Decks A–D)
- **Right Digit**: CC `79` (Channels 1–4 for Decks A–D)
- **Status Dot**: CC `77` (Channels 1–4 for Decks A–D)


