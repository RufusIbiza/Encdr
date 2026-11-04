# Reverse Engineering `NIHardwareConnectionService`: Displaying NKS2 Parameters on Maschine MK3

## 1. Executive Summary

This document details the architecture, IPC mechanisms, and reverse engineering findings from Native Instruments' `NIHardwareConnectionService` regarding **NKS2 (Native Kontrol Standard 2)** parameter streaming. It provides an actionable roadmap for the **Encdr** project to intercept and display NKS2 parameters (from Komplete Kontrol, Kontakt, and compatible plugins) on the dual screens of the **Maschine MK3**.

---

## 2. Core Architectural Revelation: The ODR Paradigm Shift

To understand why NKS2 parameters do not natively appear on Maschine MK3, one must understand how NI changed controller screen architectures between generations:

### Legacy Hardware (Maschine MK3, KK MK2, Traktor D2/S8)
* **Dumb Framebuffer Model:** Legacy controllers do not render their own user interface.
* The host DAW / application (`Maschine 2.exe`, `Komplete Kontrol.exe`) renders the graphics on the computer's CPU/GPU and streams raw pixel buffers (RGB565, BGR, or compressed chunks) over USB bulk endpoints.
* Maschine MK3 (USB PID `0x1600`) has no onboard UI engine.

### Modern NKS2 Hardware (Komplete Kontrol S-Series MK3)
* **ODR (On-Device Rendering) Model:** KK MK3 (USB PIDs `0x2100`, `0x2110`, `0x2120`) contains an embedded processor running a lightweight operating system.
* The host PC **never renders or streams screen pixels** to the keyboard.
* Instead, plugins broadcast high-level **structured data models** (parameter names, values, step sizes, display units, asset thumbnails, and browser categories) to the service.
* The service relays these models to the hardware via USB bulk transfers, and the keyboard's internal engine renders the UI natively.

### The Problem for Maschine MK3
Because Maschine MK3 cannot parse NKS2 structured data models, NI never built a bridge from NKS2 models to the Maschine MK3 screen renderer. For Encdr to bridge this gap, Encdr must:
1. **Intercept the NKS2 structured parameter data models** from the IPC layer.
2. **Render the parameter pages into pixel buffers** using Encdr's built-in GPU pipelines (`encdr-skia` or `encdr-view`).
3. **Stream the raw pixels** to the Maschine MK3 screens via USB bulk transfer.

```
+-------------------------------------------------------------+
|               DAW / Plugin (Komplete Kontrol / Kontakt)     |
+-------------------------------------------------------------+
                              |
                              | [msgpack-rpc]
                              v
+-------------------------------------------------------------+
|                NIHardwareConnectionService                  |
|                      (ODR Agent Core)                       |
+-------------------------------------------------------------+
      |                                              |
      | [msgpack-rpc over USB]                       | [msgpack-rpc over TCP:5454]
      v                                              v
+----------------------+                   +----------------------+
|  Komplete Kontrol    |                   |  Encdr Bridge Module |
|     MK3 Hardware     |                   |  (kks_mk3_simulator) |
| (On-Device Rendering)|                   +----------------------+
+----------------------+                             |
                                                     | [Extracts metadata & values]
                                                     v
                                           +----------------------+
                                           | encdr-skia / view    |
                                           | (Renders UI Frame)   |
                                           +----------------------+
                                                     |
                                                     | [Raw Pixel Blit over USB]
                                                     v
                                           +----------------------+
                                           | Maschine MK3 Screens |
                                           +----------------------+
```

---

## 3. Dissecting `NIHardwareConnectionService`

Inspection of `NIHardwareConnectionService.exe.c` (and the raw PE binary) reveals the complete communication topology:

### A. IPC Protocol: `ni::msgpack_rpc`
The entire internal and external communication system uses **`msgpack-rpc`** (MessagePack Remote Procedure Call). Data structures are serialized in binary MessagePack format with standard request/response/notification framing.

### B. Socket Paths & Transports
The service acts as the central IPC server (`ni::odr::agent::kks::ipc_server`):

* **Transport:** Boost.Asio stream sockets (`AF_UNIX` / Local Stream Protocol).
* **Windows Paths:**
  * `%PUBLIC%\Documents\Native Instruments\com.native-instruments.odr_agent.kks`
  * `%PUBLIC%\Documents\Native Instruments\com.native-instruments.nks-connect`
* **macOS Paths:**
  * `/Library/Application Support/Native Instruments/` or `/tmp/com.native-instruments.odr_agent.kks` (standard `AF_UNIX` domain socket).

### C. Key RPC Methods for Parameter Streaming
When a plugin loads or updates parameters, it issues the following RPC notifications to the connection service:

