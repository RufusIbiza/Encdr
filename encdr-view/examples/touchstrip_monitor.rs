//! Touchstrip position and touch event monitor.
//!
//! Run with: cargo run -p encdr-view --example touchstrip_monitor
//!
//! Prints all touchstrip events to console in real-time.

use encdr::{Encdr, EncdrConfig, Event};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");

    println!("=== Touchstrip Monitor ===");
    println!("Scanning for devices...\n");

    let ids = encdr.scan().expect("Scan failed");
    if ids.is_empty() {
        eprintln!("No devices found. Is the S8 plugged in?");
        return;
    }

    let device_id = ids[0];
    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected: {} ({} controls)\n", desc.name, desc.control_count());

    println!("Listening for touchstrip events...");
    println!("Move/touch either touchstrip and watch the output below.\n");
    println!("Logging to: /tmp/touchstrip_monitor.log\n");
    println!("{}", "=".repeat(80));

    // Clear log file at startup
    let _ = std::fs::write("/tmp/touchstrip_monitor.log", "=== Touchstrip Monitor Log ===\n");

    let events = encdr.events().clone();
    let mut last_values: HashMap<String, (f32, bool)> = HashMap::new();

    loop {
        while let Ok(event) = events.try_recv() {
            match &event {
                Event::Slider { name, value, .. } => {
                    if name.contains("touchstrip") {
                        let key = name.to_string();
                        let prev = last_values.get(&key).map(|(v, _)| *v).unwrap_or(-1.0);

                        if (prev - value).abs() > 0.001 {
                            let msg = format!(
                                "[SLIDER] {:<30} position: {:.4}  (raw: {:.0})",
                                name,
                                value,
                                value * 1024.0
                            );
                            println!("{}", msg);
                            let _ = writeln!(
                                OpenOptions::new()
                                    .append(true)
                                    .open("/tmp/touchstrip_monitor.log")
                                    .unwrap(),
                                "{}",
                                msg
                            );
                            last_values.insert(key.clone(), (*value, false));
                        }
                    }
                }
                Event::Touch { name, touched, .. } => {
                    if name.contains("touchstrip") {
                        let key = name.to_string();
                        let prev = last_values.get(&key).map(|(_, t)| *t).unwrap_or(false);

                        if *touched != prev {
                            let msg = format!("[TOUCH]  {:<30} {}", name, if *touched { "PRESSED" } else { "RELEASED" });
                            println!("{}", msg);
                            let _ = writeln!(
                                OpenOptions::new()
                                    .append(true)
                                    .open("/tmp/touchstrip_monitor.log")
                                    .unwrap(),
                                "{}",
                                msg
                            );
                            last_values.insert(key.clone(), (0.0, *touched));
                        }
                    }
                }
                Event::Button { name, pressed, .. } => {
                    // Show any buttons that fire when we touch strips (to detect interference)
                    if *pressed && !name.contains("pad") && !name.contains("screen") {
                        let msg = format!("[BUTTON] {:<30} PRESSED (possible interference!)", name);
                        println!("{}", msg);
                        let _ = writeln!(
                            OpenOptions::new()
                                .append(true)
                                .open("/tmp/touchstrip_monitor.log")
                                .unwrap(),
                            "{}",
                            msg
                        );
                    }
                }
                _ => {}
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
