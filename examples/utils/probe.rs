use encdr::{Encdr, EncdrConfig};

fn main() {
    // Initialize with defaults (loads built-in descriptors)
    let encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");

    println!("=== Encdr Device Probe ===\n");

    // Show loaded descriptors
    let descriptors = encdr.loaded_descriptors();
    println!("Loaded {} device descriptor(s):\n", descriptors.len());

    for desc in &descriptors {
        println!("  {} ({})", desc.name, desc.manufacturer);
        println!(
            "    VID:PID = {:04x}:{:04x}",
            desc.vendor_id.0, desc.product_id.0
        );
        println!("    Interfaces: {}", desc.interfaces.len());
        for iface in &desc.interfaces {
            let ep_in = iface
                .endpoints
                .ep_in
                .as_ref()
                .map(|ep| format!("0x{:02x} ({:?})", ep.address.0, ep.transfer_type))
                .unwrap_or_else(|| "none".into());
            let ep_out = iface
                .endpoints
                .out
                .as_ref()
                .map(|ep| format!("0x{:02x} ({:?})", ep.address.0, ep.transfer_type))
                .unwrap_or_else(|| "none".into());
            println!(
                "      [{}] #{}: IN={}, OUT={}",
                iface.id, iface.number, ep_in, ep_out
            );
        }

        // Count controls
        let mut buttons = 0;
        let mut sliders = 0;
        let mut encoders = 0;
        let mut touches = 0;
        for item in desc.all_inputs() {
            match item {
                encdr::core::descriptor::InputItemDesc::Button(_) => buttons += 1,
                encdr::core::descriptor::InputItemDesc::Slider(_) => sliders += 1,
                encdr::core::descriptor::InputItemDesc::Encoder(_) => encoders += 1,
                encdr::core::descriptor::InputItemDesc::EncoderFine(_) => encoders += 1,
                encdr::core::descriptor::InputItemDesc::Touch(_) => touches += 1,
            }
        }
        println!(
            "    Controls: {} buttons, {} sliders, {} encoders, {} touch sensors",
            buttons, sliders, encoders, touches
        );

        // LEDs
        for leds in &desc.leds {
            println!(
                "    LEDs: {} items, {} byte buffer (prefix 0x{:02x})",
                leds.items.len(),
                leds.buffer_size,
                leds.prefix_byte.0
            );
        }

        // Screens
        for screen in &desc.screens {
            println!(
                "    Screen '{}': {}x{} {:?}{}",
                screen.name,
                screen.width,
                screen.height,
                screen.pixel_format,
                if screen.partial_blit.as_ref().is_some_and(|p| p.supported) {
                    " (partial updates)"
                } else {
                    ""
                }
            );
        }

        // Quirks
        let mut quirks = Vec::new();
        if desc.quirks.dual_handle {
            quirks.push("dual_handle");
        }
        if desc.quirks.detach_kernel_driver {
            quirks.push("detach_kernel_driver");
        }
        if !quirks.is_empty() {
            println!("    Quirks: {}", quirks.join(", "));
        }
        println!();
    }

    // Scan for connected devices
    println!("--- Scanning USB bus ---\n");
    let mut encdr = encdr;
    match encdr.scan() {
        Ok(ids) if ids.is_empty() => {
            println!("  No matching devices found.");
        }
        Ok(ids) => {
            for id in ids {
                if let Some(desc) = encdr.device_descriptor(id) {
                    println!("  CONNECTED: {} [{}]", desc.name, id);
                }
            }
        }
        Err(e) => {
            println!("  Scan error: {}", e);
        }
    }
}
