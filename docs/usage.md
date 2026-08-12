# Encdr Usage Guide

## Installation

Add encdr to your `Cargo.toml`:

```toml
[dependencies]
encdr = { path = "path/to/encdr/encdr" }

# Optional: WebView screen renderer (Linux, macOS, Windows)
encdr-view = { path = "path/to/encdr/encdr-view" }
```

Encdr requires a GPU that supports wgpu (Vulkan, Metal, or DX12). The GPU is used for screen format conversion and frame diffing.

### Platform Prerequisites

**Linux:**
```bash
# USB access (nusb)
sudo apt install libudev-dev

# WebView renderer (encdr-view only)
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev
```

You'll also need a udev rule to access NI hardware without root:

```
# /etc/udev/rules.d/99-ni-controllers.rules
SUBSYSTEM=="usb", ATTR{idVendor}=="17cc", MODE="0666"
```

Then reload: `sudo udevadm control --reload-rules && sudo udevadm trigger`

**macOS:** No additional system dependencies — `encdr-view` uses the built-in WKWebView via `tao` + `wry`.

**Windows:** No additional system dependencies — `encdr-view` uses the built-in WebView2 runtime (included with Windows 10/11) via `tao` + `wry`.

---

## Core Concepts

### Encdr Facade

`Encdr` is the main entry point. It manages device detection, I/O threads, and event delivery.

```rust
use encdr::{Encdr, EncdrConfig};

let mut encdr = Encdr::new(EncdrConfig::default()).unwrap();
```

### Device Scanning

Call `scan()` to detect connected devices. This matches USB VID:PID against loaded descriptors, spawns per-device I/O threads, and emits `DeviceConnected` events.

```rust
let device_ids = encdr.scan().unwrap();
```

Encdr loads built-in descriptors automatically (NI Kontrol D2 and NI Maschine Mk3). You can add custom descriptors:

```rust
// Load all JSON files from a directory
encdr.load_descriptor_dir("./my_controllers/").unwrap();

// Or load a single JSON string
encdr.load_descriptor_json(include_str!("my_device.json")).unwrap();
```

### Event Loop

Events arrive on a crossbeam channel. Use `try_recv()` for non-blocking polling or `recv()` / `recv_timeout()` for blocking.

```rust
let events = encdr.events().clone();

loop {
    while let Ok(event) = events.try_recv() {
        match event {
            Event::DeviceConnected { id, descriptor } => {
                println!("Connected: {} ({} controls)",
                    descriptor.name, descriptor.control_count());
            }
            Event::DeviceDisconnected { id } => {
                println!("Disconnected: {:?}", id);
            }
            Event::Button { device, name, pressed } => {
                // name is &'static str, e.g. "play", "pad_3", "deck_a"
                println!("{}: {}", name, pressed);
            }
            Event::Slider { device, name, value } => {
                // value is f32, normalized 0.0 - 1.0
                println!("{}: {:.3}", name, value);
            }
            Event::Encoder { device, name, delta } => {
                // delta is signed steps (typically -1 or +1)
                println!("{}: {:+}", name, delta);
            }
            Event::EncoderFine { device, name, delta } => {
                // delta is sub-step precision (scaled by descriptor)
                println!("{}: {:+.4}", name, delta);
            }
            Event::Touch { device, name, touched } => {
                println!("{}: {}", name, if touched { "touch" } else { "release" });
            }
            Event::Grid { device, name, index, pressure } => {
                println!("{} pad {}: {:.2}", name, index, pressure);
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(1));
}
```

### Event Name Conventions

All event names come directly from the JSON device descriptor. They are interned as `&'static str` at descriptor load time, so there's no per-event allocation. The names are designed to be human-readable and match the physical hardware:

- **Buttons**: `play`, `cue`, `sync`, `shift`, `pad_1` through `pad_8`, `deck_a` through `deck_d`, `screen_left_1` through `screen_left_4`, etc.
- **Sliders**: `fader_1` through `fader_4`, `fx_dial_1` through `fx_dial_4`, `touchstrip`
- **Encoders**: `browse`, `loop_enc`
- **Fine Encoders**: `screen_encoder_1` through `screen_encoder_4`
- **Touch**: `fader_touch_1` through `fader_touch_4`, `screen_encoder_touch_1` through `screen_encoder_touch_4`, `encoder_browse_touch`, `encoder_loop_touch`

---

## LED Control

Set LEDs by their descriptor name. Encdr maps the name to the correct byte offset in the LED buffer and flushes on the next USB write cycle.

```rust
use encdr::LedValue;

// Single-color LED (brightness 0-255)
encdr.set_led(device_id, "play", LedValue::Single(127));
encdr.set_led(device_id, "play", LedValue::Off);

// RGB pad LED
encdr.set_led(device_id, "pad_1", LedValue::Rgb { r: 255, g: 0, b: 128 });

// LED strip (array of brightness values)
let strip = vec![127u8; 25]; // all LEDs at half brightness
encdr.set_led_strip(device_id, "touchstrip_blue", &strip);

// Touchstrip position indicator
let mut strip = vec![0u8; 25];
let position = 12; // LED index 0-24
strip[position] = 255;
encdr.set_led_strip(device_id, "touchstrip_orange", &strip);
```

