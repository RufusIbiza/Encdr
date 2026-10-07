# Encdr Usage Guide

> Looking for a step-by-step tutorial? See the [Quick Start Tutorial](quickstart.md) for a full Maschine Mk3 walkthrough.  
> Looking for method signatures and type definitions? See the [API Reference](api_reference.md).  
> Looking for Komplete Kontrol Mk3 DAW & ODR integration? See the [Komplete Kontrol Mk3 Guide](usage_kk_mk3.md).

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

**macOS:**
- No additional system dependencies — `encdr-view` uses the built-in WKWebView via `tao` + `wry`.
- **NI Services**: If you have Native Instruments software installed (Komplete Kontrol, Maschine, Traktor), background daemons (such as `NIHardwareAgent`, `NIHostIntegrationAgent`, or `NTKDaemon`) may hold exclusive USB connections to your controllers. Stop or suspend these services (via Activity Monitor, `launchctl`, or `killall NIHardwareAgent NIHostIntegrationAgent`) before running Encdr apps. Once your Encdr session is finished, the NI services can be restarted without incident.

**Windows:**
- No additional system dependencies — `encdr-view` uses the built-in WebView2 runtime (included with Windows 10/11) via `tao` + `wry`.
- **NI Services**: If Native Instruments software is installed, background services (such as `NIHardwareService` or `NIHostIntegrationAgent`) may claim controller USB interfaces. Stop or suspend these services (via Task Manager, Services Manager `services.msc`, or `net stop NIHardwareService`) before running Encdr apps. Once your Encdr application finishes running, they can be restarted without incident to resume standard NI software operation.

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

