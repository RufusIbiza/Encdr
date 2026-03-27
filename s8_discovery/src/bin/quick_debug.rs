use anyhow::{anyhow, Result};
use nusb::transfer::RequestBuffer;
use std::io::{self, Write};
use std::time::Duration;

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║         S8 Quick Interactive Debug                         ║");
    println!("║                                                            ║");
    println!("║  Test different interfaces/endpoints in real-time          ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Find S8
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info: &nusb::DeviceInfo| {
            info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID
        })
        .ok_or_else(|| anyhow!("S8 not found (VID:PID {:04x}:{:04x})", S8_VENDOR_ID, S8_PRODUCT_ID))?;

    println!("✓ Found S8: {}", device_info.product_string().unwrap_or("S8"));
    println!("  Bus: {}, Device: {}\n", device_info.bus_number(), device_info.device_address());

    let device = device_info.open()?;

    println!("Available interfaces on S8:");
    println!("  0-3: Audio interfaces (skip)");
    println!("  4:   Application interface (unassigned)");
    println!("  5:   HID control interface ← TRY THIS");
    println!("  6:   Vendor-specific (firmware)\n");

    loop {
        print!("\nEnter interface number (0-6, or 'q' to quit): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input == "q" {
            break;
        }

        let iface_num: u8 = match input.parse() {
            Ok(n) => n,
            Err(_) => {
                println!("Invalid input");
                continue;
            }
        };

        if iface_num > 6 {
            println!("Interface number must be 0-6");
            continue;
        }

        match device.detach_and_claim_interface(iface_num) {
            Ok(interface) => {
                println!("✓ Claimed interface {}\n", iface_num);

                println!("Testing endpoints (type 'skip' to skip, 'back' to go back):\n");

                for ep in &[0x81u8, 0x82, 0x83, 0x84, 0x85] {
                    print!("  Endpoint 0x{:02x}: ", ep);
                    io::stdout().flush()?;

                    // Do 3 reads
                    for attempt in 0..3 {
                        match tokio::time::timeout(
                            Duration::from_millis(600),
                            read_interrupt(&interface, *ep),
                        )
                        .await
                        {
                            Ok(Ok(data)) if !data.is_empty() => {
                                println!("\n    ✓ Attempt {}: {} bytes", attempt + 1, data.len());
                                println!("      Hex: {}", hex_preview(&data, 32));
                                println!("      ASCII: {}", ascii_preview(&data, 32));
                                break;
                            }
                            Ok(Ok(_)) => print!("."),
                            Ok(Err(_)) => print!("x"),
                            Err(_) => print!("T"),
                        }
                        io::stdout().flush()?;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    println!();
                }
            }
            Err(e) => {
                println!("✗ Cannot claim interface {}: {}\n", iface_num, e);
            }
        }
    }

    println!("\n✓ Done. Based on results:");
    println!("  - Which interface returned data?");
    println!("  - Which endpoint had consistent 64-byte packets?");
    println!("  - Update s8_discovery/src/main.rs with the correct values");

    Ok(())
}

fn hex_preview(data: &[u8], max: usize) -> String {
    let len = std::cmp::min(data.len(), max);
    data[..len]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn ascii_preview(data: &[u8], max: usize) -> String {
    let len = std::cmp::min(data.len(), max);
    data[..len]
        .iter()
        .map(|b| {
            if *b >= 32 && *b <= 126 {
                *b as char
            } else {
                '.'
            }
        })
        .collect()
}

async fn read_interrupt(interface: &nusb::Interface, endpoint: u8) -> Result<Vec<u8>> {
    let buf = RequestBuffer::new(256);
    interface
        .interrupt_in(endpoint, buf)
        .await
        .into_result()
        .map_err(|e| anyhow!("{}", e))
}
