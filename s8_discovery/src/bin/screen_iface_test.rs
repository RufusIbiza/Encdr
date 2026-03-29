use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

/// Test sending screen data to different interfaces/endpoints.
/// We know interface 6 / ep 0x04 = left screen.
/// Try interface 3 / ep 0x02 for right screen (the other bulk out endpoint).
#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Screen Interface Test                                    ║");
    println!("║                                                            ║");
    println!("║  Interface 6 (ep 0x04) = left screen (confirmed)           ║");
    println!("║  Testing interface 3 (ep 0x02) for right screen            ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;

    // Build test pixels
    let pixel_size = 480 * 272 * 2;
    let header = [
        0x84u8, 0x00, 0x00, 0x60, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0xE0, 0x01, 0x10,
        0x00, 0x00, 0xFF, 0x00,
    ];
    let footer = [0x03u8, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00];

    // Red pixels
    let mut red = vec![0u8; pixel_size];
    for i in (0..pixel_size).step_by(2) {
        red[i] = 0xF8;
        red[i + 1] = 0x00;
    }

    // Green pixels
    let mut green = vec![0u8; pixel_size];
    for i in (0..pixel_size).step_by(2) {
        green[i] = 0x07;
        green[i + 1] = 0xE0;
    }

    // Blue pixels
    let mut blue = vec![0u8; pixel_size];
    for i in (0..pixel_size).step_by(2) {
        blue[i] = 0x00;
        blue[i + 1] = 0x1F;
    }

    // Test 1: Confirm left screen on interface 6 / ep 0x04
    print!("Test 1: RED to interface 6 / ep 0x04 (left screen - known working) [ENTER]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let iface6 = device.detach_and_claim_interface(6)?;
    send_blit(&iface6, 0x04, &header, &red, &footer).await?;
    println!("  Sent.");

    // Test 2: Try interface 3 / ep 0x02 (MIDI bulk out — maybe it's the right screen?)
    print!("\nTest 2: GREEN to interface 3 / ep 0x02 [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let iface3 = device.detach_and_claim_interface(3)?;
    match send_blit(&iface3, 0x02, &header, &green, &footer).await {
        Ok(_) => println!("  Sent OK!"),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 3: Try ep 0x04 on interface 3
    print!("\nTest 3: BLUE to interface 3 / ep 0x04 [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    match send_blit(&iface3, 0x04, &header, &blue, &footer).await {
        Ok(_) => println!("  Sent OK!"),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 4: Try sending TWO blits to interface 6 / ep 0x04 — first red, then green
    // Maybe the second blit goes to the right screen?
    print!("\nTest 4: Two sequential blits to iface 6 / ep 0x04 (RED then GREEN) [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    send_blit(&iface6, 0x04, &header, &red, &footer).await?;
    send_blit(&iface6, 0x04, &header, &green, &footer).await?;
    println!("  Sent both.");

    // Test 5: Try ep 0x02 on interface 6
    print!("\nTest 5: GREEN to interface 6 / ep 0x02 [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    match send_blit(&iface6, 0x02, &header, &green, &footer).await {
        Ok(_) => println!("  Sent OK!"),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 6: Try interface 5 (HID) / ep 0x03 with blit data (this is what accidentally worked before)
    print!("\nTest 6: GREEN via bulk_out to interface 5 / ep 0x03 (the old accident) [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let iface5 = device.detach_and_claim_interface(5)?;
    match send_blit(&iface5, 0x03, &header, &green, &footer).await {
        Ok(_) => println!("  Sent OK!"),
        Err(e) => println!("  Error: {}", e),
    }

    println!("\n✓ Done!");
    Ok(())
}

async fn send_blit(
    iface: &nusb::Interface,
    ep: u8,
    header: &[u8; 20],
    pixels: &[u8],
    footer: &[u8; 8],
) -> Result<()> {
    let mut buf = Vec::with_capacity(header.len() + pixels.len() + footer.len());
    buf.extend_from_slice(header);
    buf.extend_from_slice(pixels);
    buf.extend_from_slice(footer);
    iface.bulk_out(ep, buf).await.into_result()?;
    Ok(())
}
