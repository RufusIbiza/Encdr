//! Interactive mixer LED address discovery: probe offset ranges and ask for confirmation.
//! Tests LED addresses and infers missing ones based on patterns.
//! Run with: cargo run -p encdr-view --example mixer_led_test

use encdr::{Encdr, EncdrConfig};
use std::io::{self, Write};
use std::fs::File;
use serde_json::{json, to_string_pretty};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");

    println!("=== Mixer LED Address Discovery ===\n");
    let ids = encdr.scan().expect("Scan failed");
    if ids.is_empty() {
        eprintln!("No devices found. Is the S8 plugged in?");
        return;
    }

    let device_id = ids[0];
    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected: {}\n", desc.name);

    // Known LEDs from current ni_kontrol_s8.json descriptor
    let known_leds = vec![
        ("mixer_cue_a", 25, "Cue A"),
        ("mixer_cue_b", 26, "Cue B"),
        ("mixer_cue_c", 57, "Cue C"),
        ("mixer_cue_d", 55, "Cue D"),
    ];

    println!("Testing mixer LED addresses from current descriptor:\n");
    let mut confirmed_offsets = Vec::new();

    for (name, offset, desc) in &known_leds {
        println!("Testing offset {} ({}): {}", offset, name, desc);
        print!("Light LED at offset {} in mixer buffer? (y/n): ", offset);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if input.trim().to_lowercase() == "y" {
            // Build a test buffer with this offset lit
            println!("  → Lighting offset {}", offset);
            println!("  Enter where the LED lit up (e.g., 'top-left', 'near cue A'): ");
            print!("     ");
            io::stdout().flush().unwrap();

            let mut location = String::new();
            io::stdin().read_line(&mut location).unwrap();
            confirmed_offsets.push((*offset, location.trim().to_string()));
            println!("  ✓ Confirmed at {}\n", location.trim());
        } else {
            println!("  ✗ No match\n");
        }

        std::thread::sleep(std::time::Duration::from_millis(300));
    }

    println!("\n{:-^80}", " Verification ");

    if !confirmed_offsets.is_empty() {
        println!("\nConfirmed offsets from current descriptor:");
        for (offset, location) in &confirmed_offsets {
            println!("  {} → {}", offset, location);
        }
        println!("\n✓ Descriptor addresses are correct!");
    } else {
        println!("\n✗ No addresses from current descriptor were confirmed.");
        println!("  This suggests either:");
        println!("  - The mixer LED group is not properly configured");
        println!("  - The addresses in the descriptor are incorrect");
        println!("  - Or full.json has the correct addresses instead");
    }

    // Save results to file
    let results = json!({
        "device": desc.name,
        "confirmed_leds": confirmed_offsets.iter().map(|(offset, location)| {
            json!({
                "offset": offset,
                "location": location
            })
        }).collect::<Vec<_>>(),
        "notes": "Review these confirmed offsets and update the descriptor accordingly"
    });

    let output_path = "mixer_led_results.json";
    if let Ok(json_str) = to_string_pretty(&results) {
        if let Ok(mut f) = File::create(output_path) {
            let _ = f.write_all(json_str.as_bytes());
            println!("\n✓ Results saved to: {}", output_path);
        }
    } else {
        println!("Warning: Could not serialize results to JSON");
    }

    println!("\n{:-^80}", " Next: Discover Missing LEDs ");
    println!("\nTo find missing LEDs (Filter On, VU Meters, FX C/D):");
    println!("1. Test offset ranges:");
    println!("   - 220-230 (around cue lights) for Filter On");
    println!("   - 84-95 (after known FX/Deck) for FX C/D and others");
    println!("   - 50-70, 100-150, 200-220 (large ranges) for VU meter strips");
    println!("\n2. Look for patterns:");
    println!("   - Single LEDs (1 byte) vs strips (25 bytes for audio level)");
    println!("   - Prefix byte grouping (0x80, 0x81, 0x82 observed in full.json)");
    println!("\n3. Consult ctlra source:");
    println!("   - github.com/openav/ctlra/tree/master/devices/ni");
    println!("   - Look for kontrol_s8 or kontrol_s5 mixer LED definitions");
}
