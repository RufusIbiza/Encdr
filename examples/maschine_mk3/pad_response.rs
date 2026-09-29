//! Maschine Mk3 Pad Responsiveness & Aftertouch Demo
//!
//! Tapping a pad highlights the touched pad both physically on the hardware
//! and on the Left display matrix. Holding the pad shows real-time aftertouch
//! pressure as a dot moving left-to-right along the progress bar on the Right
//! display. Releasing the pad turns the LED off and immediately resets the screen.
//!
//! Run with: cargo run --bin mk3_pad_response
//! Show desktop windows: cargo run --bin mk3_pad_response -- --visible

use std::time::{Duration, Instant};

use encdr::{Encdr, EncdrConfig, Event, LedValue};
use encdr_view::{ScreenContent, ScreenView};
use serde_json::json;

#[derive(Clone, Copy, Default)]
struct PadState {
    pressed: bool,
    pressure: f32,
    peak_pressure: f32,
    touch_start: Option<Instant>,
    last_duration_ms: f64,
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

    println!("==================================================");
    println!("  NI Maschine Mk3 Pad Responsiveness & Aftertouch ");
    println!("==================================================");
    println!("Scanning for Maschine Mk3...");

    let ids = encdr.scan().expect("Device scan failed");
    if ids.is_empty() {
        eprintln!("No devices found. Is your Maschine Mk3 plugged in?");
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
            eprintln!("Maschine Mk3 (17cc:1600) not found among connected devices.");
            std::process::exit(1);
        });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected to: {}", desc.name);

    // Turn off all 16 pad LEDs on start
    for i in 1..=16 {
        encdr.set_led(device_id, &format!("pad_{}", i), LedValue::Off);
    }

    // Let USB stabilize
    std::thread::sleep(Duration::from_millis(250));

    // Initialize Left & Right Screen Views
    let left_html = include_str!("../../screens/mk3_pad_response_left.html");
    let right_html = include_str!("../../screens/mk3_pad_response_right.html");

    let left_view = match ScreenView::new(
        &encdr,
        device_id,
        "left",
        ScreenContent::Html(left_html.to_string()),
        visible,
    ) {
        Ok(v) => {
            println!("✓ Left Screen (Pad Matrix) initialized");
            Some(v)
        }
        Err(e) => {
            eprintln!("✗ Left Screen failed: {}", e);
            None
        }
    };

    let right_view = match ScreenView::new(
        &encdr,
        device_id,
        "right",
        ScreenContent::Html(right_html.to_string()),
        visible,
    ) {
        Ok(v) => {
            println!("✓ Right Screen (Aftertouch Monitor) initialized");
            Some(v)
        }
        Err(e) => {
            eprintln!("✗ Right Screen failed: {}", e);
            None
        }
    };

    println!("\nPad Responsiveness Test Active!");
    println!("- Tap/Hold any pad to test touch and pressure response.");
    println!("- Press Ctrl+C to exit.\n");

    let events = encdr.events().clone();
    let mut pads = [PadState::default(); 16];
    let mut active_pad_idx: Option<usize> = None;
    let mut total_touches: u64 = 0;

    let mut dirty_left = true;
    let mut dirty_right = true;
    let mut last_render = Instant::now();

    loop {
        ScreenView::pump_events();

        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    if let Some(idx_str) = name.strip_prefix("pad_") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if (1..=16).contains(&idx) {
                                let i = idx - 1;
                                pads[i].pressed = pressed;

                                if pressed {
                                    total_touches += 1;
                                    pads[i].touch_start = Some(Instant::now());
                                    pads[i].peak_pressure = pads[i].pressure.max(0.2);
                                    active_pad_idx = Some(i);

                                    // Turn Pad LED ON (Bright White/Cyan: 0x47 in NI indexed palette)
                                    encdr.set_led(device_id, &name, LedValue::Single(0x47));
                                    println!("[TOUCH] Pad {:2} PRESSED (Touch #{})", idx, total_touches);
                                } else {
                                    // Turn Pad LED OFF immediately
                                    encdr.set_led(device_id, &name, LedValue::Off);

                                    if let Some(start) = pads[i].touch_start.take() {
                                        pads[i].last_duration_ms = start.elapsed().as_secs_f64() * 1000.0;
                                        println!(
                                            "[RELEASE] Pad {:2} RELEASED (Held: {:.1} ms, Peak: {:.0}%)",
                                            idx,
                                            pads[i].last_duration_ms,
                                            pads[i].peak_pressure * 100.0
                                        );
                                    }
                                    pads[i].pressure = 0.0;

                                    if active_pad_idx == Some(i) {
                                        active_pad_idx = None;
                                    }
                                }
                                dirty_left = true;
                                dirty_right = true;
                            }
                        }
                    }
                }
                Event::Grid { name, pressure, .. } => {
                    if let Some(idx_str) = name.strip_prefix("pad_") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if (1..=16).contains(&idx) {
                                let i = idx - 1;
                                let old_pressure = pads[i].pressure;
                                pads[i].pressure = pressure;

                                if pressure > pads[i].peak_pressure {
                                    pads[i].peak_pressure = pressure;
                                }

                                if (pressure - old_pressure).abs() > 0.015 {
                                    if pressure > 0.0 {
                                        active_pad_idx = Some(i);
                                    }
                                    dirty_left = true;
                                    dirty_right = true;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Send state updates to WebViews and render
        if dirty_left || dirty_right || last_render.elapsed() >= Duration::from_millis(16) {
            // Left view update: Pad matrix state
            if let Some(ref view) = left_view {
                let pad_data: Vec<_> = (0..16)
                    .map(|i| {
                        json!({
                            "active": pads[i].pressed || pads[i].pressure > 0.0,
                            "pressure": pads[i].pressure,
                        })
                    })
                    .collect();
                view.send("pad_state", json!(pad_data));
                ScreenView::pump_events();
                let _ = view.capture_and_submit(&encdr);
                dirty_left = false;
            }

            // Right view update: Aftertouch progress bar and dot position
            if let Some(ref view) = right_view {
                let right_payload = if let Some(i) = active_pad_idx {
                    let duration = pads[i]
                        .touch_start
                        .map(|s| s.elapsed().as_secs_f64() * 1000.0)
                        .unwrap_or(0.0);
                    json!({
                        "active": true,
                        "pad_name": format!("Pad {}", i + 1),
                        "pressure": pads[i].pressure,
                        "peak": pads[i].peak_pressure,
                        "duration_ms": duration,
                        "total_touches": total_touches,
                    })
                } else {
                    json!({
                        "active": false,
                        "pad_name": "No Pad Held",
                        "pressure": 0.0,
                        "peak": 0.0,
                        "duration_ms": 0.0,
                        "total_touches": total_touches,
                    })
                };

                view.send("pressure_state", right_payload);
                ScreenView::pump_events();
                let _ = view.capture_and_submit(&encdr);
                dirty_right = false;
            }

            last_render = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(4));
    }
}
