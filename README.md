# Encdr

### What is it?

Encdr is a Rust library that allows you to easily add Native Instruments (NI) controller support to your application. It handles all the low-level USB HID communication and screen rendering for you. It also offers additional helper functions for specific hardware features, such as push encoder detection, light rings, and 7-segment LED displays. Although most HID-based NI controllers are supported at this point, adding support for additional controllers just requires creating a new JSON descriptor file and registering it with the descriptor registry. 

Screen output is achieved in one of two ways:
- **Raw pixel data** (the default) — send raw RGBA pixel data, and Encdr will handle the colour space conversion, GPU-accelerated diffing, encoding, and USB transfer.
- **WebView** — use the wgpu-based WebView implementation in the `encdr-view` crate. This allows you to create your screen UI using HTML/CSS/Canvas and let Encdr handle GPU-accelerated rendering to the device screens.

Inspired by the [openAV-Ctlra](https://github.com/openAVproductions/openAV-Ctlra) C library, reimagined in Rust with data-driven device descriptors, zero-copy I/O, and a GPU-accelerated screen pipeline.

`#NativeInstruments` `#NI`

#### Design Philosophy

1. **Exposes hardware truthfully** — every button, slider, encoder, LED, and screen is enumerated and accessible by name. The consuming app decides what each control does.
2. **Data-driven device definitions** — new devices are added via JSON descriptor files, not Rust code. The descriptor defines USB endpoints, byte-level packet layouts, LED mappings, and screen protocols.
3. **Two-tier screen pipeline** — an optional WebView renderer (`encdr-view`) lets apps build screen UIs with HTML/CSS/Canvas, while the core module accepts raw pixel buffers. Both share the same GPU conversion/diff/transfer backend.
4. **Optimizes for latency** — async USB I/O, lock-free event delivery, GPU-side format conversion, dirty-region-only transfers.

### What can it do?

On its own, nothing. Add a sprinkling of imagination, though, and it allows you to use your NI controller for whatever you can dream up. Here are some ideas: 
- A VJ software controller with cue/master outputs on the dual screens
- A fully integrated DAW controller
- A stream deck with camera angle and screen overlay controls
- A control surface for your smart lights
- Color correction controller for photo/video editing

### What can't it do?

- **iPhone/iPad (iOS/iPadOS) support** — Apple does not allow non-standard USB communication with non-MFi certified devices without custom hardware or driver entitlements, which is outside the scope of this library.
- **Audio interface streaming** — Encdr is strictly a control surface, LED, and display driver library. Audio streaming (inputs, outputs, soundcards) is handled directly by standard OS audio subsystems (ALSA/PipeWire on Linux, CoreAudio on macOS, ASIO/WASAPI on Windows).
- **Native Instruments software integration** — Encdr communicates directly with the controller hardware; it does not interface with Traktor Pro, Maschine, or Komplete Kontrol software libraries, or proprietary NKS preset databases. It can't add additional functionality to these apps, nor can it make these apps support non-NI or previously unsupported hardware.
- **Wireless or Bluetooth operation** — All supported controllers communicate strictly over wired USB.

### How do I install it?

If you're asking this question, Encdr is probably not for you. It's a developer tool, and needs to be compiled into a larger application to be useful. It also requires you to be comfortable with Rust programming (although I've tried to make it as easy to use with AI/LLM coding tools as possible — just point your LLM at [this repository](https://github.com/RufusIbiza/Encdr), and it should be able to guide you through the process of building and running an app).

If you're a developer, you can add it to your project using Cargo:

```bash
cargo add encdr
```

And additionally, if you want to use the WebView-based screen implementation:
```bash
cargo add encdr-view
```
> [!NOTE]
> **Platform Prerequisites**: On Linux, compiling requires `libudev-dev` (plus `libwebkit2gtk-4.1-dev` and `libgtk-3-dev` if using `encdr-view`), and non-root hardware access requires setting up a udev rule ([see here](docs/usage.md#platform-prerequisites)). 
>On **macOS and Windows**, NI background services may need to be stopped if they hold the USB interfaces. 

### How can I support this project? 
- **By contributing code** - Feel free to open an issue or submit a pull request.
- **By contributing time** - If you have a Native Instruments device that isn't supported (or properly tested), providing feedback helps us get to the point where all devices are properly supported and tested. 
- **By contributing money** - Figuring out the screen and communication protocols for all the NI controllers has taken over 2 years by this point. If you'd like to support this project's continued development, you can do so through [Ko-fi](https://ko-fi.com/rufuswhite) or [GitHub Sponsors](https://github.com/sponsors/RufusIbiza/).
- **By contributing stars** - If you like Encdr, please consider giving it a star on GitHub!

### I made something with Encdr, what now?

Awesome! Please share it with the community by posting in the ['Show and Tell'](https://github.com/RufusIbiza/Encdr/discussions/categories/show-and-tell) category in the discussions tab here on GitHub!

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
## Documentation

### Getting Started with Encdr
- [Quick Start Tutorial](docs/quickstart.md) — step-by-step tutorial with Maschine Mk3 dual-screen walkthrough
- [API Reference](docs/api_reference.md) — complete programmer reference for all types, functions, and methods
- [Usage Guide](docs/usage.md) — comprehensive usage guide
- [Examples Guide](examples/README.md) — complete directory of hardware examples, screen tests, and diagnostic utilities

### Hardware Reference
- [NI Kontrol D2](docs/hardware/ni_kontrol_d2.md)
- [NI Kontrol F1](docs/hardware/ni_kontrol_f1.md)
- [NI Kontrol S2 Mk1](docs/hardware/ni_kontrol_s2_mk1.md)
- [NI Kontrol S2 Mk2](docs/hardware/ni_kontrol_s2_mk2.md)
- [NI Kontrol S4 Mk2](docs/hardware/ni_kontrol_s4_mk2.md)
- [NI Kontrol S4 Mk3](docs/hardware/ni_kontrol_s4_mk3.md)
- [NI Kontrol S5](docs/hardware/ni_kontrol_s5.md)
- [NI Kontrol S8](docs/hardware/ni_kontrol_s8.md)
- [NI Kontrol X1 Mk1](docs/hardware/ni_kontrol_x1_mk1.md)
- [NI Kontrol X1 Mk2](docs/hardware/ni_kontrol_x1_mk2.md)
- [NI Kontrol X1 Mk3](docs/hardware/ni_kontrol_x1_mk3.md)
- [NI Kontrol Z1](docs/hardware/ni_kontrol_z1.md)
- [NI Kontrol Z2](docs/hardware/ni_kontrol_z2.md)
- [NI Maschine Jam](docs/hardware/ni_maschine_jam.md)
- [NI Maschine Mikro Mk1](docs/hardware/ni_maschine_mikro_mk1.md)
- [NI Maschine Mikro Mk2](docs/hardware/ni_maschine_mikro_mk2.md)
- [NI Maschine Mikro Mk3](docs/hardware/ni_maschine_mikro_mk3.md)
- [NI Maschine Mk1](docs/hardware/ni_maschine_mk1.md)
- [NI Maschine Mk2](docs/hardware/ni_maschine_mk2.md)
- [NI Maschine Mk3](docs/hardware/ni_maschine_mk3.md)
- [NI Maschine Plus](docs/hardware/ni_maschine_plus.md)
- [NI Maschine Studio](docs/hardware/ni_maschine_studio.md)
- [NI Komplete Kontrol Mk2](docs/hardware/ni_komplete_kontrol_mk2.md)
- [NI Komplete Kontrol Mk3](docs/hardware/ni_komplete_kontrol_mk3.md)

## Supported Hardware

| Device                    | VID:PID           | Status      | Controls                                      | LEDs                                        | Screens           |
| ------------------------- | ----------------- | ----------- | --------------------------------------------- | ------------------------------------------- | ----------------- |
| NI Kontrol D2             | `17cc:1400`       | Implemented | 57 buttons/touches, 6 encoders, 9 sliders     | 8 RGB pads, 5 singles, 2 strips             | 480x272 BGR565    |
| NI Kontrol F1             | `17cc:1120`       | Implemented | 8 buttons, 4 faders, 4 rotary knobs, 1 encoder | 16 RGB matrix pads, 8 singles, 7-seg display | —                 |
| NI Kontrol S2 Mk1         | `17cc:1101`       | Implemented | 34 buttons, 2 jogwheels, 7 encoders, 16 faders/knobs | 8 dual-color pads (G/B), 24 singles, VU meters | — |
| NI Kontrol S2 Mk2         | `17cc:1320`       | Implemented | 38 buttons, 2 jogwheels, 7 encoders, 16 faders/knobs | 8 RGB pads, 20 singles, VU meters           | —                 |
| NI Kontrol S4 Mk2         | `17cc:1310`       | Implemented | 48 buttons, 2 jogwheels, 29 faders/knobs      | 8 RGB pads, 24 singles, VU meters, 2x 2-digit 7-seg | — |
| NI Kontrol S4 Mk3         | `17cc:1720`       | Implemented | 72 buttons, 6 encoders, 2 motorized jogwheels, 28 faders/knobs | 16 RGB pads, 45 singles, 8-seg VU, LED rings, Haptic Drive | 2x 320x240 BGR565 |
| NI Kontrol S5             | `17cc:1420`       | Implemented | 70 buttons/touches, 6 encoders, 21 faders/knobs | 16 RGB pads, 34 singles                     | 2x 480x272 BGR565 |
| NI Kontrol S8             | `17cc:1370`       | Implemented | 114 buttons/encoders, 29 faders/knobs         | 16 RGB pads, 100+ singles, EP0 feature LEDs | 2x 480x272 BGR565 |
| NI Kontrol X1 Mk1         | `17cc:2305`/`1000` | Implemented | 30 buttons, 4 endless encoders, 8 potentiometers | 30 single-color LEDs                        | —                 |
| NI Kontrol X1 Mk2         | `17cc:1220`       | Implemented | 31 buttons, 3 endless encoders, 1 touchstrip, 8 potentiometers | 8 RGB hotcue LEDs, 23 singles, 2x 7-seg displays, 22-seg strip | — |
| NI Traktor Kontrol X1 Mk3 | `17cc:2200`       | Implemented | 21 buttons, 4 encoders, 8 knobs               | 14 singles, 8 RGB hotcues, 2 RGB underglow  | 5x 128x64 1-bit OLED |
| NI Kontrol Z1             | `17cc:1210`       | Implemented | 7 buttons, 2 faders, 1 crossfader, 9 potentiometers, 1 encoder | 7 singles, stereo 7-segment VU meters       | —                 |
| NI Kontrol Z2             | `17cc:1230`       | Implemented | 34 buttons, 3 faders, 10 potentiometers, 3 encoders | 8 RGB cue pads, 26 singles, stereo VU meters, 2x 7-seg displays | — |
| NI Maschine Jam           | `17cc:1500`       | Implemented | 103 buttons, 1 encoder, 8 dual-touch strips   | 64 RGB matrix, 16 RGB buttons, 35 singles, 8x 11-seg strip meters, stereo VU | — |
| NI Maschine Mikro Mk1     | `17cc:1110`       | Implemented | 28 buttons, 1 rotary encoder, 16 velocity/pressure pads | 28 single-color LEDs, 16 single-color pad LEDs | 128x64 1-bit mono |
| NI Maschine Mikro Mk2     | `17cc:1200`       | Implemented | 28 buttons, 1 rotary encoder, 16 velocity/pressure pads | 28 single-color LEDs, 16 RGB pads           | 128x64 1-bit mono |
| NI Maschine Mikro Mk3     | `17cc:1700`       | Implemented | 29 buttons, 1 rotary encoder, 1 Smart Strip, 16 velocity/pressure pads | 29 singles, 16 RGB pads, 25-seg dual LED Smart Strip | 128x32 1-bit OLED |
| NI Maschine Mk1           | `17cc:0808`       | Implemented | 41 buttons, 11 endless knobs, 16 pressure pads | 16 pad LEDs, 41 singles, display backlight  | 2x 255x64 5-bit gray / 1-bit mono |
| NI Maschine Mk2           | `17cc:1140`       | Implemented | 47 buttons, 11 encoders, 16 velocity pads     | 16 RGB pads, 31 singles                     | 2x 256x64 1-bit   |
| NI Maschine Mk3           | `17cc:1600`       | Implemented | 63 buttons, 10 touches, 9 encoders, 1 slider, 16 pads | 16 RGB pads, 62 singles, 1 strip     | 2x 480x272 BGR565 |
| NI Maschine Plus          | `17cc:1820`       | Implemented | 63 buttons, 10 touches, 9 encoders, 1 slider, 16 pads | 16 RGB pads, 62 singles, 1 strip     | 2x 480x272 BGR565 |
| NI Maschine Studio        | `17cc:1300`       | Implemented | 64 buttons, 5 touches, 10 encoders, 16 pads   | 16 RGB pads, 8 RGB groups, stereo meters, 32-seg ring | 2x 480x272 BGR565 |
| NI Komplete Kontrol S-Mk2 | `17cc:1610/20/30` | Implemented | 28 buttons, 9 encoders, pitch/mod wheels, touchstrip | 20 singles, Light Guide (RGB per-key)       | 2x 480x272 BGR565 |
| NI Komplete Kontrol S-Mk3 | `17cc:2100/10/20` | Preliminary | Keybed (49/61/88 keys), 4D encoder, touchstrip, high-res encoders | Light Guide RGB strips, RGB button backlights | Full-color wide LCD |

## Workspace Structure

```
encdr/
├── encdr/                  Core crate
│   ├── descriptors/        Built-in JSON device descriptors
│   │   ├── ni_komplete_kontrol_s49_mk2.json
│   │   ├── ni_komplete_kontrol_s49_mk3.json
│   │   ├── ni_komplete_kontrol_s61_mk2.json
│   │   ├── ni_komplete_kontrol_s61_mk3.json
│   │   ├── ni_komplete_kontrol_s88_mk2.json
│   │   ├── ni_komplete_kontrol_s88_mk3.json
│   │   ├── ni_kontrol_d2.json
│   │   ├── ni_kontrol_f1.json
│   │   ├── ni_kontrol_s2_mk1.json
│   │   ├── ni_kontrol_s2_mk2.json
│   │   ├── ni_kontrol_s4_mk2.json
│   │   ├── ni_kontrol_s4_mk3.json
│   │   ├── ni_kontrol_s5.json
│   │   ├── ni_kontrol_s8.json
│   │   ├── ni_kontrol_x1_mk1.json
│   │   ├── ni_kontrol_x1_mk2.json
│   │   ├── ni_kontrol_z1.json
│   │   ├── ni_kontrol_z2.json
│   │   ├── ni_maschine_jam.json
│   │   ├── ni_maschine_mikro_mk1.json
│   │   ├── ni_maschine_mikro_mk2.json
│   │   ├── ni_maschine_mikro_mk3.json
│   │   ├── ni_maschine_mk1.json
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
│   ├── komplete_kontrol/   Komplete Kontrol Mk2 & Mk3 examples
│   ├── kontrol_d2/         D2 examples (e.g. d2_screen_test, d2_vegas)
│   ├── kontrol_f1/         F1 examples (e.g. f1_vegas)
│   ├── kontrol_s2/         S2 Mk1 & Mk2 examples (e.g. s2_mk1_vegas, s2_mk2_vegas)
│   ├── kontrol_s4/         S4 Mk2 & Mk3 examples (e.g. s4_mk2_vegas, s4_mk3_vegas)
│   ├── kontrol_s5/         S5 examples (e.g. s5_vegas)
│   ├── kontrol_s8/         S8 examples (e.g. s8_monitor, s8_screen_test, s8_vegas)
│   ├── kontrol_x1_mk1/     X1 Mk1 examples (e.g. x1_mk1_vegas)
│   ├── kontrol_x1_mk2/     X1 Mk2 examples (e.g. x1_mk2_vegas)
│   ├── kontrol_x1_mk3/     X1 Mk3 examples (e.g. x1_mk3_test)
│   ├── kontrol_z1/         Z1 examples (e.g. z1_vegas)
│   ├── kontrol_z2/         Z2 examples (e.g. z2_vegas)
│   ├── maschine_jam/       Maschine Jam examples (e.g. jam_scroller)
│   ├── maschine_mikro/     Maschine Mikro Mk1, Mk2, Mk3 examples (e.g. mikro_mk1/mk2/mk3_vegas)
│   ├── maschine_mk1/       Maschine Mk1 examples (e.g. mk1_vegas)
│   ├── maschine_mk2/       Maschine Mk2 examples (e.g. mk2_vegas)
│   ├── maschine_mk3/       Maschine Mk3 examples (e.g. mk3_screen_test, mk3_vegas, pad_response)
│   ├── maschine_plus/      Maschine Plus examples (e.g. plus_vegas)
│   ├── maschine_studio/    Maschine Studio examples (e.g. studio_vegas)
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
    ├── quickstart.md       Step-by-step tutorial
    ├── api_reference.md    Complete API reference
    └── hardware/           Hardware reference documents for all supported devices
```

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


## Changelog
### v0.7.3
- **Native Instruments Discrete LED Brightness & Half-Brightness Support**:
  - **Button LED Protocol Handling**: Added Native Instruments button LED brightness protocol handling, supporting discrete levels and PWM duty cycle encoding on Report `0x80`:
    - `0x00`: **Off**
    - `0xE4` (228): **Dim / Half-brightness** (idle state, illuminating 1 LED under the button)
    - `0x9E` (158): **Bright / Active** (active state, illuminating both LEDs under the button)
    - `0xFF` (255): **Max drive**
  - **Core LED Engine Enhancements**:
    - Added `LedValue::Dim` and `LedValue::Bright` enum variants with constants (`LedValue::OFF`, `LedValue::DIM`, `LedValue::BRIGHT`, `LedValue::MAX`, `LedValue::NI_OFF`, `LedValue::NI_DIM`, `LedValue::NI_BRIGHT`, `LedValue::NI_MAX`).
    - Added helper methods `LedValue::to_ni_single_byte(pct)` and `LedValue::single_percent(pct)` for setting arbitrary duty cycle percentages ($0..100\%$).
    - Updated `LedValue::brightness()` to return standard levels (`228` for `Dim`, `158` for `Bright`).
  - **Universal Multi-Target Mapping**:
    - Updated `LedBuilder` to automatically resolve `LedValue::Dim` and `LedValue::Bright` across all descriptor mappings: single monochrome button LEDs (`228` / `158`), RGB LEDs (`64` / `255`), and NI indexed palette LEDs (intensity `1` / intensity `3` white).
    - Updated `JogRing::set_led()` and USB device thread feature report handlers to support `Dim` and `Bright` states.
  - **Example & Telemetry**:
    - Added `mk3_button_brightness` test example demonstrating full 47-button dimming and active elevation on press/release for Maschine Mk3 / Maschine Plus.
  - **Documentation & Hardware Reference Updates**:
    - Updated hardware references for NI Maschine Mk3 ([`docs/hardware/ni_maschine_mk3.md`](docs/hardware/ni_maschine_mk3.md)), Maschine Plus ([`docs/hardware/ni_maschine_plus.md`](docs/hardware/ni_maschine_plus.md)), Maschine Mikro Mk3 ([`docs/hardware/ni_maschine_mikro_mk3.md`](docs/hardware/ni_maschine_mikro_mk3.md)), Komplete Kontrol Mk2 ([`docs/hardware/ni_komplete_kontrol_mk2.md`](docs/hardware/ni_komplete_kontrol_mk2.md)), and Komplete Kontrol Mk3 ([`docs/hardware/ni_komplete_kontrol_mk3.md`](docs/hardware/ni_komplete_kontrol_mk3.md)).
    - Updated [`docs/api_reference.md`](docs/api_reference.md) with complete `LedValue` documentation, constants, helper methods, and multi-target mapping semantics.

### v0.7.2
- **Traktor Kontrol S4 MK3 Jog Wheel LED Ring Support & Real-Time Sync**:
  - **Descriptor Enhancements**: Split the shared Report `0x32` mapping into independent `left_wheel_leds` and `right_wheel_leds` groups with dedicated 32-element `strip` arrays (`left_wheel_ring`, `right_wheel_ring`) and individually addressable segment controls (`left_ring_1..32`, `right_ring_1..32`).
  - **Input Position Mapping**: Mapped `left_jog_pos` (bytes `[15, 16]`) and `right_jog_pos` (bytes `[43, 44]`) in Report 3 to deliver 16-bit absolute platter positions (`0..2879` ticks per revolution).
  - **Core Types & Helpers**: Introduced `JogDeck`, `JogRingMode`, `JogRing` (32-segment buffer helper with spinner and arc meter generators), and `JogWheelTracker` (angular displacement tracker across manual spin and motorized turntable rotations).
  - **High-Level API**: Added `set_jog_ring_needle()`, `set_jog_ring_mode()`, `set_jog_ring_leds()`, `set_jog_ring()`, and `set_jog_ring_off()` on `Encdr`.
  - **Real-Time Hardware Synchronization**: Added `sync_jog_ring_from_event()`, `sync_jog_ring_normalized()`, `sync_jog_ring_radians()`, and `sync_jog_ring_position()` enabling ring spots to lock in 1:1 physical synchronization with the wheel under both manual manipulation and motorized turntable rotation.
  - **Vegas Demo & Documentation**: Updated `s4_mk3_vegas` demo with live spinning needle tracking, capacitive touch response, and 32-segment animated rainbow spinner chase; updated hardware reference, API reference, and usage documentation.

### v0.7.0
- **NI Maschine Mk1 Hardware Support**:
  - Added full hardware support for the original **NI Maschine (Mk1)** (`17cc:0808`), contributed by [@nullobject](https://github.com/nullobject) in [PR #2](https://github.com/RufusIbiza/Encdr/pull/2).
  - **Inputs**: Mapped all 41 buttons, 16 pressure-sensitive velocity pads (`pad_format: "id_pressure_words"`), and 11 endless rotary potentiometers (`encoding: "erp"`, with analog calibration and deadband filtering).
  - **LEDs**: Mapped all 58 LEDs across two dimming banks (`0x0c, 0x00` and `0x0c, 0x1e`), including startup initialisation for the LCD backlight.
  - **ST7529 Dual Displays & Protocol**: Added controller initialization and chunked packet framing for the dual $255 \times 64$ Sitronix ST7529 displays.
  - **Engine Features**: Added support for multi-endpoint round-robin input polling, `report_id` and `pad_format` packet routing, multi-byte LED prefixes, typed one-shot connect `init_writes`, and load-time descriptor validation.
  - **Monochrome & Mk2 Cross-Compatibility**: Added bidirectional `PixelFormat::Mono` $\leftrightarrow$ `PixelFormat::St7529Gray5` conversion in the screen pipeline, enabling standard 1-bit monochrome framebuffers (such as those from Maschine Mk2 and X1 Mk3) to render directly on Mk1 with automatic 32-byte scanline stride alignment and clipping. Native 5-bit grayscale (`PixelFormat::Rgba8888` / `Rgb888`) remains fully supported.
  - **Documentation & Example**: Added complete hardware documentation in [`docs/hardware/ni_maschine_mk1.md`](docs/hardware/ni_maschine_mk1.md) and interactive Vegas / telemetry demo in [`examples/maschine_mk1/mk1_vegas.rs`](examples/maschine_mk1/mk1_vegas.rs).

### v0.6.1
- **Maschine Studio Pad Stream Fix**:
  - Added missing `pads` input packet descriptor (Report `0x20` continuous 16-channel 12-bit ADC stream) to [`encdr/descriptors/ni_maschine_studio.json`](encdr/descriptors/ni_maschine_studio.json).
  - Enables the active pad streaming state machine, sliding median filter, and hysteresis thresholds for NI Maschine Studio controllers (`17cc:1300`), ensuring full pad responsiveness parity across all Maschine family devices.

### v0.6.0
- **New Hardware Controller Support (8 New Controllers & KK Mk3 Series)**:
  - **Traktor Kontrol Series**: Added JSON descriptors, built-in loader support, and documentation for **Traktor Kontrol F1** (`17cc:1120`), **X1 Mk1** (`17cc:2305`/`1000`), **X1 Mk2** (`17cc:1220`), **Z1** (`17cc:1210`), and **Z2** (`17cc:1230`).
  - **Maschine Mikro Series**: Added full hardware support for **Maschine Mikro Mk1** (`17cc:1110`), **Mikro Mk2** (`17cc:1200`), and **Mikro Mk3** (`17cc:1700`).
  - **Komplete Kontrol Mk3 Series**: Added preliminary descriptor definitions for **Komplete Kontrol S49 / S61 / S88 Mk3** (`17cc:2100`, `2110`, `2120`) including Light Guide keybed strips (49/61/88 keys), 4D encoder, touchstrip, and high-resolution encoders.
- **Dedicated Interactive Vegas / Telemetry Example Suite**:
  - Every single controller supported by Encdr now has a dedicated interactive Vegas demo and telemetry example in the `examples/` directory (`f1_vegas`, `x1_mk1_vegas`, `x1_mk2_vegas`, `z1_vegas`, `z2_vegas`, `s2_mk1_vegas`, `s2_mk2_vegas`, `s4_mk2_vegas`, `s4_mk3_vegas`, `s5_vegas`, `s8_vegas`, `d2_vegas`, `mikro_mk1_vegas`, `mikro_mk2_vegas`, `mikro_mk3_vegas`, `mk2_vegas`, `mk3_vegas`, `plus_vegas`, `studio_vegas`, `kk_mk2_vegas`, `kk_mk3_vegas`).
  - Features real-time animated LED wave / VU meter patterns and complete console telemetry reporting for every button, fader, encoder, and pad strike.
- **Documentation & Platform Guides**:
  - Added dedicated hardware reference documents in [`docs/hardware/`](docs/hardware/) for all newly added devices.
  - Added centralized example directory documentation in [`examples/README.md`](examples/README.md).
  - Added macOS and Windows platform setup guidance in [`docs/usage.md`](docs/usage.md) and [`docs/quickstart.md`](docs/quickstart.md) for managing Native Instruments background services (`NIHardwareAgent` / `NIHardwareService`).

### v0.5.2
- **NI Maschine Jam Hardware Support**:
  - Added full hardware support for Maschine Jam (`17cc:1500`).
  - Added data-driven descriptor [`encdr/descriptors/ni_maschine_jam.json`](encdr/descriptors/ni_maschine_jam.json) with 103 buttons (including the full 64-pad $8\times 8$ Click-Pad matrix `matrix_1_1` through `matrix_8_8`), 1 notched endless encoder with capacitive touch detection, and 8 dual-touch Smart Strips (`touchstrip_1` through `touchstrip_8` with 10-bit position resolution, dual-touch tracking, and touch state detection).
  - Added complete LED output support across 3 distinct USB reports: Report `0x80` for surrounding mode/transport buttons and stereo 8-segment VU meters, Report `0x81` for the $8\times 8$ RGB Click-Pad matrix and top/group buttons via NI packed color palette, and Report `0x82` for the 8 Smart Strip 11-segment LED bar graph meters.
  - Added hardware reference documentation in [`docs/hardware/ni_maschine_jam.md`](docs/hardware/ni_maschine_jam.md).
  - Added built-in loader registration and unit test coverage in [`encdr/src/device/loader.rs`](encdr/src/device/loader.rs).
- **New Examples**:
  - Added `jam_scroller` for the NI Maschine Jam, featuring smooth "Encdr" right-to-left text scrolling in purple over orange on the 8x8 Click-Pad matrix, phase-synchronized scrolling sine waves on the 8 Smart Strip 11-segment LED meters, and full console telemetry / visual feedback for all buttons, encoder, and touchstrips.
  - Added `mk3_reddit`, demonstrating interactive HTML/Canvas dual-screen rendering, 4D encoder navigation, touchstrip scrolling, and live media feed browsing on the Maschine Mk3.


### v0.5.0
- **nusb 0.2 Upgrade**:
  - Upgraded `nusb` from `0.1` to `0.2` (0.2.7) in `encdr` and `encdr-examples`.
  - Migrated to the 0.2 API: blocking calls via `MaybeFuture::wait()`, `Endpoint<Interrupt/Bulk, In/Out>` transfers with recycled zero-copy buffers, and `busnum()` / `bus_id()` for device IDs.
  - `EncdrError::Io` now converts from `std::io::Error` via `#[from]`.
- **Pad Responsiveness Fix (Maschine Mk3 / Plus)**:
  - Fixed quick pad taps being dropped, aftertouch updating at ~1 Hz, and pad releases arriving late enough to hit the tap timeout. Interrupt IN reads used a fixed 1024-byte buffer, but a USB transfer only completes on a short packet or a full buffer. The Mk3's 64-byte pad report exactly fills its 64-byte endpoint packet, so pad reports piled up in the kernel until 16 had arrived or a short button report flushed them, and reports were glued together so the parser could not dispatch them.
  - The read buffer in [`run_device`](encdr/src/usb/device_thread.rs) is now sized to the largest input report in the descriptor, rounded up to the endpoint's max packet size, so every report completes its own transfer. The read queue depth went from 4 to 8.
  - The Mk3 pad parser now decodes all 21 tuples a 64-byte set can hold (previously 16), and per-tuple `eprintln!` logging on the real-time read thread has been replaced with `tracing::trace!`.
- **New Example**:
  - Added `mk3_pad_response`, a pad latency and aftertouch demo with dual-screen pad matrix and pressure monitor views.

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
