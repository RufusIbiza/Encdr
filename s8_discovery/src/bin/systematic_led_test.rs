use anyhow::{anyhow, Result};
use std::time::Duration;

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║      S8 LED Systematic Test - Identify Working Addresses   ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    loop {
        println!("\nChoose LED to test:");
        println!("  1. Pad LEDs (test each pad RGB separately)");
        println!("  2. Play button LED (offset 59)");
        println!("  3. Cue button LED (offset 58)");
        println!("  4. Loop LED group (offsets 46-47)");
        println!("  5. All pads full white (RGB 127 each)");
        println!("  6. Sweep through offset range");
        println!("  q. Quit");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        match input.trim() {
            "1" => test_pad_leds().await?,
            "2" => {
                println!("\nTesting play LED (offset 59) on LEFT (0x80)...");
                test_single_led(0x80, 59).await?;
            }
            "3" => {
                println!("\nTesting cue LED (offset 58) on LEFT (0x80)...");
                test_single_led(0x80, 58).await?;
            }
            "4" => {
                println!("\nTesting loop LED group (offsets 46-47) on LEFT (0x80)...");
                for offset in 46..=47 {
                    println!("  Testing offset {}...", offset);
                    test_single_led(0x80, offset).await?;
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
            "5" => test_all_pads_white().await?,
            "6" => test_offset_sweep().await?,
            "q" => break,
            _ => println!("Invalid choice"),
        }
    }

    Ok(())
}

async fn test_pad_leds() -> Result<()> {
    println!("\nTesting LEFT pad LEDs one at a time (RED only)...\n");

    // PAD RGB offsets for index 0,3,6,9,12,15,18,21
    let pad_offsets = vec![
        (1, 2), (2, 11), (3, 20), (4, 29),
        (5, 38), (6, 47), (7, 56), (8, 65),
    ];

    for (pad_num, r_offset) in pad_offsets {
        println!("Pad {}:", pad_num);
        println!("  R at offset {}...", r_offset);
        test_single_led(0x80, r_offset).await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

async fn test_all_pads_white() -> Result<()> {
    println!("\nLighting all pads WHITE (RGB 127 each) on both decks...\n");

    for prefix in [0x80u8, 0x81] {
        let prefix_name = if prefix == 0x80 { "LEFT" } else { "RIGHT" };
        println!("Testing {}", prefix_name);

        // Pads at indices 0,3,6,9,12,15,18,21
        // RGB offsets: r = index*3 + 2, g = index*3 + 1, b = index*3
        for index in [0, 3, 6, 9, 12, 15, 18, 21] {
            let r_off = index + 2;
            let g_off = index + 1;
            let b_off = index;

            let mut data = vec![0u8; 308];
            data[r_off] = 127;
            data[g_off] = 127;
            data[b_off] = 127;

            send_led(prefix, &data).await?;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    println!("\nTurning off...");
    send_led(0x80, &vec![0u8; 308]).await?;
    send_led(0x81, &vec![0u8; 308]).await?;

    Ok(())
}

async fn test_single_led(prefix: u8, offset: usize) -> Result<()> {
    let mut data = vec![0u8; 308];
    data[offset] = 127;
    send_led(prefix, &data).await?;

    println!("  ✓ Sent (should see LED at offset {} light up)", offset);
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Turn off
    send_led(prefix, &vec![0u8; 308]).await?;
    Ok(())
}

async fn test_offset_sweep() -> Result<()> {
    println!("\nSweeping through offset 0-70 on LEFT (0x80)...");
    println!("Watch for any LEDs that light up - note the offset!\n");

    for offset in 0..70 {
        let mut data = vec![0u8; 308];
        data[offset] = 200;
        send_led(0x80, &data).await?;

        if offset % 10 == 0 {
            println!("Testing offset {}...", offset);
        }

        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // Turn off
    send_led(0x80, &vec![0u8; 308]).await?;
    println!("\nDone!");

    Ok(())
}

async fn send_led(prefix: u8, data: &[u8]) -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    let mut buffer = vec![0u8; 309];
    buffer[0] = prefix;
    let copy_len = std::cmp::min(data.len(), 308);
    buffer[1..1+copy_len].copy_from_slice(&data[..copy_len]);

    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}
