use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║          S8 Offset Scanner - Find Touchstrip LEDs          ║");
    println!("║                                                            ║");
    println!("║  Tests each offset at full brightness on LEFT deck (0x80)  ║");
    println!("║  Press ENTER to move to next offset.                       ║");
    println!("║  Type the offset number when you find a touchstrip LED.    ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let mut current_offset = 0;
    let max_offset = 308;

    loop {
        print!("Offset {}/308: Testing... [Press ENTER for next, or type offset # to jump]: ", current_offset);
        io::stdout().flush()?;

        // Light up current offset
        test_offset(0x80, current_offset, 255).await?;

        // Wait for user input
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            // Just enter - move to next offset
            current_offset += 1;
            if current_offset > max_offset {
                println!("\n✓ Reached end of buffer. No touchstrip found in this range.");
                break;
            }
        } else if input.eq_ignore_ascii_case("q") {
            // Quit
            println!("\nQuitting...");
            break;
        } else if let Ok(offset) = input.parse::<usize>() {
            // Jump to specific offset
            if offset <= max_offset {
                current_offset = offset;
                println!("  → Jumping to offset {}", offset);
            } else {
                println!("  → Invalid offset (max {})", max_offset);
            }
        } else {
            println!("  → Found touchstrip LED at offset {}!", input);
            println!("\n✓ Got it! Offset: {}", input);
            break;
        }
    }

    // Turn off
    println!("\nTurning off LEDs...");
    test_offset(0x80, 0, 0).await?;

    Ok(())
}

async fn test_offset(prefix: u8, offset: usize, brightness: u8) -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    let mut buffer = vec![0u8; 309];
    buffer[0] = prefix;
    if offset < 308 {
        buffer[1 + offset] = brightness;
    }

    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}
