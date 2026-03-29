use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

/// The splash screen accidentally sent blit data to the LED endpoint (0x03)
/// and lit up pad 1 + first touchstrip LED. Let's replicate and explore.
#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Blit-to-LED Replication Test                            ║");
    println!("║                                                            ║");
    println!("║  Replicating the accidental touchstrip LED activation      ║");
    println!("║  caused by sending screen blit data to LED endpoint 0x03   ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    // The blit header that was accidentally sent
    let header: Vec<u8> = vec![
        0x84, 0x00, 0x00, 0x60, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0xE0, 0x01, 0x10,
        0x00, 0x00, 0xFF, 0x00,
    ];
    let footer: Vec<u8> = vec![0x03, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00];

    // Test 1: Send just the 20-byte blit header via interrupt_out
    print!("Test 1: Send 20-byte blit header via interrupt_out to ep 0x03 [ENTER]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    match interface.interrupt_out(0x03, header.clone()).await.into_result() {
        Ok(_) => println!("  Sent OK"),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 2: Send a 309-byte buffer with blit header as the first 20 bytes
    print!("\nTest 2: 309-byte buffer starting with blit header via interrupt_out [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let mut buf = vec![0u8; 309];
    buf[..20].copy_from_slice(&header);
    match interface.interrupt_out(0x03, buf).await.into_result() {
        Ok(_) => println!("  Sent OK"),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 3: Send prefix 0x80 with byte 18 (offset 17) = 0xFF — that's where the FF was in the header
    print!("\nTest 3: prefix 0x80, offset 17 = 255 (where 0xFF was in blit header) [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let mut buf = vec![0u8; 309];
    buf[0] = 0x80;
    buf[18] = 0xFF; // offset 17 in LED buffer = byte index 18
    match interface.interrupt_out(0x03, buf).await.into_result() {
        Ok(_) => println!("  Sent OK"),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 4: Try prefix 0x84 — that's what the blit header byte[0] was
    print!("\nTest 4: prefix 0x84, all touchstrip offsets (93-117) = 255 [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let mut buf = vec![0u8; 309];
    buf[0] = 0x84;
    for i in 93..=117 {
        buf[1 + i] = 255;
    }
    match interface.interrupt_out(0x03, buf).await.into_result() {
        Ok(_) => println!("  Sent OK"),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 5: Replicate EXACT splash — build full blit buffer and send via bulk_out (how encdr did it)
    print!("\nTest 5: Full 261148-byte blit buffer via bulk_out to ep 0x03 (exact replication) [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let pixel_data = vec![0u8; 480 * 272 * 2]; // 261120 bytes
    let mut blit_buf = Vec::with_capacity(header.len() + pixel_data.len() + footer.len());
    blit_buf.extend_from_slice(&header);
    blit_buf.extend_from_slice(&pixel_data);
    blit_buf.extend_from_slice(&footer);
    println!("  Sending {} bytes via bulk_out...", blit_buf.len());
    match interface.bulk_out(0x03, blit_buf).await.into_result() {
        Ok(_) => println!("  Sent OK via bulk_out!"),
        Err(e) => println!("  bulk_out error: {} (trying interrupt_out with first 309 bytes...)", e),
    }

    // Test 6: Send the blit buffer but only the first 309 bytes via interrupt_out
    print!("\nTest 6: First 309 bytes of blit buffer via interrupt_out [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let mut first_309 = vec![0u8; 309];
    first_309[..20].copy_from_slice(&header);
    // rest is zeros (blank pixel data)
    match interface.interrupt_out(0x03, first_309).await.into_result() {
        Ok(_) => println!("  Sent OK"),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 7: Maybe it's about sending data to the SCREEN interface but via ep 0x03?
    // Claim interface 6 (screen) and try sending LED-like data there
    print!("\nTest 7: Claim interface 6 (screen) and send touchstrip data via bulk_out to ep 0x04 [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let screen_iface = device.detach_and_claim_interface(6)?;
    let mut buf = vec![0u8; 309];
    buf[0] = 0x80;
    for i in 93..=117 {
        buf[1 + i] = 255;
    }
    match screen_iface.bulk_out(0x04, buf).await.into_result() {
        Ok(_) => println!("  Sent OK on screen interface"),
        Err(e) => println!("  Error: {}", e),
    }

    // Clear LEDs
    print!("\nClear all LEDs [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let buf = vec![0u8; 309];
    interface.interrupt_out(0x03, buf).await.into_result()?;
    println!("  Cleared.");

    println!("\n✓ Done!");
    Ok(())
}
