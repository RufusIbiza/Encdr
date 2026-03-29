use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║            S8 LED Mapper - Interactive Offset Test         ║");
    println!("║                                                            ║");
    println!("║  Enter offset and prefix to light up individual LEDs.      ║");
    println!("║  Report which physical LED lights up.                      ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    println!("Prefixes:");
    println!("  L = 0x80 (LEFT deck)");
    println!("  R = 0x81 (RIGHT deck)");
    println!("  M = 0x82 (MIXER)\n");

    loop {
        print!("Enter offset (0-308) and prefix [L/R/M] (e.g., '59L' or 'q' to quit): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input == "q" {
            break;
        }

        // Parse input like "59L" or "59 L"
        let input = input.replace(" ", "");
        if input.len() < 2 {
            println!("Invalid input. Use format: <offset><prefix> (e.g., 59L)\n");
            continue;
        }

        let offset_str = &input[..input.len() - 1];
        let prefix_char = input.chars().last().unwrap();

        let offset: usize = match offset_str.parse() {
            Ok(n) => n,
            Err(_) => {
                println!("Invalid offset\n");
                continue;
            }
        };

        if offset > 308 {
            println!("Offset must be 0-308\n");
            continue;
        }

        let prefix = match prefix_char.to_ascii_uppercase().to_string().as_str() {
            "L" => 0x80u8,
            "R" => 0x81u8,
            "M" => 0x82u8,
            _ => {
                println!("Invalid prefix. Use L, R, or M\n");
                continue;
            }
        };

        let prefix_name = match prefix {
            0x80 => "LEFT (0x80)",
            0x81 => "RIGHT (0x81)",
            0x82 => "MIXER (0x82)",
            _ => "UNKNOWN",
        };

        println!("\n  → Testing offset {} on {} (brightness 200/255)", offset, prefix_name);

        // Create LED data with single offset set
        let mut data = vec![0u8; 308];
        data[offset] = 200;

        match send_led(prefix, &data).await {
            Ok(_) => {
                println!("  ✓ LED lit. Which physical LED is it?");
                println!("    (e.g., 'left pad 1', 'left play button', 'loop light', etc.)\n");
            }
            Err(e) => {
                println!("  ✗ Error: {}\n", e);
            }
        }
    }

    println!("\n✓ Done!");
    Ok(())
}

async fn send_led(prefix: u8, data: &[u8]) -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    let mut buffer = vec![0u8; 309];
    buffer[0] = prefix;
    let copy_len = std::cmp::min(data.len(), 308);
    buffer[1..1+copy_len].copy_from_slice(&data[..copy_len]);

    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}
