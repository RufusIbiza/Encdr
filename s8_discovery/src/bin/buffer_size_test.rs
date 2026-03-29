use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Buffer Size Test - Testing Different Report Sizes        ║");
    println!("║                                                            ║");
    println!("║  Testing touchstrip offset 93 with different buffer sizes  ║");
    println!("║  to see if the hardware requires a specific report length  ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let sizes = vec![
        (119, "119 bytes (1 prefix + 118 data — one segment)"),
        (107, "107 bytes (S5 LIGHTS_SIZE+1 = 106+1)"),
        (123, "123 bytes (D2 LEDS_SIZE+1 = 122+1)"),
        (237, "237 bytes (1 prefix + 236 data — two segments)"),
        (309, "309 bytes (full buffer — what we've been using)"),
        (64,  "64 bytes (minimum USB HID report)"),
        (128, "128 bytes"),
    ];

    for (size, desc) in &sizes {
        print!("Testing offset 93 with buffer size {} — {} [ENTER]: ", size, desc);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim().eq_ignore_ascii_case("q") {
            break;
        }

        send_with_size(0x80, 93, 255, *size).await?;
    }

    // Also test: send ALL three prefix reports like S5 does
    println!("\n--- Now testing: send ALL THREE prefix reports like S5 ---");
    print!("Sending 0x80 (119 bytes, offset 93=255), then 0x81 (119 bytes), then 0x82 (119 bytes) [ENTER]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    // Send all three reports in sequence
    send_with_size(0x80, 93, 255, 119).await?;
    send_with_size(0x81, 0, 0, 119).await?;
    send_with_size(0x82, 0, 0, 119).await?;
    println!("  Sent all three prefix reports.");

    // Try again with D2 size (123)
    print!("\nSending all three reports with D2 size (123 bytes each) [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    send_with_size(0x80, 93, 255, 123).await?;
    send_with_size(0x81, 0, 0, 123).await?;
    send_with_size(0x82, 0, 0, 123).await?;
    println!("  Sent all three prefix reports (123 bytes each).");

    println!("\n✓ Done!");
    Ok(())
}

async fn send_with_size(prefix: u8, offset: usize, brightness: u8, size: usize) -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    let mut buffer = vec![0u8; size];
    buffer[0] = prefix;

    if offset > 0 && offset < size - 1 {
        buffer[1 + offset] = brightness;
    }

    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}
