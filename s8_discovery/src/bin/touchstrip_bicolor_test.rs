use anyhow::{anyhow, Result};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Testing touchstrip with bi-color (blue + orange) approach\n");

    // Test 1: Set first blue LED only
    println!("Test 1: Blue only at offset 93");
    test_bicolor(0x80, 93, 150, 118, 0).await?;
    println!("  → Should see blue if blue is at 93\n");
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Test 2: Set first orange LED only
    println!("Test 2: Orange only at offset 118");
    test_bicolor(0x80, 93, 0, 118, 150).await?;
    println!("  → Should see orange if orange is at 118\n");
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Test 3: Set both (should show yellow/white if overlapping)
    println!("Test 3: Both blue and orange");
    test_bicolor(0x80, 93, 150, 118, 150).await?;
    println!("  → Should show purple/white if both overlap\n");
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Test 4: Try different spacing - maybe orange is at 93+25
    println!("Test 4: Blue at 93, orange at 118 (93+25)");
    test_bicolor(0x80, 93, 150, 118, 150).await?;
    println!("  → Checking if 25-offset spacing works\n");
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Turn off
    println!("Turning off...");
    test_bicolor(0x80, 93, 0, 118, 0).await?;

    Ok(())
}

async fn test_bicolor(prefix: u8, blue_offset: usize, blue_val: u8, orange_offset: usize, orange_val: u8) -> Result<()> {
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
