use anyhow::{anyhow, Result};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

fn main() -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    println!("Found: {} by {}",
             device_info.product_string().unwrap_or("S8"),
             device_info.manufacturer_string().unwrap_or("NI"));

    let device = device_info.open()?;
    let configs = device_info.configurations();
    
    for config in configs {
        println!("Configuration: {}", config.configuration_value());
        for interface in config.interfaces() {
            for alt in interface.alt_settings() {
                println!("  Interface: {}, AltSetting: {}, Class: {}, SubClass: {}, Protocol: {}", 
                         interface.interface_number(),
                         alt.alternate_setting(),
                         alt.class(),
                         alt.subclass(),
                         alt.protocol());
                for endpoint in alt.endpoints() {
                    println!("    Endpoint: 0x{:02x}, Direction: {:?}, Type: {:?}",
                             endpoint.address(),
                             endpoint.direction(),
                             endpoint.transfer_type());
                }
            }
        }
    }

    Ok(())
}
