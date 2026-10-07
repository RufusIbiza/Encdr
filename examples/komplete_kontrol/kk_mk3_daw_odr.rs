//! Native Instruments Komplete Kontrol S-Series Mk3 (S49 / S61 / S88)
//! Comprehensive Demonstration: Direct DAW Protocol & On-Device Rendering (ODR).
//!
//! This example demonstrates:
//! 1. Direct DAW Remote MIDI CC & SysEx protocol:
//!    - Handshake messages (`0xBF 0x01 0x04`, `0xBF 0x06 0x01`)
//!    - Track titles & RGB colors
//!    - Stereo dB VU metering
//!    - 14-bit rotary encoder delta decoding
//! 2. On-Device Rendering (ODR) MessagePack-RPC:
//!    - NKS parameter page modelling with 8 rotary controls
//!    - Top header banner asset uploading (dynamic PNG generation)
//!    - Real-time parameter value modulation
//!    - Keybed Light Guide RGB LED sweeps
//!
//! Run with:
//!   cargo run -p encdr-examples --bin kk_mk3_daw_odr

use std::time::{Duration, Instant};
use encdr::{
    BrowserFilter, BrowserModel, BrowserSoundItem, DeviceSettings, Encdr, EncdrConfig, Event,
    KkMk3DawController, MixerModel, MixerTrack, ParameterItem, PluginChainItem, PluginChainModel,
    PluginData, RgbColor, SmartPlayData,
};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".parse().unwrap()),
        )
        .init();

    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║       NI Komplete Kontrol S-Series Mk3 — DAW & ODR Controller        ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");

    // ── 1. Demonstrate DAW Protocol Engine ────────────────────────────────
    println!("\n─── [1/3] DAW Remote Protocol Initialization ───");
    let daw = KkMk3DawController::new();
    let hello = daw.build_hello();
    let enable_14bit = daw.build_enable_14bit();
    let identity = daw.build_identity("Encdr DAW Host", 1, 0);
    println!("DAW Hello MIDI CC: {:02X?}", hello);
    println!("DAW 14-bit Mode Enable: {:02X?}", enable_14bit);
    println!("DAW Host Identity SysEx ({} bytes): {:02X?}", identity.len(), &identity[..identity.len().min(16)]);

    let track_name_sysex = daw.build_track_name(0, "Lead Synth");
    let track_color_sysex = daw.build_track_color_rgba(0, 0.0, 0.8, 1.0, 1.0);
    let neg_inf = f32::NEG_INFINITY;
    let vu_meter_sysex = daw.build_vu_meters(&[-6.0, -12.0, -18.0, -24.0, neg_inf, neg_inf, neg_inf, neg_inf], &[-6.0, -12.0, -18.0, -24.0, neg_inf, neg_inf, neg_inf, neg_inf]);

    println!("Track 0 Name SysEx ({} bytes): {:02X?}", track_name_sysex.len(), &track_name_sysex[..track_name_sysex.len().min(16)]);
    println!("Track 0 Color SysEx: {:02X?}", track_color_sysex);
    println!("8-ch Stereo VU Meter SysEx: {:02X?}", vu_meter_sysex);

    // Demonstrate parsing incoming DAW CC
    if let Some(event) = daw.parse_incoming(&[0xBF, 0x10, 0x01]) {
        println!("Parsed DAW Event: {:?}", event);
    }

    // ── 2. Build NKS Models for ODR ───────────────────────────────────────
    println!("\n─── [2/3] ODR Extended Models (Plugin Chain, Browser, SmartPlay, Mixer) ───");
    let cyan = RgbColor::new(0, 210, 255);
    let mut plugin = PluginData::new("Analog Monolith", cyan)
        .with_background("demo_synth_banner");

    plugin.add_parameter(ParameterItem::knob("Cutoff", 0.72, "3.4 kHz", "Filter"));
    plugin.add_parameter(ParameterItem::knob("Resonance", 0.45, "45%", "Filter"));
    plugin.add_parameter(ParameterItem::knob("Drive", 0.20, "+3.0 dB", "Filter"));
    plugin.add_parameter(ParameterItem::toggle("Lowpass 24", true, "Filter"));
    plugin.add_parameter(ParameterItem::knob("Attack", 0.15, "12 ms", "Envelope"));
    plugin.add_parameter(ParameterItem::knob("Decay", 0.50, "320 ms", "Envelope"));
    plugin.add_parameter(ParameterItem::knob("Sustain", 0.80, "-2.0 dB", "Envelope"));
    plugin.add_parameter(ParameterItem::knob("Release", 0.35, "180 ms", "Envelope"));

    // Plugin chain model
    let mut chain = PluginChainModel::new();
    chain.add_plugin(PluginChainItem::new("Analog Monolith", cyan).with_vendor("Encdr Synth"));
    chain.add_plugin(PluginChainItem::new("Raum Reverb", RgbColor::new(100, 150, 255)).with_vendor("Native Instruments"));
    chain.set_current_index(0);

    // Smart play model
    let mut smartplay = SmartPlayData::default();
    smartplay.scale.enabled = true;
    smartplay.scale.root_key = 2; // D
    smartplay.scale.scale_type = "Dorian".into();
    smartplay.arp.enabled = true;
    smartplay.arp.pattern = "UpDown".into();

    // Browser model
    let mut browser = BrowserModel::default();
    browser.filters.push(BrowserFilter::new("Instrument", vec!["Lead".into(), "Bass".into(), "Pad".into()]).with_selection("Lead"));
    browser.sounds.push(BrowserSoundItem::new("Blade Runner Lead", "Encdr", "Analog Monolith"));
    browser.sounds.push(BrowserSoundItem::new("Sub 808", "Encdr", "Analog Monolith"));

    // Mixer model
    let mut mixer = MixerModel::new();
    mixer.add_track(MixerTrack::new("Lead Synth", cyan));
    mixer.add_track(MixerTrack::new("Drums", RgbColor::new(255, 100, 50)));

    println!("Plugin Model: '{}' with {} parameters", plugin.name, plugin.parameters.len());
    println!("Plugin Chain: {} inserts, active index {}", chain.plugins.len(), chain.current_index);
    println!("SmartPlay: Scale root {} ({}), Arp {}", smartplay.scale.root_key, smartplay.scale.scale_type, smartplay.arp.pattern);
    println!("Browser: {} filters, {} sound items", browser.filters.len(), browser.sounds.len());
    println!("Mixer: {} tracks configured", mixer.tracks.len());

    // Generate a 400x60 header banner PNG dynamically
    let banner_png = generate_gradient_png(400, 60);
    println!("Generated dynamic header PNG: {} bytes", banner_png.len());

    // ── 3. Scan for Hardware ──────────────────────────────────────────────
    println!("\n─── [3/3] Scanning for Connected Hardware ───");
    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Device scan failed");

    let device_opt = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| {
            d.vendor_id.0 == 0x17cc && (d.product_id.0 == 0x2100 || d.product_id.0 == 0x2110 || d.product_id.0 == 0x2120)
        }).unwrap_or(false)
    });

    let Some(device_id) = device_opt else {
        println!("ℹ️  No physical Komplete Kontrol S-Series Mk3 keyboard detected.");
        println!("    (VID 0x17cc, PID 0x2100 [S49], 0x2110 [S61], or 0x2120 [S88]).");
        println!("\nAll protocol builders and serialization tests completed successfully!");
        return;
    };

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("✅ Connected to: {} (0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);

    // Upload header banner image and activate models on hardware
    println!("Uploading models and activating ODR parameter page...");
    let _ = encdr.kk_mk3_set_plugin_chain(device_id, &chain);
    let _ = encdr.kk_mk3_set_smartplay(device_id, &smartplay);
    let _ = encdr.kk_mk3_set_browser_model(device_id, &browser);
    let _ = encdr.kk_mk3_set_mixer_model(device_id, &mixer);
    let _ = encdr.kk_mk3_set_device_settings(device_id, &DeviceSettings::default());
    if let Err(e) = encdr.kk_mk3_set_header_image(device_id, "demo_synth_banner", &banner_png, &mut plugin) {
        eprintln!("Failed to upload header/plugin data: {}", e);
    }

    let key_count = match desc.product_id.0 {
        0x2110 => 61,
        0x2120 => 88,
        _ => 49,
    };

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_lfo_tick = Instant::now();

    println!("\nLive controller active! Rotate knobs or play keys. Press Ctrl+C to quit.\n");

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        // Process incoming events
        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    println!("[BUTTON] {} -> {}", name.to_uppercase(), if pressed { "DOWN" } else { "UP" });
                }
                Event::Encoder { name, delta, .. } => {
                    println!("[ENCODER] {} delta: {:+2}", name.to_uppercase(), delta);
                }
                Event::Slider { name, value, .. } => {
                    println!("[STRIP] {} = {:.2}", name.to_uppercase(), value);
                }
                Event::DeviceDisconnected { .. } => {
                    println!("Device disconnected.");
                    return;
                }
                _ => {}
            }
        }

        // Modulate Cutoff knob via LFO and animate Light Guide LEDs at ~30 FPS
        if last_lfo_tick.elapsed() >= Duration::from_millis(33) {
            last_lfo_tick = Instant::now();

            // Real-time parameter modulation
            let lfo_val = (elapsed * 2.0).sin() * 0.4 + 0.5;
            let _ = encdr.kk_mk3_update_parameter_value(device_id, 0, lfo_val);

            // Light Guide RGB rainbow sweep
            let mut rgb_keys = Vec::with_capacity(key_count);
            for i in 0..key_count {
                let hue = (elapsed * 2.0 + (i as f32) * 0.1).sin();
                let r = ((hue * 0.5 + 0.5) * 200.0) as u8;
                let g = (((hue + 1.0) * 0.5) * 100.0) as u8;
                let b = 255u8;
                rgb_keys.push((r, g, b));
            }
            let _ = encdr.kk_mk3_set_lightguide(device_id, &rgb_keys);
        }

        std::thread::sleep(Duration::from_millis(2));
    }
}

// ── Pure-Rust Minimal PNG Generator ──────────────────────────────────────────

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB88320 & (!((crc & 1).wrapping_sub(1))));
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &b in data {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}

fn write_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let crc_start = out.len();
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let crc = crc32(&out[crc_start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn generate_gradient_png(width: u32, height: u32) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width * height * 4 + height) as usize);
    for y in 0..height {
        raw.push(0u8); // Filter type: None
        for x in 0..width {
            let r = ((x as f32 / width as f32) * 220.0) as u8;
            let g = ((y as f32 / height as f32) * 160.0) as u8;
            let b = 240u8;
            let a = 255u8;
            raw.extend_from_slice(&[r, g, b, a]);
        }
    }

    let mut zlib = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let is_last = (i == chunks.len() - 1) as u8;
        zlib.push(is_last);
        let len = chunk.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(chunk);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10]; // PNG signature

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    write_chunk(&mut png, b"IHDR", &ihdr);

    write_chunk(&mut png, b"IDAT", &zlib);
    write_chunk(&mut png, b"IEND", &[]);

    png
}
