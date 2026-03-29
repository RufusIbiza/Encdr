use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Touchstrip Prefix Test - D2 Offsets with Different Prefixes ║");
    println!("║                                                            ║");
    println!("║  Testing offsets 68-92 (blue) and 93-117 (orange) with     ║");
    println!("║  prefixes 0x81 (RIGHT) and 0x82 (MIXER)                    ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let prefixes = vec![
        (0x81u8, "RIGHT (0x81)"),
        (0x82u8, "MIXER (0x82)"),
    ];

    for (prefix, prefix_name) in prefixes {
        println!("\n============================================================");
        println!("Testing {} - Blue range (68-92)", prefix_name);
        println!("============================================================");
        test_range(prefix, 68, 92, prefix_name).await?;

        println!("\n============================================================");
        println!("Testing {} - Orange range (93-117)", prefix_name);
        println!("============================================================");
        test_range(prefix, 93, 117, prefix_name).await?;
    }

    // Turn off
    println!("\nTurning off all LEDs...");
    test_offset(0x80, 0, 0).await?;
    test_offset(0x81, 0, 0).await?;
    test_offset(0x82, 0, 0).await?;

    println!("✓ Done!");
    Ok(())
}

async fn test_range(prefix: u8, start: usize, end: usize, prefix_name: &str) -> Result<()> {
    for offset in start..=end {
        print!("Offset {}: ", offset);
        io::stdout().flush()?;

        // Light up this offset
        test_offset(prefix, offset, 255).await?;

        // Wait for user input
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if !input.is_empty() {
            println!("✓✓✓ FOUND TOUCHSTRIP LED! ✓✓✓");
            println!("  Offset {} on {} lights up a touchstrip LED!", offset, prefix_name);
            return Ok(());
        }

        // Turn off before next test
        test_offset(prefix, offset, 0).await?;
    }

    println!("No touchstrip LEDs found in this range on {}", prefix_name);
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
