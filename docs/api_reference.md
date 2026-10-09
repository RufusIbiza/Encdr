# Encdr Programmer Reference & API Documentation

This document provides a comprehensive API reference for [`encdr`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/lib.rs) and [`encdr-view`](file:///home/rufus/Documents/Projects/Encdr/encdr-view/src/lib.rs). It covers every public struct, enum, trait, and function, accompanied by signatures, field breakdowns, error states, and working code examples.

---

## Table of Contents

1. [Core Facade: `Encdr`](#1-core-facade-encdr)
2. [Configuration: `EncdrConfig`](#2-configuration-encdrconfig)
3. [Device Identifiers: `DeviceId`](#3-device-identifiers-deviceid)
4. [Hardware Events: `Event`](#4-hardware-events-event)
5. [LED Values & NI Palette: `LedValue`](#5-led-values--ni-palette-ledvalue)
6. [Seven-Segment Displays: `SevenSegment`](#6-seven-segment-displays-sevensegment)
7. [Jog Wheel LED Rings: `JogRing`, `JogDeck`, `JogRingMode`](#7-jog-wheel-led-rings-jogring-jogdeck-jogringmode)
8. [Pixel Formats: `PixelFormat`](#8-pixel-formats-pixelformat)
9. [GPU Acceleration: `GpuContext`](#9-gpu-acceleration-gpucontext)
10. [Custom Packet Decoders: `PacketHook`](#10-custom-packet-decoders-packethook)
11. [Error Handling: `EncdrError`](#11-error-handling-encdrerror)
12. [Device Descriptors & Runtime Introspection](#12-device-descriptors--runtime-introspection)
13. [WebView Offscreen Renderer: `encdr-view`](#13-webview-offscreen-renderer-encdr-view)
14. [Komplete Kontrol Mk3 DAW & On-Device Rendering (ODR)](#14-komplete-kontrol-mk3-daw--on-device-rendering-odr)

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
- `value`: State to assign ([`LedValue::Off`](#5-led-values--ni-palette-ledvalue), [`LedValue::Dim`](#5-led-values--ni-palette-ledvalue), [`LedValue::Bright`](#5-led-values--ni-palette-ledvalue), [`LedValue::Single`](#5-led-values--ni-palette-ledvalue), or [`LedValue::Rgb`](#5-led-values--ni-palette-ledvalue)).

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

#### `pub fn set_seven_segment(&self, device_id: DeviceId, name: &str, seg: SevenSegment)`
Updates a 7-segment display control by name, writing the raw segment bitmask (`bit 0 = a` .. `bit 6 = g`, `bit 7 = dp`) to the hardware.

```rust
use encdr::SevenSegment;

// Display digit '8' with decimal point
encdr.set_seven_segment(device_id, "left_loop_digit_1", SevenSegment::from_char('8').with_dot(true));
```

#### `pub fn set_seven_segment_in_group(&self, device_id: DeviceId, group: &str, name: &str, seg: SevenSegment)`
Sets a 7-segment display control within an explicit LED buffer group.

```rust
encdr.set_seven_segment_in_group(device_id, "loop_displays", "left_loop_digit_1", SevenSegment::from_char('4'));
```

#### `pub fn set_seven_segment_str(&self, device_id: DeviceId, digit_left: &str, digit_right: &str, text: &str)`
Sets a dual-digit 7-segment display from a text string. Decimal points attached to characters (e.g. `"1."` or `"1.6"`) are automatically merged onto the preceding digit's decimal point.

```rust
// Displays '1.' on left digit and '6' on right digit
encdr.set_seven_segment_str(device_id, "left_loop_digit_1", "left_loop_digit_2", "1.6");
```

#### `pub fn set_loop_display(&self, device_id: DeviceId, digit_left: &str, digit_right: &str, beats: f32, active: bool)`
Encodes standard DJ loop lengths (e.g., `32`, `16`, `8`, `4`, `2`, `1`, `0.5`, `0.25`, `0.125`) across two digits. If `active` is true, the decimal point on the right digit is illuminated.

```rust
// Displays '.2.' indicating an active 1/2 beat loop (or '.2' if inactive)
encdr.set_loop_display(device_id, "left_loop_digit_1", "left_loop_digit_2", 0.5, true);
```

```

#### `pub fn set_jog_ring_needle(&self, device_id: DeviceId, deck: JogDeck, position: u16, color: LedValue)`
Sets the motorized / haptic jog wheel LED ring on a Traktor Kontrol S4 Mk3 to single needle indicator mode.
- `deck`: [`JogDeck::Left`](#jogdeck) or [`JogDeck::Right`](#jogdeck)
- `position`: Needle tick offset (`0..2879`, where 2880 is a full revolution)
- `color`: [`LedValue::Rgb`](#ledvalue) (automatically converted to NI packed palette byte) or [`LedValue::Single`](#ledvalue)

```rust
use encdr::{JogDeck, LedValue};

encdr.set_jog_ring_needle(device_id, JogDeck::Left, 720, LedValue::Rgb { r: 0, g: 255, b: 255 });
```

#### `pub fn set_jog_ring_mode(&self, device_id: DeviceId, deck: JogDeck, mode: JogRingMode, position: u16, color: LedValue)`
Sets an S4 Mk3 jog wheel LED ring to an explicit hardware mode:
- `mode`: [`JogRingMode`](#jogringmode) (`Off`, `DimFlash`, `Needle`, `RingFlash`, `DimSpot`, or `Addressable`)

```rust
use encdr::{JogDeck, JogRingMode, LedValue};

encdr.set_jog_ring_mode(device_id, JogDeck::Right, JogRingMode::RingFlash, 0, LedValue::Rgb { r: 255, g: 0, b: 0 });
```

#### `pub fn set_jog_ring_leds(&self, device_id: DeviceId, deck: JogDeck, leds: &[u8])`
Sets the 32 individually addressable LEDs on an S4 Mk3 jog wheel ring (mode 5).

```rust
let ring = [0x7Fu8; 32];
encdr.set_jog_ring_leds(device_id, JogDeck::Right, &ring);
```

#### `pub fn set_jog_ring(&self, device_id: DeviceId, deck: JogDeck, ring: &JogRing)`
Convenience method setting an S4 Mk3 jog wheel ring from a [`JogRing`](#jogring) buffer helper.

```rust
use encdr::{JogDeck, JogRing};

let spinner = JogRing::spinner(16, 8, 0x7F);
encdr.set_jog_ring(device_id, JogDeck::Left, &spinner);
```

#### `pub fn set_jog_ring_off(&self, device_id: DeviceId, deck: JogDeck)`
Turns off the jog wheel ring for the specified deck.

```rust
encdr.set_jog_ring_off(device_id, JogDeck::Left);
```

#### `pub fn sync_jog_ring_from_event(&self, event: &Event, color: LedValue) -> Option<(JogDeck, u16)>`
Synchronizes the jog wheel ring needle spot directly with an incoming jog wheel event in real time.
- Handles both manual spin and motorized turntable platter rotation.
- Automatically matches `"left_jog_pos"` and `"right_jog_pos"` absolute position events.
- Returns `Some((deck, ticks))` if the event was an S4 Mk3 jog wheel event and updated the ring, or `None` otherwise.

```rust
// Inside event processing loop:
if let Some((deck, ticks)) = encdr.sync_jog_ring_from_event(&event, LedValue::Rgb { r: 0, g: 255, b: 255 }) {
    // LED needle is updated in 1:1 hardware sync with the physical wheel!
}
```

#### `pub fn sync_jog_ring_normalized(&self, device_id: DeviceId, deck: JogDeck, normalized: f32, color: LedValue)`
Synchronizes the jog wheel needle position from a normalized angle (`0.0 .. 1.0`, where 0.0 is 12 o'clock and 1.0 is 360°).

```rust
encdr.sync_jog_ring_normalized(device_id, JogDeck::Left, 0.25, LedValue::Rgb { r: 0, g: 255, b: 0 }); // 3 o'clock (90°)
```

#### `pub fn sync_jog_ring_radians(&self, device_id: DeviceId, deck: JogDeck, radians: f32, color: LedValue)`
Synchronizes the jog wheel needle position from an angle in radians (`0.0 .. 2*PI`).

```rust
encdr.sync_jog_ring_radians(device_id, JogDeck::Right, std::f32::consts::PI, LedValue::Rgb { r: 255, g: 0, b: 0 }); // 6 o'clock (180°)
```

#### `pub fn sync_jog_ring_position(&self, device_id: DeviceId, deck: JogDeck, ticks: u16, color: LedValue)`
Synchronizes the jog wheel needle spot directly with an absolute tick position (`0..2879`).

```rust
encdr.sync_jog_ring_position(device_id, JogDeck::Left, 1440, LedValue::Rgb { r: 255, g: 255, b: 0 });
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
#### `pub fn submit_dual_screen(&self, device_id: DeviceId, left_screen: &str, right_screen: &str, pixels: &[u8])`
Submits a single combined double-width frame of `RGBA8888` pixels (e.g. `960x272` for dual `480x272` screens or `640x240` for dual `320x240` screens).

Encdr splits the frame horizontally before feeding each half independently through GPU format conversion, frame diffing, and USB transfer. If one display's content is unchanged, zero USB packets are transmitted for that screen.

```rust
let dual_rgba = vec![0u8; 960 * 272 * 4]; // 960x272 double-width canvas
encdr.submit_dual_screen(device_id, "left", "right", &dual_rgba);
```

#### `pub fn submit_dual_screen_with_format(&self, device_id: DeviceId, left_screen: &str, right_screen: &str, pixels: &[u8], format: PixelFormat)`
Submits a single combined double-width frame with an explicit [`PixelFormat`](#6-pixel-formats-pixelformat).

```rust
use encdr::PixelFormat;

let dual_bgr = vec![0u8; 960 * 272 * 2];
encdr.submit_dual_screen_with_format(device_id, "left", "right", &dual_bgr, PixelFormat::Bgr565Be);
```

#### `pub fn write_interface(&self, device_id: DeviceId, interface: &str, data: &[u8])`
Transmits raw payload bytes directly to a named USB interface's OUT endpoint. Used for vendor-specific bulk pipes (such as Komplete Kontrol Mk3's `odr_cmd`).

```rust
encdr.write_interface(device_id, "odr_cmd", &msgpack_bytes);
```

#### `pub fn kk_mk3_set_plugin_data(&self, device_id: DeviceId, plugin_data: &PluginData) -> Result<()>`
Serializes and transmits an active instrument / effect parameter page model to a Komplete Kontrol S-Series Mk3 keyboard via ODR MsgPack-RPC over Bulk OUT `0x03`.

```rust
use encdr::{PluginData, RgbColor, ParameterItem};

let mut plugin = PluginData::new("Analog Synth", RgbColor::CYAN);
plugin.add_parameter(ParameterItem::knob("Cutoff", 0.75, "3.2 kHz", "Filter"));
encdr.kk_mk3_set_plugin_data(device_id, &plugin)?;
```

#### `pub fn kk_mk3_update_parameter_value(&self, device_id: DeviceId, param_index: u32, value: f32) -> Result<()>`
Updates a single parameter's continuous value (`0.0..1.0` or `-1.0..1.0`) on a Komplete Kontrol S-Series Mk3 keyboard. Eliminates overhead by bypassing full page serialization.

```rust
encdr.kk_mk3_update_parameter_value(device_id, 0, 0.82)?;
```

#### `pub fn kk_mk3_set_lightguide(&self, device_id: DeviceId, rgb_keys: &[(u8, u8, u8)]) -> Result<()>`
Sets the per-key 24-bit RGB Light Guide LEDs across the keyboard keybed via ODR. Accepts a slice of `(r, g, b)` tuples for each key (49, 61, or 88 keys).

```rust
let keys = vec![(0u8, 210u8, 255u8); 61]; // All keys cyan
encdr.kk_mk3_set_lightguide(device_id, &keys)?;
```

#### `pub fn kk_mk3_register_asset(&self, device_id: DeviceId, asset_id: &str, image_data: &[u8]) -> Result<()>`
Uploads and registers a PNG or JPEG graphic asset into the Komplete Kontrol Mk3 on-device high-speed cache.

```rust
let png_bytes = std::fs::read("banner.png")?;
encdr.kk_mk3_register_asset(device_id, "my_banner", &png_bytes)?;
```

#### `pub fn kk_mk3_set_header_image(&self, device_id: DeviceId, asset_id: &str, image_data: &[u8], plugin_data: &mut PluginData) -> Result<()>`
Convenience method: registers the image asset in the keyboard cache, sets `plugin_data.background = Some(asset_id)`, and dispatches the updated plugin page to the hardware.

```rust
encdr.kk_mk3_set_header_image(device_id, "my_banner", &png_bytes, &mut plugin)?;
```

#### `pub fn kk_mk3_set_plugin_chain(&self, device_id: DeviceId, chain: &PluginChainModel) -> Result<()>`
Uploads the serial insert plugin chain model to the Komplete Kontrol Mk3 keyboard display.

```rust
use encdr::{PluginChainModel, PluginChainItem, RgbColor};

let mut chain = PluginChainModel::new();
chain.add_plugin(PluginChainItem::new("Lead Synth", RgbColor::CYAN));
encdr.kk_mk3_set_plugin_chain(device_id, &chain)?;
```

#### `pub fn kk_mk3_set_plugin_chain_index(&self, device_id: DeviceId, index: u32) -> Result<()>`
Selects the focused plugin slot index within the active serial insert chain.

```rust
encdr.kk_mk3_set_plugin_chain_index(device_id, 1)?;
```

#### `pub fn kk_mk3_set_mixer_model(&self, device_id: DeviceId, mixer: &MixerModel) -> Result<()>`
Uploads the multi-track mixer state model (track labels, colors, mute, solo, arm, volumes, pans) to the Komplete Kontrol Mk3 display.

```rust
use encdr::{MixerModel, MixerTrack, RgbColor};

let mut mixer = MixerModel::new();
mixer.add_track(MixerTrack::new("Drums", RgbColor::ORANGE));
encdr.kk_mk3_set_mixer_model(device_id, &mixer)?;
```

#### `pub fn kk_mk3_set_mixer_meters(&self, device_id: DeviceId, left: &[f32], right: &[f32]) -> Result<()>`
Updates stereo VU level meters in the ODR mixer view.

```rust
encdr.kk_mk3_set_mixer_meters(device_id, &[0.8, 0.4], &[0.75, 0.45])?;
```

#### `pub fn kk_mk3_set_smartplay(&self, device_id: DeviceId, smartplay: &SmartPlayData) -> Result<()>`
Transmits Smart Play settings (scales, chords, arpeggiator engine) to the keyboard.

```rust
use encdr::SmartPlayData;

let mut sp = SmartPlayData::default();
sp.scale.enabled = true;
encdr.kk_mk3_set_smartplay(device_id, &sp)?;
```

#### `pub fn kk_mk3_set_browser_model(&self, device_id: DeviceId, browser: &BrowserModel) -> Result<()>`
Populates the on-device preset/sound browser columns, category tags, and sound list.

```rust
use encdr::{BrowserModel, BrowserFilter};

let mut browser = BrowserModel::default();
browser.filters.push(BrowserFilter::new("Type", vec!["Bass".into(), "Lead".into()]));
encdr.kk_mk3_set_browser_model(device_id, &browser)?;
```

#### `pub fn kk_mk3_set_device_settings(&self, device_id: DeviceId, settings: &DeviceSettings) -> Result<()>`
Updates hardware preferences: display backlight brightness (0..100), LED brightness (0..100), Light Guide enable, and velocity curve.

```rust
use encdr::DeviceSettings;

let mut settings = DeviceSettings::default();
settings.display_brightness = 90;
encdr.kk_mk3_set_device_settings(device_id, &settings)?;
```

#### `pub fn kk_mk3_set_page(&self, device_id: DeviceId, page: KkMk3Page) -> Result<()>`
Switches the active on-device screen template (`Parameters`, `Browser`, `Mixer`, `SmartPlay`, `PluginChain`, or `Settings`).

```rust
use encdr::KkMk3Page;

encdr.kk_mk3_set_page(device_id, KkMk3Page::Mixer)?;
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
Performs a graceful clean exit on the specified controller:
1. Submits the default screensaver image as the final frame across all color screens (or clears monochrome displays).
2. Clears all illuminated LEDs to off (both standard output groups and feature-report quirk LEDs).
3. Synchronously joins worker threads to guarantee all USB transfers are completed before releasing claimed interfaces.
4. Removes the device from the active device list.

#### `pub fn shutdown(&mut self)`
Disconnects all active devices and cleanly terminates worker threads using the clean exit procedure. Also invoked automatically when `Encdr` is dropped.

#### `pub fn clean_exit(&mut self)`
Explicit clean shutdown method. Submits the screensaver image to all screens, extinguishes all lit LEDs, and disconnects all controllers. Alias for `shutdown()`.

```rust
// In application shutdown handler or exit path:
encdr.clean_exit();
```

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
    Dim,
    Bright,
    Single(u8),
    Rgb { r: u8, g: u8, b: u8 },
}
```

### Constants

| Constant | Value | Description |
| :--- | :--- | :--- |
| `LedValue::OFF` | `LedValue::Off` | Off state |
| `LedValue::DIM` | `LedValue::Dim` | Dim / half-brightness level (idle / 1 LED illuminated) |
| `LedValue::BRIGHT` | `LedValue::Bright` | Bright / active level (active / 2 LEDs illuminated) |
| `LedValue::MAX` | `LedValue::Single(255)` | Maximum drive level |
| `LedValue::NI_OFF` | `0` (`0x00`) | Raw byte for NI single-color button off state |
| `LedValue::NI_DIM` | `228` (`0xE4`) | Raw byte for NI single-color button dim / half-brightness (idle) |
| `LedValue::NI_BRIGHT` | `158` (`0x9E`) | Raw byte for NI single-color button bright / active |
| `LedValue::NI_MAX` | `255` (`0xFF`) | Raw byte for NI single-color button maximum drive |

### Methods

#### `pub fn brightness(&self) -> u8`
Returns the effective brightness (0–255) of the value.
- `Off`: `0`
- `Dim`: `228` (`LedValue::NI_DIM`)
- `Bright`: `158` (`LedValue::NI_BRIGHT`)
- `Single(b)`: `b`
- `Rgb { r, g, b }`: $\max(r, g, b)$

#### `pub fn to_ni_single_byte(pct: u8) -> u8`
Converts a brightness percentage ($0..100$) to the Native Instruments PWM duty cycle byte format used by single-color button LEDs:
- `0`: Off (`0x00`)
- `30`: Dim / half-brightness (`0xE4` = 228)
- `100`: Bright / active (`0x9E` = 158)

#### `pub fn single_percent(pct: u8) -> Self`
Convenience constructor returning `LedValue::Off` for $0$, or `LedValue::Single(to_ni_single_byte(pct))` for $1..=100$.

#### `pub fn to_ni_palette_byte(r: u8, g: u8, b: u8) -> u8`
Maps an 8-bit RGB color to the Native Instruments packed 1-byte hardware palette format used by Maschine Mk3, Maschine Mikro Mk3, and Komplete Kontrol Mk2:
- `0x00`: Off
- Bits 7..2: Palette color index (1..17: Red, Orange, Amber, Yellow, Green, Mint, Cyan, Blue, Indigo, Violet, Magenta, Crimson, White, etc.)
- Bits 1..0: 2-bit intensity level (0..3)

```rust
let ni_byte = LedValue::to_ni_palette_byte(255, 0, 0); // Red at full brightness -> 0x07
```

### Multi-Target & Protocol-Aware Mapping Semantics

When `LedValue::Dim` or `LedValue::Bright` is dispatched via `set_led` or `set_led_in_group`, `LedBuilder` automatically tailors output to the hardware target type and the descriptor's configured [`LedProtocol`](#5-led-values--ni-palette-ledvalue):
- **Single (Monochrome Button LED):**
  - **`nhl2`** (Maschine Mk3, Plus, Mikro Mk3, KK Mk2/Mk3, X1 Mk3): `Dim` emits `228` (`0xE4` = half-brightness / 1 LED illuminated), `Bright` emits `158` (`0x9E` = active / both LEDs illuminated).
  - **`linear_7bit`** (Maschine Jam, Traktor S2/S4 Mk3, etc.): `Dim` emits `38` (~30%), `Bright` emits `127` (100%).
  - **`linear_8bit`** (Maschine Studio, Maschine Mk2, Traktor S4/S5/S8, etc.): `Dim` emits `76` (~30%), `Bright` emits `255` (100%).
  - Descriptors can also declare explicit `dim_value` and `bright_value` overrides per LED group.
- **RGB LEDs:** `Dim` emits `(64, 64, 64)`, `Bright` emits `(255, 255, 255)`.
- **Indexed Palette LEDs:** `Dim` emits `(17 << 2) | 1` (dim white), `Bright` emits `(17 << 2) | 3` (bright white).

---

## 6. Seven-Segment Displays: `SevenSegment`

Defined in [`encdr::core::seven_segment::SevenSegment`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/core/seven_segment.rs#L16-L195).

Controllers like the Traktor Kontrol S4 MK2, Traktor Kontrol X1 MK2, and Traktor Kontrol F1 feature multi-digit 7-segment LED displays. The `SevenSegment` struct models segment bitmasks, character translation, brightness conversion, and DJ loop length formatting.

### Segment Layout & Bitmask Constants

```text
      -- a (bit 0, 0x01) --
     |                     |
  f (bit 5, 0x20)       b (bit 1, 0x02)
     |                     |
      -- g (bit 6, 0x40) --
     |                     |
  e (bit 4, 0x10)       c (bit 2, 0x04)
     |                     |
      -- d (bit 3, 0x08) --     * dp (bit 7, 0x80)
```

| Constant | Value | Description |
|---|---|---|
| `SEG_A` | `0x01` | Top horizontal segment |
| `SEG_B` | `0x02` | Top right vertical segment |
| `SEG_C` | `0x04` | Bottom right vertical segment |
| `SEG_D` | `0x08` | Bottom horizontal segment |
| `SEG_E` | `0x10` | Bottom left vertical segment |
| `SEG_F` | `0x20` | Top left vertical segment |
| `SEG_G` | `0x40` | Center horizontal segment |
| `SEG_DP` | `0x80` | Decimal point / status dot |
| `BLANK` | `0x00` | All segments off |

### Constructors & Conversion Methods

#### `pub const fn from_mask(mask: u8) -> Self`
Creates a `SevenSegment` instance directly from a raw 8-bit segment bitmask.

#### `pub const fn raw_mask(&self) -> u8`
Returns the raw 8-bit segment bitmask.

#### `pub const fn with_dot(mut self, dot: bool) -> Self`
Sets or clears the decimal point segment (`SEG_DP`).

#### `pub const fn has_dot(&self) -> bool`
Returns `true` if the decimal point segment is illuminated.

#### `pub fn from_char(c: char) -> Self`
Translates alphanumeric characters (`'0'..'9'`, `'a'..'z'`, `'-'`, `'_'`, `'='`, `'/'`, `'.'`) to their 7-segment representation.

#### `pub fn from_digit(digit: u8) -> Self`
Translates a numeric digit or hexadecimal nibble (`0..=15`) to its 7-segment representation.

#### `pub fn to_brightness_array(&self, on_brightness: u8, off_brightness: u8) -> [u8; 8]`
Decomposes the 7-segment bitmask into an 8-byte array of segment brightnesses `[a, b, c, d, e, f, g, dp]` for controllers where each segment is individually addressed as an LED channel.

#### `pub fn encode_str(s: &str) -> Vec<Self>`
Parses a string into a sequence of `SevenSegment` instances. Embedded periods (`'.'`) are merged into the preceding character's decimal point.

#### `pub fn encode_loop_length(beats: f32, loop_active: bool) -> [Self; 2]`
Encodes standard DJ loop beat lengths following Native Instruments Traktor notation. Whole beats count normally (`32`, `16`, ` 8`, ` 4`, ` 2`, ` 1`), while sub-beat fractions count in reverse with the decimal point lit (`.2` for 1/2 beat, `.4` for 1/4 beat, `.8` for 1/8 beat, `1.6` for 1/16 beat, `3.2` for 1/32 beat). When `loop_active` is true, the decimal point on `right` is illuminated.

```rust
use encdr::SevenSegment;

// Encode a 1/4 beat loop (inactive)
let [left, right] = SevenSegment::encode_loop_length(0.25, false);
assert_eq!(left, SevenSegment::from_char('.'));
assert_eq!(right, SevenSegment::from_char('4'));

// Encode a 1/2 beat loop (active)
let [left, right] = SevenSegment::encode_loop_length(0.5, true);
assert_eq!(left, SevenSegment::from_char('.'));
assert_eq!(right, SevenSegment::from_char('2').with_dot(true));
```

---

## 7. Jog Wheel LED Rings: `JogRing`, `JogDeck`, `JogRingMode`

Defined in [`encdr::core::jog_ring`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/core/jog_ring.rs).

Models the dual motorized/haptic jog wheel LED rings on the Traktor Kontrol S4 MK3 (USB Output Report `0x32`).

### `JogDeck`
Identifies the deck for jog wheel operations:
- `JogDeck::Left` (Deck A/C, index 0)
- `JogDeck::Right` (Deck B/D, index 1)

### `JogRingMode`
Hardware operating modes supported by the S4 MK3 ring controller:
| Mode | Value | Description |
|---|---|---|
| `JogRingMode::Off` | `0` | All ring LEDs off |
| `JogRingMode::DimFlash` | `1` | Dim pulsing/flashing ring |
| `JogRingMode::Needle` | `2` | Single illuminated needle indicator (0..2879 ticks) |
| `JogRingMode::RingFlash` | `3` | Full ring flash with base color |
| `JogRingMode::DimSpot` | `4` | Dim spot indicator (0..2879 ticks) |
| `JogRingMode::Addressable` | `5` | 32 individually addressable LEDs |

### `JogRing`
Helper struct for constructing 32-segment ring buffers:
- `JogRing::new() -> Self`: All 32 LEDs off.
- `JogRing::solid(val: u8) -> Self`: All 32 LEDs set to brightness / palette byte.
- `JogRing::spinner(head_idx: usize, tail_len: usize, head_val: u8) -> Self`: Rotating spinner with fading tail.
- `JogRing::meter(fill_count: usize, val: u8) -> Self`: Arc meter fill (0..=32 segments).
- `set(&mut self, index: usize, val: u8)`: Sets segment index with modulo 32 wrapping.
- `set_led(&mut self, index: usize, val: LedValue)`: Sets segment converting `LedValue::Rgb` to NI palette byte.
- `as_bytes(&self) -> &[u8; 32]`: Returns reference to underlying 32-byte array.

### `JogWheelTracker`
Helper struct for tracking angular rotation from manual or motorized movements:
- `JogWheelTracker::new() -> Self`: Starts at tick 0.
- `from_ticks(ticks: u16) -> Self`: Starts at specified tick (0..2879).
- `set_position(&mut self, ticks: u16)`: Sets absolute tick position.
- `set_normalized(&mut self, normalized: f32)`: Sets position from normalized 0.0..1.0 value.
- `set_radians(&mut self, radians: f32)`: Sets position from angle in radians (0.0 .. 2*PI).
- `advance(&mut self, delta: f32, ticks_per_unit: f32)`: Advances position by encoder delta or motor displacement.
- `current_ticks(&self) -> u16`: Returns current tick position (0..2879).
- `current_normalized(&self) -> f32`: Returns current position as normalized 0.0..1.0.
- `current_segment(&self) -> usize`: Returns current 32-segment LED index (0..31).

---

## 8. Pixel Formats: `PixelFormat`

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
    /// ST7529 5-bit grayscale, 3 pixels per 2 bytes, inverted (Maschine Mk1)
    St7529Gray5,
}
```

### Methods

#### `pub fn bytes_per_pixel(&self) -> usize`
Returns byte density per pixel (`2` for BGR565/RGB565, `3` for RGB888, `4` for RGBA8888, and `1` for Mono and St7529Gray5, which are packed below a byte per pixel).

#### `pub fn black_fill(&self) -> u8`
Returns the native byte value that fills a frame with black (`0xFF` for the inverted St7529Gray5, `0x00` otherwise).

---

## 9. GPU Acceleration: `GpuContext`

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

## 10. Custom Packet Decoders: `PacketHook`

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

## 11. Error Handling: `EncdrError`

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
    Io(#[from] std::io::Error),

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

## 12. Device Descriptors & Runtime Introspection

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
    pub protocol: Option<ScreenProtocol>,
}
```
- `pixel_count(&self) -> usize`: Width $\times$ Height.
- `byte_size(&self) -> usize`: Total frame byte buffer size.

---

## 13. WebView Offscreen Renderer: `encdr-view`

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

#### `pub fn new_offscreen(width: u32, height: u32, content: ScreenContent, visible: bool) -> Result<Self, String>`
Creates a standalone headless WebView with explicit dimensions without requiring a physical device descriptor. Ideal for generating image assets, graphics caches, or Komplete Kontrol Mk3 header banners ($1200\times 240$).

```rust
let view = ScreenView::new_offscreen(1200, 240, ScreenContent::Html(banner_html), false)?;
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

#### `pub fn capture_pixels(&self) -> Result<(u32, u32, Vec<u8>), String>`
Grabs the composited web surface and returns raw RGBA pixel data `(width, height, rgba_bytes)`.

#### `pub fn capture_png(&self) -> Result<Vec<u8>, String>`
Grabs the composited web surface and encodes it into compressed PNG bytes in memory. Ideal for pushing directly to Komplete Kontrol Mk3 via `kk_mk3_set_header_image`.

```rust
if view.is_frame_ready() {
    let png_bytes = view.capture_png()?;
    encdr.kk_mk3_set_header_image(device_id, "banner", &png_bytes, &mut plugin)?;
}
```

#### `pub fn capture_and_submit(&self, encdr: &Encdr) -> Result<(), String>`
Immediately grabs the composited surface using platform-native capture mechanisms (Cairo snapshot on Linux, `takeSnapshot` on macOS, `CapturePreview` on Windows) and submits the RGBA buffer to Encdr for USB transfer to a physical screen.

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

#### `pub fn device_id(&self) -> Option<DeviceId>`
Returns the `DeviceId` associated with this view, or `None` if created via `new_offscreen`.

#### `pub fn screen_name(&self) -> &str`
Returns the target screen identifier string.

### `DualScreenView` Methods

Defined in [`encdr-view::DualScreenView`](file:///home/rufus/Documents/Projects/Encdr/encdr-view/src/lib.rs#L224-L394).

`DualScreenView` hosts a single offscreen WebView sized to `(left_width + right_width) x height` (e.g. `960x272` on Maschine Mk3 / KK Mk2 / Traktor S8, or `640x240` on Traktor S4 Mk3). 

#### Advantages over Two Separate `ScreenView`s:
- **50% RAM & CPU savings**: Runs only 1 WebKit/WebView2 browser engine instance instead of 2.
- **Single DOM/State tree**: Animate or layout UI components seamlessly across both displays using standard CSS grid or flexbox (`width: 50%` per deck or screen half).
- **Zero-cost diffing preserved**: Before sending across USB, Encdr slices the frame in half. If one screen's content has not changed, zero USB packets are transmitted for that screen.

#### `pub fn new(encdr: &Encdr, device_id: DeviceId, content: ScreenContent, visible: bool) -> Result<Self, String>`
Creates a new double-width WebView renderer defaulting to screen names `"left"` and `"right"`.

```rust
use encdr_view::{ScreenContent, DualScreenView};

let dual_view = DualScreenView::new(
    &encdr,
    device_id,
    ScreenContent::File("./ui/dual_screen.html".to_string()),
    false,
)?;
```

#### `pub fn new_with_screens(encdr: &Encdr, device_id: DeviceId, left_screen: &str, right_screen: &str, content: ScreenContent, visible: bool) -> Result<Self, String>`
Creates a new double-width WebView renderer with custom screen identifiers.

#### `pub fn capture_and_submit(&self, encdr: &Encdr) -> Result<(), String>`
Captures the double-width surface and dispatches it via `encdr.submit_dual_screen_with_format`.

#### `pub fn poll(&self, encdr: &Encdr)`
Checks `is_frame_ready()`, and if new content was composited, captures and submits the dual-screen frame to Encdr.

```rust
// Application loop:
ScreenView::pump_events();
dual_view.poll(&encdr);
```

#### `pub fn send(&self, channel: &str, data: serde_json::Value)`
Dispatches a JSON event to `window.encdr.onMessage(channel, data)` across the shared WebView page.

#### `pub fn load_html(&self, html: &str) -> Result<(), String>`
Replaces the currently loaded HTML page.

#### `pub fn eval(&self, js: &str) -> Result<(), String>`
Executes arbitrary JavaScript within the shared WebView.

#### `pub fn left_screen_name(&self) -> &str`
Returns the left display identifier string.

#### `pub fn right_screen_name(&self) -> &str`
Returns the right display identifier string.

---

## 14. Komplete Kontrol Mk3 DAW & On-Device Rendering (ODR)

Defined in [`encdr::device::komplete_kontrol`](file:///home/rufus/Documents/Projects/Encdr/encdr/src/device/komplete_kontrol/mod.rs) and re-exported at the crate root.

Provides typed builders, parsers, and data models for Native Instruments Komplete Kontrol S-Series Mk3 (S49, S61, S88) controllers across both the dedicated DAW Remote MIDI port (`MIDI Channel 16` / SysEx) and the high-speed USB bulk ODR MessagePack-RPC command pipe (`odr_cmd`).

### `KkMk3DawController`

Stateless builder and parser for the dedicated `"KONTROL S-Series MK3 DAW"` MIDI port.

#### Constructors & Handshake Methods
- `pub fn new() -> Self`: Initializes the controller helper.
- `pub fn build_hello(&self) -> [u8; 3]`: Handshake greeting (`[0xBF, 0x01, 0x04]`) activating DAW mode.
- `pub fn build_enable_14bit(&self) -> [u8; 3]`: Command (`[0xBF, 0x06, 0x01]`) enabling 14-bit high-resolution rotary knob updates via SysEx `0x7F`.
- `pub fn build_goodbye(&self) -> [u8; 3]`: Goodbye message (`[0xBF, 0x02, 0x00]`) releasing DAW mode back to standalone operation.
- `pub fn build_identity(&self, app_name: &str, major_ver: u8, minor_ver: u8) -> Vec<u8>`: Builds DAW host identification SysEx message.
- `pub fn build_surface_configuration_vertical(&self) -> Vec<u8>`: Configures vertical track orientation layout.

#### Track Strips & Mixer SysEx Methods
- `pub fn build_track_enabled(&self, track_idx: u8, enabled: bool) -> Vec<u8>`: Enables or disables a track slot (0..7).
- `pub fn build_track_selected(&self, track_idx: u8, selected: bool) -> Vec<u8>`: Sets track focus selection state.
- `pub fn build_track_mute(&self, track_idx: u8, muted: bool) -> Vec<u8>`: Sets track mute state.
- `pub fn build_track_solo(&self, track_idx: u8, soloed: bool) -> Vec<u8>`: Sets track solo state.
- `pub fn build_track_armed(&self, track_idx: u8, armed: bool) -> Vec<u8>`: Sets record arm status.
- `pub fn build_track_name(&self, track_idx: u8, name: &str) -> Vec<u8>`: Sets track title label.
- `pub fn build_track_color(&self, track_idx: u8, hex_color: &str) -> Vec<u8>`: Sets track color as `#AARRGGBB` hex string.
- `pub fn build_track_color_rgba(&self, track_idx: u8, r: f32, g: f32, b: f32, a: f32) -> Vec<u8>`: Sets track color from normalized float components.
- `pub fn build_volume_display(&self, track_idx: u8, display_str: &str) -> Vec<u8>`: Sets formatted volume display text (e.g. `"-6.0 dB"`).
- `pub fn build_pan_display(&self, track_idx: u8, display_str: &str) -> Vec<u8>`: Sets formatted pan display text (e.g. `"L 25"` or `"C"`).
- `pub fn build_vu_meters(&self, left_db: &[f32; 8], right_db: &[f32; 8]) -> Vec<u8>`: Builds 8-channel stereo logarithmic VU meter SysEx packet (`-70.0 dB` to `+6.0 dB`).

#### Parameter & Chain SysEx Methods
- `pub fn build_plugin_chain_info(&self, plugin_names: &[&str]) -> Vec<u8>`: Transmits null-separated plugin insert names for the current track.
- `pub fn build_select_plugin(&self, chain_index: u8) -> Vec<u8>`: Focuses an insert plugin slot index on the display.
- `pub fn build_parameter_name(&self, knob_idx: u8, name: &str) -> Vec<u8>`: Sets parameter label for knob 0..7.
- `pub fn build_parameter_display_value(&self, knob_idx: u8, value_str: &str) -> Vec<u8>`: Sets formatted value text for knob 0..7.
- `pub fn build_parameter_page_info(&self, total_pages: u8, current_page_idx: u8) -> Vec<u8>`: Sets page count and active page index.
- `pub fn build_tempo_bpm(&self, bpm: f32) -> Vec<u8>`: Transmits project tempo encoded in 10-nanosecond beat intervals.

#### Incoming Event Parsing
- `pub fn parse_incoming(&self, bytes: &[u8]) -> Option<KkMk3DawEvent>`: Parses raw MIDI bytes received from Channel 16 or SysEx into typed events.

```rust
use encdr::{KkMk3DawController, KkMk3DawEvent};

let daw = KkMk3DawController::new();
if let Some(event) = daw.parse_incoming(&[0xBF, 0x10, 0x01]) {
    match event {
        KkMk3DawEvent::Button { name, pressed, .. } => println!("Button {} = {}", name, pressed),
        KkMk3DawEvent::KnobAdjustment14Bit { group, index, delta } => println!("Knob #{}: {:+.4}", index, delta),
        _ => {}
    }
}
```

### `KkMk3DawEvent`

Parsed event variants dispatched from the DAW Remote port:
- `Button { cc: u8, name: &'static str, value: u8, pressed: bool }`
- `Navigation { cc: u8, name: &'static str, delta: i8 }`
- `BankMapping(BankMappingMode)`
- `SelectedTrackVolumeDelta(i8)`
- `SelectedTrackPanDelta(i8)`
- `KnobAdjustment14Bit { group: KnobGroup, index: u8, delta: f32 }`
- `PluginSelected { chain_index: u8 }`
- `TempoChangeBpm(f32)`
- `OtherSysEx(Vec<u8>)`

---

### On-Device Rendering (ODR) Data Models

#### `RgbColor`
RGB color representation for UI themes, accents, and Light Guide LEDs:
- `pub const fn new(r: u8, g: u8, b: u8) -> Self`
- Predefined constants: `RgbColor::RED`, `ORANGE`, `YELLOW`, `GREEN`, `CYAN`, `BLUE`, `PURPLE`, `MAGENTA`, `WHITE`, `OFF`.

#### `PluginData`
Top-level parameter page model driving the 8 rotary encoders, header banner, and color theming:
- `pub fn new(name: impl Into<String>, color: RgbColor) -> Self`
- `pub fn with_background(mut self, asset_id: impl Into<String>) -> Self`
- `pub fn add_parameter(&mut self, param: ParameterItem) -> &mut Self`

#### `ParameterItem`
Single parameter slot (0..7) mapped to a rotary encoder:
- `pub fn new(name: impl Into<String>, value: f32, display_value: impl Into<String>, display_type: WidgetDisplayType, section_name: impl Into<String>) -> Self`
- `pub fn knob(name: impl Into<String>, value: f32, display_value: impl Into<String>, section: impl Into<String>) -> Self`
- `pub fn toggle(name: impl Into<String>, on: bool, section: impl Into<String>) -> Self`

#### `WidgetDisplayType`
Visual representation rendered for a parameter control:
- `Knob`: Radial arc gauge.
- `Range`: Linear slider bar.
- `Toggle`: 2-state On/Off switch.
- `Trigger`: Momentary button.
- `Increment`: Stepped value selector.
- `Relative`: Bipolar detented indicator (`-1.0..1.0`).
- `Text`: Static text readout.
- `Disabled`: Inactive slot.

#### `PluginChainModel` & `PluginChainItem`
Serial insert chain rendered on the keyboard:
- `PluginChainItem::new(name, color)` with optional `.with_vendor(...)` and `.bypassed(bool)`.
- `PluginChainModel::new()`, `.add_plugin(...)`, `.set_current_index(u32)`.

#### `MixerModel` & `MixerTrack`
Multi-channel mixer model rendered on the keyboard:
- `MixerTrack::new(name, color)` with fields `volume`, `pan`, `volume_display`, `pan_display`, `muted`, `soloed`, `armed`, `selected`.
- `MixerModel::new()`, `.add_track(...)`.

#### `SmartPlayData`
Onboard scale, chord, and arpeggiator engine state:
- `scale: ScaleConfig`: `enabled`, `root_key` (0..11), `scale_type` (`"Major"`, `"Minor"`, `"Dorian"`, etc.), `scale_mode` (`"Guide"` or `"Mute"`).
- `arp: ArpConfig`: `enabled`, `pattern` (`"Up"`, `"Down"`, `"UpDown"`), `rate` (`"1/16"`, `"1/8"`), `gate`, `octaves`, `swing`.
- `chord: ChordConfig`: `enabled`, `chord_mode`, `chord_type`.

#### `BrowserModel`, `BrowserFilter`, `BrowserSoundItem`
Sound and preset browser model:
- `BrowserFilter::new(name, options).with_selection(opt)`
- `BrowserSoundItem::new(name, vendor, product)`
- `BrowserModel`: contains `filters: Vec<BrowserFilter>`, `sounds: Vec<BrowserSoundItem>`, `selected_sound_index: Option<u32>`.

#### `DeviceSettings`
Hardware preferences:
- `display_brightness: u8` (0..100)
- `led_brightness: u8` (0..100)
- `lightguide_enabled: bool`
- `velocity_curve: String` (`"Linear"`, `"Soft"`, `"Hard"`)

#### `KkMk3Page`
Onboard view template selector:
- `Parameters`: Parameter controls.
- `Browser`: Sound & preset browser.
- `Mixer`: Multi-track mixer view.
- `SmartPlay`: Scales, chords, and arpeggiator view.
- `PluginChain`: Insert chain view.
- `Settings`: Hardware device settings.

#### `OdrRpcFramer`
MessagePack-RPC packet framer used internally to encode notifications (`[2, method, params]`) and requests (`[0, msg_id, method, params]`) over Bulk OUT `0x03`.
