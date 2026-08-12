use anyhow::{anyhow, Result};
use std::fs::OpenOptions;
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let prefix_hex = args.get(1).cloned().unwrap_or_else(|| "0xf3".to_string());
    let prefix = u8::from_str_radix(prefix_hex.trim_start_matches("0x"), 16)?;
    let log_file_path = args.get(2).cloned().unwrap_or_else(|| "mixer_discovery_0xf3.log".to_string());

    println!("╔═══════════════════════════════╗");
    println!("║ S8 Mixer LED Discovery (0xf3) ║");
    println!("╚═══════════════════════════════╝\n");
    println!("Prefix:   0x{:02x}", prefix);
    println!("Log file: {}", log_file_path);
    println!("Controls: [Enter] Next, [b] Back, [j] Jump, [q] Quit\n");

    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    let mut log_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&log_file_path)?;

    writeln!(log_file, "--- Starting Discovery for Prefix 0x{:02x} ---", prefix)?;

    let mut current_offset = 0;
    send_all_off(&interface, prefix).await?;

    loop {
        send_led(&interface, prefix, current_offset, 127).await?;
        
        print!("\rOffset: {:3} [Description or Enter] ", current_offset);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let cmd = input.trim();

        send_led(&interface, prefix, current_offset, 0).await?;

        match cmd {
            "q" => break,
            "b" => if current_offset > 0 { current_offset -= 1; },
            "j" => {
                print!("Jump to offset: ");
                io::stdout().flush()?;
                let mut jump_str = String::new();
                io::stdin().read_line(&mut jump_str)?;
                if let Ok(target) = jump_str.trim().parse() {
                    current_offset = target;
                }
            }
            _ => {
                let desc = if cmd.is_empty() { "skip" } else { cmd };
                if !cmd.is_empty() {
                    println!("  Offset {} -> {}", current_offset, desc);
                    writeln!(log_file, "Offset {}: {}", current_offset, desc)?;
                    log_file.flush()?;
                }
                current_offset += 1;
            }
        }
        
        if current_offset > 307 {
            println!("\nEnd of buffer reached.");
            break;
        }
    }

    send_all_off(&interface, prefix).await?;
    println!("\nDiscovery complete. Log saved to {}", log_file_path);
    Ok(())
}

async fn send_led(interface: &nusb::Interface, prefix: u8, offset: usize, value: u8) -> Result<()> {
    // 309 bytes total: 1-byte Report ID + 308-byte payload
    let mut buffer = vec![0u8; 309];
    buffer[0] = prefix; // Report ID
    if offset < 308 {
        buffer[1 + offset] = value;
    }
    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}

async fn send_all_off(interface: &nusb::Interface, prefix: u8) -> Result<()> {
    let mut buffer = vec![0u8; 309];
    buffer[0] = prefix;
    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}
