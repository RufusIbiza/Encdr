use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

/// Exactly replicate Steyr's D2 LED sending technique for S8.
/// Steyr sends: [0x80] + [122 bytes LED data] = 123 bytes total via interrupt_out.
/// D2 touchstrip orange: bytes 93-117, blue: bytes 68-92 of the 122-byte buffer.
#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Steyr-Style LED Test — D2 buffer format on S8           ║");
    println!("║                                                            ║");
    println!("║  Sends 123 bytes: [prefix] + [122 LED bytes]              ║");
    println!("║  Exactly how Steyr sends LEDs to D2                        ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    // Test 1: D2-style 123-byte buffer, all touchstrip orange LEDs (93-117) full brightness
    print!("Test 1: 123-byte buffer, touchstrip ORANGE (offsets 93-117) all 255 [ENTER]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let mut buf = vec![0u8; 123];
    buf[0] = 0x80;
    for i in 93..=117 {
        buf[1 + i] = 255;
    }
    interface.interrupt_out(0x03, buf).await.into_result()?;
    println!("  Sent.");

    // Test 2: D2-style 123-byte buffer, all touchstrip blue LEDs (68-92) full brightness
    print!("\nTest 2: 123-byte buffer, touchstrip BLUE (offsets 68-92) all 255 [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let mut buf = vec![0u8; 123];
    buf[0] = 0x80;
    for i in 68..=92 {
        buf[1 + i] = 255;
    }
    interface.interrupt_out(0x03, buf).await.into_result()?;
    println!("  Sent.");

    // Test 3: Both blue AND orange at full brightness
    print!("\nTest 3: 123-byte buffer, BOTH blue (68-92) + orange (93-117) all 255 [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let mut buf = vec![0u8; 123];
    buf[0] = 0x80;
    for i in 68..=117 {
        buf[1 + i] = 255;
    }
    interface.interrupt_out(0x03, buf).await.into_result()?;
    println!("  Sent.");

    // Test 4: Same but with prefix 0x81 (right deck)
    print!("\nTest 4: 123-byte buffer, prefix 0x81, orange (93-117) all 255 [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let mut buf = vec![0u8; 123];
    buf[0] = 0x81;
    for i in 93..=117 {
        buf[1 + i] = 255;
    }
    interface.interrupt_out(0x03, buf).await.into_result()?;
    println!("  Sent.");

    // Test 5: Try endpoint 0x01 instead of 0x03 (Steyr uses EP 0x01)
    print!("\nTest 5: 123-byte buffer, prefix 0x80, orange (93-117), endpoint 0x01 [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let mut buf = vec![0u8; 123];
    buf[0] = 0x80;
    for i in 93..=117 {
        buf[1 + i] = 255;
    }
    match interface.interrupt_out(0x01, buf).await.into_result() {
        Ok(_) => println!("  Sent on endpoint 0x01."),
        Err(e) => println!("  Endpoint 0x01 failed: {} — expected, S8 uses 0x03", e),
    }

    // Test 6: 3-report sequence like S5 — send all 3 prefixes, 123 bytes each
    print!("\nTest 6: S5-style 3-report sequence (0x80+0x81+0x82), 123 bytes each, orange (93-117) [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    for prefix in [0x80, 0x81, 0x82] {
        let mut buf = vec![0u8; 123];
        buf[0] = prefix;
        for i in 93..=117 {
            buf[1 + i] = 255;
        }
        interface.interrupt_out(0x03, buf).await.into_result()?;
    }
    println!("  Sent all 3 prefixes.");

    // Test 7: Full 309-byte S8 buffer but also send a known-working LED to confirm hardware is responding
    print!("\nTest 7: 309-byte buffer, prefix 0x80, pad 1 RED (offset 0) + touchstrip orange (93-117) [ENTER]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    let mut buf = vec![0u8; 309];
    buf[0] = 0x80;
    buf[1] = 255; // pad 1 red — known working
    for i in 93..=117 {
        buf[1 + i] = 255;
    }
    interface.interrupt_out(0x03, buf).await.into_result()?;
    println!("  Sent (pad 1 red should light as confirmation).");

    // Clear
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
