//! Systematic probe of mixer LED offsets AFTER handshake.
//!
//! Workflow:
//! 1. Send 0xf3 prefix offset 1 = handshake (enables mixer control)
//! 2. Then test offsets on 0x80, 0x81, 0x82 to find LED addresses
//!
//! Run with: cargo run --bin mixer_offset_map

use std::io::{self, Write};

fn main() {
    println!("=== S8 Mixer LED Offset Map (After 0xf3 Handshake) ===\n");

    println!("Step 1: Send 0xf3 handshake");
    println!("  Prefix: 0xf3");
    println!("  Offset: 1");
    println!("  Value: 0x01 (enables Traktor mode)\n");

    println!("Ready? Once handshake is sent, we'll probe for LED offsets.\n");
    println!("Which prefix should we test first? (0x80, 0x81, or 0x82)");
    print!("> ");
    io::stdout().flush().unwrap();

    let mut prefix_input = String::new();
    io::stdin().read_line(&mut prefix_input).unwrap();

    let prefix = match prefix_input.trim() {
        "0x80" | "80" => 0x80u8,
        "0x81" | "81" => 0x81u8,
        "0x82" | "82" => 0x82u8,
        _ => 0x80u8,
    };

    println!("\nSystematically testing offsets 0-30 on 0x{:02x} prefix:\n", prefix);

    println!("For each offset on 0x{:02x}:", prefix);
    println!("  1. Describe which LEDs light up (or none)");
    println!("  2. We'll record the mapping\n");

    println!("Offset map for 0x{:02x} prefix (after 0xf3 handshake):\n", prefix);
    println!("{:<8} | {:<50} | Description", "Offset", "LEDs that lit up?");
    println!("{:-<70}", "");

    let mut results = Vec::new();

    // Test offsets 0-20 systematically
    for offset in 0..=20 {
        print!("{:<8} | ", offset);
        io::stdout().flush().unwrap();

        let mut response = String::new();
        io::stdin().read_line(&mut response).unwrap();

        let desc = response.trim();
        if !desc.is_empty() && desc.to_lowercase() != "n" && desc.to_lowercase() != "no" {
            results.push((offset, desc.to_string()));
            println!("✓ {}", desc);
        } else {
            println!("✗ (no response)");
        }
    }

    println!("\n{:-^70}", " Offset Map Results ");
    println!("\nResponsive offsets on 0x{:02x}:\n", prefix);

    for (offset, desc) in &results {
        println!("  Offset {:2}: {}", offset, desc);
    }

    if results.is_empty() {
        println!("  (No offsets responded on 0x{:02x})", prefix);
        println!("\nTry testing a different prefix (0x80, 0x81, or 0x82)");
    }

    println!("\n{:-^70}", " Next Steps ");
    println!("\nIf offsets responded:");
    println!("  1. Record which prefix(es) have LED controls");
    println!("  2. Map out all responsive offsets across all prefixes");
    println!("  3. Update descriptor with correct addresses");
    println!("\nIf no offsets responded:");
    println!("  1. Try different prefix");
    println!("  2. Or test different value ranges (offsets might need higher values)");

    // Save results
    if !results.is_empty() {
        println!("\nOffsets to add to descriptor:");
        for (offset, desc) in &results {
            println!("  {{ \"name\": \"mixer_group_{}\", \"offset\": {}, \"desc\": \"{}\" }}",
                     offset, offset, desc);
        }
    }
}
