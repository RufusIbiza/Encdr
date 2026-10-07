# NI Komplete Kontrol S-Series Mk3 Hardware Reference

## Overview

| Property     | Value                                                              |
| ------------ | ------------------------------------------------------------------ |
| Manufacturer | Native Instruments                                                 |
| Product      | Komplete Kontrol S49 Mk3 / S61 Mk3 / S88 Mk3                       |
| VID:PID      | `0x17cc:0x2100` (S49), `0x17cc:0x2110` (S61), `0x17cc:0x2120` (S88)|
| USB Speed    | High Speed (480 Mbps)                                              |
| Screen       | 1x High-Resolution Full-Color Glass Display (On-Device Rendering)  |
| Protocols    | Direct USB HID + Bulk Transport (ODR MsgPack-RPC) + DAW MIDI/SysEx |

The Komplete Kontrol S-Series Mk3 features an on-board dual-core ARM SoC running embedded Linux, polyphonic aftertouch keybeds, a continuous unibody glass surface with a full-color display, anodized aluminum rotary encoders, 4D directional encoder, pitch and modulation wheels, multi-touch expression strip, and RGB per-key Light Guide.

Unlike previous generation hardware (e.g. Maschine Mk3 or Kontrol S4 Mk3) which stream raw framebuffers directly from the host computer, the Mk3 utilizes an **On-Device Rendering (ODR)** architecture: the keyboard executes its own rendering engine locally and is driven via structured state models and MessagePack-RPC, alongside a direct **DAW Remote MIDI/SysEx** channel for DAW integration.

---

## USB Interfaces & Endpoints

| Interface | Type | Direction | Endpoint | Description |
|-----------|------|-----------|----------|-------------|
| 0 | Interrupt | IN | `0x81` | Real-time inputs: buttons, encoders, touchstrip, expression |
| 0 | Interrupt | OUT | `0x01` | Button LEDs, mode indicators, and per-key RGB Light Guide |
| 0 | Bulk | OUT | `0x03` | ODR Command / RPC pipe |
| 0 | Bulk | OUT | `0x04` | ODR Data / Asset stream pipe |
| 0 | Bulk | IN | `0x84` | ODR Event / Return stream pipe |

---

## Low-Level Hardware Reports (Interrupt Endpoint)

```text
I-1 b28 n2 I-2 b8 WC b10 I-3 b8 W3 W2 B1 I-AA W19 O-80 B2A O-81 B1E O-82 B31 O-A0 BE O-A1 BC8 W1 B1 O-A2 B2C O-A3 B90 O-A4 B80 O-AF B2 O-F3 B1 O-F4 B20 F-D0 D1 B1C F-D8 D2 W4 B10 F-D9 B20 F-F8 W2 B6
```

### Input Reports (`0x81` IN)

* **Report `0x01` (`I-1`):**
  * `b28` (40 bits = 5 bytes): Button state bitmask.
  * `n2` (2 nibbles = 1 byte): 4D encoder relative notched counter (`wrap16`).
* **Report `0x02` (`I-2`):**
  * `b8 WC b10`: Pitch and modulation expression touchstrip data.
* **Report `0x03` (`I-3`):**
  * `b8 W3 W2 B1`: Pedals and expression inputs.
* **Report `0xAA` (`I-AA`):**
  * `W19` (19 words = 38 bytes): 8 touch-sensitive continuous rotary encoders.

### Output Reports (`0x01` OUT)

* **Report `0x80` (`O-80`):** `B2A` (42 bytes): Button and mode indicator LEDs.
* **Report `0x81` (`O-81`):** `B1E` (30 bytes): Secondary button LEDs.
* **Report `0x82` (`O-82`):** Per-key RGB Light Guide:
  * **S49 Mk3:** `B31` (49 bytes for 49 keys)
  * **S61 Mk3:** `B3D` (61 bytes for 61 keys)
  * **S88 Mk3:** `B58` (88 bytes for 88 keys)

---

## DAW Remote Control Protocol (MIDI CC & SysEx)

The keyboard exposes a dedicated bidirectional DAW port (e.g. `KONTROL S49/S61/S88 MK3 DAW`, `MIDIIN2/MIDIOUT2`, or `NKS Connect`). This interface provides direct access to on-screen mixer tracks, VU meters, track colors, plugin chains, parameter titles, and values without requiring proprietary services.

