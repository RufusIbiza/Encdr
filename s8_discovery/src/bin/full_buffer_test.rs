use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Full Buffer Test - Testing Touchstrip at Documented Offsets ║");
    println!("║                                                            ║");
    println!("║  Based on Traktor decompile:                              ║");
    println!("║  LEFT touchstrip: offsets 93-117 on prefix 0x80           ║");
    println!("║  RIGHT touchstrip: offsets 211-235 on prefix 0x81         ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    println!("Testing LEFT touchstrip with full 309-byte buffer...\n");

    // Test each left touchstrip LED one by one
    for led_index in 0..25 {
        let offset = 93 + led_index;
        print!("LED {}: offset {} - Press ENTER to test (or 'q' to quit): ", led_index, offset);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().eq_ignore_ascii_case("q") {
            println!("\nSwitching to RIGHT touchstrip...");
            break;
        }

        // Send full buffer with this offset lit
        test_offset_full_buffer(0x80, offset, 255).await?;
    }

    println!("\nTesting RIGHT touchstrip with full 309-byte buffer...\n");

    // Test each right touchstrip LED one by one
    for led_index in 0..25 {
        let offset = 211 + led_index;
        print!("LED {}: offset {} - Press ENTER to test (or 'q' to quit): ", led_index, offset);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().eq_ignore_ascii_case("q") {
            break;
        }

        // Send full buffer with this offset lit on the right deck prefix
        test_offset_full_buffer(0x81, offset - 118, 255).await?;
    }

    // Turn off
    println!("\nTurning off all LEDs...");
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;
    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;
    let buffer = vec![0u8; 309];
    interface.interrupt_out(0x03, buffer).await.into_result()?;

    println!("✓ Done!");
    Ok(())
}

async fn test_offset_full_buffer(prefix: u8, offset: usize, brightness: u8) -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    // Create a full 309-byte buffer
    let mut buffer = vec![0u8; 309];
    buffer[0] = prefix;

    // Set the LED at the specified offset
    if offset < 308 {
        buffer[1 + offset] = brightness;
    }

    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}
