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
| 1 | Bulk | IN | `0x81` | DAW MIDI In (USB MIDI Streaming Class) |
| 1 | Bulk | OUT | `0x01` | DAW MIDI Out (USB MIDI Streaming Class) |
| 2 | Interrupt | IN | `0x82` | Real-time inputs: buttons, encoders, touchstrip, expression (HID) |
| 2 | Interrupt | OUT | `0x02` | Button LEDs, mode indicators, RGB Light Guide (HID) |
| 3 | Bulk | OUT | `0x03` | ODR heartbeat only (`odr_cmd`) |
| 3 | Bulk | OUT | `0x04` | ODR handshake and all RPC commands, incl. Light Guide (`odr_stream`) |
| 3 | Bulk | IN | `0x83` | ODR replies and events from the keyboard, e.g. knob turns (`odr_stream`) |
| — | — | — | — | A separate DFU interface (Windows driver `nikksm3dfu`) is used only for firmware updates. |

On Windows, NI's driver package binds WinUSB to interface 3 (Device Manager: `KONTROL S49 MK3 ODR`, service `WINUSB`), so it can be claimed without Zadig. On macOS interface 3 is claimed as raw USB and interface 2 goes through IOHIDManager. NI's background services must be stopped first, since they hold interface 3.

---

## Low-Level Hardware Reports (Interrupt Endpoint)

> These HID report layouts come from NI's protocol definitions and are not yet mapped in Encdr's descriptor (only the 4D encoder is). In ODR mode the 8 knobs are read from the ODR stream instead (see below).

```text
I-1 b28 n2 I-2 b8 WC b10 I-3 b8 W3 W2 B1 I-AA W19 O-80 B2A O-81 B1E O-82 B31 O-A0 BE O-A1 BC8 W1 B1 O-A2 B2C O-A3 B90 O-A4 B80 O-AF B2 O-F3 B1 O-F4 B20 F-D0 D1 B1C F-D8 D2 W4 B10 F-D9 B20 F-F8 W2 B6
```

### Input Reports (`0x82` IN, interface 2)

* **Report `0x01` (`I-1`):**
  * `b28` (40 bits = 5 bytes): Button state bitmask.
  * `n2` (2 nibbles = 1 byte): 4D encoder relative notched counter (`wrap16`).
* **Report `0x02` (`I-2`):**
  * `b8 WC b10`: Pitch and modulation expression touchstrip data.
* **Report `0x03` (`I-3`):**
  * `b8 W3 W2 B1`: Pedals and expression inputs.
* **Report `0xAA` (`I-AA`):**
  * `W19` (19 words = 38 bytes): 8 touch-sensitive continuous rotary encoders.

### Output Reports (`0x02` OUT, interface 2)

* **Report `0x80` (`O-80`):** `B2A` (42 bytes): Button and mode indicator LEDs. Uses Native Instruments' standard NHL2 4-level discrete duty cycle protocol:
  * `0x00`: **Off** (`LedValue::Off` / `LedValue::OFF`)
  * `0xE4` (228): **Dim / Half-brightness** (`LedValue::Dim` / `LedValue::DIM`) — idle / half-brightness
  * `0x9E` (158): **Bright / Active** (`LedValue::Bright` / `LedValue::BRIGHT`) — active state
  * `0xFF` (255): **Max drive** (`LedValue::Single(255)` / `LedValue::MAX`)
  * Arbitrary percentages ($0..100\%$) encode as `0x80 | pct` via `LedValue::single_percent(pct)`.
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

The keyboard renders its own UI. The host drives it with MessagePack-RPC messages over interface 3. The messages listed as verified below have been confirmed on an S49 Mk3 with a standalone test program.

### Framing

Every message in either direction is `<u32 little-endian length><MessagePack value>`. The value is a MessagePack-RPC notification, `[2, method, params]`. A message can span several USB transfers, and one transfer can hold several messages, so the reader must reassemble frames by length.

### Symbol registry

The session opens with a handshake whose method name is the string `"handshake"`, sent on `0x04`:

```text
[2, "handshake", [{ "host_protocol_version": "2.5.0",
                    "symbol_registry": [ ...437 strings... ],
                    "agent_version": "2.1.5 (R14)",
                    "utc_clock": "YYYY-MM-DDTHH:MM:SS+0000",
                    "disable_heartbeat_validation": false }]]
```

From then on, method names and most map keys are sent as their index in `symbol_registry`, e.g. `66` = `client_instance_add`, `14` = `instance_00`. Some keys are sent as plain strings (for example the `client_instance_add` fields and the inner `parameter_value` map of a page). The registry is in `encdr/src/device/komplete_kontrol/odr/symbols.rs`.

### Session lifecycle

