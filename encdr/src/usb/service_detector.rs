use std::sync::atomic::{AtomicBool, Ordering};

static WARNED_SCAN: AtomicBool = AtomicBool::new(false);

/// Detect any running Native Instruments background services that may claim
/// USB interfaces and prevent Encdr from communicating directly with controllers.
#[cfg(target_os = "windows")]
pub fn detect_active_services() -> Vec<String> {
    let mut found = Vec::new();

    // Check named pipe (always created by NIHardwareService on Windows)
    if std::path::Path::new(r"\\.\pipe\NIHWMainHandler").exists() {
        found.push("NIHardwareService (Named Pipe active)".to_string());
    } else if let Ok(output) = std::process::Command::new("tasklist")
        .args(["/NH", "/FI", "IMAGENAME eq NIHardwareService.exe"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        if text.contains("NIHardwareService.exe") {
            found.push("NIHardwareService.exe".to_string());
        }
    }

    if let Ok(output) = std::process::Command::new("tasklist")
        .args(["/NH", "/FI", "IMAGENAME eq NIHostIntegrationAgent.exe"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        if text.contains("NIHostIntegrationAgent.exe") {
            found.push("NIHostIntegrationAgent.exe".to_string());
        }
    }

    if let Ok(output) = std::process::Command::new("tasklist")
        .args(["/NH", "/FI", "IMAGENAME eq NIHardwareConnectionService.exe"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        if text.contains("NIHardwareConnectionService.exe") {
            found.push("NIHardwareConnectionService.exe".to_string());
        }
    }

    if let Ok(output) = std::process::Command::new("tasklist")
        .args(["/NH", "/FI", "IMAGENAME eq NTKDaemon.exe"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        if text.contains("NTKDaemon.exe") {
            found.push("NTKDaemon.exe".to_string());
        }
    }

    found
}

#[cfg(target_os = "macos")]
pub fn detect_active_services() -> Vec<String> {
    let mut found = Vec::new();

    for proc in &[
        "NIHardwareConnectionService",
        "NIHardwareConnectionAgent",
        "NIHardwareService",
        "NIHardwareAgent",
        "NIHostIntegrationAgent",
        "NTKDaemon",
    ] {
        if let Ok(output) = std::process::Command::new("pgrep")
            .arg("-x")
            .arg(proc)
            .output()
        {
            if output.status.success() && !output.stdout.is_empty() {
                found.push((*proc).to_string());
            }
        }
    }

    found
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn detect_active_services() -> Vec<String> {
    Vec::new()
}

/// Check for active NI background services during device scanning and log a warning once.
pub fn warn_active_services_if_detected() {
    if WARNED_SCAN.swap(true, Ordering::Relaxed) {
        return;
    }

    let services = detect_active_services();
    if !services.is_empty() {
        emit_service_warning(&services);
    }
}

/// Diagnostically log interface claiming failure with OS-specific instructions.
pub fn diagnose_claim_failure(iface_num: u8, iface_id: &str, error: &dyn std::fmt::Display) {
    let services = detect_active_services();

    if !services.is_empty() {
        tracing::error!(
            "Failed to claim USB interface {} ('{}'): {}. \
             Active Native Instruments service(s) detected: [{}]. \
             These services claim exclusive USB ownership.",
            iface_num,
            iface_id,
            error,
            services.join(", ")
        );
        emit_service_warning(&services);
    } else {
        tracing::error!(
            "Failed to claim USB interface {} ('{}'): {}",
            iface_num,
            iface_id,
            error
        );

        #[cfg(target_os = "linux")]
        {
            tracing::warn!(
                "On Linux, USB interface claim failures are typically due to missing udev permissions. \
                 Ensure your user has read/write access or create /etc/udev/rules.d/70-ni-controllers.rules: \
                 SUBSYSTEM==\"usb\", ATTR{{idVendor}}==\"17cc\", MODE=\"0666\", GROUP=\"plugdev\""
            );
        }
    }
}

fn emit_service_warning(services: &[String]) {
    #[cfg(target_os = "windows")]
    {
        tracing::warn!(
            "Native Instruments background service(s) active: [{}]. \
             On Windows, these services hold exclusive USB handles on NI controllers. \
             If Encdr fails to claim device interfaces, suspend or terminate them:\n  \
             • PowerShell / Command Prompt (Admin):\n    \
               net stop NIHardwareService\n    \
               taskkill /F /IM NIHardwareService.exe /IM NIHardwareConnectionService.exe /IM NIHostIntegrationAgent.exe /IM NTKDaemon.exe\n  \
             • Or open services.msc and stop 'Native Instruments Hardware Service'.",
            services.join(", ")
        );
    }

    #[cfg(target_os = "macos")]
    {
        tracing::warn!(
            "Native Instruments background service(s) active: [{}]. \
             On macOS, these services hold exclusive USB handles on NI controllers. \
             If Encdr fails to claim device interfaces, suspend or unload them:\n  \
             • Terminal:\n    \
               sudo launchctl unload -w /Library/LaunchDaemons/com.native-instruments.Hardware*.plist 2>/dev/null\n    \
               launchctl unload -w /Library/LaunchAgents/com.native-instruments.*.plist 2>/dev/null\n  \
             • Or run: killall NIHardwareConnectionService NIHardwareService NIHardwareAgent NIHostIntegrationAgent NTKDaemon",
            services.join(", ")
        );
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = services;
    }
}
