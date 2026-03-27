use anyhow::{anyhow, Result};
use nusb::transfer::RequestBuffer;
use std::io::Write;
use std::time::Duration;

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║         S8 USB Diagnostic Tool                             ║");
    println!("║                                                            ║");
    println!("║  This tool will help debug USB communication with the S8   ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Find S8
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info: &nusb::DeviceInfo| {
            info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID
        })
        .ok_or_else(|| anyhow!("S8 not found"))?;

    println!("Found: {}", device_info.product_string().unwrap_or("S8"));
    println!("Bus: {}, Device: {}\n", device_info.bus_number(), device_info.device_address());

    let device = device_info.open()?;

    // Try each interface
    println!("Scanning interfaces...\n");
    for iface_num in 0..4 {
        match device.detach_and_claim_interface(iface_num) {
            Ok(interface) => {
                println!("✓ Interface {} claimed", iface_num);

                // Try different endpoints
                for ep in &[0x81u8, 0x82, 0x83, 0x84] {
                    print!("  Trying endpoint 0x{:02x}... ", ep);
                    std::io::stdout().flush().ok();

                    match tokio::time::timeout(
                        Duration::from_millis(500),
                        read_interrupt(&interface, *ep),
                    )
                    .await
                    {
                        Ok(Ok(data)) if !data.is_empty() => {
                            println!("✓ Got {} bytes", data.len());
                            println!("    Data: {}", hex::encode(&data[..std::cmp::min(32, data.len())]));
                        }
                        Ok(Ok(_)) => println!("(no data)"),
                        Ok(Err(e)) => println!("✗ Error: {}", e),
                        Err(_) => println!("(timeout)"),
                    }
                }
                println!();
            }
            Err(e) => {
                println!("✗ Interface {}: {}", iface_num, e);
            }
        }
    }

    println!("\nDiagnostic complete. Check which interface/endpoint returned data.");
    println!("Use that info to update the discovery tool.\n");

    Ok(())
}

async fn read_interrupt(interface: &nusb::Interface, endpoint: u8) -> Result<Vec<u8>> {
    let buf = RequestBuffer::new(256);
    interface
        .interrupt_in(endpoint, buf)
        .await
        .into_result()
        .map_err(|e| anyhow!("{}", e))
}