| RPC Method Name | Description |
| :--- | :--- |
| `client_parameter_page_set_data` | Sets the parameter page schema (knob titles, units, min/max limits, step counts). |
| `client_parameter_page_set_plugin_data` | Sends plugin identification, vendor info, and asset references. |
| `client_parameter_page_set_parameter_value` | Real-time parameter updates (when knobs are turned in the plugin GUI or automated). |
| `client_parameter_page_set_host_owned_viewstate` | Active page index, bank offsets, and visibility toggles. |
| `client_browser_set_data_model` | Preset browser entries, categories, and sound attributes. |
| `client_mixer_set_track_data` | DAW track volumes, pan values, and mute/solo states. |

### D. The Native Hardware Simulator Mode (`NI_ODR_AGENT_ENABLE_TCP`)
Line 331050 of `NIHardwareConnectionService.exe.c` reveals an official, built-in debug/simulator interface:

```c
cVar5 = FUN_140395030("NI_ODR_AGENT_ENABLE_TCP");
if (cVar5 != '\0') {
    // Allocates tcp::device_factory listening on port 0x154e (TCP port 5454)
    lVar6 = FUN_140294990(plStack_158, 0x154e);
}
```

When the environment variable `NI_ODR_AGENT_ENABLE_TCP` is defined:
1. `NIHardwareConnectionService` creates `ni::odr::transport::tcp::device_factory`.
2. It calls `async_accept` on **TCP port 5454** (`0x154e`).
3. It instantiates `ni::odr::agent::kks::kks_mk3_simulator::prepareTCP`.
4. Any TCP client connecting to `127.0.0.1:5454` is treated as a **virtual Komplete Kontrol MK3 keyboard**.

---

## 4. Evaluation of Technical Options

### Option 1: TCP Simulator Emulation (`localhost:5454`) — *Recommended*
* **Viability:** **Highest**
* **Cross-Platform:** **100% (Windows and macOS)**
* **How It Works:**
  1. Set the environment variable `NI_ODR_AGENT_ENABLE_TCP=1` for `NIHardwareConnectionService`.
  2. Encdr (or a companion daemon) connects as a TCP client to `127.0.0.1:5454`.
  3. Encdr answers the initial `agent_heartbeat_request_handshake` with `handshake_reply`.
  4. The service treats Encdr as a connected KK MK3 and automatically routes all `parameter_page_observer` and `browser_model_observer` updates to Encdr over the TCP stream using standard msgpack-rpc.
* **Advantages:**
  * Zero low-level OS hooks or driver hacks required.
  * Native NI code path designed specifically for running hardware emulators.
  * Cross-platform by default (standard TCP loopback socket).

### Option 2: Transparent IPC Socket Proxy (UDS MITM) — *Viable Alternative*
* **Viability:** **High**
* **Cross-Platform:** **100% (Windows AF_UNIX / macOS Unix Domain Sockets)**
* **How It Works:**
  1. An Encdr proxy process starts and binds to the known socket path (`com.native-instruments.odr_agent.kks`).
  2. The proxy forwards all traffic bidirectionally between the real `NIHardwareConnectionService` and the plugin.
  3. While proxying, it passively deserializes the MessagePack stream and listens for `client_parameter_page_set_data` and `client_parameter_page_set_parameter_value`.
* **Advantages:**
  * Does not require setting environment variables on the background NI service.
  * Passive; does not alter hardware discovery.

### Option 3: Process Memory Hooking — *Not Recommended*
* **Viability:** **Low / Fragile**
* **Cross-Platform:** **Poor**
* **Drawbacks:**
  * DLL injection on Windows requires constant maintenance of function offsets and vtable pointers across Native Access updates.
  * On macOS, System Integrity Protection (SIP) and hardened runtime make process memory injection prohibitively complex for end users.

---

## 5. Implementation Roadmap for Encdr

To implement NKS2 support on Maschine MK3 in Encdr:

```
[NI Plugin] ---> (msgpack-rpc) ---> [NIHardwareConnectionService]
                                                | (TCP :5454)
                                                v
                                    [Encdr NKS2 Client]
                                                |
                                        [Parameter State]
                                                |
                                                v
                                    [Skia / WebView UI Frame]
                                                |
                                                v
                                    [Maschine MK3 Screen Blit]
```

1. **TCP Client Module:**
   Add an `encdr-nks` module that connects to `127.0.0.1:5454`. Use `rmp-serde` (Rust MessagePack) to deserialize incoming RPC requests and notifications.
2. **State Store:**
   Maintain an active parameter page cache:
   * Current plugin name & icon.
   * Page index and total pages.
   * 8 parameter slots: `name`, `display_value`, `normalized_value` (0.0 to 1.0), `unit`.
3. **Screen Rendering:**
   * Feed the parameter cache into an `encdr-skia` canvas or `encdr-view` layout matching the Maschine MK3 480x272 dual-screen layout.
   * Render the 8 rotary knob dials, labels, and values on Screen 2, and plugin info/browsing on Screen 1.
4. **Physical Controls Feedback:**
   Route Maschine MK3 knob turn events from Encdr back to the plugin by emitting the corresponding parameter change notifications over the socket.
