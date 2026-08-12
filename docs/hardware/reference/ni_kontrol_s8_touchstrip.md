# NI Kontrol S8 Touchstrip Implementation Reference

## Overview

The NI Kontrol S8 features two 25-LED touchstrips (one per deck). These touchstrips provide visual feedback for track position, phase, and scratching.

## Technical Specifications

| Property                 | Value                        |
| ------------------------ | ---------------------------- |
| LEDs per Touchstrip      | 25                           |
| Total Touchstrip LEDs    | 50                           |
| Report Size              | 176 bytes (Sliders/Touch)    |
| LED Buffer Index (Left)  | `0x5d` to `0x75` (93 - 117)  |
| LED Buffer Index (Right) | `0xd3` to `0xeb` (211 - 235) |

## Traktor Source Analysis

### Registration Function: `sub_141765d30`

This function is responsible for registering a range of LEDs for a control. It is used for the touchstrips on the S8.

**Signature:**
```cpp
void** sub_141765d30(void* port, int64_t* name, int32_t start_index, int32_t step, int32_t hardware_id, int32_t type, int32_t count)
```

**Call Sites for S8:**

*   **Left Touchstrip:**
    ```cpp
    sub_141765d30(result, &var_168, 0x5d, 2, 0x44, 0xb, 0x19);
    ```
    - `start_index`: `0x5d` (93)
    - `count`: `0x19` (25)
    - `hardware_id`: `0x44` (ID 68)

*   **Right Touchstrip:**
    ```cpp
    sub_141765d30(result, &var_8d8, 0xb7, 2, 0x9e, 0xb, 0x19);
    ```
    - `start_index`: `0xb7` (183) — *Note: This is relative to the right deck offset.*
    - `count`: `0x19` (25)

### LED Buffer Segmentation

The S8 output buffer consists of 309 LEDs. These are likely sent in three reports using prefixes `0x80`, `0x81`, and `0x82`.

*   **Report 1 (`0x80`):** Contains LEDs 0 - 117 (includes Left Touchstrip at 93-117).
*   **Report 2 (`0x81`):** Contains LEDs 118 - 235 (includes Right Touchstrip).
*   **Report 3 (`0x82`):** Contains the remaining mixer LEDs.

## Implementation Details

### Input Handling
The touchstrip position is sent in the 176-byte "sliders" report.
- **Left Position:** Bytes 45-46 (16-bit, 0-1024).
- **Left Touch:** Byte 16, Mask `0x01` (in the 46-byte buttons report).

### LED Output
To turn on the left touchstrip LEDs, send data to the control interface with prefix `0x80`. The touchstrip segment starts at byte 94 (after the prefix).

To turn on the right touchstrip LEDs, send data with prefix `0x81`.

## Troubleshooting Touchstrip LEDs

If the lights are not turning on:
1. Verify that the total buffer size is 309 and that it is being correctly segmented into the three prefixed reports.
2. Ensure that the `0x80` report has at least 118 bytes of data (including prefix).
3. Ensure that the `0x81` report is being sent to the same endpoint as the `0x80` report.
4. Check if the device requires an initialization sequence to enable LED updates.
