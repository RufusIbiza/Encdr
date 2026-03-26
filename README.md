# Encdr

A lightweight Rust crate for communicating with proprietary USB HID controller hardware. Provides data-driven device definitions, real-time input parsing, LED/screen output, GPU-accelerated frame management, and an optional WebView-based screen renderer.

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
│                     Consumer App (e.g. Steyr)                   │
│                                                                 │
│  ← Receives: named events (button, slider, encoder, touch)     │
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
       │               │            │             │ OR raw pixels │
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
       │            Device Instance (data-driven)              │
       │         Loaded from JSON device descriptor            │
       └──────────────────────┬────────────────────────────────┘
                              │
                     ┌────────▼────────┐
                     │   USB Transport  │
                     │     (nusb)       │
                     └─────────────────┘
```

## Workspace Structure

```
encdr/
├── encdr/                  Core crate
│   ├── descriptors/        Built-in JSON device descriptors
│   │   └── ni_kontrol_d2.json
│   └── src/
│       ├── lib.rs          Encdr facade + public API
│       ├── core/           Event types, descriptor model, LED types, errors
│       ├── device/         Packet parser, encoder state, LED builder, hooks
│       ├── screen/         GPU pipeline, format conversion, frame diff, protocol
│       └── usb/            Device thread, hotplug, transport
│
├── encdr-view/             WebView screen renderer (optional)
│   └── src/
│       ├── lib.rs          ScreenView public API
│       ├── webview.rs      Offscreen WebView lifecycle (wry + WebKitGTK)
│       ├── capture.rs      Pixel capture via WebKit snapshot
│       └── bridge.rs       Rust ↔ JS message passing
│
├── encdr/examples/         Core crate examples
│   ├── probe.rs            List connected devices and all controls
│   └── monitor.rs          Print all events from all devices
│
├── encdr-view/examples/    WebView examples
│   └── screen_test.rs      Show knob/button state on D2 screen
│
├── screens/                Example HTML screen templates
│   └── d2_controls.html    D2 control visualizer
│
└── docs/                   Detailed documentation
    ├── usage.md            How to use the crate
    └── hardware/
        └── ni_kontrol_d2.md  D2 hardware reference
```

## Supported Hardware

| Device | VID:PID | Status | Controls | LEDs | Screens |
|--------|---------|--------|----------|------|---------|
| NI Kontrol D2 | `17cc:1400` | Implemented | 57 buttons/touches, 6 encoders, 9 sliders | 8 RGB pads, 5 singles, 2 strips | 480x272 BGR565 |
| NI Maschine Mk3 | — | Planned | — | — | — |

## Dependencies

| Purpose | Crate | Why |
|---------|-------|-----|
| USB transport | `nusb` | Pure Rust, async, cross-platform |
| GPU compute | `wgpu` | Format conversion + frame diff shaders |
| Lock-free channel | `crossbeam-channel` | SPSC event delivery |
| Descriptor parsing | `serde` + `serde_json` | JSON device descriptors |
| Pixel buffer utils | `bytemuck` | Zero-copy transmutes |
| Logging | `tracing` | Structured, zero-overhead when disabled |
| Error handling | `thiserror` | Typed errors |
| Async executor | `futures-lite` | Lightweight internal async |
| WebView (optional) | `wry` + `tao` | Offscreen HTML rendering |
| WebKit snapshot | `webkit2gtk` + `cairo-rs` | Linux pixel capture |

## Documentation

See [docs/usage.md](docs/usage.md) for a comprehensive usage guide and [docs/hardware/ni_kontrol_d2.md](docs/hardware/ni_kontrol_d2.md) for a complete D2 hardware reference.

## License

MIT OR Apache-2.0