### D2 LED Names

| Name                | Type       | Description                 |
| ------------------- | ---------- | --------------------------- |
| `pad_1` - `pad_8`   | RGB        | Performance pad LEDs        |
| `play`              | Single     | Play button backlight       |
| `cue`               | Single     | Cue button backlight        |
| `sync_green`        | Single     | Sync button green component |
| `sync_red`          | Single     | Sync button red component   |
| `shift`             | Single     | Shift button backlight      |
| `deck_a` - `deck_d` | Single     | Deck selector backlights    |
| `touchstrip_blue`   | Strip (25) | Touchstrip blue channel     |
| `touchstrip_orange` | Strip (25) | Touchstrip orange channel   |

### Mk3 LED Names

The Mk3 has two LED buffer groups (button LEDs and touchstrip), all single-color.

| Name                                       | Type   | Description                       |
| ------------------------------------------ | ------ | --------------------------------- |
| `play`, `rec`, `stop`                      | Single | Transport control backlights      |
| `restart`, `erase`, `tap`, `follow`        | Single | Transport secondary backlights    |
| `shift`, `fixed_vel`                       | Single | Modifier backlights               |
| `pad_mode`, `keyboard`, `chords`, `step`   | Single | Pad mode backlights               |
| `scene`, `pattern`, `events`, `variations` | Single | Sequencer mode backlights         |
| `duplicate`, `select`, `solo`, `mute`      | Single | Pad action backlights             |
| `top_1` - `top_8`                          | Single | Top row button backlights         |
| `group_a` - `group_h`                      | Single | Group selector backlights         |
| `channel`, `plugin`, `arranger`, `mixer`   | Single | View mode backlights              |
| `browser`, `sampling`                      | Single | Browser/sampling backlights       |
| `arrow_left`, `arrow_right`                | Single | Navigation arrow backlights       |
| `file`, `settings`, `auto`, `macro`        | Single | Utility backlights                |
| `volume`, `swing`, `note_repeat`, `tempo`  | Single | Parameter backlights              |
| `lock`, `pitch`, `mod`, `perform`, `notes` | Single | Mode backlights                   |
| `encoder_up/down/left/right`               | Single | Encoder push direction backlights |
| `touchstrip`                               | Strip  | Touchstrip LED array              |

---

## Screen Output

### Raw Pixel Buffer (Tier 1)

For apps that render their own pixels:

```rust
// RGBA8888 input - Encdr handles GPU conversion to BGR565, diffing, and USB transfer
let pixels = vec![0u8; 480 * 272 * 4]; // black screen
encdr.submit_screen(device_id, "main", &pixels);

// Or submit in native format to skip GPU conversion
use encdr::PixelFormat;
let native_pixels = vec![0u8; 480 * 272 * 2]; // BGR565-BE
encdr.submit_screen_with_format(device_id, "main", &native_pixels, PixelFormat::Bgr565Be);
```

The screen pipeline automatically:
1. Converts RGBA8 to the device's native pixel format (BGR565-BE for D2) via GPU compute shader
2. Compares against the previous frame to find dirty regions
3. Sends only the changed region (partial blit) if <50% dirty, or a full blit otherwise
4. Sends periodic keyframes (~every 60 frames) to prevent drift

### WebView Renderer (Tier 2)

`encdr-view` renders HTML/CSS/Canvas content in an offscreen WebView and feeds the captured pixels into the core pipeline. This lets you build screen UIs with standard web technologies.

```rust
use encdr_view::{ScreenView, ScreenContent};

// Create a WebView backed screen
// Create headless (offscreen) — set to `true` to show a desktop debug window
let view = ScreenView::new(
    &encdr,
    device_id,
    "main",
    ScreenContent::File("./screens/deck.html".to_string()),
    false,
).unwrap();

// Push state updates - JS in the WebView handles rendering
view.send("track", serde_json::json!({
    "title": "Blue Monday",
    "artist": "New Order",
    "bpm": 130.0,
}));

// In your main loop: pump events and capture frames
loop {
    ScreenView::pump_events(); // Drive platform event loop (GTK/tao)
    view.poll(&encdr);          // Capture & submit if dirty
    std::thread::sleep(std::time::Duration::from_millis(16));
}
```

The HTML page receives state via the injected `window.encdr` bridge:

```html
<script>
window.encdr = {
    onMessage(channel, data) {
        if (channel === 'track') {
            document.getElementById('title').textContent = data.title;
            document.getElementById('artist').textContent = data.artist;
        }
        // After DOM updates, encdr automatically captures on next animation frame
    }
};
</script>
```

The WebView approach works with standard HTML, CSS, Canvas, SVG — anything the browser compositor renders. Pixel capture uses native platform APIs (not JavaScript `getImageData`), so it captures the full composited output:

- **Linux**: WebKitGTK snapshot → Cairo surface → RGBA
- **macOS**: WKWebView `takeSnapshot` → NSBitmapImageRep → RGBA
- **Windows**: WebView2 `CapturePreview` → PNG decode → RGBA

---

