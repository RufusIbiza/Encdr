//! NI Maschine Mk3 partial screen update test.
//!
//! Draws a static gradient on both screens and moves one small box on the left
//! screen, so each frame differs from the last in a small area only. Run with
//! `RUST_LOG=encdr=debug` and watch the "Sending N bytes" lines: a full frame
//! is ~261 KB, a partial update far less.
//!
//! The pad and button LEDs are set once at the start and never again. Press the
//! controller's instance button to hand the hardware to NI's software and back:
//! the screens and LEDs should be redrawn by themselves on return.
//!
//! Runs for 60 seconds, or the number of seconds given as the first argument.
//!
//! Run with:
//!   RUST_LOG=encdr=debug cargo run -p encdr-examples --bin mk3_partial_test [-- SECONDS]

use std::time::{Duration, Instant};
use encdr::{Encdr, EncdrConfig, LedValue};

const SCREEN_W: usize = 960;
const SCREEN_H: usize = 272;
const BOX: usize = 48;

/// Static gradient background with a red box on the left screen (and a green
/// box on the right screen, stationary) as RGBA8888.
fn frame(t: f32) -> Vec<u8> {
    let mut px = vec![255u8; SCREEN_W * SCREEN_H * 4];
    for y in 0..SCREEN_H {
        for x in 0..SCREEN_W {
            let i = (y * SCREEN_W + x) * 4;
            px[i] = (x * 255 / SCREEN_W) as u8;
            px[i + 1] = (y * 255 / SCREEN_H) as u8;
            px[i + 2] = 96;
        }
    }
    let span_x = 480 - BOX;
    let span_y = SCREEN_H - BOX;
    let bx = ((t * 0.5).fract() * 2.0 - 1.0).abs() * span_x as f32;
    let by = ((t * 0.8).fract() * 2.0 - 1.0).abs() * span_y as f32;
    fill(&mut px, bx as usize, by as usize, [255, 0, 0]);
    fill(&mut px, 480 + 200, 100, [0, 255, 0]);
    px
}

fn fill(px: &mut [u8], x0: usize, y0: usize, rgb: [u8; 3]) {
    for y in y0..y0 + BOX {
        for x in x0..x0 + BOX {
            let i = (y * SCREEN_W + x) * 4;
            px[i..i + 3].copy_from_slice(&rgb);
        }
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".parse().unwrap()))
        .init();

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");
    let device_id = ids
        .into_iter()
        .find(|&id| encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1600).unwrap_or(false))
        .unwrap_or_else(|| {
            eprintln!("NI Maschine Mk3 (0x17cc:0x1600) not found.");
            std::process::exit(1);
        });

    // Static LED state, set once: 16 pads in a colour ramp and a few buttons.
    for i in 0..16 {
        let (r, g, b) = if i % 2 == 0 { (255, 0, 80 + i as u8 * 10) } else { (0, 200, 255 - i as u8 * 10) };
        encdr.set_led(device_id, &format!("pad_{i}"), LedValue::Rgb { r, g, b });
    }
    for button in ["play", "rec", "stop", "browser", "mixer"] {
        encdr.set_led(device_id, button, LedValue::Bright);
    }

    let seconds: u64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(60);
    println!("Moving a box on the left screen for {seconds} seconds. Press the instance button to hand over and back...");
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(seconds) {
        encdr.submit_dual_screen(device_id, "left", "right", &frame(start.elapsed().as_secs_f32()));
        std::thread::sleep(Duration::from_millis(50));
    }
}
