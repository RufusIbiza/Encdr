use anyhow::{anyhow, Result};
use std::io::{self, Write};
use std::time::Duration;

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔═══════════════════════════════╗");
    println!("║ S8 Prefix Scanner (Final)     ║");
    println!("╚═══════════════════════════════╝\n");
    println!("Iterating through Report IDs (0x00-0xFF)...");
    println!("Watch the mixer for ANY activity.\n");

    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    for prefix in 0x00u8..=0xff {
        print!("\rTesting Prefix: 0x{:02x}...", prefix);
        io::stdout().flush()?;

        // Send 'all-on'
        send_all(&interface, prefix, 127).await?;
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Clear it
        send_all(&interface, prefix, 0).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("\nScan complete.");
    Ok(())
}

async fn send_all(interface: &nusb::Interface, prefix: u8, value: u8) -> Result<()> {
    // 309 bytes total: 1-byte Report ID + 308-byte payload
    let mut buffer = vec![value; 309];
    buffer[0] = prefix; // Report ID
    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}
