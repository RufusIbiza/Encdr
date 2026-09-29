# Encdr Programmer Reference & API Documentation

This document provides a comprehensive API reference for [`encdr`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/lib.rs) and [`encdr-view`](file:///home/rufus/Documents/Projects/Encdr/encdr-view/src/lib.rs). It covers every public struct, enum, trait, and function, accompanied by signatures, field breakdowns, error states, and working code examples.

---

## Table of Contents

1. [Core Facade: `Encdr`](#1-core-facade-encdr)
2. [Configuration: `EncdrConfig`](#2-configuration-encdrconfig)
3. [Device Identifiers: `DeviceId`](#3-device-identifiers-deviceid)
4. [Hardware Events: `Event`](#4-hardware-events-event)
5. [LED Values & NI Palette: `LedValue`](#5-led-values--ni-palette-ledvalue)
6. [Pixel Formats: `PixelFormat`](#6-pixel-formats-pixelformat)
7. [GPU Acceleration: `GpuContext`](#7-gpu-acceleration-gpucontext)
8. [Custom Packet Decoders: `PacketHook`](#8-custom-packet-decoders-packethook)
9. [Error Handling: `EncdrError`](#9-error-handling-encdrerror)
10. [Device Descriptors & Runtime Introspection](#10-device-descriptors--runtime-introspection)
11. [WebView Offscreen Renderer: `encdr-view`](#11-webview-offscreen-renderer-encdr-view)

---

## 1. Core Facade: `Encdr`

Defined in [`encdr::Encdr`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/lib.rs#L36-L42).

`Encdr` is the top-level facade managing USB discovery, descriptor registries, background I/O threads, and event dispatch channels.

### Methods

#### `pub fn new(config: EncdrConfig) -> Result<Self>`
Initializes the runtime. Registers built-in hardware descriptors (unless disabled via config) and any directories specified in `config.descriptor_dirs`.

```rust
use encdr::{Encdr, EncdrConfig};

let mut encdr = Encdr::new(EncdrConfig::default())?;
```

#### `pub fn scan(&mut self) -> Result<Vec<DeviceId>>`
Enumerates active USB devices using `nusb`. Matches connected devices against loaded descriptors by `vendor_id` and `product_id`. For each matching controller:
1. Detaches host OS kernel drivers if required by descriptor quirks.
2. Claims control and screen interfaces.
3. Spawns dedicated background worker threads for input polling and output transfers.
4. Emits an `Event::DeviceConnected` event.

Returns a vector of newly connected [`DeviceId`](#3-device-identifiers-deviceid)s.

```rust
let new_devices = encdr.scan()?;
println!("Discovered {} new controllers", new_devices.len());
```

#### `pub fn events(&self) -> &crossbeam_channel::Receiver<Event>`
Returns a reference to the lock-free SPSC/MPSC crossbeam channel receiver. All input events from all connected hardware devices are delivered here.

```rust
let events = encdr.events().clone();
while let Ok(event) = events.try_recv() {
    // Process event
}
```

#### `pub fn set_led(&self, device_id: DeviceId, name: &str, value: LedValue)`
Updates an LED state by control name. Matches against the first LED group containing a control with that name.

- `device_id`: Target device.
- `name`: Identifier as declared in the device descriptor JSON (e.g. `"play"`, `"pad_1"`).
- `value`: State to assign ([`LedValue::Off`](#5-led-values--ni-palette-ledvalue), [`LedValue::Single`](#5-led-values--ni-palette-ledvalue), or [`LedValue::Rgb`](#5-led-values--ni-palette-ledvalue)).

```rust
use encdr::LedValue;

// Single-color LED
encdr.set_led(device_id, "play", LedValue::Single(255));

// RGB LED (automatically mapped to hardware palette or direct RGB offsets)
encdr.set_led(device_id, "pad_1", LedValue::Rgb { r: 255, g: 0, b: 0 });
```

#### `pub fn set_led_in_group(&self, device_id: DeviceId, group: &str, name: &str, value: LedValue)`
Targeted LED update specifying both the report/buffer group and control name. Essential for controllers with overlapping names across separate USB output reports (such as Maschine Studio or Traktor Kontrol S4 Mk3).

```rust
encdr.set_led_in_group(device_id, "buttons", "play", LedValue::Single(255));
encdr.set_led_in_group(device_id, "pad_leds", "group_a", LedValue::Rgb { r: 0, g: 255, b: 0 });
```

#### `pub fn set_led_strip(&self, device_id: DeviceId, name: &str, values: &[u8])`
Updates an array of single-color LEDs (e.g. touchstrips, level meters, or jogwheel LED rings).

- `name`: Name of the strip control (e.g. `"touchstrip_blue"`, `"jogwheel_ring"`).
- `values`: Slice of brightness bytes (0–255) corresponding to each segment.

```rust
let ring_pattern = [255u8; 32];
encdr.set_led_strip(device_id, "jogwheel_ring", &ring_pattern);
```

#### `pub fn set_led_strip_in_group(&self, device_id: DeviceId, group: &str, name: &str, values: &[u8])`
Sets a multi-segment strip within an explicit buffer group.

```rust
let left_meter = [128u8; 16];
encdr.set_led_strip_in_group(device_id, "master_meters", "meter_left", &left_meter);
```

#### `pub fn submit_screen(&self, device_id: DeviceId, screen: &str, pixels: &[u8])`
Submits a raw frame of `RGBA8888` pixel bytes to a specified display. The frame is fed through the GPU compute pipeline, converted to the display's native format (e.g., BGR565 or 1-bit Mono), dirty-rect diffed against the previous frame, and transferred via USB.

```rust
let rgba = vec![0u8; 480 * 272 * 4]; // Black frame
encdr.submit_screen(device_id, "left", &rgba);
```

#### `pub fn submit_screen_with_format(&self, device_id: DeviceId, screen: &str, pixels: &[u8], format: PixelFormat)`
Submits a raw frame with an explicit [`PixelFormat`](#6-pixel-formats-pixelformat). Supplying the display's native format directly bypasses GPU format conversion.

```rust
use encdr::PixelFormat;

let bgr_pixels = vec![0u8; 480 * 272 * 2]; // Native BGR565-BE
encdr.submit_screen_with_format(device_id, "left", &bgr_pixels, PixelFormat::Bgr565Be);
```

#### `pub fn load_descriptor_dir(&mut self, path: impl AsRef<Path>) -> Result<()>`
Loads all JSON device descriptors located in a directory.

```rust
encdr.load_descriptor_dir("./custom_hardware/")?;
```

#### `pub fn load_descriptor_json(&mut self, json: &str) -> Result<Arc<DeviceDescriptor>>`
Parses and registers an individual device descriptor from a JSON string. Returns a reference-counted handle to the loaded descriptor.

```rust
let desc = encdr.load_descriptor_json(include_str!("my_controller.json"))?;
```

#### `pub fn device_descriptor(&self, device_id: DeviceId) -> Option<&Arc<DeviceDescriptor>>`
Returns the cached [`DeviceDescriptor`](#10-device-descriptors--runtime-introspection) of an active device.

```rust
if let Some(desc) = encdr.device_descriptor(device_id) {
    println!("Device: {} ({} screens)", desc.name, desc.screens.len());
}
```

#### `pub fn connected_devices(&self) -> Vec<DeviceId>`
Returns a list of all currently active [`DeviceId`](#3-device-identifiers-deviceid) instances.

#### `pub fn loaded_descriptors(&self) -> Vec<Arc<DeviceDescriptor>>`
Returns all loaded descriptors in the registry.

#### `pub fn disconnect(&mut self, device_id: DeviceId)`
Gracefully terminates background I/O threads, clears all LEDs on the device, releases claimed USB interfaces, and removes the device from the active device list.

#### `pub fn shutdown(&mut self)`
Disconnects all active devices and cleanly terminates worker threads. Also called automatically on `Drop`.

---

## 2. Configuration: `EncdrConfig`

Defined in [`encdr::EncdrConfig`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/lib.rs#L24-L32).

```rust
pub struct EncdrConfig {
    pub gpu: Option<GpuContext>,
    pub descriptor_dirs: Vec<String>,
    pub skip_builtins: bool,
}
```

### Fields

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `gpu` | `Option<GpuContext>` | `None` | Shared `wgpu` device and queue. If `None`, Encdr creates its own high-performance GPU context. |
| `descriptor_dirs` | `Vec<String>` | `vec![]` | Additional filesystem directories scanned for `.json` device descriptors at startup. |
| `skip_builtins` | `bool` | `false` | When `true`, built-in descriptors (D2, S8, S5, S4, S2, Mk3, etc.) are omitted from the registry. |

---

## 3. Device Identifiers: `DeviceId`

Defined in [`encdr::core::event::DeviceId`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/core/event.rs#L7-L24).

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct DeviceId(u64);
```

An opaque 64-bit identifier encoding the USB bus, device address, vendor ID, and product ID.
- Implements `Display`: Formats as `dev:0001000217cc1600`.
- Implements `Hash`, `Eq`, and `Copy` for use in hash maps and collections.

---

## 4. Hardware Events: `Event`

Defined in [`encdr::core::event::Event`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/core/event.rs#L29-L69).

Every hardware input event carries the source `DeviceId` and an interned `&'static str` identifier corresponding to the JSON descriptor. Because names are interned when descriptors load, no string allocations occur during event polling.

```rust
#[derive(Debug, Clone)]
pub enum Event {
    DeviceConnected {
        id: DeviceId,
        descriptor: Arc<DeviceDescriptor>,
    },
    DeviceDisconnected {
        id: DeviceId,
    },
    Button {
        device: DeviceId,
        name: &'static str,
        pressed: bool,
    },
    Slider {
        device: DeviceId,
        name: &'static str,
        value: f32,
    },
    Encoder {
        device: DeviceId,
        name: &'static str,
        delta: i32,
    },
    EncoderFine {
        device: DeviceId,
        name: &'static str,
        delta: f32,
    },
    Touch {
        device: DeviceId,
        name: &'static str,
        touched: bool,
    },
    Grid {
        device: DeviceId,
        name: &'static str,
        index: u8,
        pressure: f32,
    },
}
```

### Event Variants

- `DeviceConnected`: Fired when a supported USB device is plugged in or detected by `scan()`.
- `DeviceDisconnected`: Fired when a device is unplugged or disconnected via `disconnect()`.
- `Button`: Triggered by mechanical buttons or momentary switches. `pressed` is `true` on keydown, `false` on release.
- `Slider`: Continuous potentiometer, linear fader, touchstrip position, or analog knob. `value` is normalized between `0.0` and `1.0`.
- `Encoder`: Stepped rotary encoder. `delta` is signed (typically `+1` clockwise, `-1` counter-clockwise).
- `EncoderFine`: High-resolution endless dial, capacitive knob rotation, or jogwheel. `delta` is a floating-point delta scaled by the descriptor's `scale` factor.
- `Touch`: Capacitive touch sensor (e.g. knob tops, touchstrip surfaces). `touched` is `true` while contact is maintained.
- `Grid`: Drum pad or velocity grid event. `index` is pad index (0-based) and `pressure` is normalized pressure (`0.0` to `1.0`).

### Helper Methods

```rust
impl Event {
    /// Returns the DeviceId associated with this event.
    pub fn device_id(&self) -> DeviceId;
}
```

---

## 5. LED Values & NI Palette: `LedValue`

Defined in [`encdr::core::led::LedValue`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/core/led.rs#L2-L86).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedValue {
    Off,
    Single(u8),
    Rgb { r: u8, g: u8, b: u8 },
}
```

### Methods

#### `pub fn brightness(&self) -> u8`
Returns the effective brightness (0–255) of the value. For RGB values, returns $\max(r, g, b)$.

#### `pub fn to_ni_palette_byte(r: u8, g: u8, b: u8) -> u8`
Maps an 8-bit RGB color to the Native Instruments packed 1-byte hardware palette format used by Maschine Mk3, Maschine Mikro Mk3, and Komplete Kontrol Mk2:
- `0x00`: Off
- Bits 7..2: Palette color index (1..17: Red, Orange, Amber, Yellow, Green, Mint, Cyan, Blue, Indigo, Violet, Magenta, Crimson, White, etc.)
- Bits 1..0: 2-bit intensity level (0..3)

```rust
let ni_byte = LedValue::to_ni_palette_byte(255, 0, 0); // Red at full brightness -> 0x07
```

---

## 6. Pixel Formats: `PixelFormat`

Defined in [`encdr::core::descriptor::PixelFormat`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/core/descriptor.rs#L318-L335).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    Bgr565Be,
    Rgb565Le,
    Rgb888,
    Rgba8888,
    Mono,
}
```

### Methods

#### `pub fn bytes_per_pixel(&self) -> usize`
Returns byte density per pixel (`2` for BGR565/RGB565, `3` for RGB888, `4` for RGBA8888, and `1` for Mono).

---

## 7. GPU Acceleration: `GpuContext`

Defined in [`encdr::screen::gpu::GpuContext`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/screen/gpu.rs#L5-L36).

```rust
pub struct GpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
}
```

### Constructors

#### `pub async fn new() -> Option<Self>`
Creates a standalone GPU context requesting a high-performance adapter (`Vulkan`, `Metal`, or `DX12`).

```rust
let gpu = GpuContext::new().await.expect("Failed to initialize GPU");
```

#### `pub fn from_existing(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self`
Wraps existing `wgpu` device and queue instances from a host application to eliminate redundant context creation and memory overhead.

---

## 8. Custom Packet Decoders: `PacketHook`

Defined in [`encdr::device::hooks::PacketHook`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/device/hooks.rs#L6-L16).

```rust
pub trait PacketHook: Send + Sync + 'static {
    /// Intercepts raw USB input packets before standard generic JSON parsing.
    /// Return `true` to consume the packet (skipping default parsing),
    /// or `false` to let the generic parser process it.
    fn on_packet(
        &mut self,
        device_id: DeviceId,
        packet: &[u8],
        events: &mut Vec<Event>,
    ) -> bool;
}
```

---

## 9. Error Handling: `EncdrError`

Defined in [`encdr::core::error::EncdrError`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/core/error.rs#L4-L34).

```rust
#[derive(thiserror::Error, Debug)]
pub enum EncdrError {
    #[error("USB error: {0}")]
    Usb(#[from] nusb::Error),

    #[error("USB transfer error: {0}")]
    Transfer(#[from] nusb::transfer::TransferError),

    #[error("Descriptor error: {0}")]
    Descriptor(String),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(std::io::Error),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Device disconnected")]
    Disconnected,

    #[error("Screen not found: {0}")]
    ScreenNotFound(String),

    #[error("LED not found: {0}")]
    LedNotFound(String),

    #[error("GPU error: {0}")]
    Gpu(String),
}

pub type Result<T> = std::result::Result<T, EncdrError>;
```

---

## 10. Device Descriptors & Runtime Introspection

Defined in [`encdr::core::descriptor`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/core/descriptor.rs).

Descriptors provide a complete declarative specification of a hardware controller.

### `DeviceDescriptor`
```rust
pub struct DeviceDescriptor {
    pub name: String,
    pub manufacturer: String,
    pub vendor_id: HexU16,
    pub product_id: HexU16,
    pub interfaces: Vec<InterfaceDesc>,
    pub input_packets: Vec<InputPacketDesc>,
    pub leds: Vec<LedLayoutDesc>,
    pub screens: Vec<ScreenDesc>,
    pub quirks: QuirksDesc,
}
```

#### Introspection Methods
- `pub fn control_count(&self) -> usize`: Returns the count of all mapped inputs across all input packets.
- `pub fn all_inputs(&self) -> impl Iterator<Item = &InputItemDesc>`: Enumerates all controls (buttons, sliders, encoders).
- `pub fn interface_by_id(&self, id: &str) -> Option<&InterfaceDesc>`: Finds an interface descriptor by string name (e.g. `"control"`, `"screen"`).

### `ScreenDesc`
```rust
pub struct ScreenDesc {
    pub name: String,
    pub interface: String,
    pub width: u16,
    pub height: u16,
    pub pixel_format: PixelFormat,
    pub full_blit: ScreenBlitDesc,
    pub partial_blit: Option<PartialBlitDesc>,
}
```
- `pixel_count(&self) -> usize`: Width $\times$ Height.
- `byte_size(&self) -> usize`: Total frame byte buffer size.

---

## 11. WebView Offscreen Renderer: `encdr-view`

Defined in [`encdr-view::ScreenView`](file:///home/rufus/Documents/Projects/Encdr/encdr-view/src/lib.rs#L61-L70).

`encdr-view` renders modern web content (HTML, CSS, SVG, Canvas, WebGL) offscreen in a hardware-sized WebView and pipes pixel captures directly into Encdr's screen pipeline.

### Loading Content: `ScreenContent`
```rust
pub enum ScreenContent {
    /// Inline HTML string content.
    Html(String),
    /// Path to an HTML file on disk.
    File(String),
}
```

### `ScreenView` Methods

#### `pub fn new(encdr: &Encdr, device_id: DeviceId, screen_name: &str, content: ScreenContent, visible: bool) -> Result<Self, String>`
Creates a new offscreen WebView renderer sized precisely to the target screen's dimensions.
- `device_id`: Connected device handle.
- `screen_name`: Name of display as defined in descriptor (e.g. `"left"`, `"right"`, `"main"`).
- `content`: HTML source code or file path.
- `visible`: If `true`, opens a desktop window preview (useful for UI debugging).

```rust
use encdr_view::{ScreenContent, ScreenView};

let view = ScreenView::new(
    &encdr,
    device_id,
    "left",
    ScreenContent::File("./screens/deck.html".to_string()),
    false, // headless offscreen
)?;
```

#### `pub fn send(&self, channel: &str, data: serde_json::Value)`
Dispatches a JSON message to JavaScript running inside the WebView. In the page, this triggers the `window.encdr.onMessage(channel, data)` callback.

```rust
view.send("track_info", serde_json::json!({
    "title": "Strobe",
    "bpm": 128.0,
    "elapsed": 142.5
}));
```

#### `pub fn pump_events() -> bool`
Drives platform windowing and event loops. **Must be called periodically in the application loop**:
- **Linux**: Pumps GTK iteration (`gtk::main_iteration_do(false)`).
- **macOS / Windows**: Pumps `tao` event queue.

```rust
// In your application loop:
ScreenView::pump_events();
```

#### `pub fn is_frame_ready(&self) -> bool`
Returns `true` if the WebView's DOM/CSS compositor has rendered new pixels since the last capture.

#### `pub fn capture_and_submit(&self, encdr: &Encdr) -> Result<(), String>`
Immediately grabs the composited surface using platform-native capture mechanisms (Cairo snapshot on Linux, `takeSnapshot` on macOS, `CapturePreview` on Windows) and submits the RGBA buffer to Encdr.

```rust
view.capture_and_submit(&encdr)?;
```

#### `pub fn poll(&self, encdr: &Encdr)`
Convenience method: checks `is_frame_ready()`, and if true, captures and submits the new frame to Encdr.

```rust
view.poll(&encdr);
```

#### `pub fn load_html(&self, html: &str) -> Result<(), String>`
Replaces the currently loaded HTML page with new HTML markup dynamically.

#### `pub fn eval(&self, js: &str) -> Result<(), String>`
Executes an arbitrary JavaScript string within the WebView runtime.

#### `pub fn device_id(&self) -> DeviceId`
Returns the `DeviceId` associated with this view.

#### `pub fn screen_name(&self) -> &str`
Returns the target screen identifier string.
