use anyhow::{anyhow, Result};
use std::time::Duration;

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔═══════════════════════════════╗");
    println!("║ S8 Targeted Mixer LED Test    ║");
    println!("╚═══════════════════════════════╝\n");

    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    println!("1. Sending Handshake (0xf3 [0x01])...");
    send_raw(&interface, 0xf3, &[0x01]).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("2. Attempting to CLEAR all LEDs for 0x80, 0x81, 0x82...");
    // Clear using the 310-byte structure which worked for decks
    for p in &[0x80u8, 0x81, 0x82] {
        send_raw(&interface, *p, &vec![*p; 1 + 308]).await?; // [ID, ID, 0, 0...]
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    println!("3. Testing Channel 1 Cue (Prefix 0x80, Offset 26)...");
    println!("   Trying 310-byte structure...");
    send_led(&interface, 0x80, 26, 127).await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    send_led(&interface, 0x80, 26, 0).await?;

    println!("3. Testing Channel 1 Filter On (Prefix 0x81, Offset 219)...");
    send_led(&interface, 0x81, 219, 127).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    send_led(&interface, 0x81, 219, 0).await?;

    println!("4. Testing Mixer Snap (Prefix 0x81, Offset 212)...");
    send_led(&interface, 0x81, 212, 127).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    send_led(&interface, 0x81, 212, 0).await?;

    println!("5. Testing Master VU Left Start (Prefix 0x81, Offset 154)...");
    send_led(&interface, 0x81, 154, 127).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    send_led(&interface, 0x81, 154, 0).await?;

    println!("6. Testing Channel 1 VU Start (Prefix 0x82, Offset 247)...");
    send_led(&interface, 0x82, 247, 127).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    send_led(&interface, 0x82, 247, 0).await?;

    println!("\nTest complete.");
    Ok(())
}

async fn send_raw(interface: &nusb::Interface, prefix: u8, data: &[u8]) -> Result<()> {
    let mut buffer = vec![0u8; 1 + data.len()];
    buffer[0] = prefix;
    buffer[1..].copy_from_slice(data);
    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}

async fn send_led(interface: &nusb::Interface, prefix: u8, offset: usize, value: u8) -> Result<()> {
    // 310 bytes: Report ID (0) + Prefix (1) + 308 data
    let mut buffer = vec![0u8; 310];
    buffer[0] = prefix; // Report ID
    buffer[1] = prefix; // Prefix in data (Header)
    if offset < 308 {
        buffer[2 + offset] = value;
    }
    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}
