use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

/// Test different header byte values to find the right screen address.
/// Left screen works with header byte[2] = 0x00. Try other values for right.
#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Screen Header Test — Finding Right Screen Address        ║");
    println!("║                                                            ║");
    println!("║  Left screen works with byte[2] = 0x00                     ║");
    println!("║  Testing different byte[2] values for right screen         ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let screen_iface = device.detach_and_claim_interface(6)?;

    // Build a visible test pattern — red screen (so we can see it)
    let width = 480;
    let height = 272;
    let pixel_size = width * height * 2; // BGR565
    let mut pixels = vec![0u8; pixel_size];

    // Fill with bright red in BGR565 big-endian: 0xF800
    for i in (0..pixel_size).step_by(2) {
        pixels[i] = 0xF8;
        pixels[i + 1] = 0x00;
    }

    let footer: [u8; 8] = [0x03, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00];

    // First confirm left screen still works
    print!("Confirm: Send RED screen to LEFT (byte[2]=0x00) [ENTER]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let header_left = [
        0x84, 0x00, 0x00, 0x60, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0xE0, 0x01, 0x10,
        0x00, 0x00, 0xFF, 0x00,
    ];
    send_screen(&screen_iface, &header_left, &pixels, &footer).await?;
    println!("  Sent to left.");

    // Now try different byte[2] values for right screen
    let test_values: Vec<(u8, &str)> = vec![
        (0x01, "0x01 (Mk3 pattern)"),
        (0x02, "0x02"),
        (0x03, "0x03"),
        (0x04, "0x04"),
        (0x10, "0x10"),
        (0x20, "0x20"),
        (0x40, "0x40"),
        (0x80, "0x80"),
    ];

    // Use green for right screen tests so we can distinguish
    let mut green_pixels = vec![0u8; pixel_size];
    // Green in BGR565 big-endian: 0x07E0
    for i in (0..pixel_size).step_by(2) {
        green_pixels[i] = 0x07;
        green_pixels[i + 1] = 0xE0;
    }

    for (val, desc) in &test_values {
        print!("\nTest byte[2] = {} — [ENTER] (or 'q' to quit): ", desc);
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        if input.trim().eq_ignore_ascii_case("q") {
            break;
        }

        let mut header = header_left;
        header[2] = *val;
        send_screen(&screen_iface, &header, &green_pixels, &footer).await?;
        println!("  Sent with byte[2] = 0x{:02x}", val);
    }

    // Also try varying byte[1] instead
    println!("\n--- Now testing byte[1] variations (byte[2] stays 0x00) ---");
    let byte1_values: Vec<(u8, &str)> = vec![
        (0x01, "byte[1]=0x01"),
        (0x02, "byte[1]=0x02"),
        (0x04, "byte[1]=0x04"),
        (0x08, "byte[1]=0x08"),
        (0x10, "byte[1]=0x10"),
        (0x80, "byte[1]=0x80"),
    ];

    for (val, desc) in &byte1_values {
        print!("\nTest {} — [ENTER] (or 'q' to quit): ", desc);
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        if input.trim().eq_ignore_ascii_case("q") {
            break;
        }

        let mut header = header_left;
        header[1] = *val;
        send_screen(&screen_iface, &header, &green_pixels, &footer).await?;
        println!("  Sent with byte[1] = 0x{:02x}", val);
    }

    println!("\n✓ Done!");
    Ok(())
}

async fn send_screen(
    iface: &nusb::Interface,
    header: &[u8; 20],
    pixels: &[u8],
    footer: &[u8; 8],
) -> Result<()> {
    let mut buf = Vec::with_capacity(header.len() + pixels.len() + footer.len());
    buf.extend_from_slice(header);
    buf.extend_from_slice(pixels);
    buf.extend_from_slice(footer);
    iface.bulk_out(0x04, buf).await.into_result()?;
    Ok(())
}