1. Host → `0x04`: `handshake`. Keyboard → `0x83`: `handshake_reply`, followed by settings messages.
2. Host → `0x03`: `[2, sym(heartbeat), []]` every **8 s** after that. The keyboard checks heartbeats unless the handshake sets `disable_heartbeat_validation`, and its exact timeout isn't known.
3. Host → `0x04`, each with `instance_00` as the first parameter:
   - `client_instance_add {name, version, type: "standalone", is_host: true}`
   - `client_page_set parameter`
   - `client_focus_acquire`
   - `client_instance_set_host_owned_instance_client_state {...}`
4. Host → `0x04`: page content (below).
5. If the keyboard sends `agent_heartbeat_request_handshake`, the host repeats from step 1. The keyboard has then lost the session, so steps 3–4 must be sent again.

### Verified messages (host → keyboard, `0x04`)

| Method | Params after `instance_00` |
|---|---|
| `client_instance_parameter_page_model_set_plugin_data` | `{name, background: <cache index>, parameters: {1..8: {"continuous_parameter": {"parameter_value": {"value", "display_value"}, "parameter_style": <style map>}}}, plugin_color: {r,g,b,is_rgb565}, nks_control_color}` |
| `client_instance_parameter_page_model_set_host_owned_viewstate` | `{nks1_layout: [[{section_name, parameters: [{id, name}]}...]], nks2_layout: nil}` |
| `client_instance_parameter_page_model_update_parameter_value` | `slot (1..8), {continuous_parameter: {value: f32, display_value}}` |
| `client_instance_plugin_chain_model_set` | `{plugins: [{name, identifier: bin16, can_bypass, bypass_enabled, ...}], current_plugin_index}` |
| `client_instance_lightguide_set` | `{client_midi_addressing: [[note, colour_index 0..63], ...]}`, where `44` = unlit |
| `system_command` | `[<command symbol>]`: `restart_renderer`, `reboot`, `reboot_for_dfu`, `self_test`, `enable_serial_console`, `request_application_log`, `request_crashdumps` |

Parameter styles supported natively on the hardware display:
- `knob_parameter_style`: continuous circular arc gauge
- `power_parameter_style`: 2-state On/Off push button
- `waveform_parameter_style`: `{waveform_parameter_style: sym(<waveform>)}` (e.g. `sine`, `saw`, `square`, `pwm`, `wavetable`)
- `filtertype_parameter_style`: `{filtertype_parameter_style: sym(<filter>)}` (e.g. `resonant_lo_pass`, `hi_pass`, `band_pass`, `bell`)
- `tempo_sync_parameter_style`: beat-synchronized note value
- `value_parameter_style`: numerical value without circular arc
- `menu_parameter_style`: stepped dropdown item

### Keyboard → host (`0x83`)

* `handshake_reply`: hardware connection acknowledged.
* `agent_heartbeat_request_handshake`: session watchdog reset; triggers automatic host re-handshake and page restoration.
* `device_property_changed` / `device_properties`: reports foot pedal connection state:
  * `pedal_sustain_connected` (Jack 1)
  * `pedal_expression_connected` (Jack 2)
  * `pedal_assign1_connected` (Jack 3)
  * `pedal_assign2_connected` (Jack 4)
* `focus_changed` / `set_focus`: reports active UI focus between keyboard standalone mode and DAW/host instance.
* `plugin_action [instance, {action: value}]`:
  * `plugin_parameter_value_change`: relative encoder delta `[slot 1..8, delta]`
  * `plugin_parameter_value_set`: absolute encoder value `[slot 1..8, value]`
  * `plugin_parameter_erp_touched`: capacitive knob touch `[slot 1..8, bool]`
  * `plugin_parameter_set_page_index`: on-screen parameter page switch `[page_index]`
  * `daw_*`: on-screen transport and track actions (`daw_play`, `daw_stop`, `daw_record`, `daw_tempo`, `daw_select_track`, `daw_mute_track`, `daw_solo_track`, etc.)
  * `plugin_chain_*`: plugin insert selection, bypass toggle, move, remove
  * `browser_*`: preset sound loading, category filtering, favorites toggle

### Not yet verified

These method names exist in the registry, but their payloads haven't been verified on hardware yet:
* `client_instance_mixer_set_track_data` / `_set_meters`
* `client_instance_smartplay_set_data`
* `client_instance_browser_set_data_model`
* `client_instance_plugin_chain_model_set_current_index`
* `assets_add` (image upload)
* `client_page_set` for pages other than `parameter`

Device settings (`display_brightness`, `led_brightness`, `lightguide_enabled`, `global_led_color`) are not in the registry; `set_device_settings` / `get_device_settings` are the likely methods, with string keys.