### Connection & Handshake

All DAW channel messages use **MIDI Channel 16** (status byte `0xBF` for Control Change):

| Event | Status | Data 1 | Data 2 | Description |
|-------|--------|--------|--------|-------------|
| **Hello** | `0xBF` | `0x01` | `0x04` | Handshake greeting (DAW protocol v4) |
| **Enable 14-bit SysEx** | `0xBF` | `0x06` | `0x01` | Enables high-resolution 14-bit parameter updates |
| **Goodbye** | `0xBF` | `0x02` | `0x00` | Session deactivation / cleanup |

### Button Control Changes (Channel 16)

| Button | CC (Hex) | Range / Value | Behavior |
|--------|----------|---------------|----------|
| **Shift** | `0x04` | `0` or `127` | Momentary |
| **Play** | `0x10` | `0` or `1` | Toggle LED |
| **Record** | `0x12` | `0` or `1` | Toggle LED |
| **Count-In** | `0x13` | `0` or `1` | Toggle LED |
| **Stop** | `0x14` | `0` or `1` | Momentary / Toggle |
| **Loop** | `0x16` | `0` or `1` | Toggle LED |
| **Metronome** | `0x17` | `0` or `1` | Toggle LED |
| **Tap Tempo** | `0x18` | `0` or `127` | Momentary |
| **Undo** | `0x20` | `0` or `1` | Toggle / State |
| **Redo** | `0x21` | `0` or `1` | Toggle / State |
| **Quantize** | `0x22` | `0` or `1` | Toggle / State |
| **Auto** | `0x23` | `0` or `1` | Automation write enable |

### Navigation & Jog (Channel 16)

Navigation controls transmit relative 2's complement values:

| Control | CC (Hex) | Description |
|---------|----------|-------------|
| **Bank Mapping Mode** | `0x05` | `0`: Mixer tracks, `1`: Plugin parameters |
| **Track Select** | `0x30` | Relative increment/decrement (`0x01` / `0x7F`) |
| **Bank Select** | `0x31` | Relative bank navigation |
| **Clip Select** | `0x32` | Relative clip navigation |
| **Move Transport** | `0x34` | Nudge cursor / transport playhead |
| **Move Loop** | `0x35` | Nudge cycle loop range |

### Mixer Channel Strips & Knobs (Channel 16)

| Target | CC (Hex) | Format / Function |
|--------|----------|-------------------|
| **Track Select (0..7)** | `0x42` | Raw index selection |
| **Track Mute (0..7)** | `0x43` | Mute toggle |
| **Track Solo (0..7)** | `0x44` | Solo toggle |
| **Selected Track Volume** | `0x64` | Relative 2's complement fine adjustment |
| **Selected Track Pan** | `0x65` | Relative 2's complement fine adjustment |
| **Track Volume Knobs (0..7)** | `0x50` .. `0x57` | Absolute 7-bit value (`0..127`) |
| **Track Pan Knobs (0..7)** | `0x58` .. `0x5F` | Absolute 7-bit value (`0..127`) |
| **Parameter Knobs (0..7)** | `0x70` .. `0x77` | Absolute 7-bit value (`0..127`) |

---

## SysEx Display & Text Protocol

Display content, VU meters, track labels, and parameter pages are managed through standard SysEx packets.

### SysEx Frame Header

```text
F0 00 21 09 00 00 44 43 01 00 <command_id> <value> <track_index> <payload...> F7
```

* Manufacturer Sysex ID: `0x00 0x21 0x09`
* Sub-header: `0x00 0x00 0x44 0x43 0x01 0x00`
* Terminating Byte: `0xF7`

### SysEx Command Table

