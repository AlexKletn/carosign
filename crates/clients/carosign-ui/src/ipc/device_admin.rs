pub(crate) fn set_disabled(id: &str, disabled: bool) -> &'static str {
    let result = std::process::Command::new("/usr/bin/pkexec")
        .args([
            "--disable-internal-agent",
            "/usr/bin/carosign-config",
            "device-admin",
            if disabled { "disable" } else { "enable" },
            id,
        ])
        .status();
    match result.ok().and_then(|status| status.code()) {
        Some(0) => "ui-device-action-saved",
        Some(126 | 127) => "ui-device-action-denied",
        _ => "ui-device-action-failed",
    }
}
