//! Probe mixer offsets: lights up each offset at 0x82 prefix (106-byte buffer).
//! First, replace the mixer group in ni_kontrol_s8.json with buffer_size=106 and these test items.
//! Run with: cargo run -p encdr-view --example mixer_offset_probe

use encdr::{Encdr, EncdrConfig, core::led::LedValue};
use std::io::{self, Write};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    println!("=== Mixer Offset Probe (0x82, 106-byte buffer) ===\n");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");
    let ids = encdr.scan().expect("Scan failed");
    if ids.is_empty() {
        eprintln!("No devices found.");
        return;
    }

    let device_id = ids[0];
    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected: {}\n", desc.name);

    println!("Testing mixer (0x82) LED offsets with 106-byte buffer:");
    println!("Each offset will light briefly. Note which ones respond.\n");

    let test_names = vec![
        "test_0", "test_1", "test_2", "test_3", "test_4", "test_5",
        "test_10", "test_20", "test_30", "test_50",
    ];

    let offsets = vec![0, 1, 2, 3, 4, 5, 10, 20, 30, 50];

    println!("{:-^80}", " Testing ");
    println!("Offset | Name      | Result");
    println!("{:-^80}", "");

    let mut hits = Vec::new();

    for (name, offset) in test_names.iter().zip(offsets.iter()) {
        print!("{:3}    | {:<10} | ", offset, name);
        io::stdout().flush().unwrap();

        // Light the LED
        if !desc.leds.iter().any(|group| group.id == "mixer" && group.items.iter().any(|item| item.name() == *name)) {
            println!("(LED not in descriptor - add it!)");
            continue;
        }
        encdr.set_led_in_group(device_id, "mixer", name, LedValue::Single(200));

        std::thread::sleep(std::time::Duration::from_millis(800));

        // Ask user
        print!("lit? (y/n): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if input.trim().to_lowercase() == "y" {
            println!("✓");
            hits.push((*offset, name.to_string()));
        } else {
            println!("✗");
        }

        // Dark
        let _ = encdr.set_led_in_group(device_id, "mixer", name, LedValue::Off);
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    println!("\n{:-^80}", " Results ");
    if !hits.is_empty() {
        println!("\nWorking offsets on 0x82 (106-byte buffer):");
        for (offset, name) in hits {
            println!("  ✓ Offset {}: {}", offset, name);
        }
    } else {
        println!("\n✗ No offsets responded.");
    }
}