| ID (Hex) | Command Name | `value` | `track` | Payload Description |
|----------|--------------|---------|---------|---------------------|
| `0x03` | `surface_configuration` | `1` | `0` | String: `"track_orientation"` (enables vertical layout) |
| `0x07` | `identity` | Major | Minor | String: Host Application / DAW name |
| `0x19` | `set_tempo` | `0` | `0` | 5 bytes: 10ns per beat encoded across 7-bit chunks |
| `0x40` | `track_enabled` | `0` or `1` | `0..7` | Empty payload |
| `0x41` | `focus_follow` | `0` | `0` | String: NI Focus Follow parameter target name |
| `0x42` | `track_selected` | `0` or `1` | `0..7` | Empty payload |
| `0x43` | `mute_button` | `0` or `1` | `0..7` | Empty payload |
| `0x44` | `solo_button` | `0` or `1` | `0..7` | Empty payload |
| `0x45` | `track_armed` | `0` or `1` | `0..7` | Empty payload |
| `0x46` | `volume` | `0` | `0..7` | String: Formatted display text (e.g. `"-3.5 dB"`) |
| `0x47` | `pan` | `0` | `0..7` | String: Formatted display text (e.g. `"L 20"`, `"C"`) |
| `0x48` | `track_name` | `0` | `0..7` | String: Track name ASCII string |
| `0x49` | `vu_meter` | `2` | `0` | 16 raw bytes: 8 pairs of `(Left, Right)` 7-bit dB values |
| `0x4B` | `track_color` | `0` | `0..7` | String: Hex color `#AARRGGBB` (e.g. `"#FF0088FF"`) |
| `0x70` | `selected_track_select_plugin`| `0` | `chain_idx` | Empty payload |
| `0x71` | `selected_track_plugin_chain_info` | `0` | `0` | String: Null-separated (`\0`) list of plugin names |
| `0x72` | `selected_track_param_name` | `0` | `knob_idx` | String: Parameter label for knob (0..7) |
| `0x73` | `selected_track_param_display_value` | `0` | `knob_idx` | String: Parameter formatted value (e.g. `"440 Hz"`) |
| `0x74` | `selected_track_param_page_num` | `total_pages` | `current_page` | Empty payload (page count & current page index) |
| `0x7F` | `adjust_parameter_value` | — | — | 14-bit knob increment from hardware (see below) |

### 14-bit High Resolution Parameter Adjustments (`0x7F`)

When 14-bit updates are enabled (`0xBF 0x06 0x01`), the keyboard sends high-resolution relative adjustments:

* Byte 11: Group (`0x00` = Volume, `0x01` = Pan, `0x02` = Track Parameter)
* Byte 12: Knob Index (`0..7`)
* Byte 13: LSB (`0..127`)
* Byte 14: MSB (`0..127`)

Decoding formula:
```rust
let diff = (lsb as i16) | ((msb as i16) << 7);
let signed_diff = if (diff & 0x2000) != 0 { diff - 0x4000 } else { diff };
let delta_float = (signed_diff as f32) / 8192.0;
```

---

## On-Device Rendering (ODR) & MessagePack-RPC Stack

For direct control over embedded UI views (Browser, Scales, Arp, Device Settings, Custom Graphics Cache), the keyboard runs an embedded MessagePack-RPC engine over USB bulk pipes or a local service bridge.

### Transport

* **Physical USB:** Interface 0, Bulk Out `0x03` (RPC commands), Bulk Out `0x04` (data/assets), Bulk In `0x84` (stream responses).
* **Local Socket:** Named socket `com.native-instruments.nks-connect` or TCP port `5454` (when TCP bridge is enabled).

### Wire Protocol Framing

Standard MessagePack-RPC framing:
* **Notification (`0x93`):** `[2, method_name, [params...]]`
* **Request (`0x94`):** `[0, msg_id, method_name, [params...]]`
* **Response (`0x94`):** `[1, msg_id, error, result]`

### Key Model Methods

* `client_lightguide_set_leds`: Direct programmatic control of the entire Light Guide RGB strip.
* `client_host_set_project_tree`: Hierarchical track and group layout.
* `client_host_set_selected_track`: Focused track selection for embedded view routing.
* `client_host_set_track_plugin_chain`: Active plugins on the current channel.
* `client_instance_mixer_set_meters`: Continuous stereo and multi-bus level metering.
* `client_instance_mixer_set_track_data`: Track volumes, pans, mutes, solos, and colors.
* `client_instance_parameter_page_model_set`: Full multi-page parameter definitions and banks.
* `client_smartplay_set_data`: Scales, key modes, chord sets, and arpeggiator engine configurations.
* `asset_device_model`: Chunked upload of graphic assets (icons, plug-in artwork) directly into the keyboard's high-speed flash cache.
* `midi_template_transfer`: Export and import of standalone MIDI templates.
