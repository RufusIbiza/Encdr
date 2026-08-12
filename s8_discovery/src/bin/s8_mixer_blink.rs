use anyhow::{anyhow, Result};
use std::time::Duration;
use nusb::transfer::RequestBuffer;

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

struct S8Controller {
    interface: nusb::Interface,
}

impl S8Controller {
    async fn new() -> Result<Self> {
        let devices = nusb::list_devices()?;
        let device_info = devices
            .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
            .ok_or_else(|| anyhow!("S8 device not found"))?;

        let device = device_info.open()?;
        let interface = device.detach_and_claim_interface(5)?; // HID control interface
        
        Ok(S8Controller { interface })
    }

    async fn send_handshake(&self) -> Result<()> {
        println!("Sending handshake 0xf3 [0x01]...");
        // Prefix 0xf3, offset 1, value 1
        let buffer = vec![0xf3, 0x01]; 
        self.interface.interrupt_out(0x03, buffer).await.into_result()?;
        Ok(())
    }

    async fn set_led(&self, prefix: u8, index: usize, value: u8) -> Result<()> {
        // Traktor uses a 310-byte report for LED segments
        // [ReportID, Prefix Header, ...308 data bytes...]
        let mut buffer = vec![0u8; 310];
        buffer[0] = prefix;
        buffer[1] = prefix;
        
        // The index is absolute within the 309-LED stream, 
        // but segments usually handle relative offsets.
        // However, based on mixer_test.rs, we use the absolute index
        // and the device seems to know which segment it belongs to.
        if index < 308 {
            buffer[2 + index] = value;
        }
        
        self.interface.interrupt_out(0x03, buffer).await.into_result()?;
        Ok(())
    }

    async fn clear_all(&self) -> Result<()> {
        for p in &[0x80u8, 0x81, 0x82] {
            let mut buffer = vec![0u8; 310];
            buffer[0] = *p;
            buffer[1] = *p;
            self.interface.interrupt_out(0x03, buffer).await.into_result()?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════╗");
    println!("║ NI Kontrol S8 Mixer LED Discovery Tool ║");
    println!("╚════════════════════════════════════════╝\n");

    let s8 = S8Controller::new().await?;

    // 1. Handshake
    s8.send_handshake().await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 2. Clear
    println!("Clearing all LEDs...");
    s8.clear_all().await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 3. Define the LEDs we found
    let mixer_leds = vec![
        ("Snap", 0x81, 212),
        ("Quantize", 0x81, 213),
        
        ("Cue A", 0x80, 25), // 0x19
        ("Cue B", 0x80, 26), // 0x1a
        ("Cue C", 0x80, 56), // 0x38
        ("Cue D", 0x80, 60), // 0x3c
        
        ("Filter On A", 0x81, 218), // 0xda
        ("Filter On B", 0x81, 219), // 0xdb
        ("Filter On C", 0x81, 220), // 0xdc
        ("Filter On D", 0x81, 221), // 0xdd
        
        ("FX Assign 1.1", 0x82, 46), // Absolute 282 (0x11a) - 236
        ("FX Assign 1.2", 0x82, 47), // Absolute 283 (0x11b) - 236
        
        ("Deck Input A", 0x80, 80),  // 0x50
        
        ("Master VU L Start", 0x81, 154), // 0x9a
        ("Master VU R Start", 0x81, 163), // 0xa3
    ];

    println!("Blinking found mixer LEDs...");
    for (name, prefix, index) in &mixer_leds {
        println!("  Testing {} (Prefix 0x{:02x}, Index {})...", name, prefix, index);
        s8.set_led(*prefix, *index, 127).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        s8.set_led(*prefix, *index, 0).await?;
    }

    // 4. VU Meter Special Sequence
    println!("\nTesting Channel 1 VU Sequence (Prefix 0x82, Start 247)...");
    let vu_seq = vec![247, 248, 249, 250, 251, 252, 253, 254, 255, 256, 257, 258, 259, 260, 261];
    for &idx in &vu_seq {
        s8.set_led(0x82, idx - 236, 127).await?; // Adjusting for 0x82 base
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
    for &idx in &vu_seq {
        s8.set_led(0x82, idx - 236, 0).await?;
    }

    println!("\nDone.");
    Ok(())
}
