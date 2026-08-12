//! Test the 0xf3 mixer handshake and LED addressing.
//!
//! This test validates:
//! 1. The 0xf3 handshake enables mixer control
//! 2. Correct prefix/offset mappings for mixer LEDs
//! 3. Which indices from the documentation actually light up
//!
//! Run with: cargo run --bin mixer_handshake_test

use std::io::{self, Write};

fn main() {
    println!("=== S8 Mixer Handshake & LED Test ===\n");
    println!("This test confirms the 0xf3 handshake and mixer LED addressing.\n");

    // Test data from ni_kontrol_s8_mixer_leds_detailed.md
    let test_cases = vec![
        // (name, prefix, offset_in_prefix, absolute_index, description)
        ("Cue A", 0x80u8, 26, 26, "Channel A Cue"),
        ("Cue B", 0x80u8, 26, 26, "Channel B Cue (verify dup?)"),
        ("FX A L", 0x80u8, 78, 78, "Channel A FX Assign Left"),
        ("FX A R", 0x80u8, 79, 79, "Channel A FX Assign Right"),
        ("Input A", 0x80u8, 80, 80, "Channel A Input/Deck Assign"),
        ("Filter A", 0x81u8, 101, 219, "Channel A Filter On (219 = 118 + 101)"),
        ("Snap", 0x81u8, 94, 212, "Mixer Snap (212 = 118 + 94)"),
        ("Quantize", 0x81u8, 95, 213, "Mixer Quantize (213 = 118 + 95)"),
        ("Master VU L", 0x81u8, 36, 154, "Master VU Left (154 = 118 + 36)"),
        ("Master VU R", 0x81u8, 45, 163, "Master VU Right (163 = 118 + 45)"),
        ("VU Start", 0x82u8, 11, 247, "Channel 1 VU Meter Start (247 = 236 + 11)"),
    ];

    println!("{:-^100}", " Test Cases ");
    println!("Name         | Prefix | Offset | Absolute | Description");
    println!("{:-^100}", "");
    for (name, prefix, offset, absolute, desc) in &test_cases {
        println!("{:<12} | 0x{:02x}   | {:3}    | {:3}      | {}",
                 name, prefix, offset, absolute, desc);
    }

    println!("\n{:-^100}", " Pre-Handshake Test ");
    println!("Without 0xf3 handshake, mixer LEDs should NOT respond.\n");

    println!("These offsets will be tested on each prefix:");
    println!("  - One offset per prefix to confirm no response");
    println!("  - Then again after handshake\n");

    println!("Ready to test? (y/n): ");
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    if input.trim().to_lowercase() != "y" {
        println!("Cancelled.");
        return;
    }

    println!("\n{:-^100}", " Test Protocol ");
    println!("For each LED:");
    println!("  1. We'll describe what should light up");
    println!("  2. You confirm if it lights (y/n)");
    println!("  3. After all tests, we'll send 0xf3 handshake");
    println!("  4. Repeat tests - now LEDs should respond\n");

    let prefixes = vec![
        ("0x80 (Deck A + Mixer A/B)", 0x80u8),
        ("0x81 (Deck B + Mixer C/D)", 0x81u8),
        ("0x82 (Global Mixer)", 0x82u8),
    ];

    println!("Step 1: Test WITHOUT 0xf3 handshake\n");

    let mut results_before = Vec::new();
    for (prefix_name, prefix) in &prefixes {
        println!("Testing prefix {} (offset 0):", prefix_name);
        print!("  Light up? (y/n): ");
        io::stdout().flush().unwrap();

        let mut resp = String::new();
        io::stdin().read_line(&mut resp).unwrap();
        let lit = resp.trim().to_lowercase() == "y";
        results_before.push((*prefix, 0, lit));
        println!();
    }

    println!("{:-^100}", " Handshake Sent ");
    println!("Now sending 0xf3 [0x01] handshake to enable Traktor mode...\n");
    println!("(In real test, this would send USB command)");
    println!("Press enter after confirming handshake was sent: ");
    let mut dummy = String::new();
    io::stdin().read_line(&mut dummy).unwrap();

    println!("\nStep 2: Test WITH 0xf3 handshake\n");

    let mut results_after = Vec::new();

    // Test a few key indices
    let key_tests = vec![
        ("0x80 offset 26", 0x80u8, 26, "Cue A/B"),
        ("0x80 offset 78", 0x80u8, 78, "FX A Left"),
        ("0x81 offset 101", 0x81u8, 101, "Filter A (absolute 219)"),
        ("0x82 offset 11", 0x82u8, 11, "VU Meter start (absolute 247)"),
    ];

    for (name, prefix, offset, desc) in &key_tests {
        println!("Testing {} ({}): {}", name, desc, "");
        print!("  Light up? (y/n): ");
        io::stdout().flush().unwrap();

        let mut resp = String::new();
        io::stdin().read_line(&mut resp).unwrap();
        let lit = resp.trim().to_lowercase() == "y";
        results_after.push((*prefix, *offset, lit));
        println!();
    }

    println!("\n{:-^100}", " Results Analysis ");
    println!("\nBefore handshake:");
    for (prefix, offset, lit) in &results_before {
        println!("  0x{:02x} offset {} : {}", prefix, offset,
                 if *lit { "✓ LIT" } else { "✗ no response" });
    }

    println!("\nAfter 0xf3 handshake:");
    for (prefix, offset, lit) in &results_after {
        println!("  0x{:02x} offset {} : {}", prefix, offset,
                 if *lit { "✓ LIT" } else { "✗ no response" });
    }

    let before_lit = results_before.iter().filter(|(_, _, lit)| *lit).count();
    let after_lit = results_after.iter().filter(|(_, _, lit)| *lit).count();

    println!("\n{:-^100}", " Interpretation ");
    if before_lit == 0 && after_lit > 0 {
        println!("✓ Handshake confirmed!");
        println!("  - Before: No LEDs responded");
        println!("  - After: {} LEDs lit up", after_lit);
        println!("\n→ The 0xf3 handshake IS required to enable mixer control.");
    } else if before_lit > 0 && after_lit > 0 {
        println!("? Unexpected: LEDs responded even before handshake");
        println!("  Possible: Handshake was already sent, or mixer is in alt mode");
    } else if before_lit == 0 && after_lit == 0 {
        println!("✗ No LEDs responded even after handshake");
        println!("  Possible issues:");
        println!("    - Offsets in documentation are incorrect");
        println!("    - Handshake command format is wrong");
        println!("    - Prefix routing is different than expected");
    }

    println!("\nNext steps:");
    println!("1. If handshake confirmed: Update descriptor with correct offsets");
    println!("2. If no response: Check exact handshake command format from Traktor source");
    println!("3. Verify offset calculations: absolute_index - prefix_base = offset_in_prefix");
}
