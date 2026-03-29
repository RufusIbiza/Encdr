# NI Kontrol S8 Screen Addressing (Single Endpoint Multiplexing)

This document explains how the Native Instruments Kontrol S8 addresses its two screens when both are connected to the same USB Interface/Endpoint.

---

## 1. Physical Interface

*   **USB Interface:** 6
*   **USB Endpoint:** `0x04` (Bulk Out)
*   **Protocol:** NI Bulk Blit (Multiplexed)

Unlike the D2, which uses different endpoints for different screens, the S8 sends data for **both** screens to the same physical endpoint (`0x04`).

---

## 2. Multiplexing Protocol

To distinguish between the two displays, Traktor modifies the command ID or a field in the blit header.

### Screen Command Constants

| Screen | Traktor Internal Constant | Likely Header Offset 1 |
|--------|---------------------------|------------------------|
| **Left** | `0x03566775`              | `0x66`                 |
| **Right** | `0x03567375`              | `0x67`                 |

### Header Structure Analysis

The standard blit header is 20 bytes. For the S8, the 4th byte (offset 3) is likely used as the Screen ID.

**Left Screen Header:**
`84 00 00 66 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00`

**Right Screen Header:**
`84 00 00 67 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00`

*   `0x84`: Command ID.
*   `0x66` / `0x67`: Destination Screen ID.
*   `0x01 E0` (480): Width.
*   `0x01 10` (272): Height.

---

## 3. Traktor Source Functions

The following functions in `Traktor.exe` handle the screen addressing logic:

### `sub_140c9ff70` (Left Screen Update)
This function sets up the parameters for the left screen update, specifically using the constant `0x3566775`.

```cpp
int64_t sub_140c9ff70(int64_t* port, int32_t arg2, int32_t* arg3, int512_t arg4) {
    // ...
    int32_t screen_cmd = 0x3566775; // Left Screen ID
    int32_t endpoint_param = 8;
    // Calls the low-level USB dispatch function at rax + 0xe8
    return (*(uint64_t*)(rax + 0xe8))(arg4, &endpoint_param);
}
```

### `sub_140ca33e0` (Right Screen Update)
This function sets up the parameters for the right screen update, specifically using the constant `0x3567375`.

```cpp
int64_t sub_140ca33e0(int64_t* port, int512_t arg2) {
    // ...
    int32_t screen_cmd = 0x3567375; // Right Screen ID
    int32_t endpoint_param = 0xc;
    // Calls the low-level USB dispatch function at rax + 0xe8
    return (*(uint64_t*)(rax + 0xe8))(arg2, &endpoint_param);
}
```

---

## 4. Commentary

The use of a single endpoint for dual screens simplifies the USB descriptor but requires the hardware to parse the incoming bulk data to route pixels to the correct LCD panel. 

The bit difference between `0x66` and `0x67` (only bit 0 changes) is a classic hardware addressing pattern. 

**Implementation Advice:**
When porting this to a new driver:
1.  Open Interface 6.
2.  Use Endpoint `0x04` for all screen data.
3.  Modify byte 4 of your 20-byte blit header to `0x66` for the left screen and `0x67` for the right screen.
4.  Ensure the blit footer (`03 00 00 00 40 00 00 00`) is appended to every transfer.
