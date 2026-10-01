//! NI Traktor Kontrol S5 interactive Vegas LED demo & dual-screen telemetry.
//!
//! Run with:
//!   cargo run -p encdr-examples --bin s5_vegas

use std::time::{Duration, Instant};
use encdr::{Encdr, EncdrConfig, Event, LedValue, PixelFormat};

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let hp = (h % 360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8)
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".parse().unwrap()))
        .init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       NI Traktor Kontrol S5 — Vegas Mode & Telemetry         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Scanning for connected devices...\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");

    let device_id = ids.into_iter().find(|&id| {
        encdr.device_descriptor(id).map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1420).unwrap_or(false)
    }).unwrap_or_else(|| {
        eprintln!("❌ NI Traktor Kontrol S5 (0x17cc:0x1420) not found.");
        std::process::exit(1);
    });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!(" Connected: {} (VID:PID 0x{:04x}:0x{:04x})", desc.name, desc.vendor_id.0, desc.product_id.0);
    println!(" Controls available: {}\n", desc.control_count());
    println!("Slide touchstrips, move faders, or hit RGB pads to interact.\n");

    let events = encdr.events().clone();
    let start_time = Instant::now();
    let mut last_led_update = Instant::now();
    let mut last_screen_update = Instant::now();

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();

        while let Ok(event) = events.try_recv() {
            match event {
                Event::Button { name, pressed, .. } => {
                    println!("[{:8.3}s] [BUTTON] {} -> {}", elapsed, name.to_uppercase(), if pressed { "\x1b[1;32mPRESSED\x1b[0m" } else { "\x1b[2mRELEASED\x1b[0m" });
                }
                Event::Slider { name, value, .. } => {
                    println!("[{:8.3}s] [ANALOG] {} = {:5.1}%", elapsed, name.to_uppercase(), value * 100.0);
                }
                Event::Encoder { name, delta, .. } => {
                    println!("[{:8.3}s] [ENCODER] {} delta: {:+2}", elapsed, name.to_uppercase(), delta);
                }
                Event::DeviceDisconnected { .. } => return,
                _ => {}
            }
        }

        // Animate 16 RGB pads across Decks A and B
        if last_led_update.elapsed() >= Duration::from_millis(30) {
            last_led_update = Instant::now();

            for i in 1..=8 {
                let pad_l = format!("left_pad_{}", i);
                let pad_r = format!("right_pad_{}", i);
                let hue_l = (elapsed * 60.0 + (i as f32) * 45.0) % 360.0;
                let hue_r = (elapsed * 60.0 + (i as f32 + 4.0) * 45.0) % 360.0;
                let (rl, gl, bl) = hsv_to_rgb(hue_l, 1.0, 0.8);
                let (rr, gr, br) = hsv_to_rgb(hue_r, 1.0, 0.8);
                encdr.set_led(device_id, &pad_l, LedValue::Rgb { r: rl, g: gl, b: bl });
                encdr.set_led(device_id, &pad_r, LedValue::Rgb { r: rr, g: gr, b: br });
            }
        }

        // Render animated color gradients to both 480x272 displays
        if last_screen_update.elapsed() >= Duration::from_millis(50) {
            last_screen_update = Instant::now();
            let mut left_pixels = vec![0u8; 480 * 272 * 2];
            let mut right_pixels = vec![0u8; 480 * 272 * 2];

            let hue_l = (elapsed * 40.0) % 360.0;
            let hue_r = (elapsed * 40.0 + 180.0) % 360.0;
            let (rl, gl, bl) = hsv_to_rgb(hue_l, 1.0, 0.8);
            let (rr, gr, br) = hsv_to_rgb(hue_r, 1.0, 0.8);

            let bgr_l = (((bl >> 3) as u16) << 11) | (((gl >> 2) as u16) << 5) | ((rl >> 3) as u16);
            let bgr_r = (((br >> 3) as u16) << 11) | (((gr >> 2) as u16) << 5) | ((rr >> 3) as u16);

            for chunk in left_pixels.chunks_exact_mut(2) {
                chunk[0] = (bgr_l >> 8) as u8;
                chunk[1] = bgr_l as u8;
            }
            for chunk in right_pixels.chunks_exact_mut(2) {
                chunk[0] = (bgr_r >> 8) as u8;
                chunk[1] = bgr_r as u8;
            }

            encdr.submit_screen_with_format(device_id, "left", &left_pixels, PixelFormat::Bgr565Be);
            encdr.submit_screen_with_format(device_id, "right", &right_pixels, PixelFormat::Bgr565Be);
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
