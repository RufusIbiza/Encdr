# Encdr Examples

This directory contains standalone examples, interactive Vegas mode demos, and hardware telemetry suites for all controllers supported by the **Encdr** USB framework.

All examples can be run directly via Cargo with:
```bash
cargo run -p encdr-examples --bin <example_name>
```

---

## Example Directory

| Binary Name | Target Hardware | Category | Description |
| :--- | :--- | :--- | :--- |
| [`jam_scroller`](#maschine-jam-pad-scroller--sine-meters) | NI Maschine Jam | Pad Matrix & LED Animation | Scrolls "Encdr" across the $8\times 8$ pad matrix in purple/orange with synchronized sine wave meters. |
| [`f1_vegas`](#traktor-kontrol-f1-vegas-mode) | NI Kontrol F1 | Interactive Vegas Mode | Rainbow wave across 16 RGB Remix Deck pads, 7-segment digital counter, and button chase animations. |
| [`x1_mk1_vegas`](#traktor-kontrol-x1-mk1-vegas-mode) | NI Kontrol X1 Mk1 | Interactive Vegas Mode | LED matrix chase wave across 24 dual-color LEDs and full knob/encoder telemetry. |
| [`x1_mk2_vegas`](#traktor-kontrol-x1-mk2-vegas-mode) | NI Kontrol X1 Mk2 | Interactive Vegas Mode | Dual touchstrip VU light waves, RGB hotcue spectrum cycling, loop displays, and telemetry. |
| [`x1_mk3_test`](#traktor-kontrol-x1-mk3-test) | NI Kontrol X1 Mk3 | OLED & RGB Underglow | Renders to all 5 monochrome OLED screens with RGB underglow lighting and button matrix. |
| [`z1_vegas`](#traktor-kontrol-z1-vegas-mode) | NI Kontrol Z1 | Interactive Vegas Mode | Dual 7-segment VU meters, cue/mode button chase patterns, and 2-channel mixer telemetry. |
| [`z2_vegas`](#traktor-kontrol-z2-vegas-mode) | NI Kontrol Z2 | Interactive Vegas Mode | 7-segment VU level meters, loop length LED indicators, transport LEDs, and mixer telemetry. |
| [`s2_mk1_vegas`](#traktor-kontrol-s2-mk1-vegas-mode) | NI Kontrol S2 Mk1 | Interactive Vegas Mode | Deck A/B LED pulse chase, transport/cue animations, and 2-deck mixer/jogwheel telemetry. |
| [`s2_mk2_vegas`](#traktor-kontrol-s2-mk2-vegas-mode) | NI Kontrol S2 Mk2 | Interactive Vegas Mode | Dual-color remix deck pads, deck indicator chase, transport LEDs, and mixer telemetry. |
| [`s4_mk2_vegas`](#traktor-kontrol-s4-mk2-vegas-mode) | NI Kontrol S4 Mk2 | Interactive Vegas Mode | 4-channel VU meters, dual 7-segment loop displays, RGB remix pads, and 4-deck telemetry. |
| [`s4_mk3_vegas`](#traktor-kontrol-s4-mk3-vegas-mode) | NI Kontrol S4 Mk3 | Interactive Vegas Mode | Dual Haptic Drive motorized jogwheel LED rings (real-time 1:1 hardware needle tracking & 32-segment spinner), RGB pads, dual screens, and telemetry. |
| [`s5_vegas`](#traktor-kontrol-s5-vegas-mode) | NI Kontrol S5 | Interactive Vegas Mode | 16 RGB remix deck pads, dual 25-LED touchstrips, screen buttons, and 4-channel telemetry. |
| [`s8_vegas`](#traktor-kontrol-s8-vegas-mode) | NI Kontrol S8 | Interactive Vegas Mode | 16 RGB pads, dual 25-LED touchstrips, mixer channel buttons, and full 4-channel surface telemetry. |
| [`d2_vegas`](#traktor-kontrol-d2-vegas-mode) | NI Kontrol D2 | Interactive Vegas Mode | 8 RGB performance pads, 25-LED blue/orange touchstrip meters, loop circle animation, and telemetry. |
| [`mikro_mk1_vegas`](#maschine-mikro-mk1-vegas-mode) | NI Maschine Mikro Mk1 | Interactive Vegas Mode | 16 monochrome pad velocity pulse wave, function button chase, and full telemetry. |
| [`mikro_mk2_vegas`](#maschine-mikro-mk2-vegas-mode) | NI Maschine Mikro Mk2 | Interactive Vegas Mode | Sweeping RGB rainbow wave across 16 pads, button LED chase, and telemetry. |
| [`mikro_mk3_vegas`](#maschine-mikro-mk3-vegas-mode) | NI Maschine Mikro Mk3 | Interactive Vegas Mode | Sweeping RGB rainbow wave across 16 pads, Smart Strip LED wave, and telemetry. |
| [`mk2_vegas`](#maschine-mk2-vegas-mode) | NI Maschine Mk2 | Interactive Vegas Mode | Sweeping RGB rainbow wave across 16 pads, Group A–H RGB cycling, button chase, and telemetry. |
| [`mk3_vegas`](#maschine-mk3-vegas-mode) | NI Maschine Mk3 | Interactive Vegas Mode | Sweeping RGB rainbow wave across 16 pads, Group A–H cycling, Smart Strip wave, and telemetry. |
| [`mk3_button_brightness`](#maschine-mk3-control-button-brightness--dimming-test) | NI Maschine Mk3 | LED Brightness & Dimming | Illuminates all 47 monochrome control buttons at DIM (half-brightness), elevating to BRIGHT on press. |
| [`plus_vegas`](#maschine-plus-vegas-mode) | NI Maschine Plus | Interactive Vegas Mode | Standalone-ready Vegas mode with 16 RGB pads, Smart Strip animations, and full telemetry. |
| [`studio_vegas`](#maschine-studio-vegas-mode) | NI Maschine Studio | Interactive Vegas Mode | 32-segment Jogwheel ring spinner, stereo master VU meters, 16 RGB pads, and telemetry. |
| [`kk_mk1_vegas`](#komplete-kontrol-mk1-vegas-mode) | NI Komplete Kontrol Mk1 | Interactive Vegas Mode | Sweeping Light Guide rainbow wave across the keybed (25/49/61/88 keys), 8 OLED displays, and telemetry. |
| [`kk_mk2_vegas`](#komplete-kontrol-mk2-vegas-mode) | NI Komplete Kontrol Mk2 | Interactive Vegas Mode | Sweeping Light Guide rainbow wave across the keybed (49/61/88 keys) and button telemetry. |
| [`kk_mk3_vegas`](#komplete-kontrol-mk3-vegas-mode) | NI Komplete Kontrol Mk3 | Interactive Vegas Mode | Sweeping Light Guide wave across the next-gen keybed (49/61/88 keys) and 4D encoder telemetry. |
| [`kk_mk3_daw_odr`](#komplete-kontrol-mk3-daw--odr-controller) | NI Komplete Kontrol Mk3 | DAW Remote & On-Device UI | Direct DAW protocol (handshake, track strip, stereo VU meters, 14-bit knob decoding) & On-Device Rendering (parameter pages, dynamic image banners, serial FX chain, and Light Guide). |
| [`mk3_reddit`](#maschine-mk3-reddit-browser) | NI Maschine Mk3 | Dual Screen & Media | Reddit browser with HTML/Canvas dual screens, 4D encoder navigation, and touchstrip scrolling. |
| [`mk3_pad_rainbow`](#maschine-mk3-pad-rainbow--telemetry) | NI Maschine Mk3 | RGB Pads & Screens | Smooth rainbow pad/group animations, pad strike flashing, and WebKit telemetry screens. |
| [`mk3_pad_response`](#maschine-mk3-pad-latency--aftertouch) | NI Maschine Mk3 / Plus | Pads & Sensors | Low-latency pad response and continuous polyphonic aftertouch pressure benchmark. |
| [`mk3_screen_test`](#maschine-mk3-dual-screen-test) | NI Maschine Mk3 | Dual Screen | Basic dual-screen test rendering graphic patterns to left and right 480×272 displays. |
| [`mk3_dual_screen`](#maschine-mk3-unified-dual-screen-test) | NI Maschine Mk3 | Unified Dual Screen | High-performance single 960×272 WebView with DualScreenView, zero-cost splitting, and telemetry. |
| [`touchstrip_monitor`](#touchstrip-monitor) | NI Maschine Mk3 | Touch Sensors | Real-time touchstrip capacitive touch and position telemetry monitor. |
| [`touchstrip_position_test`](#touchstrip-position-test) | NI Kontrol S8 / Mk3 | Touch & LEDs | Tests touch tracking and LED positioning along the touchstrip. |
| [`d2_screen_test`](#traktor-kontrol-d2-screen-test) | NI Kontrol D2 | Screen & Controls | Renders knob and button positions on the D2's 480×272 display via `encdr-view`. |
| [`s8_screen_test`](#traktor-kontrol-s8-dual-screen-test) | NI Kontrol S8 | Dual Screen | Dual-screen rendering and real-time control telemetry on the Traktor Kontrol S8. |
| [`s8_monitor`](#traktor-kontrol-s8-monitor) | NI Kontrol S8 | Input Telemetry | Full-surface input event monitor for S8 buttons, faders, knobs, and encoders. |
| [`mixer_led_test`](#traktor-s8-mixer-led-test) | NI Kontrol S8 Mixer | Mixer LEDs | Validates VU meters, channel fader tracking, and cue/filter LED indicators. |
| [`mixer_s5_style_test`](#mixer-s5s8-style-test) | NI Kontrol S5 / S8 | Mixer Reports | Tests S5/S8 mixer report formats and dynamic LED feedback. |
| [`mixer_offset_probe`](#mixer-led-offset-probe) | Hardware Diagnostic | LED Probing | Interactive tool to cycle through LED offsets and discover undocumented LED registers. |
| [`monitor`](#universal-event-monitor) | Universal (All Devices) | Diagnostics | Universal real-time input event monitor and descriptor validation tool. |
| [`probe`](#universal-usb-probe) | Universal (All USB) | Diagnostics | Probes connected USB devices, checks descriptors, and inspects USB endpoints. |

---

## Hardware Examples

### Maschine Jam Pad Scroller & Sine Meters
* **Binary:** `jam_scroller`
* **Hardware:** Native Instruments Maschine Jam (`0x17cc:0x1500`)
* **Path:** [`examples/maschine_jam/jam_scroller.rs`](maschine_jam/jam_scroller.rs)

Demonstrates full-surface control and multi-report LED driving on the Maschine Jam:
- Scrolls the word **"Encdr"** right-to-left across the 64-pad $8\times 8$ Click-Pad matrix in purple on an orange background.
- Animates scrolling sine waves across the 8 Smart Strip 11-segment LED meters at the bottom, locked to the exact horizontal scroll speed of the text.
- Full interactive telemetry: logs pad presses/releases, 39 function/transport buttons (illuminating corresponding LEDs), encoder rotation/touch/push, and touchstrip slider gestures.
- Turn the rotary encoder to adjust scroll speed; press the encoder to reverse scroll direction.

```bash
cargo run -p encdr-examples --bin jam_scroller
```

---

### Traktor Kontrol F1 Vegas Mode
* **Binary:** `f1_vegas`
* **Hardware:** Native Instruments Traktor Kontrol F1 (`0x17cc:0x1120`)
* **Path:** [`examples/kontrol_f1/f1_vegas.rs`](kontrol_f1/f1_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Sweeping rainbow wave across all 16 Remix Deck RGB pads with white flash feedback on strike.
- Animated rolling counter on dual 7-segment displays.
- Pulsing mode/stop button chase animations and real-time fader/knob telemetry.

```bash
cargo run -p encdr-examples --bin f1_vegas
```

---

### Traktor Kontrol X1 Mk1 Vegas Mode
* **Binary:** `x1_mk1_vegas`
* **Hardware:** Native Instruments Traktor Kontrol X1 Mk1 (`0x17cc:0x2305` / `0x1000`)
* **Path:** [`examples/kontrol_x1_mk1/x1_mk1_vegas.rs`](kontrol_x1_mk1/x1_mk1_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Chasing LED wave across all 24 dual-color button LEDs.
- Real-time logging of all 8 analog potentiometers, 4 notched rotary encoders, and 24 buttons.

```bash
cargo run -p encdr-examples --bin x1_mk1_vegas
```

---

### Traktor Kontrol X1 Mk2 Vegas Mode
* **Binary:** `x1_mk2_vegas`
* **Hardware:** Native Instruments Traktor Kontrol X1 Mk2 (`0x17cc:0x1220`)
* **Path:** [`examples/kontrol_x1_mk2/x1_mk2_vegas.rs`](kontrol_x1_mk2/x1_mk2_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Bouncing light waves across both 11-LED touchstrip arrays.
- Color spectrum cycling across 8 RGB hotcue buttons.
- Animated loop size sequences on dual 7-segment digital displays.

```bash
cargo run -p encdr-examples --bin x1_mk2_vegas
```

---

### Traktor Kontrol X1 MK3 Test
* **Binary:** `x1_mk3_test`
* **Hardware:** Native Instruments Traktor Kontrol X1 MK3 (`0x17cc:0x2210`)
* **Path:** [`examples/kontrol_x1_mk3/x1_mk3_test.rs`](kontrol_x1_mk3/x1_mk3_test.rs)

Comprehensive hardware test for the Traktor Kontrol X1 MK3:
- Renders custom text and graphics to all 5 monochrome OLED screens (128×64 pixels each).
- Dynamic RGB underglow lighting and hotcue button LED control.
- Real-time logging of 21 buttons, 4 encoders, and 8 analog knobs.

```bash
cargo run -p encdr-examples --bin x1_mk3_test
```

---

### Traktor Kontrol Z1 Vegas Mode
* **Binary:** `z1_vegas`
* **Hardware:** Native Instruments Traktor Kontrol Z1 (`0x17cc:0x1210`)
* **Path:** [`examples/kontrol_z1/z1_vegas.rs`](kontrol_z1/z1_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Bouncing stereo 7-segment VU meter bars on Channel A & B.
- Chasing pulse wave on cue/mode buttons and full 2-channel mixer telemetry.

```bash
cargo run -p encdr-examples --bin z1_vegas
```

---

### Traktor Kontrol Z2 Vegas Mode
* **Binary:** `z2_vegas`
* **Hardware:** Native Instruments Traktor Kontrol Z2 (`0x17cc:0x1230`)
* **Path:** [`examples/kontrol_z2/z2_vegas.rs`](kontrol_z2/z2_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- 7-segment VU meter LED strips, dual loop displays, cue/flux/sync button animations, and mixer telemetry.

```bash
cargo run -p encdr-examples --bin z2_vegas
```

---

### Traktor Kontrol S2 Mk1 Vegas Mode
* **Binary:** `s2_mk1_vegas`
* **Hardware:** Native Instruments Traktor Kontrol S2 Mk1 (`0x17cc:0x1101`)
* **Path:** [`examples/kontrol_s2/s2_mk1_vegas.rs`](kontrol_s2/s2_mk1_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Chasing LED wave across deck buttons, transport controls, and full 2-channel DJ mixer telemetry.

```bash
cargo run -p encdr-examples --bin s2_mk1_vegas
```

---

### Traktor Kontrol S2 Mk2 Vegas Mode
* **Binary:** `s2_mk2_vegas`
* **Hardware:** Native Instruments Traktor Kontrol S2 Mk2 (`0x17cc:0x1320`)
* **Path:** [`examples/kontrol_s2/s2_mk2_vegas.rs`](kontrol_s2/s2_mk2_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Dual-color remix pads, deck indicators, transport LEDs, and mixer telemetry.

```bash
cargo run -p encdr-examples --bin s2_mk2_vegas
```

---

### Traktor Kontrol S4 Mk2 Vegas Mode
* **Binary:** `s4_mk2_vegas`
* **Hardware:** Native Instruments Traktor Kontrol S4 Mk2 (`0x17cc:0x1310`)
* **Path:** [`examples/kontrol_s4/s4_mk2_vegas.rs`](kontrol_s4/s4_mk2_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- 4-channel VU level meters, dual 7-segment loop displays, RGB remix pads, and 4-deck telemetry.

```bash
cargo run -p encdr-examples --bin s4_mk2_vegas
```

---

### Traktor Kontrol S4 Mk3 Vegas Mode
* **Binary:** `s4_mk3_vegas`
* **Hardware:** Native Instruments Traktor Kontrol S4 Mk3 (`0x17cc:0x1720`)
* **Path:** [`examples/kontrol_s4/s4_mk3_vegas.rs`](kontrol_s4/s4_mk3_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Real-time 1:1 hardware needle position tracking on manual spin or motorized turntable rotation (`sync_jog_ring_from_event`).
- Animated 32-segment circular rainbow spinner chase on the jog wheel LED ring.
- Sweeping RGB hotcue pad rainbow animations with touch-reactive color shifts.
- Dual 320x240 LCD display gradients via USB bulk transfer.
- Full console telemetry for capacitive jog touches, faders, knobs, encoders, and buttons.

```bash
cargo run -p encdr-examples --bin s4_mk3_vegas
```

---

### Traktor Kontrol S5 Vegas Mode
* **Binary:** `s5_vegas`
* **Hardware:** Native Instruments Traktor Kontrol S5 (`0x17cc:0x1420`)
* **Path:** [`examples/kontrol_s5/s5_vegas.rs`](kontrol_s5/s5_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Rainbow wave on 16 RGB pads, dual 25-LED touchstrips, screen buttons, and 4-channel mixer telemetry.

```bash
cargo run -p encdr-examples --bin s5_vegas
```

---

### Traktor Kontrol S8 Vegas Mode
* **Binary:** `s8_vegas`
* **Hardware:** Native Instruments Traktor Kontrol S8 (`0x17cc:0x1370`)
* **Path:** [`examples/kontrol_s8/s8_vegas.rs`](kontrol_s8/s8_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Rainbow wave on 16 RGB pads, dual 25-LED touchstrips, mixer buttons, and full 4-channel mixer telemetry.

```bash
cargo run -p encdr-examples --bin s8_vegas
```

---

### Traktor Kontrol D2 Vegas Mode
* **Binary:** `d2_vegas`
* **Hardware:** Native Instruments Traktor Kontrol D2 (`0x17cc:0x1400`)
* **Path:** [`examples/kontrol_d2/d2_vegas.rs`](kontrol_d2/d2_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- 8 RGB performance pads, 25-LED blue/orange touchstrip meters, loop circle animation, and telemetry.

```bash
cargo run -p encdr-examples --bin d2_vegas
```

---

### Maschine Mikro Mk1 Vegas Mode
* **Binary:** `mikro_mk1_vegas`
* **Hardware:** Native Instruments Maschine Mikro Mk1 (`0x17cc:0x1110`)
* **Path:** [`examples/maschine_mikro/mikro_mk1_vegas.rs`](maschine_mikro/mikro_mk1_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- 16 monochrome pad velocity pulse waves, button chase, and full telemetry.

```bash
cargo run -p encdr-examples --bin mikro_mk1_vegas
```

---

### Maschine Mikro Mk2 Vegas Mode
* **Binary:** `mikro_mk2_vegas`
* **Hardware:** Native Instruments Maschine Mikro Mk2 (`0x17cc:0x1200`)
* **Path:** [`examples/maschine_mikro/mikro_mk2_vegas.rs`](maschine_mikro/mikro_mk2_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Sweeping RGB rainbow wave across 16 pads, button LED chase, and telemetry.

```bash
cargo run -p encdr-examples --bin mikro_mk2_vegas
```

---

### Maschine Mikro Mk3 Vegas Mode
* **Binary:** `mikro_mk3_vegas`
* **Hardware:** Native Instruments Maschine Mikro Mk3 (`0x17cc:0x1700`)
* **Path:** [`examples/maschine_mikro/mikro_mk3_vegas.rs`](maschine_mikro/mikro_mk3_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Sweeping RGB rainbow wave across 16 pads, Smart Strip LED wave, and telemetry.

```bash
cargo run -p encdr-examples --bin mikro_mk3_vegas
```

---

### Maschine Mk2 Vegas Mode
* **Binary:** `mk2_vegas`
* **Hardware:** Native Instruments Maschine Mk2 (`0x17cc:0x1140`)
* **Path:** [`examples/maschine_mk2/mk2_vegas.rs`](maschine_mk2/mk2_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Sweeping RGB rainbow wave across 16 pads, Group A–H RGB cycling, button chase, and telemetry.

```bash
cargo run -p encdr-examples --bin mk2_vegas
```

---

### Maschine Mk3 Vegas Mode
* **Binary:** `mk3_vegas`
* **Hardware:** Native Instruments Maschine Mk3 (`0x17cc:0x1600`)
* **Path:** [`examples/maschine_mk3/mk3_vegas.rs`](maschine_mk3/mk3_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Sweeping RGB rainbow wave across 16 pads, Group A–H cycling, Smart Strip wave, and telemetry.

```bash
cargo run -p encdr-examples --bin mk3_vegas
```

---

### Maschine Mk3 Control Button Brightness & Dimming Test
* **Binary:** `mk3_button_brightness`
* **Hardware:** Native Instruments Maschine Mk3 (`0x17cc:0x1600`) / Maschine Plus (`0x17cc:0x1820`)
* **Path:** [`examples/maschine_mk3/button_brightness.rs`](maschine_mk3/button_brightness.rs)

Tests Native Instruments discrete button brightness levels:
- Illuminates all 47 monochrome function, transport, edit, and mode buttons at `LedValue::Dim` (`0xE4` / 228, half-brightness).
- Elevates pressed buttons to `LedValue::Bright` (`0x9E` / 158, active state / both LEDs lit), returning to `Dim` upon release.
- Keeps RGB pads and Group A–H buttons unlit.

```bash
cargo run -p encdr-examples --bin mk3_button_brightness
```

---

### Maschine Plus Vegas Mode
* **Binary:** `plus_vegas`
* **Hardware:** Native Instruments Maschine Plus (`0x17cc:0x1820`)
* **Path:** [`examples/maschine_plus/plus_vegas.rs`](maschine_plus/plus_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Standalone-ready Vegas mode with 16 RGB pads, Smart Strip animations, and full telemetry.

```bash
cargo run -p encdr-examples --bin plus_vegas
```

---

### Maschine Studio Vegas Mode
* **Binary:** `studio_vegas`
* **Hardware:** Native Instruments Maschine Studio (`0x17cc:0x1300`)
* **Path:** [`examples/maschine_studio/studio_vegas.rs`](maschine_studio/studio_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- 32-segment Jogwheel ring spinner, stereo master VU meters, 16 RGB pads, and telemetry.

```bash
cargo run -p encdr-examples --bin studio_vegas
```

---

### Komplete Kontrol Mk1 Vegas Mode
* **Binary:** `kk_mk1_vegas`
* **Hardware:** Native Instruments Komplete Kontrol S25 / S49 / S61 / S88 Mk1 (`0x17cc:0x1340` / `0x1350` / `0x1360` / `0x1410`)
* **Path:** [`examples/komplete_kontrol/kk_mk1_vegas.rs`](komplete_kontrol/kk_mk1_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Sweeping Light Guide rainbow wave across the keybed (25/49/61/88 keys).
- Diagnostic text and status blitted to all 8 monochrome 128×32 OLED displays (`display_1` .. `display_8`).
- Button LED brightness cycling and full rotary encoder / touchstrip telemetry.

```bash
cargo run -p encdr-examples --bin kk_mk1_vegas
```

---

### Komplete Kontrol Mk2 Vegas Mode
* **Binary:** `kk_mk2_vegas`
* **Hardware:** Native Instruments Komplete Kontrol S49 / S61 / S88 Mk2 (`0x17cc:0x1610` / `0x1620` / `0x1630`)
* **Path:** [`examples/komplete_kontrol/kk_mk2_vegas.rs`](komplete_kontrol/kk_mk2_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Sweeping Light Guide rainbow wave across the keybed (49/61/88 keys) and button telemetry.

```bash
cargo run -p encdr-examples --bin kk_mk2_vegas
```

---

### Komplete Kontrol Mk3 Vegas Mode
* **Binary:** `kk_mk3_vegas`
* **Hardware:** Native Instruments Komplete Kontrol S49 / S61 / S88 Mk3 (`0x17cc:0x2100` / `0x2110` / `0x2120`)
* **Path:** [`examples/komplete_kontrol/kk_mk3_vegas.rs`](komplete_kontrol/kk_mk3_vegas.rs)

Interactive Vegas demo and hardware telemetry:
- Sweeping Light Guide wave across the next-gen keybed (49/61/88 keys) and 4D encoder telemetry.

```bash
cargo run -p encdr-examples --bin kk_mk3_vegas
```

---

### Komplete Kontrol Mk3 DAW & ODR Controller
* **Binary:** `kk_mk3_daw_odr`
* **Hardware:** Native Instruments Komplete Kontrol S49 / S61 / S88 Mk3 (`0x17cc:0x2100` / `0x2110` / `0x2120`)
* **Path:** [`examples/komplete_kontrol/kk_mk3_daw_odr.rs`](komplete_kontrol/kk_mk3_daw_odr.rs)

Comprehensive demonstration of the next-generation Komplete Kontrol S-Series Mk3 architecture:
- **Direct DAW Remote Control**: Demonstrates handshake greeting, 14-bit high-resolution rotary knob SysEx mode enable, track labeling, RGB track color encoding (`#AARRGGBB`), 8-channel stereo logarithmic dB VU meters, and 14-bit rotary encoder delta parsing.
- **On-Device Rendering (ODR)**: Constructs an 8-parameter synth model with rotary knobs, continuous ranges, and 2-state toggle switches via MessagePack-RPC over Bulk OUT `0x03`.
- **Dynamic In-Memory Header Banner**: Generates custom PNG graphical banners dynamically on the host and registers them into the on-device flash cache.
- **Extended Models**: Demonstrates serial plugin insert chains, multi-track mixer cards, Smart Play scales/arpeggiators, preset sound browser, and display brightness settings.
- **Light Guide Animation**: Drives smooth 60 FPS RGB sweeps across the polyphonic aftertouch keybed.

```bash
cargo run -p encdr-examples --bin kk_mk3_daw_odr
```

---

### Maschine Mk3 Reddit Browser
* **Binary:** `mk3_reddit`
* **Hardware:** Native Instruments Maschine Mk3 (`0x17cc:0x1600`)
* **Path:** [`examples/maschine_mk3/reddit_browser.rs`](maschine_mk3/reddit_browser.rs)

A full interactive dual-screen application using `encdr-view` (WebKit / HTML / Canvas):
- Left display shows post listing, metadata, and post content.
- Right display renders media previews, comments, and telemetry.
- Navigate posts with the 4D directional encoder and scroll feeds smoothly using the touchstrip.
- Optional `--visible` flag to open desktop mirror windows for debugging.

```bash
cargo run -p encdr-examples --bin mk3_reddit
cargo run -p encdr-examples --bin mk3_reddit -- --visible
```

---

### Maschine Mk3 Pad Rainbow & Telemetry
* **Binary:** `mk3_pad_rainbow`
* **Hardware:** Native Instruments Maschine Mk3 (`0x17cc:0x1600`)
* **Path:** [`examples/maschine_mk3/pad_rainbow.rs`](maschine_mk3/pad_rainbow.rs)

Demonstrates simultaneous pad LED animation and dual-screen rendering:
- Smooth HSV rainbow color cycling across all 16 RGB pads and Group buttons A–H.
- Instant bright white flash feedback on pad hits.
- Left screen displays an interactive $4\times 4$ pad matrix visualizer; right screen renders group statuses and real-time event logs.

```bash
cargo run -p encdr-examples --bin mk3_pad_rainbow
```

---

### Maschine Mk3 Pad Latency & Aftertouch
* **Binary:** `mk3_pad_response`
* **Hardware:** Native Instruments Maschine Mk3 (`0x17cc:0x1600`) / Maschine Plus
* **Path:** [`examples/maschine_mk3/pad_response.rs`](maschine_mk3/pad_response.rs)

Benchmark tool for measuring pad response latency and continuous aftertouch pressure:
- Tests sub-millisecond pad triggers and release packet parsing.
- Dual-screen real-time visualizer showing pressure curves per pad.

```bash
cargo run -p encdr-examples --bin mk3_pad_response
```

---

### Maschine Mk3 Dual Screen Test
* **Binary:** `mk3_screen_test`
* **Hardware:** Native Instruments Maschine Mk3 (`0x17cc:0x1600`)
* **Path:** [`examples/maschine_mk3/mk3_screen_test.rs`](maschine_mk3/mk3_screen_test.rs)

Basic dual-screen test verifying bulk USB screen blits to both 480×272 displays.

```bash
cargo run -p encdr-examples --bin mk3_screen_test
```

---

### Maschine Mk3 Unified Dual Screen Test
* **Binary:** `mk3_dual_screen`
* **Hardware:** Native Instruments Maschine Mk3 (`0x17cc:0x1600`)
* **Path:** [`examples/maschine_mk3/mk3_dual_screen.rs`](maschine_mk3/mk3_dual_screen.rs)

High-performance dual-screen demo powered by `DualScreenView` (single 960×272 offscreen WebView):
- Slashes browser engine RAM/CPU overhead in half by rendering both displays in a single HTML/Canvas document.
- Zero-cost hardware frame splitting in `encdr`: slices the 960×272 buffer into left and right 480×272 halves row-by-row and runs independent dirty-rect diffing.
- If only one screen's controls change, zero USB transfers are emitted for the other display.
- Real-time encoder, touch, button, and slider telemetry.

```bash
cargo run -p encdr-examples --bin mk3_dual_screen
cargo run -p encdr-examples --bin mk3_dual_screen -- --visible
```

---

### Touchstrip Monitor
* **Binary:** `touchstrip_monitor`
* **Hardware:** Native Instruments Maschine Mk3 (`0x17cc:0x1600`)
* **Path:** [`examples/maschine_mk3/touchstrip_monitor.rs`](maschine_mk3/touchstrip_monitor.rs)

Logs raw touchstrip capacitive touch data, slider positions, and gesture dynamics.

```bash
cargo run -p encdr-examples --bin touchstrip_monitor
```

---

### Touchstrip Position Test
* **Binary:** `touchstrip_position_test`
* **Hardware:** Native Instruments Kontrol S8 / Maschine Mk3
* **Path:** [`examples/maschine_mk3/touchstrip_position_test.rs`](maschine_mk3/touchstrip_position_test.rs)

Interactive test where touchstrip position controls an LED indicator that tracks finger movement in real time.

```bash
cargo run -p encdr-examples --bin touchstrip_position_test
```

---

### Traktor Kontrol D2 Screen Test
* **Binary:** `d2_screen_test`
* **Hardware:** Native Instruments Traktor Kontrol D2 (`0x17cc:0x1400`)
* **Path:** [`examples/kontrol_d2/d2_screen_test.rs`](kontrol_d2/d2_screen_test.rs)

Renders real-time knob positions, buttons, and deck telemetry to the D2's 480×272 display via `encdr-view`.

```bash
cargo run -p encdr-examples --bin d2_screen_test
```

---

### Traktor Kontrol S8 Dual Screen Test
* **Binary:** `s8_screen_test`
* **Hardware:** Native Instruments Traktor Kontrol S8 (`0x17cc:0x1370`)
* **Path:** [`examples/kontrol_s8/s8_screen_test.rs`](kontrol_s8/s8_screen_test.rs)

Tests dual-screen rendering on the Traktor Kontrol S8, driving both Left and Right 480×272 LCDs simultaneously with interactive visualizers.

```bash
cargo run -p encdr-examples --bin s8_screen_test
```

---

### Traktor Kontrol S8 Monitor
* **Binary:** `s8_monitor`
* **Hardware:** Native Instruments Traktor Kontrol S8 (`0x17cc:0x1370`)
* **Path:** [`examples/kontrol_s8/s8_monitor.rs`](kontrol_s8/s8_monitor.rs)

Full-surface event monitor verifying that all S8 buttons, faders, knobs, encoders, and touchstrips map cleanly to descriptor events.

```bash
cargo run -p encdr-examples --bin s8_monitor
```

---

### Traktor S8 Mixer LED Test
* **Binary:** `mixer_led_test`
* **Hardware:** Native Instruments Traktor Kontrol S8 Mixer
* **Path:** [`examples/mixer/mixer_led_test.rs`](mixer/mixer_led_test.rs)

Tests channel VU level meter strips, cue/filter buttons, and channel fader tracking.

```bash
cargo run -p encdr-examples --bin mixer_led_test
```

---

### Mixer S5/S8 Style Test
* **Binary:** `mixer_s5_style_test`
* **Hardware:** Native Instruments Traktor Kontrol S5 / S8
* **Path:** [`examples/mixer/mixer_s5_style_test.rs`](mixer/mixer_s5_style_test.rs)

Validates mixer packet parsing and LED update formats across S5 and S8 style report configurations.

```bash
cargo run -p encdr-examples --bin mixer_s5_style_test
```

---

## Utility & Diagnostic Tools

### Universal Event Monitor
* **Binary:** `monitor`
* **Hardware:** Any Encdr-supported controller
* **Path:** [`examples/utils/monitor.rs`](utils/monitor.rs)

Connects to any detected controller and prints all parsed events (`Button`, `Slider`, `Encoder`, `EncoderFine`, `Touch`, `Grid`) with normalized values, deltas, and timing info.

```bash
cargo run -p encdr-examples --bin monitor
```

---

### Universal USB Probe
* **Binary:** `probe`
* **Hardware:** Any USB device
* **Path:** [`examples/utils/probe.rs`](utils/probe.rs)

Scans connected USB devices, matches them against Encdr descriptors, and dumps interface, endpoint, and USB descriptor configurations.

```bash
cargo run -p encdr-examples --bin probe
```

---

### Mixer LED Offset Probe
* **Binary:** `mixer_offset_probe`
* **Hardware:** Any controller requiring LED offset mapping
* **Path:** [`examples/mixer/mixer_offset_probe.rs`](mixer/mixer_offset_probe.rs)

Interactive calibration utility to light up specific LED byte offsets one by one to discover or verify hardware LED register mappings.

```bash
cargo run -p encdr-examples --bin mixer_offset_probe
```
