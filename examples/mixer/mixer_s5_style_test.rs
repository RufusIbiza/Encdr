//! Test S5-style mixer packet (106-byte buffer, 0x82 prefix) on S8 hardware.
//! Probe offset ranges for mixer LEDs. S5 uses ~107 byte packets per prefix.
//! Run with: cargo run -p encdr-view --example mixer_s5_style_test

use encdr::{Encdr, EncdrConfig};
use std::io::{self, Write};
use std::fs::File;
use serde_json::{json, to_string_pretty};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    println!("=== S5-Style Mixer Packet Test (0x82, 106-byte buffer) ===\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");
    if ids.is_empty() {
        eprintln!("No devices found.");
        return;
    }

    let device_id = ids[0];
    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected: {}\n", desc.name);

    // Common mixer LED offset ranges based on S8/S5 patterns
    let test_offsets = vec![
        ("Cue A", 0, "Single LED"),
        ("Cue B", 1, "Single LED"),
        ("Cue C", 2, "Single LED"),
        ("Cue D", 3, "Single LED"),
        ("Filter A", 4, "Single LED"),
        ("Filter B", 5, "Single LED"),
        ("Filter C", 6, "Single LED"),
        ("Filter D", 7, "Single LED"),
        ("FX A1", 10, "Single LED"),
        ("FX A2", 11, "Single LED"),
        ("FX B1", 12, "Single LED"),
        ("FX B2", 13, "Single LED"),
        ("FX C1", 14, "Single LED"),
        ("FX C2", 15, "Single LED"),
        ("FX D1", 16, "Single LED"),
        ("FX D2", 17, "Single LED"),
        ("Deck A", 20, "Single LED"),
        ("Deck B", 21, "Single LED"),
        ("Deck C", 22, "Single LED"),
        ("Deck D", 23, "Single LED"),
        ("VU A (strip start)", 30, "25-byte strip"),
        ("VU B (strip start)", 55, "25-byte strip"),
        ("VU C (strip start)", 80, "25-byte strip"),
    ];

    println!("Testing offset ranges with 0x82 prefix (106-byte buffer):\n");

    let mut confirmed = Vec::new();

    // Test first 10 offsets for quick feedback
    for (name, offset, type_str) in test_offsets.iter().take(25) {
        println!("Testing offset {}: {} ({})", offset, name, type_str);
        print!("  Light up? (y/n): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if input.trim().to_lowercase() == "y" {
            println!("  ✓ Confirmed!");
            confirmed.push((*name, *offset, *type_str));
        } else {
            println!("  ✗ No response");
        }

        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // Save results
    if !confirmed.is_empty() {
        println!("\n{:-^80}", " Results ");
        println!("\nConfirmed offsets for 0x82 mixer prefix (106-byte buffer):\n");

        let mut results = Vec::new();
        for (name, offset, type_str) in &confirmed {
            println!("  Offset {:3}: {} ({})", offset, name, type_str);
            results.push(json!({
                "name": name,
                "offset": offset,
                "type": type_str
            }));
        }

        let output = json!({
            "device": desc.name,
            "prefix": "0x82",
            "buffer_size": 106,
            "confirmed_leds": results,
            "notes": "Use these offsets to update mixer LED group in descriptor"
        });

        if let Ok(json_str) = to_string_pretty(&output) {
            if let Ok(mut f) = File::create("mixer_s5_results.json") {
                let _ = f.write_all(json_str.as_bytes());
                println!("\n✓ Results saved to mixer_s5_results.json");
            }
        }
    } else {
        println!("\nNo offsets responded. 0x82 with 106-byte buffer may not be the right approach.");
        println!("Consider trying:");
        println!("  - Smaller buffer sizes (< 100 bytes)");
        println!("  - Different prefix bytes");
        println!("  - Different packet structure");
    }
}
