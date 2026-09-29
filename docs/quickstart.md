# Quick Start Tutorial: Native Instruments Maschine Mk3

This tutorial walks through building a complete, high-performance controller application with the **Native Instruments Maschine Mk3** using `encdr` and `encdr-view`.

By the end of this guide, you will have a working application that:
1. Connects to the Maschine Mk3 over USB (claiming dual interfaces).
2. Listens to button presses, continuous sliders, high-precision screen encoders, and capacitive touch events.
3. Illuminates button LEDs and 16 RGB velocity pads.
4. Renders interactive HTML/CSS/Canvas interfaces onto the dual 480×272 color LCD screens in real-time.

---

## 1. Prerequisites and Installation

### Cargo Dependencies

Add `encdr` and `encdr-view` to your project's `Cargo.toml`:

```toml
[dependencies]
encdr = "0.4.0"
encdr-view = "0.4.0"
crossbeam-channel = "0.5"
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
```

> **Note**: If working inside the Encdr workspace repository, you can reference path dependencies:
> ```toml
> encdr = { path = "encdr" }
> encdr-view = { path = "encdr-view" }
> ```

### Platform Setup

#### Linux
Encdr uses `nusb` for asynchronous USB access and `webkit2gtk` for offscreen WebView rendering.

1. Install system libraries:
   ```bash
   sudo apt install libudev-dev libwebkit2gtk-4.1-dev libgtk-3-dev
   ```

2. Add a `udev` rule to grant non-root access to Native Instruments USB devices:
   ```bash
   echo 'SUBSYSTEM=="usb", ATTR{idVendor}=="17cc", MODE="0666"' | sudo tee /etc/udev/rules.d/99-ni-controllers.rules
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```

#### macOS
No additional system packages are required. `encdr-view` uses macOS's built-in `WKWebView` via `tao` and `wry`.

#### Windows
No additional system packages are required. `encdr-view` uses the Microsoft Edge WebView2 runtime (pre-installed on Windows 10/11).

---

## 2. Hardware Architecture & USB Quirks

The Maschine Mk3 (`17cc:1600`) has a dual-interface architecture:
- **Interface 4**: Control interface.
  - Endpoint `0x84` (Interrupt IN): Delivers raw 42-byte packets containing button matrix state, encoder deltas, capacitive touch flags, and Smart Strip touch data.
  - Endpoint `0x03` (Interrupt OUT): Accepts LED output reports (`0x80` for button backlights, `0x81` for 16 RGB pads and Smart Strip).
- **Interface 5**: Screen interface.
  - Endpoint `0x04` (Bulk OUT): High-throughput bulk transfers sending 480×272 BGR565-BE frames to both left and right displays.

Encdr handles these complexities automatically using descriptor quirks:
- `dual_handle: true`: Claims both USB interfaces from the single physical device concurrently.
- `detach_kernel_driver: true`: Automatically unbinds the host OS HID driver on Linux so the control interface can be claimed cleanly.

---

## 3. Initializing Encdr and Scanning Devices

