use std::{error::Error, path::Path};

use super::{configuration, policy_admin};

pub(super) fn run(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    // SAFETY: geteuid has no inputs and does not access caller memory.
    if unsafe { libc::geteuid() } != 0 {
        return Err("administrator privileges required".into());
    }
    let [operation, id] = arguments else {
        return Err("expected device-admin enable|disable ID".into());
    };
    let disabled = match operation.as_str() {
        "disable" => true,
        "enable" => false,
        _ => return Err("expected device-admin enable|disable ID".into()),
    };
    if id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("expected a 32-digit authenticator ID".into());
    }
    let path = Path::new(carosignd::config::DEFAULT_PROVIDER_ALLOWLIST);
    let expected = configuration::read(path)?;
    carosignd::config::validate_provider_catalog(&expected)?;
    let proposed = update(&expected, id, disabled).ok_or("device is not configured")?;
    carosignd::config::validate_provider_catalog(&proposed)?;
    if !policy_admin::replace(path, &expected, &proposed)? {
        return Err("configuration changed while updating device".into());
    }
    Ok(())
}

fn update(document: &str, id: &str, disabled: bool) -> Option<String> {
    let mut updated = String::with_capacity(document.len() + 9);
    let mut found = false;
    for line in document.split_inclusive('\n') {
        let mut fields = line.split_ascii_whitespace();
        if !line.trim_start().starts_with('#')
            && fields
                .nth(3)
                .is_some_and(|value| value.eq_ignore_ascii_case(id))
        {
            found = true;
            let body = line.trim_end();
            let enabled_line = body
                .strip_suffix("disabled")
                .filter(|prefix| {
                    prefix.ends_with(|character: char| character.is_ascii_whitespace())
                })
                .map_or(body, str::trim_end);
            updated.push_str(enabled_line);
            if disabled {
                updated.push_str(" disabled");
            }
            if line.ends_with('\n') {
                updated.push('\n');
            }
        } else {
            updated.push_str(line);
        }
    }
    found.then_some(updated)
}

#[cfg(test)]
#[path = "device_admin.tests.rs"]
mod tests;
