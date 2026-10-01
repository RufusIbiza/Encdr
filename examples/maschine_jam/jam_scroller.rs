//! NI Maschine Jam scrolling text & animated touchstrip LED meter demo.
//!
//! Features:
//! - Scrolls "Encdr" from right to left across the 8x8 Click-Pad matrix in purple on an orange background.
//! - Shows animated scrolling sine waves across the 8 Smart Strip 11-segment LED meters at the bottom,
//!   moving horizontally at the exact same speed as the text.
//! - Full interactive control feedback:
//!   - Pressing any pad or button flashes it and prints the event to the console.
//!   - Turning the endless rotary encoder adjusts the scroll speed in real time.
//!   - Pressing the encoder toggles scroll direction (right-to-left vs left-to-right).
//!   - Touching or sliding any Smart Strip logs touch positions and interacts with the meters.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin jam_scroller

use std::collections::HashMap;
use std::time::{Duration, Instant};

use encdr::{Encdr, EncdrConfig, Event, LedValue};

// ── Color Definitions ────────────────────────────────────────────────────────

const COLOR_PURPLE: LedValue = LedValue::Rgb {
    r: 160,
    g: 0,
    b: 255,
};

const COLOR_ORANGE: LedValue = LedValue::Rgb {
    r: 255,
    g: 90,
    b: 0,
};

const COLOR_WHITE: LedValue = LedValue::Single(0x47); // NI palette full-intensity white

// ── 8-Row Bitmap Font for "Encdr" ────────────────────────────────────────────
//
// 8 rows tall (Row 0 = top pad row 1, Row 7 = bottom pad row 8).
// Each column is represented as an 8-bit mask (bit 0 = Row 0, bit 7 = Row 7).

