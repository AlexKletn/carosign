//! Service settings: strict and silent.
//!
//! ⚠️ Rule 13: Customization can **harden** limits, but not extend them beyond the schema
//! ceilings. Extension is a separate solution, not a string in a file.
//!
//! ⚠️ It's a tough one: an unfamiliar field is a failure, otherwise a typo will quietly leave
//! the silence, and the person will think that he has set it up.

use core::fmt;

/// Default path of the socket (document 07).
pub const DEFAULT_SOCKET: &str = "/run/carosignd/control.sock";

/// The way to the storage is by default.
pub const DEFAULT_STORE: &str = "/var/lib/carosign/metadata.redb";

/// Path to the system authorization policy.
pub const DEFAULT_POLICY: &str = "/etc/carosign/policy.conf";
pub const DEFAULT_PROVIDER_ALLOWLIST: &str = "/etc/carosign/providers.conf";

fn absolute(value: &str, field: &'static str) -> Result<String, ConfigError> {
    if !value.starts_with('/') || value.as_bytes().contains(&0) {
        return Err(ConfigError::BadPath { field });
    }
    Ok(value.to_string())
}

// The limit includes the final zero pathname UNIX socket, and we check it before the
// repository opens, otherwise the impossibility of publishing will be revealed too late.
fn socket_path(value: &str) -> Result<String, ConfigError> {
    let path = absolute(value, "socket")?;
    // SAFETY: zero-state sockaddr_un It is permissible to determine the length.
    let address: libc::sockaddr_un = unsafe { core::mem::zeroed() };
    if value.len() >= address.sun_path.len() {
        return Err(ConfigError::BadPath { field: "socket" });
    }
    Ok(path)
}

fn bounded(value: &str, field: &'static str, max: u64) -> Result<u64, ConfigError> {
    let parsed: u64 = value
        .parse()
        .map_err(|_| ConfigError::Malformed { field })?;
    if parsed > max {
        return Err(ConfigError::OutOfBounds {
            field,
            value: parsed,
            max,
        });
    }
    Ok(parsed)
}

#[path = "config.tests.rs"]
#[cfg(test)]
mod tests;

mod config;
pub use config::Config;
mod config_error;
pub use config_error::ConfigError;

pub fn validate_provider_catalog(contents: &str) -> std::io::Result<()> {
    crate::provider_catalog::ProviderCatalog::validate_configuration(contents.as_bytes())
}
