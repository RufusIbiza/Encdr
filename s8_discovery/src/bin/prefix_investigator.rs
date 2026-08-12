use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    // Default to the range 0xec to 0xf6 (approx 10 before 0xf6)
    let start_hex = args.get(1).cloned().unwrap_or_else(|| "0x01".to_string());
    let end_hex = args.get(2).cloned().unwrap_or_else(|| "0x20".to_string());
    
    let start = u8::from_str_radix(start_hex.trim_start_matches("0x"), 16)?;
    let end = u8::from_str_radix(end_hex.trim_start_matches("0x"), 16)?;

    println!("╔═══════════════════════════════╗");
    println!("║ S8 Targeted Prefix Investigator║");
    println!("╚═══════════════════════════════╝\n");
    println!("Investigating range: 0x{:02x} to 0x{:02x}", start, end);
    println!("Press [Enter] to advance to the next prefix.\n");

    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    for prefix in start..=end {
        println!("--> Current Prefix: 0x{:02x}  [WATCH MIXER]", prefix);
        
        // Turn ALL ON for this prefix (309 bytes: Report ID + 308 data)
        send_all(&interface, prefix, 127).await?;
        
        print!("  (Active) Press Enter for next... ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        // Turn OFF before moving on
        send_all(&interface, prefix, 0).await?;
    }

    println!("\nRange complete.");
    Ok(())
}

async fn send_all(interface: &nusb::Interface, prefix: u8, value: u8) -> Result<()> {
    // 309 bytes total: 1-byte Report ID + 308-byte payload
    let mut buffer = vec![value; 309];
    buffer[0] = prefix; // Report ID
    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}
