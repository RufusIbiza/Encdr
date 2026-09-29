# Encdr

Native Instruments USB HID hardware and screen communication layer.

Provides data-driven device definitions, real-time input parsing, LED/screen output, GPU-accelerated frame management, and an optional WebView-based screen renderer.

`#NativeInstruments` `#NI`

Born from the [openAV-Ctlra](https://github.com/openAVproductions/openAV-Ctlra) C library, reimagined in Rust with data-driven device descriptors, zero-copy I/O, and a GPU-accelerated screen pipeline.

## Quick Start

```rust
use std::time::Duration;
use encdr::{Encdr, EncdrConfig, Event, LedValue};

fn main() {
    let mut encdr = Encdr::new(EncdrConfig::default()).unwrap();
    let ids = encdr.scan().unwrap();
    let events = encdr.events().clone();

    loop {
        while let Ok(event) = events.try_recv() {
            match event {
                Event::DeviceConnected { id, descriptor } => {
                    println!("Connected: {}", descriptor.name);
                }
                Event::Button { device, name, pressed } => {
                    println!("{}: {}", name, if pressed { "ON" } else { "OFF" });
                    // Mirror button state to its LED
                    encdr.set_led(device, name, if pressed {
                        LedValue::Single(127)
                    } else {
                        LedValue::Off
                    });
                }
                Event::Slider { device, name, value } => {
                    println!("{}: {:.2}", name, value);
                }
                _ => {}
            }
        }

        // Send a raw RGBA pixel buffer to the screen
        // (Encdr handles GPU conversion to BGR565, diffing, and USB transfer)
        // encdr.submit_screen(device_id, "main", &rgba_pixels);

        std::thread::sleep(Duration::from_millis(8));
    }
}
```

## Design Philosophy

1. **Exposes hardware truthfully** — every button, slider, encoder, LED, and screen is enumerated and accessible by name. The consuming app decides what each control does.
2. **Data-driven device definitions** — new devices are added via JSON descriptor files, not Rust code. The descriptor defines USB endpoints, byte-level packet layouts, LED mappings, and screen protocols.
3. **Two-tier screen pipeline** — an optional WebView renderer (`encdr-view`) lets apps build screen UIs with HTML/CSS/Canvas, while the core module accepts raw pixel buffers. Both share the same GPU conversion/diff/transfer backend.
4. **Optimizes for latency** — async USB I/O, lock-free event delivery, GPU-side format conversion, dirty-region-only transfers.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Consumer App (e.g. Bitwig)                  │
│                                                                 │
│  ← Receives: named events (button, slider, encoder, touch)      │
│  → Sends:    LED state, screen content (HTML or raw pixels)     │
└──────────────┬──────────────────────────────┬───────────────────┘
               │                              │
       ┌───────▼───────┐            ┌─────────▼──────────────────┐
       │  Input Path   │            │   Output Path              │
       │               │            │                            │
       │  USB read     │            │  ┌───────────────────────┐ │
       │  → parse      │            │  │ encdr-view (optional) │ │
       │  → normalize  │            │  │ Offscreen WebView     │ │
       │  → emit event │            │  │ HTML/CSS/Canvas → px  │ │
       │  (lock-free)  │            │  └──────────┬────────────┘ │
       │               │            │             │ OR raw pixels│
       └───────────────┘            │  ┌──────────▼────────────┐ │
               │                    │  │ encdr::screen (core)  │ │
               │                    │  │ GPU format convert    │ │
               │                    │  │ GPU frame diff        │ │
               │                    │  │ Partial blit extract  │ │
               │                    │  └──────────┬────────────┘ │
               │                    │        USB bulk write      │
               │                    └─────────────┬──────────────┘
               │                                  │
       ┌───────▼──────────────────────────────────▼───────────┐
       │            Device Instance (data-driven)             │
       │         Loaded from JSON device descriptor           │
       └──────────────────────┬───────────────────────────────┘
                              │
                     ┌────────▼────────┐
                     │   USB Transport │
                     │     (nusb)      │
                     └─────────────────┘
```

## Workspace Structure

```
encdr/
├── encdr/                  Core crate
│   ├── descriptors/        Built-in JSON device descriptors
│   │   ├── ni_komplete_kontrol_s49_mk2.json
│   │   ├── ni_komplete_kontrol_s61_mk2.json
│   │   ├── ni_komplete_kontrol_s88_mk2.json
│   │   ├── ni_kontrol_d2.json
│   │   ├── ni_kontrol_s2_mk1.json
│   │   ├── ni_kontrol_s2_mk2.json
│   │   ├── ni_kontrol_s4_mk2.json
│   │   ├── ni_kontrol_s4_mk3.json
│   │   ├── ni_kontrol_s5.json
│   │   ├── ni_kontrol_s8.json
│   │   ├── ni_maschine_mk2.json
│   │   ├── ni_maschine_mk3.json
│   │   ├── ni_maschine_plus.json
│   │   ├── ni_maschine_studio.json
│   │   └── ni_traktor_kontrol_x1_mk3.json
│   └── src/
│       ├── lib.rs          Encdr facade + public API
│       ├── core/           Event types, descriptor model, LED types, errors
│       ├── device/         Packet parser, encoder state, LED builder, hooks
│       ├── screen/         GPU pipeline, format conversion, frame diff, protocol
│       └── usb/            Device thread, hotplug, transport
│
├── encdr-view/             WebView screen renderer (Linux, macOS, Windows)
│   └── src/
│       ├── lib.rs          ScreenView public API
│       ├── bridge.rs       Rust ↔ JS message passing
│       ├── webview.rs      Linux: GTK + WebKitGTK offscreen WebView
│       ├── capture.rs      Linux: pixel capture via WebKit snapshot
│       ├── webview_macos.rs  macOS: tao + wry offscreen WebView
│       ├── capture_macos.rs  macOS: pixel capture via WKWebView takeSnapshot
│       ├── webview_windows.rs  Windows: tao + wry offscreen WebView
│       └── capture_windows.rs  Windows: pixel capture via WebView2 CapturePreview
│
├── examples/               Collection of examples organized by controller
│   ├── kontrol_d2/         D2 examples (e.g. d2_screen_test)
│   ├── kontrol_s8/         S8 examples (e.g. s8_monitor, s8_screen_test)
│   ├── kontrol_x1_mk3/     X1 Mk3 examples (e.g. x1_mk3_test)
│   ├── maschine_mk3/       Mk3 examples (e.g. mk3_screen_test, touchstrip_monitor)
│   ├── mixer/              Mixer and LED testing examples
│   └── utils/              General utilities (e.g. probe, monitor)
│
├── s8_discovery/           Scripts used during the mapping of the S8 HID address space (experimental)
│
├── screens/                HTML screen templates
│   ├── d2_controls.html    D2 control visualizer (DOM/SVG)
│   ├── mk3_left.html       Mk3 left screen (Canvas)
│   └── mk3_right.html      Mk3 right screen (Canvas)
│
└── docs/                   Detailed documentation
    ├── usage.md            How to use the crate
    └── hardware/
        ├── ni_kontrol_d2.md            D2 hardware reference
        ├── ni_komplete_kontrol_mk2.md  Komplete Kontrol S-Series Mk2 reference
        ├── ni_kontrol_s2_mk1.md        Traktor Kontrol S2 MK1 reference
        ├── ni_kontrol_s2_mk2.md        Traktor Kontrol S2 MK2 reference
        ├── ni_kontrol_s4_mk2.md        Traktor Kontrol S4 MK2 reference
        ├── ni_kontrol_s4_mk3.md        Traktor Kontrol S4 MK3 (Haptic Drive) reference
        ├── ni_kontrol_s5.md            Traktor Kontrol S5 reference
        ├── ni_kontrol_s8.md            S8 hardware reference
        ├── ni_kontrol_x1_mk3.md        Traktor Kontrol X1 MK3 hardware reference
        ├── ni_maschine_mk2.md          Maschine Mk2 hardware reference
        ├── ni_maschine_mk3.md          Mk3 hardware reference
        ├── ni_maschine_plus.md         Maschine Plus hardware reference
        └── ni_maschine_studio.md       Maschine Studio hardware reference
```

## Supported Hardware

| Device                    | VID:PID           | Status      | Controls                                      | LEDs                                        | Screens           |
| ------------------------- | ----------------- | ----------- | --------------------------------------------- | ------------------------------------------- | ----------------- |
| NI Kontrol D2             | `17cc:1400`       | Implemented | 57 buttons/touches, 6 encoders, 9 sliders     | 8 RGB pads, 5 singles, 2 strips             | 480x272 BGR565    |
| NI Kontrol S2 Mk1         | `17cc:1101`       | Implemented | 34 buttons, 2 jogwheels, 7 encoders, 16 faders/knobs | 8 dual-color pads (G/B), 24 singles, VU meters | — |
| NI Kontrol S2 Mk2         | `17cc:1320`       | Implemented | 38 buttons, 2 jogwheels, 7 encoders, 16 faders/knobs | 8 RGB pads, 20 singles, VU meters           | —                 |
| NI Kontrol S4 Mk2         | `17cc:1310`       | Implemented | 48 buttons, 2 jogwheels, 29 faders/knobs      | 8 RGB pads, 24 singles, VU meters           | —                 |
| NI Kontrol S4 Mk3         | `17cc:1720`       | Implemented | 72 buttons, 6 encoders, 2 motorized jogwheels, 28 faders/knobs | 16 RGB pads, 45 singles, 8-seg VU, LED rings, Haptic Drive | 2x 320x240 BGR565 |
| NI Kontrol S5             | `17cc:1420`       | Implemented | 70 buttons/touches, 6 encoders, 21 faders/knobs | 16 RGB pads, 34 singles                     | 2x 480x272 BGR565 |
| NI Kontrol S8             | `17cc:1370`       | Implemented | 114 buttons/encoders, 29 faders/knobs         | 16 RGB pads, 100+ singles, EP0 feature LEDs | 2x 480x272 BGR565 |
| NI Maschine Mk2           | `17cc:1200`       | Implemented | 47 buttons, 11 encoders, 16 velocity pads     | 16 RGB pads, 31 singles                     | 2x 256x64 1-bit   |
| NI Maschine Mk3           | `17cc:1600`       | Implemented | 63 buttons, 10 touches, 9 encoders, 1 slider, 16 pads | 16 RGB pads, 62 singles, 1 strip     | 2x 480x272 BGR565 |
| NI Maschine Plus          | `17cc:1820`       | Implemented | 63 buttons, 10 touches, 9 encoders, 1 slider, 16 pads | 16 RGB pads, 62 singles, 1 strip     | 2x 480x272 BGR565 |
| NI Maschine Studio        | `17cc:1300`       | Implemented | 64 buttons, 5 touches, 10 encoders, 16 pads   | 16 RGB pads, 8 RGB groups, stereo meters, 32-seg ring | 2x 480x272 BGR565 |
| NI Komplete Kontrol S-Mk2 | `17cc:1610/20/30` | Implemented | 28 buttons, 9 encoders                        | 20 singles, Light Guide (RGB per-key)       | 2x 480x272 BGR565 |
| NI Traktor Kontrol X1 Mk3 | `17cc:2200`       | Implemented | 21 buttons, 4 encoders, 8 knobs               | 14 singles, 8 RGB hotcues, 2 RGB underglow  | 5x 128x64 1-bit OLED |

## Dependencies

| Purpose                 | Crate                      | Why                                      |
| ----------------------- | -------------------------- | ---------------------------------------- |
| USB transport           | `nusb`                     | Pure Rust, async, cross-platform         |
| GPU compute             | `wgpu`                     | Format conversion + frame diff shaders   |
| Lock-free channel       | `crossbeam-channel`        | SPSC event delivery                      |
| Descriptor parsing      | `serde` + `serde_json`     | JSON device descriptors                  |
| Pixel buffer utils      | `bytemuck`                 | Zero-copy transmutes                     |
| Logging                 | `tracing`                  | Structured, zero-overhead when disabled  |
| Error handling          | `thiserror`                | Typed errors                             |
| Async executor          | `futures-lite`             | Lightweight internal async               |
| WebView (optional)      | `wry` + `tao`              | Offscreen HTML rendering                 |
| WebKit snapshot (Linux) | `webkit2gtk` + `cairo-rs`  | Pixel capture via WebKit snapshot        |
| Obj-C bridge (macOS)    | `objc2` + `block2`         | WKWebView `takeSnapshot` pixel capture   |
| COM/WebView2 (Windows)  | `webview2-com` + `windows` | WebView2 `CapturePreview` pixel capture  |
| PNG decode (Windows)    | `png`                      | Decode CapturePreview PNG output to RGBA |

## Documentation

- [Quick Start Tutorial](docs/quickstart.md) — step-by-step tutorial with Maschine Mk3 dual-screen walkthrough
- [API Reference](docs/api_reference.md) — complete programmer reference for all types, functions, and methods
- [Usage Guide](docs/usage.md) — comprehensive usage guide
- [NI Kontrol D2](docs/hardware/ni_kontrol_d2.md) — D2 hardware reference
- [NI Kontrol S2 Mk1](docs/hardware/ni_kontrol_s2_mk1.md) — Traktor Kontrol S2 MK1 hardware reference
- [NI Kontrol S2 Mk2](docs/hardware/ni_kontrol_s2_mk2.md) — Traktor Kontrol S2 MK2 hardware reference
- [NI Kontrol S4 Mk2](docs/hardware/ni_kontrol_s4_mk2.md) — Traktor Kontrol S4 MK2 hardware reference
- [NI Kontrol S4 Mk3](docs/hardware/ni_kontrol_s4_mk3.md) — Traktor Kontrol S4 MK3 (Haptic Drive, dual screens) hardware reference
- [NI Kontrol S5](docs/hardware/ni_kontrol_s5.md) — Traktor Kontrol S5 hardware reference & screen protocol
- [NI Kontrol S8](docs/hardware/ni_kontrol_s8.md) — S8 hardware reference
- [NI Maschine Mk2](docs/hardware/ni_maschine_mk2.md) — Mk2 hardware reference
- [NI Maschine Mk3](docs/hardware/ni_maschine_mk3.md) — Mk3 hardware reference
- [NI Maschine Plus](docs/hardware/ni_maschine_plus.md) — Maschine Plus (Controller Mode) hardware reference
- [NI Maschine Studio](docs/hardware/ni_maschine_studio.md) — Maschine Studio hardware reference
- [NI Komplete Kontrol Mk2](docs/hardware/ni_komplete_kontrol_mk2.md) — Komplete Kontrol S-Series Mk2 hardware reference
- [NI Traktor Kontrol X1 MK3](docs/hardware/ni_kontrol_x1_mk3.md) — X1 MK3 hardware reference

## Changelog
### v0.4.5
- **wgpu v30 Compute Pipeline Upgrade**:
  - Upgraded `wgpu` from `23` to `30.0.1` in `encdr`.
  - Adapted GPU compute pipeline layout descriptors (`immediate_size: 0`, `Option<&BindGroupLayout>`), device polling (`PollType::wait_indefinitely()`), and mapped buffer range error handling.
  - Added unit tests verifying GPU compute RGBA $\rightarrow$ BGR565-BE conversion bit-exact matching against the CPU reference.
- **WebView Ecosystem Upgrades**:
  - Upgraded `wry` to `0.57` and `tao` to `0.37` in `encdr-view`.
- **Screen Pipeline Diffing & Initial Frame Fix**:
  - Fixed a critical bug in [`ScreenManager::submit`](encdr/src/screen/mod.rs) where `self.prev_frame` was copied before evaluating `DirtyRect::Full`, causing full screen updates and initial WebView renders to be dropped as duplicates on non-keyframe ticks.
  - Ensured frame 1 is always forced as a full keyframe blit.
  - Added [`test_screen_manager_lifecycle`](encdr/src/screen/mod.rs) verifying forced initial frames, identical frame suppression, and full/partial dirty blits.
- **Maschine MK3 Hardware Validation**:
  - Validated physical hardware dual-display rendering and real-time event interaction with Native Instruments Maschine MK3.

### v0.4.1
- **7-Segment Display Subsystem & Helpers**:
  - Added [`SevenSegment`](encdr/src/core/seven_segment.rs) abstraction for multi-segment numeric/alphanumeric displays (Traktor Kontrol S4 MK2, X1 MK2, F1).
  - Implemented segment bitmask constants (`SEG_A`..`SEG_G`, `SEG_DP`), alphanumeric translation (`from_char`, `from_digit`), raw bitmask manipulation, and per-segment brightness arrays (`to_brightness_array`).
  - Added string encoding (`encode_str`) with automatic decimal point merging.
  - Added DJ loop length encoding (`encode_loop_length`) adhering to Traktor hardware notation: whole beats (`32`..`1`) and fractional sub-beats (`.2`, `.4`, `.8`, `1.6`, `3.2`) with active loop status indication.
- **Traktor Kontrol S4 MK2 Loop Displays**:
  - Added USB Output Report `0xd5` (32 bytes) mapping in `ni_kontrol_s4_mk2.json` for Deck A/C (`left_loop_digit_1`, `left_loop_digit_2`, `left_loop_dot`) and Deck B/D (`right_loop_digit_1`, `right_loop_digit_2`, `right_loop_dot`).
  - Added high-level helper methods to `Encdr`: `set_seven_segment`, `set_seven_segment_in_group`, `set_seven_segment_str`, `set_loop_display`, and `set_loop_display_with_dot`.
- **Documentation Updates**:
  - Updated [`docs/hardware/ni_kontrol_s4_mk2.md`](docs/hardware/ni_kontrol_s4_mk2.md) with Report `0xd5` layout, segment bitmasks, Traktor display notation table, and legacy MIDI CC mappings.
  - Updated [`docs/api_reference.md`](docs/api_reference.md) with Section 6 for `SevenSegment` and new facade methods.

### v0.4.0
- **Quick Start Tutorial**:
  - Added a dedicated, comprehensive tutorial in [`docs/quickstart.md`](docs/quickstart.md).
  - Expanded on the Native Instruments Maschine Mk3 dual-screen walkthrough, event handling, RGB pad control, and platform prerequisites.
- **Programmer Reference / API Documentation**:
  - Added complete reference documentation in [`docs/api_reference.md`](docs/api_reference.md) detailing every public struct, enum, trait, and function across `encdr` and `encdr-view`.
- **Documentation Restructuring**:
  - Streamlined `README.md` and added cross-navigation links across guides.

### v0.3.4
- **Traktor Kontrol S5 Support**:
  - Added hardware descriptor for NI Kontrol S5 (`17cc:1420`).
  - Added support for dual 480x272 BGR565 color displays via USB Bulk transfer (Interface 4, EP `0x03`).
  - Mapped 70 buttons/touches, 6 encoders, 21 faders/EQs/touchstrips, 16 RGB pads, and single-color LEDs.
  - Added hardware reference documentation in `docs/hardware/ni_kontrol_s5.md`.
- **Traktor Kontrol S4 MK3 Support**:
  - Added hardware descriptor for NI Kontrol S4 MK3 (`17cc:1720`).
  - Added support for dual 320x240 BGR565 color displays via USB Bulk transfer (Interface 4, EP `0x03`).
  - Implemented Motorized Jogwheel Haptic Drive protocol (Output Report `0x31`, 10 bytes: directional torque, velocity, and zero-speed force feedback per deck).
  - Implemented Jog Wheel LED Ring protocol (Output Report `0x32`, 40 bytes: position needle, flash/spot modes, and individually addressable LEDs).
  - Mapped 72 buttons, 6 encoders, 28 faders/knobs/EQs, 14-segment channel level meters (Report `0x81`), RGB pads, and backlights (Report `0x80`).
  - Added hardware reference documentation in `docs/hardware/ni_kontrol_s4_mk3.md`.
- **Traktor Kontrol S4 MK2 Support**:
  - Added hardware descriptor for NI Kontrol S4 MK2 (`17cc:1310`).
  - Mapped 48 buttons, 2 high-resolution jogwheels (32-bit position counters), 29 faders/knobs, RGB pads, and VU meters.
  - Added hardware reference documentation in `docs/hardware/ni_kontrol_s4_mk2.md`.
- **Traktor Kontrol S2 MK2 & MK1 Support**:
  - Added hardware descriptor for NI Kontrol S2 MK2 (`17cc:1320`) with 8-bit jogwheel counters, 4-bit nibble encoders, and RGB pads.
  - Added hardware descriptor for NI Kontrol S2 MK1 (`17cc:1101`) with standard USB HID, 32-bit jogwheels, and dual-color (Green/Blue) pads.
  - Added hardware reference documentation in `docs/hardware/ni_kontrol_s2_mk2.md` and `docs/hardware/ni_kontrol_s2_mk1.md`.
- **Core Engine & Parser Improvements**:
  - Fixed bitmask shift overflow in `encdr/src/device/parser.rs` for encoders configured with 8-bit resolution.
  - Embedded all new descriptors into compile-time built-ins in `DescriptorRegistry::load_builtins`.
  - Added automated unit test suite covering descriptor validation and event parsing for all 5 new controllers.
- Bumped workspace crates to `v0.3.4`.

### v0.3.3
- Documentation and metadata updates: refined crate description and keywords.

### v0.3.2
- **Maschine Plus & Maschine Studio Support**:
  - Added hardware descriptor for NI Maschine Plus (`17cc:1820`) running in USB Controller Mode.
  - Added hardware descriptor for NI Maschine Studio (`17cc:1300`) with 4-report LED engine (`0x80`, `0x81`, `0x82`, `0x83`).
  - Mapped 64 buttons, 10 encoders (including 32-segment optical jogwheel with `wrap16_wide` decoding), and 5 capacitive knob touch sensors for Maschine Studio.
  - Added multi-report LED handling for stereo 16-segment audio level meters and 32-segment circular jogwheel LED ring.
  - Added comprehensive hardware documentation for both controllers in `docs/hardware/`.
  - Updated `docs/usage.md` with multi-report LED guidance and modern device listings.

### v0.3.1
- **Traktor Kontrol X1 MK3 Support**:
  - Added hardware descriptor for NI Traktor Kontrol X1 MK3 (`17cc:2200`).
  - Added support for 5 concurrent 128x64 1-bit monochrome OLED screens (`left_fx`, `left_loop`, `center_mode`, `right_loop`, `right_fx`) via Interrupt OUT Report IDs `0xE0`..`0xE4`.
  - Mapped 21 buttons, 4 continuous push encoders (`wrap16`), and 8 analog FX potentiometers.
  - Implemented 49-byte LED feedback protocol in Report `0x80` for button backlights, RGB hotcues, and RGB underglow light guides.
  - Added interactive demonstration application (`examples/kontrol_x1_mk3/x1_mk3_test.rs`).
  - Added comprehensive hardware documentation (`docs/hardware/ni_kontrol_x1_mk3.md`).
- **Monochrome & Shared Interface Screen Engine**:
  - Fixed packed buffer calculation in `ScreenDesc::byte_size()` for `PixelFormat::Mono` to prevent buffer size mismatch panics.
  - Added Interrupt OUT transfer support in `run_screens()` for devices with interrupt-driven screen endpoints.
  - Implemented interface handle cloning in `device_thread.rs` to allow multiple screens and input loops to safely share USB interface handles.
  - Added static frame change detection in `ScreenManager::submit()` to suppress redundant duplicate full frames on monochrome displays.
- Bumped workspace crates to `v0.3.1`.

### v0.3.0
- **Komplete Kontrol S-Series Mk2 Keyboards**:
  - Added support for S49 Mk2 (`17cc:1610`), S61 Mk2 (`17cc:1620`), and S88 Mk2 (`17cc:1630`).
  - Added dual-interface support (Interface 2 for controls/LEDs, Interface 3 for dual 480x272 screens) using `dual_handle` and `detach_kernel_driver` quirks.
  - Mapped 28 buttons, 8 screen encoders + 1 4D directional joystick/push encoder, and transport controls.
  - Implemented RGB Light Guide per-key LED arrays and button LEDs.
  - Added hardware reference documentation (`docs/hardware/ni_komplete_kontrol_mk2.md`).
- **Maschine Mk3 Velocity & Pressure Pads**:
  - Implemented Report `0x02` pad packet decoding with double-pumped 128-byte packet handling (Set A & Set B 64-byte chunks).
  - Reverse-engineered 3-byte tuple decoding (`pad_index`, `d1`, `d2`) with 12-bit pressure extraction (`((d1 & 0xf) << 8) | d2`).
  - Implemented state-machine event dispatch matching NI's hardware service: `0x00` (Switch ON), `0x10` (Hit attack velocity ON), `0x20`/`0x30` (Switch/Hit OFF release), and `0x40` (continuous Aftertouch).
  - Added 16 RGB Pad LED control via indexed palette and brightness levels in Report `0x81`.
  - Added `examples/maschine_mk3/pad_rainbow.rs` demonstration and HTML screen visualizers (`screens/mk3_pad_matrix.html`, `screens/mk3_group_status.html`).
- **Maschine Mk2 Support**:
  - Added descriptor and hardware documentation for NI Maschine Mk2 (`17cc:1200`).
- **Kontrol S8 Mixer Protocol Implementation**:
  - Added all 20 analog mixer knobs across channels A, B, C, D (Gain, Hi EQ, Mid EQ, Low EQ, and Filter) to Report ID 2 (`sliders` packet).
  - Implemented 4-bit 1-byte resolution handling for Low EQ on Channels C and D (`max_value: 15`).
  - Added Master Tempo rotary encoder (`tempo_encoder`, Report 1 byte 3, 4-bit `wrap16`) and `mixer_tempo` button (byte 23 mask `0x04`).
  - Implemented `feature_report_leds` quirk architecture in core descriptor and background USB device thread (`device_thread.rs`) using EP0 Control Transfers (`SET_REPORT`, `0xF4`) for Cue/PFL (`0x26`), Filter On (`0x25`), and Direct Thru (`0x24`) LEDs.
  - Updated Kontrol S8 hardware documentation with complete mixer input/output and quirk details.
- **Core & LED Enhancements**:
  - Added `LedBuilder` for fluent and type-safe LED state construction.
  - Added multi-group LED batching and background worker thread dispatch.
- Bumped workspace crates to `v0.3.0`.

### v0.2.0
- Added GPU-accelerated screen diffing and blitting pipeline.
- Added WebView screen renderer via `encdr-view`.
- Initial Kontrol S8, D2, and Maschine Mk3 controller support.

## License

[GPL-3.0-or-later](LICENSE)
