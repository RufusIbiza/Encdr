use anyhow::{anyhow, Result};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Testing touchstrip address with different brightness values\n");

    let tests = vec![
        (0, "Off"),
        (50, "Dim"),
        (100, "Medium"),
        (150, "Bright 1"),
        (200, "Bright 2"),
        (255, "Max"),
    ];

    for (brightness, label) in tests {
        println!("Testing offset 93L with brightness {} ({})", brightness, label);
        test_offset(0x80, 93, brightness).await?;
        println!("  → What color is it?\n");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    Ok(())
}

async fn test_offset(prefix: u8, offset: usize, brightness: u8) -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    let mut buffer = vec![0u8; 309];
    buffer[0] = prefix;
    buffer[1 + offset] = brightness;

    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}
