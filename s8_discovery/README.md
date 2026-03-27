# S8 Control Discovery Tool

This tool automatically discovers the USB HID control mappings for the NI Kontrol S8 by monitoring packet changes as you interact with the hardware.

## Usage

```bash
cd s8_discovery
cargo run --release
```

### Requirements

- S8 must be plugged into the computer
- On Linux: USB access permissions (see udev rules)
- On macOS/Windows: Standard device access

### How it Works

1. Connects to the S8 via USB HID
2. Captures an initial "baseline" packet (idle state)
3. For each control:
   - Prompts you to interact with it (press button, turn encoder, slide fader)
   - Listens to USB packets for 5 seconds
   - Analyzes the difference between baseline and captured packets
   - Reports the byte offset and change magnitude
4. Outputs a JSON snippet ready to be merged into the descriptor

### Output

The tool produces:
- A list of discovered controls and their byte offsets
- Bit masks for buttons (if detectable)
- A JSON snippet for the `input_packets` section
- A summary report

### Limitations

- Encoders may be difficult to decode (depends on encoding scheme)
- Multi-byte controls (sliders, encoders) may need manual refinement
- Touch controls may require special handling

## Manual Refinement

After discovery, you may need to:

1. **Adjust bit masks** for buttons (the tool reports byte changes, but may need manual bit extraction)
2. **Confirm slider byte order** (big-endian vs little-endian)
3. **Map encoder directions** (CW vs CCW, scale factors)
4. **Test with `/examples/monitor.rs`** to verify packet interpretations

## Next Steps

Once discovery is complete:

1. Update `encdr/descriptors/ni_kontrol_s8.json` with new mappings
2. Run `cargo run -p encdr --example monitor` to verify
3. Run `cargo run -p encdr --example probe` to see full device info
