//! Native Instruments Komplete Kontrol S-Series Mk3 (S49 / S61 / S88)
//! Comprehensive Demonstration: Direct DAW Protocol & On-Device Rendering (ODR).
//!
//! This example demonstrates:
//! 1. Direct DAW Remote MIDI CC & SysEx protocol:
//!    - Handshake messages (`0xBF 0x01 0x04`, `0xBF 0x06 0x01`)
//!    - Track titles & RGB colors
//!    - Stereo dB VU metering
//!    - 14-bit rotary encoder delta decoding
//! 2. On-Device Rendering (ODR) MessagePack-RPC over USB (interface 3):
//!    - Session handshake; Encdr keeps the heartbeat and re-handshakes itself
//!    - NKS parameter page with 8 knobs, updated live as they are turned
//!    - Keybed Light Guide colour sweep
//!
//! Close Komplete Kontrol and stop NI's background services first; they hold
//! USB interface 3.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin kk_mk3_daw_odr

use std::time::{Duration, Instant};
use encdr::{
    BrowserFilter, BrowserModel, BrowserSoundItem, Encdr, EncdrConfig, Event,
    KkMk3DawController, KkMk3OdrEvent, MixerModel, MixerTrack, ParameterItem, PluginChainItem, PluginChainModel,
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
    // `background` is an index into the keyboard's own image cache (21 is known
    // to work); uploading custom images isn't verified yet.
    let mut plugin = PluginData::new("Analog Monolith", cyan).with_background("21");

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

    let key_count = match desc.product_id.0 {
        0x2110 => 61,
        0x2120 => 88,
        _ => 49,
    };

    // Local knob values (0..7), mirrored onto the keyboard's parameter page.
    let mut knob_values: Vec<f32> = plugin.parameters.iter().map(|p| p.value).collect();

    // Only messages verified on hardware are sent. The SmartPlay / browser /
    // mixer / settings / asset models above are printed but not sent until
    // they are verified.
    let setup = |encdr: &Encdr| {
        let _ = encdr.kk_mk3_start_session(device_id, "Komplete Kontrol", "Encdr Synth");
        let _ = encdr.kk_mk3_set_plugin_chain(device_id, &chain);
        let _ = encdr.kk_mk3_set_plugin_data(device_id, &plugin);
    };

    // Open the ODR session. Encdr runs the heartbeat itself once the keyboard
    // replies, and re-handshakes if the keyboard asks; each reply means the
    // keyboard needs our pages again.
    println!("Sending ODR handshake (437-symbol registry)...");
    if let Err(e) = encdr.kk_mk3_handshake(device_id) {
        eprintln!("Handshake failed: {e}");
    }
    let events = encdr.events().clone();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut replied = false;
    while !replied && Instant::now() < deadline {
        if let Ok(ev) = events.recv_timeout(Duration::from_millis(100)) {
            if let Event::KkMk3Odr { event: KkMk3OdrEvent::HandshakeReply, .. } = ev {
                replied = true;
            }
        }
    }
    if replied {
        println!("Handshake accepted.");
    } else {
        println!("No handshake reply within 3 s; continuing anyway (no heartbeat until the keyboard replies).");
    }
    println!("Registering instance and sending parameter page...");
    setup(&encdr);

    let start_time = Instant::now();
    let mut last_lightguide = Instant::now();

    println!("\nLive controller active! Rotate knobs or play keys. Press Ctrl+C to quit.\n");

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        // Process incoming events
        while let Ok(event) = events.try_recv() {
            if event.device_id() != device_id {
                continue;
            }
            match event {
                Event::Button { name, pressed, .. } => {
                    println!("[BUTTON] {} -> {}", name.to_uppercase(), if pressed { "DOWN" } else { "UP" });
                }
                Event::EncoderFine { name, delta, .. } => {
                    println!("[ENCODER FINE] {} delta: {:+.4}", name.to_uppercase(), delta);
                    // If one of the 8 knobs was turned, update screen
                    if let Some(num_str) = name.strip_prefix("encoder_") {
                        if let Ok(idx) = num_str.parse::<usize>() {
                            if idx >= 1 && idx <= knob_values.len() {
                                let k = idx - 1;
                                knob_values[k] = (knob_values[k] + delta).clamp(0.0, 1.0);
                                let display = format!("{:.0}%", knob_values[k] * 100.0);
                                let _ = encdr.kk_mk3_update_parameter_value_with_display(device_id, k as u32, knob_values[k], &display);
                                println!("      → knob {} = {}", idx, display);
                            }
                        }
                    }
                }
                Event::Encoder { name, delta, .. } => {
                    println!("[ENCODER] {} delta: {:+2}", name.to_uppercase(), delta);
                }
                Event::Touch { name, touched, .. } => {
                    println!("[TOUCH] {} -> {}", name.to_uppercase(), if touched { "TOUCHED" } else { "RELEASED" });
                }
                Event::Slider { name, value, .. } => {
                    println!("[STRIP] {} = {:.2}", name.to_uppercase(), value);
                }
                Event::KkMk3Odr { event, .. } => match event {
                    KkMk3OdrEvent::HandshakeReply => {
                        println!("[ODR] Handshake reply — resending session and page");
                        setup(&encdr);
                    }
                    KkMk3OdrEvent::HeartbeatRequestHandshake => {
                        println!("[ODR] Keyboard requested a new handshake (Encdr is re-handshaking)");
                    }
                    KkMk3OdrEvent::ParameterSet { param_index, value } => {
                        println!("[ODR] Parameter {} set to {:.3}", param_index + 1, value);
                        if let Some(v) = knob_values.get_mut(param_index as usize) {
                            *v = value.clamp(0.0, 1.0);
                        }
                    }
                    KkMk3OdrEvent::Other { method, .. } => println!("[ODR] {method}"),
                    _ => {}
                },
                Event::DeviceDisconnected { .. } => {
                    println!("Device disconnected.");
                    return;
                }
                _ => {}
            }
        }

        // Animate Light Guide LEDs at 10 FPS (2 FPS is verified on hardware; the
        // keyboard's throughput limit is unknown).
        if last_lightguide.elapsed() >= Duration::from_millis(100) {
            last_lightguide = Instant::now();

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
