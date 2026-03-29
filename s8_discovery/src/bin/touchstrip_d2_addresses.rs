use anyhow::{anyhow, Result};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Testing touchstrip with D2 addresses (68=blue, 93=orange)\n");

    // Test 1: Blue only (offset 68)
    println!("Test 1: Blue only (offset 68)");
    test_d2_addresses(0x80, 68, 150, 93, 0).await?;
    println!("  → Should see blue if D2 blue address works\n");
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Test 2: Orange only (offset 93)
    println!("Test 2: Orange only (offset 93)");
    test_d2_addresses(0x80, 68, 0, 93, 150).await?;
    println!("  → Should see orange if D2 orange address works\n");
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Test 3: Both colors
    println!("Test 3: Both blue and orange");
    test_d2_addresses(0x80, 68, 150, 93, 150).await?;
    println!("  → Should show purple/white if both work\n");
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Test 4: Try all 25 blue LEDs
    println!("Test 4: Sweep all 25 blue touchstrip LEDs (68-92)");
    for offset in 68..=92 {
        test_d2_addresses(0x80, offset, 150, 255, 0).await?;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    println!("  → Should see each blue LED light briefly\n");
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Turn off
    println!("Turning off...");
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;
    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;
    let buffer = vec![0u8; 309];
    interface.interrupt_out(0x03, buffer).await.into_result()?;

    Ok(())
}

async fn test_d2_addresses(prefix: u8, blue_offset: usize, blue_val: u8, orange_offset: usize, orange_val: u8) -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    let mut buffer = vec![0u8; 309];
    buffer[0] = prefix;
    buffer[1 + blue_offset] = blue_val;
    buffer[1 + orange_offset] = orange_val;

    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}
