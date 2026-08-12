//! Test S8 mixer LED addressing based on Traktor decompiled source.
//!
//! Theory: The S8 requires 310-byte packets (1 prefix + 309 data) with LEDs
//! spread across three prefixes as documented in the Traktor source.
//!
//! This test:
//! 1. Sends 0xf3 handshake to enable Traktor mode
//! 2. Tests documented mixer LED indices on each prefix
//! 3. Confirms which addresses light up which physical LEDs
//!
//! Run with: cargo run --bin traktor_mode_test

use std::io::{self, Write};

fn main() {
    println!("=== S8 Traktor Mode LED Test (Based on Decompiled Source) ===\n");

    println!("Theory: S8 uses 310-byte packets (1 prefix + 309 data)");
    println!("with LEDs distributed across three prefixes.\n");

    println!("Documented Mixer LED Indices (from Traktor source):\n");
    println!("Prefix 0x80 (0-117):");
    println!("  - Channel A Deck Input: index 80, offset 80");
    println!();

    println!("Prefix 0x81 (118-235):");
    println!("  - Channel A Cue: index 223, offset 105 (223-118)");
    println!("  - Channel A Filter: index 219, offset 101 (219-118)");
    println!("  - Master VU L: index 154, offset 36 (154-118), count 9");
    println!("  - Master VU R: index 163, offset 45 (163-118), count 9");
    println!();

    println!("Prefix 0x82 (236-308):");
    println!("  - Channel A FX L: index 282, offset 46 (282-236)");
    println!("  - Channel A FX R: index 283, offset 47 (283-236)");
    println!();

    println!("Test Plan:");
    println!("  1. Send 0xf3 handshake to enable Traktor mode");
    println!("  2. Test each documented index with 310-byte packet");
    println!("  3. Record which LEDs respond\n");

    println!("Prerequisites:");
    println!("  - USB device connected and recognized");
    println!("  - Direct USB access (may require libusb or equivalent)\n");

    println!("Ready to test? (y/n): ");
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    if input.trim().to_lowercase() != "y" {
        println!("Cancelled.");
        return;
    }

    println!("\n{:-^100}", " Step 1: Send 0xf3 Handshake ");
    println!("\nCommand to send:");
    println!("  Prefix: 0xf3");
    println!("  Data[0]: 0x01");
    println!("  Remaining: 308 bytes of zeros\n");

    println!("Instructions:");
    println!("  Option A: Use encdr library (if handshake support is added)");
    println!("  Option B: Use libusb tool:");
    println!("    libusb-dev -v 0x17cc:0x1370 -w 0x02 0xf3 0x01 [zeros...]");
    println!("  Option C: Use another USB tool (e.g., pyusb, hidapi)\n");

    println!("Confirm handshake sent: (y/n) ");
    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm).unwrap();
    if confirm.trim().to_lowercase() != "y" {
        println!("Skipped. Cannot proceed without handshake.");
        return;
    }

    println!("\n{:-^100}", " Step 2: Test Documented Indices ");

    let test_cases = vec![
        // (name, prefix, offset, absolute_index, description)
        ("Deck Input A", 0x80u8, 80, 80, "Channel A Deck Input/Assign"),
        ("Cue A", 0x81u8, 105, 223, "Channel A Cue"),
        ("Filter A", 0x81u8, 101, 219, "Channel A Filter On"),
        ("FX A L", 0x82u8, 46, 282, "Channel A FX Assign Left"),
        ("FX A R", 0x82u8, 47, 283, "Channel A FX Assign Right"),
        ("Master VU L (start)", 0x81u8, 36, 154, "Master VU Left (count 9)"),
        ("Master VU R (start)", 0x81u8, 45, 163, "Master VU Right (count 9)"),
    ];

    println!("\nTesting 310-byte packets. For each offset:\n");
    println!("  1. Create packet with 1 prefix byte + 309 data bytes");
    println!("  2. Set the offset to 200 (bright)");
    println!("  3. Rest to 0");
    println!("  4. Send to device");
    println!("  5. Report which LED lit up\n");

    println!("{:<12} | {:<10} | {:<8} | {:<40}", "Name", "Prefix", "Offset", "LED Lit?");
    println!("{:-<80}", "");

    let mut results = Vec::new();

    for (name, prefix, offset, absolute, desc) in test_cases {
        print!("{:<12} | 0x{:02x}     | {:3}    | ", name, prefix, offset);
        io::stdout().flush().unwrap();

        let mut response = String::new();
        io::stdin().read_line(&mut response).unwrap();

        if !response.trim().is_empty() && response.trim().to_lowercase() != "n" {
            println!("✓");
            results.push((name, prefix, offset, absolute, response.trim().to_string()));
        } else {
            println!("✗");
        }
    }

    println!("\n{:-^100}", " Results ");
    println!("\nConfirmed working indices:\n");

    for (name, prefix, offset, absolute, led_desc) in &results {
        println!("  0x{:02x} offset {:3} (index {:3}): {} → {}",
                 prefix, offset, absolute, name, led_desc);
    }

    if results.is_empty() {
        println!("  (No indices responded)");
        println!("\nPossible issues:");
        println!("  - Handshake not properly sent or recognized");
        println!("  - Packet size/format incorrect (must be exactly 310 bytes)");
        println!("  - Index offsets miscalculated from Traktor source");
        println!("  - Different S8 firmware version with different mapping");
    }

    println!("\n{:-^100}", " Next Steps ");
    println!("\nIf results confirmed:");
    println!("  1. Save this mapping");
    println!("  2. Verify with additional indices (other channels A/B/C/D)");
    println!("  3. Update descriptor with confirmed addresses");
    println!("\nIf no results:");
    println!("  1. Verify 0xf3 handshake format");
    println!("  2. Check packet size is exactly 310 bytes");
    println!("  3. Consult Traktor source at /media/rufus/Big Love/Documents/Projects/Traktor Decompile");
}