fn build_text_columns() -> Vec<u8> {
    // 8 rows: bit 0 (top) .. bit 7 (bottom)
    // Row 0: .
    // Row 1: E E E E       .       .       d       .
    // Row 2: E             .       .       d       .
    // Row 3: E E E     n n n     c c c   d d d   r r r
    // Row 4: E         n   n   c         d   d   r
    // Row 5: E         n   n   c         d   d   r
    // Row 6: E E E E   n   n     c c c   d d d   r
    // Row 7: .

    // Helper to build a column from 8 row bits
    let col = |r0: u8, r1: u8, r2: u8, r3: u8, r4: u8, r5: u8, r6: u8, r7: u8| -> u8 {
        (r0 & 1)
            | ((r1 & 1) << 1)
            | ((r2 & 1) << 2)
            | ((r3 & 1) << 3)
            | ((r4 & 1) << 4)
            | ((r5 & 1) << 5)
            | ((r6 & 1) << 6)
            | ((r7 & 1) << 7)
    };

    let mut cols = Vec::new();

    // Initial padding (8 blank columns so text enters from the right)
    cols.extend_from_slice(&[0; 8]);

    // 'E' (4 cols)
    cols.push(col(0, 1, 1, 1, 1, 1, 1, 0));
    cols.push(col(0, 1, 0, 1, 0, 0, 1, 0));
    cols.push(col(0, 1, 0, 1, 0, 0, 1, 0));
    cols.push(col(0, 1, 0, 0, 0, 0, 1, 0));

    // space (1 col)
    cols.push(0);

    // 'n' (4 cols)
    cols.push(col(0, 0, 0, 1, 1, 1, 1, 0));
    cols.push(col(0, 0, 0, 1, 0, 0, 0, 0));
    cols.push(col(0, 0, 0, 1, 0, 0, 0, 0));
    cols.push(col(0, 0, 0, 1, 1, 1, 1, 0));

    // space (1 col)
    cols.push(0);

    // 'c' (3 cols)
    cols.push(col(0, 0, 0, 0, 1, 1, 0, 0));
    cols.push(col(0, 0, 0, 1, 0, 0, 1, 0));
    cols.push(col(0, 0, 0, 1, 0, 0, 1, 0));

    // space (1 col)
    cols.push(0);

    // 'd' (4 cols)
    cols.push(col(0, 0, 0, 0, 1, 1, 0, 0));
    cols.push(col(0, 0, 0, 1, 0, 0, 1, 0));
    cols.push(col(0, 0, 0, 1, 0, 0, 1, 0));
    cols.push(col(0, 1, 1, 1, 1, 1, 1, 0));

    // space (1 col)
    cols.push(0);

    // 'r' (3 cols)
    cols.push(col(0, 0, 0, 1, 1, 1, 1, 0));
    cols.push(col(0, 0, 0, 1, 0, 0, 0, 0));
    cols.push(col(0, 0, 0, 1, 0, 0, 0, 0));

    // Trailing padding (8 blank columns before repeating)
    cols.extend_from_slice(&[0; 8]);

    cols
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".parse().unwrap()),
        )
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       NI Maschine Jam — Encdr Text & Sine Waves Demo         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");

    let ids = encdr.scan().expect("Device scan failed");
    let device_id = ids
        .into_iter()
        .find(|&id| {
            encdr
                .device_descriptor(id)
                .map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1500)
                .unwrap_or(false)
        })
        .unwrap_or_else(|| {
            eprintln!("❌ NI Maschine Jam (0x17cc:0x1500) not found.");
            eprintln!("   Please check the USB connection and udev rules.");
            std::process::exit(1);
        });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}", desc.control_count());
    println!();
    println!("Controls:");
    println!("  • Pad Matrix   : 8x8 scrolling \"Encdr\" banner (purple text on orange background)");
    println!("  • Touchstrips  : 8 bottom meters showing matching animated sine waves");
    println!("  • Encoder      : Turn to change scroll speed, Press to reverse direction");
    println!("  • Interactive  : Press any button / touch any strip to view real-time feedback");
    println!("  • Exit         : Press Ctrl+C to quit\n");
    println!("{}", "─".repeat(64));

    let events = encdr.events().clone();
    let text_bitmap = build_text_columns();
    let total_columns = text_bitmap.len() as f32;

    // Animation state
    let start_time = Instant::now();
    let mut last_frame = Instant::now();
    let mut scroll_pos: f32 = 0.0;
    let mut scroll_speed: f32 = 6.0; // columns per second
    let mut scroll_direction: f32 = 1.0; // 1.0 = right to left, -1.0 = left to right

    // Controller input states for interactive feedback
    let mut pressed_pads = [[false; 8]; 8]; // [row 0..7][col 0..7]
    let mut pressed_buttons: HashMap<String, bool> = HashMap::new();
    let mut strip_positions = [0.0f32; 8];
    let mut strip_touched = [false; 8];

    // Frame throttling
    let mut last_led_update = Instant::now();
    const TARGET_FRAME_DURATION: Duration = Duration::from_millis(25); // ~40 FPS

    loop {
        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f32();
        last_frame = now;
        let elapsed = start_time.elapsed().as_secs_f32();

        // ── 1. Process all incoming hardware events ──────────────────────────
        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    pressed_buttons.insert(name.to_string(), pressed);

                    // Check if it is a matrix pad (matrix_1_1 .. matrix_8_8)
                    if let Some(rest) = name.strip_prefix("matrix_") {
                        let parts: Vec<&str> = rest.split('_').collect();
                        if parts.len() == 2 {
                            if let (Ok(r), Ok(c)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                                if (1..=8).contains(&r) && (1..=8).contains(&c) {
                                    pressed_pads[r - 1][c - 1] = pressed;
                                    println!(
                                        "[{:8.3}s] [PAD] Row {}, Col {} ({}) -> {}",
                                        elapsed,
                                        r,
                                        c,
                                        name,
                                        if pressed { "\x1b[1;32mPRESSED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" }
                                    );
                                    continue;
                                }
                            }
                        }
                    }

                    // Check if it is an encoder press / touch
                    if name == "encoder_press" {
                        if pressed {
                            scroll_direction = -scroll_direction;
                            println!(
                                "[{:8.3}s] [ENCODER] Press -> Reversed scroll direction (dir: {})",
                                elapsed,
                                if scroll_direction > 0.0 { "Right → Left" } else { "Left → Right" }
                            );
                        }
                        continue;
                    } else if name == "encoder_touch" {
                        println!(
                            "[{:8.3}s] [ENCODER] Capacitive Touch -> {}",
                            elapsed,
                            if pressed { "\x1b[1;36mTOUCHED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" }
                        );
                        continue;
                    }

                    // Check if it is a Group button (group_a .. group_h)
                    if name.starts_with("group_") {
                        println!(
                            "[{:8.3}s] [GROUP] {} -> {}",
                            elapsed,
                            name.to_uppercase(),
                            if pressed { "\x1b[1;35mPRESSED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" }
                        );
                        // Provide instant visual LED feedback for the group button
                        encdr.set_led(
                            device_id,
                            name,
                            if pressed { COLOR_WHITE } else { LedValue::Off },
                        );
                        continue;
                    }

                    // Check if it is a Top numbered button (top_1 .. top_8)
                    if name.starts_with("top_") {
                        println!(
                            "[{:8.3}s] [TOP BUTTON] {} -> {}",
                            elapsed,
                            name.to_uppercase(),
                            if pressed { "\x1b[1;34mPRESSED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" }
                        );
                        encdr.set_led(
                            device_id,
                            name,
                            if pressed { COLOR_WHITE } else { LedValue::Off },
                        );
                        continue;
                    }

                    // All other buttons (transport, modes, d-pad, etc.)
                    println!(
                        "[{:8.3}s] [BUTTON] {} -> {}",
                        elapsed,
                        name.to_uppercase(),
                        if pressed { "\x1b[1;33mPRESSED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" }
                    );

                    // Light up the single LED for function buttons while pressed
                    encdr.set_led(
                        device_id,
                        name,
                        if pressed { LedValue::Single(127) } else { LedValue::Off },
                    );
                }

                Event::Encoder { name, delta, .. } => {
                    scroll_speed = (scroll_speed + (delta as f32) * 0.5).clamp(1.0, 30.0);
                    println!(
                        "[{:8.3}s] [ENCODER] {} rotated delta: {:+2} | New Scroll Speed: {:.1} cols/sec",
                        elapsed, name, delta, scroll_speed
                    );
                }

                Event::Slider { name, value, .. } => {
                    if let Some(num_str) = name.strip_prefix("touchstrip_") {
                        // Check if primary or dual
                        let is_dual = num_str.ends_with("_dual");
                        let strip_str = num_str.trim_end_matches("_dual");
                        if let Ok(idx) = strip_str.parse::<usize>() {
                            if (1..=8).contains(&idx) {
                                if !is_dual {
                                    strip_positions[idx - 1] = value;
                                    println!(
                                        "[{:8.3}s] [TOUCHSTRIP {}] Position: {:5.1}% (raw: {:.3})",
                                        elapsed,
                                        idx,
                                        value * 100.0,
                                        value
                                    );
                                } else {
                                    println!(
                                        "[{:8.3}s] [TOUCHSTRIP {} DUAL] Position: {:5.1}%",
                                        elapsed,
                                        idx,
                                        value * 100.0
                                    );
                                }
                            }
                        }
                    }
                }

                Event::Touch { name, touched, .. } => {
                    if let Some(num_str) = name.strip_prefix("touchstrip_") {
                        let strip_str = num_str.trim_end_matches("_touch");
                        if let Ok(idx) = strip_str.parse::<usize>() {
                            if (1..=8).contains(&idx) {
                                strip_touched[idx - 1] = touched;
                                println!(
                                    "[{:8.3}s] [TOUCHSTRIP {} TOUCH] -> {}",
                                    elapsed,
                                    idx,
                                    if touched { "\x1b[1;36mCONTACT\x1b[0m" } else { "\x1b[2mRELEASE\x1b[0m" }
                                );
                            }
                        }
                    }
                }

                Event::DeviceDisconnected { .. } => {
                    eprintln!("\n⚠️ Device disconnected.");
                    return;
                }

                _ => {}
            }
        }

        // ── 2. Advance scroll position ───────────────────────────────────────
        scroll_pos += scroll_speed * scroll_direction * dt;
        while scroll_pos < 0.0 {
            scroll_pos += total_columns;
        }
        while scroll_pos >= total_columns {
            scroll_pos -= total_columns;
        }

        // ── 3. Update LEDs at target frame rate ──────────────────────────────
        if last_led_update.elapsed() >= TARGET_FRAME_DURATION {
            last_led_update = Instant::now();

            let base_col_idx = scroll_pos.floor() as usize;

            // Update 8x8 Pad Matrix
            for col in 0..8 {
                // Determine sample column in the scrolling text bitmap
                let text_col = (base_col_idx + col) % (text_bitmap.len());
                let col_mask = text_bitmap[text_col];

                for row in 0..8 {
                    let pad_name = format!("matrix_{}_{}", row + 1, col + 1);

                    // Check if currently physically pressed
                    if pressed_pads[row][col] {
                        encdr.set_led(device_id, &pad_name, COLOR_WHITE);
                    } else {
                        // Check if pixel in character mask is set
                        let is_text = (col_mask & (1 << row)) != 0;
                        let color = if is_text { COLOR_PURPLE } else { COLOR_ORANGE };
                        encdr.set_led(device_id, &pad_name, color);
                    }
                }
            }

            // Update 8 Smart Strip 11-Segment LED Meters with Scrolling Sine Waves
            //
            // Spatial wavelength: lambda = 8 columns (one complete wave fits the 8 strips).
            // Wave moves at the exact same horizontal speed/direction as the text:
            // Phase = (col + scroll_pos) / 8.0 * 2 * PI
            const WAVELENGTH: f32 = 8.0;
            const SEGMENTS_COUNT: usize = 11;

            for col in 0..8 {
                let strip_name = format!("touchstrip_meter_{}", col + 1);

                // Compute sine wave phase matching text scroll
                let x = (col as f32) + scroll_pos;
                let phase = (x / WAVELENGTH) * 2.0 * std::f32::consts::PI;
                let sine_val = phase.sin(); // -1.0 .. 1.0

                // Normalize to [0.0 .. 1.0], then scale to [0.0 .. 11.0]
                let normalized = 0.5 + 0.5 * sine_val;
                let target_level = normalized * (SEGMENTS_COUNT as f32);

                let mut segments = [0u8; SEGMENTS_COUNT];

                // If user is currently touching this strip, blend touch position with the wave
                if strip_touched[col] {
                    let touch_level = strip_positions[col] * (SEGMENTS_COUNT as f32);
                    for (s, seg) in segments.iter_mut().enumerate() {
                        let seg_f = s as f32;
                        if seg_f + 1.0 <= touch_level {
                            *seg = 127;
                        } else if seg_f < touch_level {
                            *seg = ((touch_level - seg_f) * 127.0).round() as u8;
                        } else {
                            *seg = 0;
                        }
                    }
                } else {
                    // Render smooth anti-aliased sine bar
                    for (s, seg) in segments.iter_mut().enumerate() {
                        let seg_f = s as f32;
                        if seg_f + 1.0 <= target_level {
                            *seg = 127; // Full brightness
                        } else if seg_f < target_level {
                            // Smoothly illuminate the top segment
                            *seg = ((target_level - seg_f) * 127.0).round().clamp(10.0, 127.0) as u8;
                        } else {
                            *seg = 0;
                        }
                    }
                }

                encdr.set_led_strip(device_id, &strip_name, &segments);
            }
        }

        // Small sleep to prevent busy spinning while keeping sub-millisecond input responsiveness
        std::thread::sleep(Duration::from_millis(1));
    }
}