Create an [`Encdr`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/lib.rs#L36-L42) instance and scan for connected hardware:

```rust
use encdr::{Encdr, EncdrConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize Encdr with default configuration (loads all built-in descriptors)
    let mut encdr = Encdr::new(EncdrConfig::default())?;

    println!("Scanning for Native Instruments controllers...");
    let device_ids = encdr.scan()?;

    if device_ids.is_empty() {
        eprintln!("No devices found. Ensure the Maschine Mk3 is connected and powered on.");
        return Ok(());
    }

    let device_id = device_ids[0];
    let descriptor = encdr.device_descriptor(device_id).unwrap();
    println!("Connected to {} ({} controls)", descriptor.name, descriptor.control_count());

    Ok(())
}
```

---

## 4. Handling Input Events

Events from the controller arrive over a non-blocking [`crossbeam_channel::Receiver<Event>`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/core/event.rs#L29-L69).

Clone the event receiver and poll it inside your application loop:

```rust
use encdr::Event;

let events = encdr.events().clone();

while let Ok(event) = events.try_recv() {
    match event {
        Event::Button { name, pressed, .. } => {
            println!("Button '{}' state: {}", name, if pressed { "PRESSED" } else { "RELEASED" });
        }
        Event::EncoderFine { name, delta, .. } => {
            // Screen encoders (screen_encoder_1 .. 8) provide scaled fine deltas
            println!("Encoder '{}' moved: {:+.4}", name, delta);
        }
        Event::Slider { name, value, .. } => {
            // Smart Strip delivers normalized f32 positions (0.0 to 1.0)
            println!("Slider '{}' position: {:.3}", name, value);
        }
        Event::Touch { name, touched, .. } => {
            // Capacitive touch sensors on encoder knobs and touchstrip
            println!("Touch sensor '{}': {}", name, if touched { "TOUCH" } else { "RELEASE" });
        }
        Event::Encoder { name, delta, .. } => {
            // 4-way push encoder navigation steps
            println!("4-D Encoder '{}' step: {:+}", name, delta);
        }
        _ => {}
    }
}
```

### Key Control Names for Maschine Mk3

| Control Type | Names in Descriptor | Description |
| ------------ | ------------------- | ----------- |
| **Pads** | `pad_1` – `pad_16` | 16 velocity-sensitive pads |
| **Screen Buttons** | `top_1` – `top_8` | Buttons above the left and right displays |
| **Screen Encoders** | `screen_encoder_1` – `screen_encoder_8` | Endless rotary dials beneath displays |
| **Screen Encoder Touches** | `screen_encoder_touch_1` – `screen_encoder_touch_8` | Capacitive touch top on knobs |
| **Groups** | `group_a` – `group_h` | Group selector buttons |
| **Transport** | `play`, `rec`, `stop`, `restart`, `erase`, `tap`, `follow` | Transport control cluster |
| **Navigation** | `encoder_up`, `encoder_down`, `encoder_left`, `encoder_right` | 4-directional joystick clicks |
| **Touchstrip** | `touchstrip` | Dual-mode touch slider |

---

## 5. Controlling LEDs and RGB Pads

The Maschine Mk3 features two LED output reports:
1. **Report `0x80` (`buttons`)**: Single-color brightness backlights (0–255).
2. **Report `0x81` (`pad_leds`)**: 16 RGB pads and Smart Strip position LEDs.

Encdr maps names directly to the correct report buffer:

```rust
use encdr::LedValue;

// 1. Single-color button backlight (0 = Off, 255 = Maximum brightness)
encdr.set_led(device_id, "play", LedValue::Single(255));
encdr.set_led(device_id, "rec", LedValue::Single(128));

// Turn off an LED
encdr.set_led(device_id, "play", LedValue::Off);

// 2. Full RGB color on performance pads
// Encdr automatically maps RGB values to the native NI hardware palette!
encdr.set_led(device_id, "pad_1", LedValue::Rgb { r: 255, g: 0, b: 0 });   // Red
encdr.set_led(device_id, "pad_2", LedValue::Rgb { r: 0, g: 255, b: 0 });   // Green
encdr.set_led(device_id, "pad_3", LedValue::Rgb { r: 0, g: 128, b: 255 }); // Sky Blue
encdr.set_led(device_id, "pad_4", LedValue::Rgb { r: 255, g: 255, b: 0 }); // Yellow

// 3. Smart Strip LED array (25-segment brightness array)
let mut strip = [0u8; 25];
strip[12] = 255; // Center LED lit
encdr.set_led_strip(device_id, "touchstrip", &strip);
```

---

## 6. Dual Display Output with `encdr-view`

The Maschine Mk3 has two 480×272 color screens named `"left"` and `"right"`.

Using `encdr-view`, you can design rich graphical displays with HTML, CSS, SVG, and HTML5 Canvas, and push state directly from Rust:

### HTML Template (`screens/mk3_screen.html`)

```html
<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  body {
    margin: 0;
    width: 480px;
    height: 272px;
    background: #121214;
    color: #e0e0e0;
    font-family: sans-serif;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    box-sizing: border-box;
    padding: 16px;
    overflow: hidden;
  }
  .header {
    font-size: 20px;
    font-weight: bold;
    color: #00d2ff;
    border-bottom: 2px solid #222;
    padding-bottom: 8px;
  }
  .encoders {
    display: flex;
    justify-content: space-around;
  }
  .knob {
    text-align: center;
  }
  .knob-val {
    font-size: 24px;
    font-weight: bold;
    color: #ffaa00;
  }
</style>
</head>
<body>
  <div class="header" id="title">Deck A - Track 1</div>
  <div class="encoders">
    <div class="knob"><div id="v1" class="knob-val">50%</div><div>Cutoff</div></div>
    <div class="knob"><div id="v2" class="knob-val">20%</div><div>Reso</div></div>
    <div class="knob"><div id="v3" class="knob-val">80%</div><div>Drive</div></div>
    <div class="knob"><div id="v4" class="knob-val">0%</div><div>Reverb</div></div>
  </div>

  <script>
    // Injected Encdr bridge
    window.encdr = {
      onMessage(channel, data) {
        if (channel === 'param') {
          document.getElementById('v' + data.index).textContent = Math.round(data.value * 100) + '%';
        }
      }
    };
  </script>
</body>
</html>
```

### Initializing Screen Views in Rust

```rust
use encdr_view::{ScreenContent, ScreenView};

// Create offscreen WebViews for left and right displays
// Passing `false` creates headless views; pass `true` to show debug desktop windows
let left_html = std::fs::read_to_string("screens/mk3_left.html")?;
let right_html = std::fs::read_to_string("screens/mk3_right.html")?;

let left_view = ScreenView::new(
    &encdr,
    device_id,
    "left",
    ScreenContent::Html(left_html),
    false,
).expect("Failed to initialize left screen");

let right_view = ScreenView::new(
    &encdr,
    device_id,
    "right",
    ScreenContent::Html(right_html),
    false,
).expect("Failed to initialize right screen");
```

---

## 7. Complete Runnable Application

Here is a full, production-ready example demonstrating events, LED toggling, and real-time dual-screen rendering.

```rust
use std::collections::HashMap;
use std::time::Duration;

use encdr::{Encdr, EncdrConfig, Event, LedValue};
use encdr_view::{ScreenContent, ScreenView};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize Encdr
    let mut encdr = Encdr::new(EncdrConfig::default())?;

    println!("Scanning for Maschine Mk3...");
    let devices = encdr.scan()?;
    if devices.is_empty() {
        eprintln!("Maschine Mk3 not found!");
        return Ok(());
    }
    let dev = devices[0];

    // 2. Initialize dual WebViews (embed HTML or read from disk)
    let left_html = include_str!("../screens/mk3_left.html");
    let right_html = include_str!("../screens/mk3_right.html");

    let left_view = ScreenView::new(
        &encdr, dev, "left",
        ScreenContent::Html(left_html.to_string()), false,
    ).map_err(|e| format!("Left screen error: {}", e))?;

    let right_view = ScreenView::new(
        &encdr, dev, "right",
        ScreenContent::Html(right_html.to_string()), false,
    ).map_err(|e| format!("Right screen error: {}", e))?;

    // 3. Local control state
    let events = encdr.events().clone();
    let mut knob_values: HashMap<String, f64> = HashMap::new();
    for i in 1..=8 {
        knob_values.insert(format!("screen_encoder_{}", i), 0.5);
    }

    let mut dirty = true;

    println!("Maschine Mk3 active. Press buttons, turn knobs, or hit Ctrl+C to exit.");

    // 4. Main Event Loop
    loop {
        // Platform event pump (GTK on Linux, tao on macOS/Windows)
        ScreenView::pump_events();

        // Process incoming controller events
        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    if pressed {
                        // Light up the button while pressed
                        encdr.set_led(dev, name, LedValue::Single(255));
                    } else {
                        encdr.set_led(dev, name, LedValue::Off);
                    }

                    // Forward event text to screens
                    let msg = serde_json::json!({ "btn": name, "pressed": pressed });
                    left_view.send("event", msg.clone());
                    right_view.send("event", msg);
                    dirty = true;
                }
                Event::EncoderFine { name, delta, .. } => {
                    if let Some(val) = knob_values.get_mut(name) {
                        *val = (*val + delta as f64).clamp(0.0, 1.0);
                        let msg = serde_json::json!({ "encoder": name, "value": *val });
                        left_view.send("encoder", msg.clone());
                        right_view.send("encoder", msg);
                        dirty = true;
                    }
                }
                _ => {}
            }
        }

        // If UI state changed, poll and submit captured frames
        if dirty {
            left_view.poll(&encdr);
            right_view.poll(&encdr);
            dirty = false;
        }

        std::thread::sleep(Duration::from_millis(8));
    }
}
```

---

## 8. Running the Built-in Workspace Example

The Encdr repository includes a full interactive demo for the Maschine Mk3 with custom UI templates in the [`screens/`](file:///home/rufus/Documents/Projects/Encdr/screens) directory:

```bash
# Run with hardware LCD screens:
cargo run -p encdr-examples --bin mk3_screen_test

# Run with desktop debug preview windows:
cargo run -p encdr-examples --bin mk3_screen_test -- --visible

# Run the 16-pad RGB rainbow cycle test:
cargo run -p encdr-examples --bin mk3_pad_rainbow

# Run the high-resolution touchstrip visualizer:
cargo run -p encdr-examples --bin touchstrip_monitor
```

---

## Next Steps

- Consult the complete [API Reference](api_reference.md) for detailed documentation of every type, method, and error variant.
- Review the [Maschine Mk3 Hardware Reference](hardware/ni_maschine_mk3.md) for full endpoint, report ID, and byte-offset specifications.
- Explore the [Usage Guide](usage.md) for zero-copy raw pixel rendering, multi-device setups, and GPU context sharing.
