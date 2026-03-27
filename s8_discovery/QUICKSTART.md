# S8 Discovery - Quick Start

## Build

```bash
cd /path/to/Encdr
cargo build --release -p s8_discovery
```

The binary will be at: `target/release/s8_discovery`

## Run

```bash
cd /path/to/Encdr
./target/release/s8_discovery
```

**Requirements:**
- S8 must be plugged into the computer
- On Linux: You need USB access (add udev rule or run as sudo)

## What to Expect

1. **Initialization**: The tool will find your S8 and read the initial USB packet
2. **For each control**: The tool will:
   - Tell you which control to interact with
   - Give you 5 seconds to press/turn/slide it
   - Capture USB packet changes
   - Report which byte changed and by how much
3. **Output**: A JSON snippet you can paste into the descriptor

## After Discovery

1. Copy the JSON output from the report
2. Update `encdr/descriptors/ni_kontrol_s8.json` with the new mappings
3. Test with: `cargo run -p encdr --example monitor`
4. Press/turn controls and see them appear in the event stream

## Troubleshooting

### "S8 device not found"
- Make sure S8 is plugged in and powered on
- On Linux, try: `sudo ./target/release/s8_discovery`
- Check: `lsusb | grep 17cc:1370` (should show NI device)

### "Failed to claim interface"
- Device may be in use by another application
- Close Traktor, VirtualDJ, or other NI software
- Try unplugging and replugging the S8

### "No significant change detected"
- Control may not be connected properly
- Try pressing harder/turning farther
- Some controls may require longer press/hold

## Tips

- Start with buttons (easiest to detect)
- Then do sliders/faders (should be multi-byte values)
- Encoders last (may be tricky)
- Pay attention to which byte changes for each control
- For sliders, look for 2-byte (12-bit or 16-bit) changes
