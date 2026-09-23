//! Maschine Mk3 pad rainbow & dual-screen hardware telemetry test:
//! - Cycles all 16 pads and Group buttons A-H with smooth rainbow colors
//! - Flashes pads and group buttons bright white when pressed
//! - Renders an interactive 4x4 Pad Matrix on the Left Display via WebKit WebView
//! - Renders Group Buttons and a real-time Event Log on the Right Display via WebKit WebView
//!
//! Everything — HID input handling, LED updates, and screen rendering (WebView JS
//! eval, WebKit snapshot capture, USB blit) — runs on a single thread. An earlier
//! version split screen rendering onto its own thread to keep a slow WebKit
//! snapshot capture from delaying pad/LED response, but that regressed the screens
//! to showing nothing at all on real hardware, so it's reverted back to this
//! single-threaded structure, which is the last confirmed-working baseline for the
//! screens. Splitting screen rendering off again is worth revisiting, but as a
//! separate, carefully hardware-tested change — not bundled with protocol fixes.
//!
//! Run with: cargo run -p encdr-examples --bin mk3_pad_rainbow
//! Show desktop windows: cargo run -p encdr-examples --bin mk3_pad_rainbow -- --visible

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use encdr::{Encdr, EncdrConfig, Event, LedValue};
use encdr_view::{ScreenContent, ScreenView};

fn hsv_to_rgb(hue: f32, sat: f32, val: f32) -> (u8, u8, u8) {
    let c = val * sat;
    let h = (hue % 360.0) / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = val - c;

    let (r1, g1, b1) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}

const GROUP_NAMES: [&str; 8] = [
    "group_a", "group_b", "group_c", "group_d",
    "group_e", "group_f", "group_g", "group_h",
];

