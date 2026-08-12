# Testing S8 Traktor Mode LED Addressing

This guide walks through testing the S8 mixer LED addressing based on Traktor decompiled source.

## Prerequisites

1. **S8 device connected** and recognized by OS
2. **USB access tool** - one of:
   - `libusb-dev` (Linux)
   - `pyusb` (Python, cross-platform)
   - Custom USB tool with HID support

## Step 1: Run the Interactive Test

```bash
cargo run --bin traktor_mode_test
```

This will guide you through the testing process.

## Step 2: Send USB Commands

### Using `libusb-dev` (Linux)

```bash
# Send 0xf3 handshake (enable Traktor mode)
libusb-dev -v 0x17cc:0x1370 -w 0x02 \
  0xf3 0x01 \
  $(python3 -c "print(' '.join('0' for _ in range(308)))")

# Then test individual LED offsets, e.g., Deck Input A (offset 80 @ 0x80)
libusb-dev -v 0x17cc:0x1370 -w 0x02 \
  0x80 $(python3 -c "import sys; d = ['0'] * 309; d[80] = 'c8'; print(' '.join(d))") 
```

### Using Python + pyusb

```python
import usb.core
import usb.util

# Find device (NI Kontrol S8)
dev = usb.core.find(idVendor=0x17cc, idProduct=0x1370)
if dev is None:
    raise ValueError("Device not found")

# Send 0xf3 handshake
handshake = bytearray([0xf3, 0x01] + [0x00] * 308)
dev.ctrl_transfer(
    bmRequestType=0x21,  # USB_TYPE_CLASS | USB_RECIP_INTERFACE
    bRequest=0x09,       # SET_REPORT
    wValue=0x0302,       # Report ID (0x03) + Report Type (Output = 0x02)
    wIndex=0,            # Interface 0
    data_or_wlength=handshake
)
print("Handshake sent")

# Test Deck Input A (offset 80 @ 0x80)
test_packet = bytearray([0x80] + [0x00] * 309)
test_packet[80 + 1] = 200  # Set offset 80 to brightness 200

dev.ctrl_transfer(
    bmRequestType=0x21,
    bRequest=0x09,
    wValue=0x0302,
    wIndex=0,
    data_or_wlength=test_packet
)
print("Test packet sent - check which LED lit up")
```

### Using `hidapi` (cross-platform)

```python
import hid

# Find device
for device_info in hid.enumerate(0x17cc, 0x1370):
    dev = hid.device()
    dev.open_path(device_info['path'])
    
    # Send handshake
    handshake = [0xf3, 0x01] + [0x00] * 308
    dev.write(handshake)
    print("Handshake sent")
    
    # Test offset
    test = [0x80] + [0x00] * 309
    test[80 + 1] = 200
    dev.write(test)
    print("Test sent")
    
    dev.close()
    break
```

## Expected Behavior

After sending the handshake, the mixer should enter Traktor control mode. Then:

1. **Sending 0x80 offset 80 = 200** should light the Channel A Deck Input LED
2. **Sending 0x81 offset 105 = 200** should light the Channel A Cue LED
3. **Sending 0x81 offset 101 = 200** should light the Channel A Filter LED
4. etc.

## Documenting Results

As you test each index, note:
- Which prefix and offset
- Which physical LED lit up
- Any unexpected behavior

The test script will help you organize these findings for descriptor updates.

## References

- Traktor source: `/media/rufus/Big Love/Documents/Projects/Traktor Decompile`
- S8 documentation: `/home/rufus/Documents/Projects/Encdr/docs/hardware/reference/ni_kontrol_s8_lighting_logic.txt`
- Detailed source: `/home/rufus/Documents/Projects/Encdr/docs/hardware/reference/ni_kontrol_s8_lighting_source_detailed.txt`
