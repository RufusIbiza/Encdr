fn main() {
    let devs: Vec<_> = nusb::list_devices().unwrap().collect();
    println!("{} USB devices found", devs.len());
    for dev_info in &devs {
        if dev_info.vendor_id() == 0x17cc {
            println!("NI device: {:04x}:{:04x} {}", dev_info.vendor_id(), dev_info.product_id(), dev_info.product_string().unwrap_or("?"));
        }
    }
    if let Some(dev_info) = devs.iter().find(|d| d.vendor_id() == 0x17cc && d.product_id() == 0x1370) {
        println!("Opening S8...");
        let dev = dev_info.open().unwrap();
        let cfg = dev.active_configuration().unwrap();
        println!("Config {}", cfg.configuration_value());
        for iface in cfg.interfaces() {
            for alt in iface.alt_settings() {
                println!("  iface {} alt={} class=0x{:02x} subclass=0x{:02x} proto=0x{:02x}",
                    alt.interface_number(), alt.alternate_setting(),
                    alt.class(), alt.subclass(), alt.protocol());
                for ep in alt.endpoints() {
                    println!("    ep 0x{:02x} {:?} {:?} pkt={}",
                        ep.address(), ep.direction(), ep.transfer_type(), ep.max_packet_size());
                }
            }
        }
    }
}