Encdr loads built-in descriptors automatically for any supported connected NI hardware ( See [README.md](README.md#supported-hardware) for the list of supported controllers).

You can also add custom descriptors:

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

// RGB pad LED (automatically converted to NI hardware palette when indexed)
encdr.set_led(device_id, "pad_1", LedValue::Rgb { r: 255, g: 0, b: 128 });

// Target a specific LED group if control names exist in multiple reports
encdr.set_led_in_group(device_id, "buttons", "play", LedValue::Single(255));
encdr.set_led_in_group(device_id, "pad_leds", "group_a", LedValue::Rgb { r: 0, g: 255, b: 0 });

// LED strip (array of brightness values)
let strip = vec![127u8; 25]; // all LEDs at half brightness
encdr.set_led_strip(device_id, "touchstrip_blue", &strip);

// Strip targeted to a specific group (e.g. Maschine Studio level meters)
encdr.set_led_strip_in_group(device_id, "master_meters", "meter_left", &vec![255u8; 16]);
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

The Mk3 has two LED buffer groups: Report `0x80` (`buttons`, single-color brightness) and Report `0x81` (`pad_leds`, 16 RGB pads and Smart Strip).

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

### Maschine Plus LED Names

When connected in USB controller mode, the Maschine Plus uses the identical LED mapping and buffer reports as the Maschine Mk3.

### Maschine Studio LED Names

The Maschine Studio features a 4-report LED architecture (`0x80`, `0x81`, `0x82`, `0x83`) driving over 100 individual indicators:

#### Button LEDs (Report `0x80`)
| Name | Type | Description |
| ---- | ---- | ----------- |
| `channel`, `plugin`, `arrange`, `mix` | Single | View/mode button backlights |
| `browse`, `sampling`, `all`, `auto` | Single | Utility mode backlights |
| `tap`, `snap`, `macro`, `note_repeat` | Single | Performance backlights |
| `scene`, `pattern`, `pad_mode`, `navigate` | Single | Sequencer action backlights |
| `duplicate`, `select`, `solo`, `mute` | Single | Pad action backlights |
| `loop`, `metro`, `grid`, `play`, `rec`, `erase` | Single | Transport control backlights |
| `copy`, `paste`, `note`, `nudge` | Single | Edit function backlights |
| `undo`, `redo`, `quantize`, `clear` | Single | Edit history backlights |
| `back`, `nav_left`, `nav_right`, `enter` | Single | Navigation cluster backlights |

#### Pad & Group RGB LEDs (Report `0x81`)
| Name | Type | Description |
| ---- | ---- | ----------- |
| `group_a` - `group_h` | RGB | 8 full RGB Group buttons |
| `pad_1` - `pad_16` | Indexed RGB | 16 velocity/pressure pads (Native Instruments palette indices) |

#### Master Section & Meter LEDs (Report `0x82`)
| Name | Type | Description |
| ---- | ---- | ----------- |
| `in_1` - `in_4` | Single | Audio input selector backlights |
| `master`, `group`, `sound`, `cue` | Single | Level monitoring mode backlights |
| `top_1` - `top_8` | Single | 8 top display button backlights |
| `meter_left` | Strip (16) | Left channel audio level meter (16-segment ladder) |
| `meter_right` | Strip (16) | Right channel audio level meter (16-segment ladder) |

```rust
// Controlling the stereo level meters on Maschine Studio:
let mut left_meter = vec![0u8; 16];
let mut right_meter = vec![0u8; 16];
// Light up first 10 segments on left, 8 on right
left_meter[..10].fill(255);
right_meter[..8].fill(255);
encdr.set_led_strip(device_id, "meter_left", &left_meter);
encdr.set_led_strip(device_id, "meter_right", &right_meter);
```

#### Jogwheel LED Ring (Report `0x83`)
| Name | Type | Description |
| ---- | ---- | ----------- |
| `jogwheel_ring` | Strip (32) | 32-segment circular progress/position ring around jogwheel |

```rust
// Illuminating a position cursor on the jogwheel ring:
let mut ring = vec![0u8; 32];
let cursor_pos = 14; // 0..31 around the dial
ring[cursor_pos] = 255;
encdr.set_led_strip(device_id, "jogwheel_ring", &ring);
```

#### Traktor Kontrol S4 Mk3 Jog Wheel Rings (Report `0x32`)
The S4 Mk3 features dual motorized/haptic jog wheels with multi-mode 32-segment circular LED rings. Encdr provides dedicated high-level methods:

```rust
use encdr::{JogDeck, JogRing, JogRingMode, LedValue};

// 1. Position needle mode (0..2879 ticks per revolution)
encdr.set_jog_ring_needle(device_id, JogDeck::Left, 720, LedValue::Rgb { r: 0, g: 255, b: 255 });

// 2. Hardware flash / pulse / spot modes
encdr.set_jog_ring_mode(device_id, JogDeck::Right, JogRingMode::RingFlash, 0, LedValue::Rgb { r: 255, g: 0, b: 0 });

// 3. Addressable 32-segment ring animation / spinner
let spinner = JogRing::spinner(16, 8, 0x7F);
encdr.set_jog_ring(device_id, JogDeck::Left, &spinner);

// 4. Raw strip array
let leds = [0x7Fu8; 32];
encdr.set_jog_ring_leds(device_id, JogDeck::Right, &leds);

// 5. Automatic live sync with jog wheel motion (manual turning or motorized spin)
while let Ok(event) = events.try_recv() {
    encdr.sync_jog_ring_from_event(&event, LedValue::Rgb { r: 0, g: 220, b: 255 });
}
```

---


## Screen Output

### Raw Pixel Buffer (Tier 1)

For apps that render their own pixels:

```rust
use encdr::PixelFormat;

// RGBA8888 input - Encdr handles GPU conversion, dirty-region diffing, and USB transfer:
let rgba_pixels = vec![0u8; 480 * 272 * 4]; // black screen
encdr.submit_screen(device_id, "left", &rgba_pixels);

// Or submit in native format to skip GPU conversion:
// 1. Dual Color Screens (D2, S8, Mk3, Plus, Studio, KK Mk2: 480x272 BGR565-BE)
let bgr_pixels = vec![0u8; 480 * 272 * 2]; // 261,120 bytes
encdr.submit_screen_with_format(device_id, "left", &bgr_pixels, PixelFormat::Bgr565Be);
encdr.submit_screen_with_format(device_id, "right", &bgr_pixels, PixelFormat::Bgr565Be);

// 2. Monochrome 1-bit Screens (X1 Mk3: 5x 128x64 OLED, Maschine Mk2: 2x 256x64)
// Packed 1 bit per pixel (8 pixels per byte, MSB first)
let x1_oled_frame = vec![0xFFu8; (128 * 64) / 8]; // 1024 bytes (all white)
encdr.submit_screen_with_format(device_id, "left_fx", &x1_oled_frame, PixelFormat::Mono);
encdr.submit_screen_with_format(device_id, "center_mode", &x1_oled_frame, PixelFormat::Mono);
```

#### Screen Addressing by Controller
| Device | Screen Names | Resolution & Format |
| ------ | ------------ | ------------------- |
| **NI Kontrol D2** | `"main"` | $480 \times 272$, BGR565-BE |
| **NI Kontrol S8** | `"left"`, `"right"` | $480 \times 272$, BGR565-BE |
| **NI Kontrol S5** | `"left"`, `"right"` | $480 \times 272$, BGR565-BE |
| **NI Kontrol S4 Mk3** | `"left"`, `"right"` | $320 \times 240$, BGR565-BE |
| **NI Maschine Mk3 / Plus / Studio** | `"left"`, `"right"` | $480 \times 272$, BGR565-BE |
| **NI Komplete Kontrol S-Mk2** | `"left"`, `"right"` | $480 \times 272$, BGR565-BE |
| **NI Maschine Mk2** | `"left"`, `"right"` | $256 \times 64$, 1-bit Mono |
| **NI Maschine Mk1** | `"left"`, `"right"` | $255 \times 64$, 5-bit grayscale (ST7529) or 1-bit Mono |
| **NI Traktor Kontrol X1 Mk3** | `"left_fx"`, `"left_loop"`, `"center_mode"`, `"right_loop"`, `"right_fx"` | $128 \times 64$, 1-bit Mono |

The screen pipeline automatically:
1. Converts RGBA8 to the device's native pixel format (BGR565-BE or Mono) via GPU compute shader
2. Compares against the previous frame to find dirty regions
3. Sends partial blits if supported and changed region is $<50\%$ of the screen, or a full blit otherwise
4. Suppresses redundant USB transmission when frames are static (especially on monochrome displays)
5. Sends periodic keyframes (~every 60 frames) to prevent display drift

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

Devices are defined by JSON files. See the device references in [`docs/hardware/`](hardware/) (such as [D2](hardware/ni_kontrol_d2.md), [Mk3](hardware/ni_maschine_mk3.md), [Studio](hardware/ni_maschine_studio.md), [S8](hardware/ni_kontrol_s8.md), and [X1 Mk3](hardware/ni_kontrol_x1_mk3.md)) for complete annotated examples. The key sections:

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
| Fine encoder / Jogdial | `encoder_fine` | `Event::EncoderFine` | `bytes`, `encoding`, `scale`, `deadband` (`erp` only)             |

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
| `erp`         | Endless rotary potentiometer: two analog taps decoded to 0–999 per turn (Maschine Mk1) |

The `wrap16_wide` encoding detects direction via shortest path around the 65536-step ring. Use `encoder_fine` with `wrap16_wide` for jogdials:
```json
{ "type": "encoder_fine", "name": "jogwheel", "bytes": [5, 6], "encoding": "wrap16_wide", "scale": 1000.0 }
```

The `erp` encoding reads two analog wiper taps `bytes: [b, a]`, decodes them to an absolute position (0–999 per turn), and emits shortest-path deltas around that ring. With `scale: 1000.0`, deltas are fractions of a turn. `deadband` holds back movements smaller than that many units, absorbing analog jitter without losing motion:
```json
{ "type": "encoder_fine", "name": "volume", "bytes": [17, 18], "encoding": "erp", "scale": 1000.0, "deadband": 10 }
```

### Routing Packets by Report ID and Endpoint

By default, packets are matched to `input_packets` entries by their length. Devices that need more can opt in to these fields:

- **Multiple input endpoints.** Every interface named by an input packet gets its own IN endpoint read, interrupt or bulk. Several logical interfaces may share one USB interface `number`.
- **`alt_setting`** (on an interface): selected right after the interface is claimed.
- **`report_id`** (on a packet): routes by the packet's first byte instead of its length. Once any packet on an interface sets `report_id` or `pad_format`, that interface is routed this way, and unmatched packets are ignored.
- **`pad_format: "id_pressure_words"`**: decodes a header-less stream of little-endian words, with the pad index in bits 15–12 and 12-bit pressure below.
- **LED `prefix`**: a multi-byte header such as `["0x0c", "0x1e"]`, used instead of `prefix_byte`. Each LED group must set exactly one of the two.
- **Single LED `default`**: a value written on connect (for example, a display backlight).
- **`quirks.init_writes`**: raw writes sent once on connect, after reads are queued, e.g. `{ "interface": "control", "data": ["0x0b", "0x01"] }`.
- **Screen `protocol`**: controller-specific init and framing (e.g. `{ "type": "ni_st7529", "display": 0 }`). Its transfers are framed by the protocol, so `full_blit` keeps an empty header and footer.

Descriptors are checked when they load, so an LED group without exactly one kind of prefix, an init write without an OUT endpoint, or a byte value over `0xff` is reported as a `Descriptor` error naming the device.

The [Maschine Mk1 descriptor](hardware/ni_maschine_mk1.md) uses all of these.

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

# Traktor Kontrol X1 Mk3: Interactive 5-screen OLED test + RGB hotcues
cargo run -p encdr-examples --bin x1_mk3_test

# Traktor Kontrol S8: Dual 480x272 screen visualizer + deck controls
cargo run -p encdr-examples --bin s8_screen_test

# Kontrol D2: Knob/button positions on the screen
cargo run -p encdr-examples --bin d2_screen_test

# Maschine Mk3: Dual-screen encoder and button visualizer
cargo run -p encdr-examples --bin mk3_screen_test

# Maschine Mk3: Pad RGB animation loop
cargo run -p encdr-examples --bin mk3_pad_rainbow

# Maschine Mk3: High-resolution Smart Strip touch monitor
cargo run -p encdr-examples --bin touchstrip_monitor
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