## GPU Context Sharing

If your app already has a wgpu device (e.g., for audio processing or rendering), you can share it with Encdr to avoid creating a second GPU context:

```rust
use std::sync::Arc;
use encdr::{Encdr, EncdrConfig, GpuContext};

let gpu = GpuContext::from_existing(
    Arc::clone(&my_device),
    Arc::clone(&my_queue),
);

let encdr = Encdr::new(EncdrConfig {
    gpu: Some(gpu),
    ..Default::default()
}).unwrap();
```

If no GPU context is provided, Encdr creates its own with `wgpu::PowerPreference::HighPerformance`.

---

## Custom Device Descriptors

Devices are defined by JSON files. See [hardware/ni_kontrol_d2.md](hardware/ni_kontrol_d2.md) and [hardware/ni_maschine_mk3.md](hardware/ni_maschine_mk3.md) for complete annotated examples. The key sections:

- **`interfaces`**: USB interface numbers and endpoint addresses
- **`input_packets`**: Packet layouts with byte offsets, bitmasks, and encodings
- **`leds`**: LED buffer layout with byte offsets and types (RGB, single, strip)
- **`screens`**: Screen dimensions, pixel format, and blit protocol (headers/footers)
- **`quirks`**: Device-specific flags

### Supported Input Types

| Type                   | JSON `type`    | Event                | Fields                                                            |
| ---------------------- | -------------- | -------------------- | ----------------------------------------------------------------- |
| Button                 | `button`       | `Event::Button`      | `byte`, `mask`                                                    |
| Touch sensor           | `touch`        | `Event::Touch`       | `byte` + `mask` (single-byte), or `bytes` (multi-byte, value > 0) |
| Slider/fader           | `slider`       | `Event::Slider`      | `byte`/`bytes`, `bits`, `normalize`, `max_value`                  |
| Notched encoder        | `encoder`      | `Event::Encoder`     | `byte`, `bits`, `bit_offset`, `encoding`                          |
| Fine encoder / Jogdial | `encoder_fine` | `Event::EncoderFine` | `bytes`, `encoding`, `scale`                                      |

Touch sensors support two modes:
- **Single-byte**: `"byte": 9, "mask": "0x02"` — standard bitmask check
- **Multi-byte**: `"bytes": [13, 14]` — touched when any byte is non-zero (e.g. touchstrip position value > 0)

### Supported Encoder Encodings

| Encoding      | Description                                                     |
| ------------- | --------------------------------------------------------------- |
| `wrap16`      | 4-bit counter with wraparound (used by D2 browse/loop encoders) |
| `signed16`    | 16-bit signed delta (used by D2 screen encoders)                |
| `unsigned16`  | 16-bit unsigned (absolute position)                             |
| `wrap16_wide` | 16-bit counter with full wraparound (jogwheels/jogdials)        |

The `wrap16_wide` encoding detects direction via shortest path around the 65536-step ring. Use `encoder_fine` with `wrap16_wide` for jogdials:
```json
{ "type": "encoder_fine", "name": "jogwheel", "bytes": [5, 6], "encoding": "wrap16_wide", "scale": 1000.0 }
```

### PacketHook Escape Hatch

For protocols that can't be expressed in JSON, register a custom hook:

```rust
use encdr::PacketHook;
use encdr::core::event::{DeviceId, Event};

struct MyHook;

impl PacketHook for MyHook {
    fn on_packet(
        &mut self,
        device_id: DeviceId,
        data: &[u8],
        events: &mut Vec<Event>,
    ) -> bool {
        // Return true to consume the packet (skip normal parsing)
        // Return false to let the normal parser handle it
        false
    }
}
```

---

## Device Descriptor Introspection

You can enumerate a device's capabilities at runtime:

```rust
let desc = encdr.device_descriptor(device_id).unwrap();

println!("Device: {} by {}", desc.name, desc.manufacturer);
println!("Controls: {}", desc.control_count());

for input in desc.all_inputs() {
    println!("  {}: {:?}", input.name(), input);
}

for screen in &desc.screens {
    println!("  Screen '{}': {}x{} {:?}",
        screen.name, screen.width, screen.height, screen.pixel_format);
}

for leds in &desc.leds {
    for led in &leds.items {
        println!("  LED: {}", led.name());
    }
}
```

---

## Examples

Run examples from the workspace root:

```bash
# List all loaded descriptors and scan for connected devices
cargo run -p encdr-examples --bin probe

# Print all events from connected devices (Ctrl+C to quit)
cargo run -p encdr-examples --bin monitor

# Show D2 knob/button positions on the screen (requires D2 plugged in)
cargo run -p encdr-examples --bin d2_screen_test

# Show Maschine Mk3 encoder/button state on dual screens (requires Mk3 plugged in)
cargo run -p encdr-examples --bin mk3_screen_test
```

---

## Cleanup and Shutdown

Encdr clears all LEDs on device disconnect and joins I/O threads cleanly:

```rust
// Disconnect a specific device
encdr.disconnect(device_id);

// Disconnect all and shut down
encdr.shutdown();

// Or just drop — Drop impl calls shutdown()
drop(encdr);
```