#[derive(serde::Serialize, Clone)]
struct LogEvent {
    time: f32,
    text: String,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".parse().unwrap()),
        )
        .init();

    let visible = std::env::args().any(|a| a == "--visible");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");

    println!("=== NI Maschine Mk3 Pad & WebView Screen Demo ===");
    println!("Scanning for connected devices...");

    let ids = encdr.scan().expect("Scan failed");
    if ids.is_empty() {
        eprintln!("No devices found. Is the Maschine Mk3 plugged in over USB?");
        return;
    }

    let device_id = ids
        .into_iter()
        .find(|&id| {
            encdr
                .device_descriptor(id)
                .map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1600)
                .unwrap_or(false)
        })
        .unwrap_or_else(|| {
            eprintln!("Maschine Mk3 (0x17cc:0x1600) not found among connected devices.");
            std::process::exit(1);
        });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);

    // Allow USB interfaces to settle
    std::thread::sleep(Duration::from_millis(300));

    // Initialize WebViews for both screens. A screen that fails to initialize is
    // logged and skipped rather than aborting the whole demo — pads and LEDs below
    // don't depend on either screen.
    let left_html = include_str!("../../screens/mk3_pad_matrix.html");
    let right_html = include_str!("../../screens/mk3_group_status.html");

    let left_view = match ScreenView::new(
        &encdr, device_id, "left",
        ScreenContent::Html(left_html.to_string()), visible,
    ) {
        Ok(v) => { println!("Left Screen WebView initialized"); Some(v) }
        Err(e) => { eprintln!("Left Screen WebView failed: {}", e); None }
    };

    let right_view = match ScreenView::new(
        &encdr, device_id, "right",
        ScreenContent::Html(right_html.to_string()), visible,
    ) {
        Ok(v) => { println!("Right Screen WebView initialized"); Some(v) }
        Err(e) => { eprintln!("Right Screen WebView failed: {}", e); None }
    };

    println!("Pads, Group buttons, and WebViews active. Tap pads or group buttons to interact.");
    println!("Press Ctrl+C to quit.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();

    let mut pressed_pads = [false; 16];
    let mut pad_pressures = [0.0f32; 16];
    let mut pressed_groups = [false; 8];
    let mut last_pad_hit = [Instant::now() - Duration::from_secs(10); 16];
    let mut last_group_hit = [Instant::now() - Duration::from_secs(10); 8];
    let mut last_logged_pad_lit = [false; 16];
    let mut last_logged_grp_lit = [false; 8];
    let mut last_rendered_pad_states = [false; 16];
    let mut last_rendered_group_states = [false; 8];
    let mut event_log: VecDeque<LogEvent> = VecDeque::with_capacity(10);
    let mut last_pad_event = String::from("AWAITING INPUT...");

    let mut last_screen_render = Instant::now();
    let mut last_led_update = Instant::now();
    let mut next_screen_to_render = 0;
    let mut dirty_left = true;
    let mut dirty_right = true;
    let mut dirty_leds = true;

    const MIN_FLASH_DURATION: Duration = Duration::from_millis(150);

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        // ── PRIORITY 1: Drain ALL pending HID events ──────────────────────
        // This must run first and fast — no blocking calls between here and
        // the LED update below. Events are already parsed on the device
        // thread and waiting in the crossbeam channel.
        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    eprintln!("[{:.4}s][APP-RECV] Button: {} pressed={}", elapsed, name, pressed);
                    if let Some(idx_str) = name.strip_prefix("pad_") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if (1..=16).contains(&idx) {
                                pressed_pads[idx - 1] = pressed;
                                if pressed {
                                    last_pad_hit[idx - 1] = Instant::now();
                                } else {
                                    pad_pressures[idx - 1] = 0.0;
                                }
                                dirty_leds = true;
                                dirty_left = true;
                            }
                        }
                        let state = if pressed { "HIT" } else { "RELEASED" };
                        let msg = format!("{}: {}", name.to_uppercase(), state);
                        last_pad_event = msg.clone();
                        event_log.push_front(LogEvent {
                            time: elapsed,
                            text: msg,
                        });
                        if event_log.len() > 7 {
                            event_log.pop_back();
                        }
                        dirty_right = true;
                    } else if name.starts_with("group_") {
                        if let Some(c) = name.chars().last() {
                            let grp_idx = (c as u8).saturating_sub(b'a') as usize;
                            if grp_idx < 8 {
                                pressed_groups[grp_idx] = pressed;
                                if pressed {
                                    last_group_hit[grp_idx] = Instant::now();
                                }
                                dirty_leds = true;
                            }
                        }
                        let msg = format!("{} {}", name.to_uppercase(), if pressed { "PRESS" } else { "RELEASE" });
                        event_log.push_front(LogEvent {
                            time: elapsed,
                            text: msg,
                        });
                        if event_log.len() > 7 {
                            event_log.pop_back();
                        }
                        dirty_right = true;
                    }
                }
                Event::Grid { name, pressure, .. } => {
                    if let Some(idx_str) = name.strip_prefix("pad_") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if (1..=16).contains(&idx) {
                                let old_p = pad_pressures[idx - 1];
                                pad_pressures[idx - 1] = pressure;
                                if (pressure - old_p).abs() > 0.03 {
                                    dirty_left = true;
                                }
                                if pressure == 0.0 || (pressure - old_p).abs() > 0.2 {
                                    eprintln!("[{:.4}s][APP-RECV] Grid: {} pressure={:.2}", elapsed, name, pressure);
                                }
                                if pressure > 0.0 {
                                    last_pad_event = format!("{}: {:.0}% PRESSURE", name.to_uppercase(), pressure * 100.0);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Compute visual states (held or within minimum flash duration for quick taps)
        let visual_pad_states: [bool; 16] = std::array::from_fn(|i| {
            pressed_pads[i] || last_pad_hit[i].elapsed() < MIN_FLASH_DURATION
        });
        if visual_pad_states != last_rendered_pad_states {
            dirty_left = true;
        }

        let visual_group_states: [bool; 8] = std::array::from_fn(|i| {
            pressed_groups[i] || last_group_hit[i].elapsed() < MIN_FLASH_DURATION
        });
        if visual_group_states != last_rendered_group_states {
            dirty_right = true;
        }

        // Trigger immediate LED refresh if any pad or group flash state changed
        if visual_pad_states.iter().zip(&last_logged_pad_lit).any(|(v, l)| *v != *l)
            || visual_group_states.iter().zip(&last_logged_grp_lit).any(|(v, l)| *v != *l)
        {
            dirty_leds = true;
        }

        // Calculate hues for this frame
        let mut pad_hues = [0.0f32; 16];
        for i in 1..=16 {
            pad_hues[i - 1] = (elapsed * 25.0 + (i as f32) * 22.5) % 360.0;
        }

        let mut group_hues = [0.0f32; 8];
        for i in 0..8 {
            group_hues[i] = (elapsed * 35.0 + (i as f32) * 45.0) % 360.0;
        }

        // ── PRIORITY 2: LED updates (~30 FPS or instant on state change) ──
        // These are non-blocking channel sends — the LED thread does
        // the actual USB interrupt OUT transfer independently.
        if dirty_leds || last_led_update.elapsed() >= Duration::from_millis(33) {
            dirty_leds = false;
            last_led_update = Instant::now();

            // Update Pad LEDs
            for i in 1..=16 {
                let pad_name = format!("pad_{}", i);
                let is_lit = visual_pad_states[i - 1];
                if is_lit != last_logged_pad_lit[i - 1] {
                    last_logged_pad_lit[i - 1] = is_lit;
                    eprintln!(
                        "[{:.4}s][APP-LED] Pad {} ({}) -> {} (pressed={}, hit_age={:.1}ms)",
                        elapsed,
                        i,
                        pad_name,
                        if is_lit { "WHITE" } else { "RAINBOW" },
                        pressed_pads[i - 1],
                        last_pad_hit[i - 1].elapsed().as_secs_f32() * 1000.0
                    );
                }
                if is_lit {
                    // Flash white at full brightness when pressed or tapped
                    encdr.set_led(device_id, &pad_name, LedValue::Single(0x47));
                } else {
                    let hue = pad_hues[i - 1];
                    let (r, g, b) = hsv_to_rgb(hue, 0.7, 0.35); // Gentle pastel
                    encdr.set_led(device_id, &pad_name, LedValue::Rgb { r, g, b });
                }
            }

            // Update Group A-H LEDs with cycling rainbow colors
            for (i, &grp_name) in GROUP_NAMES.iter().enumerate() {
                let is_lit = visual_group_states[i];
                if is_lit != last_logged_grp_lit[i] {
                    last_logged_grp_lit[i] = is_lit;
                    eprintln!(
                        "[{:.4}s][APP-LED] Group {} ({}) -> {} (pressed={})",
                        elapsed,
                        i,
                        grp_name,
                        if is_lit { "WHITE" } else { "RAINBOW" },
                        pressed_groups[i]
                    );
                }
                if is_lit {
                    // Flash white when pressed or tapped
                    encdr.set_led(device_id, grp_name, LedValue::Single(0x47));
                } else {
                    let hue = group_hues[i];
                    let (r, g, b) = hsv_to_rgb(hue, 0.85, 0.45);
                    encdr.set_led(device_id, grp_name, LedValue::Rgb { r, g, b });
                }
            }
        }

        // ── PRIORITY 3: Screen rendering (lower priority, paced) ──────────
        // WebKit snapshot capture is expensive (~10-50ms). We alternate between
        // screens and enforce a minimum pause between captures so snapshotting
        // never monopolizes the main loop or starves pad events and LEDs.
        if (dirty_left || dirty_right) && last_screen_render.elapsed() >= Duration::from_millis(80) {
            if next_screen_to_render == 0 {
                if dirty_left {
                    dirty_left = false;
                    last_rendered_pad_states = visual_pad_states;
                    if let Some(view) = &left_view {
                        let left_data = serde_json::json!({
                            "pad_states": visual_pad_states,
                            "pad_pressures": pad_pressures,
                            "pad_hues": pad_hues,
                            "last_event": last_pad_event,
                        });
                        view.send("pads", left_data);
                        view.poll(&encdr);
                        let _ = view.capture_and_submit(&encdr);
                    }
                }
                next_screen_to_render = 1;
            } else {
                if dirty_right {
                    dirty_right = false;
                    last_rendered_group_states = visual_group_states;
                    if let Some(view) = &right_view {
                        let right_data = serde_json::json!({
                            "group_states": visual_group_states,
                            "group_hues": group_hues,
                            "events": event_log.iter().cloned().collect::<Vec<_>>(),
                        });
                        view.send("telemetry", right_data);
                        view.poll(&encdr);
                        let _ = view.capture_and_submit(&encdr);
                    }
                }
                next_screen_to_render = 0;
            }
            last_screen_render = Instant::now();
        }

        // Pump pending GTK events on every loop iteration
        ScreenView::pump_events();

        std::thread::sleep(Duration::from_millis(1));
    }
}
