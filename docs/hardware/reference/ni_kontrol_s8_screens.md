# NI Kontrol S8 Dual-Screen Addressing Reference

This document details how the two screens on the Native Instruments Kontrol S8 are addressed and updated by Traktor.

---

## 1. Physical Addressing (USB Level)

Unlike some controllers that use a header field or a single large buffer to address multiple screens, the S8 addresses its two screens via separate USB interfaces and endpoints.

| Screen | USB Interface | Endpoint | Protocol |
|--------|---------------|----------|----------|
| **Left** | 1 (`screen_left`) | `0x02` (Bulk Out) | 0x84 Blit |
| **Right** | 2 (`screen_right`) | `0x03` (Bulk Out) | 0x84 Blit |

---

## 2. Source Code Implementation

The separation of the two screens is established during the port initialization in `sub_14067fac0` and `sub_140684c90`.

### Registration Functions

Traktor uses two distinct functions to register the screen objects:

*   **`sub_140665170`**: Used for the primary (Left) screen.
*   **`sub_140665830`**: Used for the secondary (Right) screen.

These functions create the `LCDDisplay` objects and link them to the respective `BulkDisplayHandler` instances that target the correct USB interfaces.

### Addressing Logic (`sub_140690400`)

This function is responsible for registering the high-level screen elements (Touch and Platter) and mapping them to the correct physical screen.

```cpp
// From sub_140690400
// Registers elements for a specific deck/screen
int64_t result = sub_140665170(arg1, &var_108, &var_188, rax_3);
```

---

## 3. Screen Blit Protocol (0x84)

Both screens use the same command protocol for pixel data transfers. This is the same protocol used by the NI Kontrol D2 and S5.

### Command Header (20 bytes)
Every blit operation begins with a 20-byte header:
`84 00 00 60 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00`

*   `0x84`: Command ID for Bulk Blit.
*   `0x01 E0` (480): Width.
*   `0x01 10` (272): Height.

### Pixel Data
- **Format:** BGR565 (Big-Endian).
- **Size:** 480 * 272 * 2 bytes = 261,120 bytes.

### Footer (8 bytes)
`03 00 00 00 40 00 00 00`

---

## 4. Commentary: Why the Right Screen Might Not Be Updating

If the right screen is not updating in a custom implementation, the issue is likely one of the following:

1.  **Interface Selection:** Ensure the software is explicitly opening **Interface 2** (Endpoint `0x03`) for the right screen. Sending data to Interface 1 will always update the left screen regardless of the data content.
2.  **Dual Handle Quirk:** The S8 descriptor includes the `dual_handle` quirk. This means the OS might see the device as multiple separate HID/Bulk devices. The driver must ensure it has handles to both the control interface AND both screen interfaces.
3.  **Endpoint 0x03:** Some implementations mistakenly assume that because the D2 uses `0x02`, all NI screens use `0x02`. On the S8, `0x03` is the required endpoint for the right-side bulk data.
4.  **Protocol Consistency:** The header and footer MUST be identical for both screens. There is no "Screen ID" field inside the 0x84 protocol header; the routing is handled entirely by the USB transport layer.
