use carosign_runtime::operation::observer::{OBSERVERS_HARD_MAX, OBSERVERS_MAX};

use super::{
    ConfigError, DEFAULT_POLICY, DEFAULT_SOCKET, DEFAULT_STORE, absolute, bounded, socket_path,
};

/// Settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub socket: String,
    /// Warehouse.
    pub store: String,
    /// A separate previously created storage of hardware resource fences.
    pub resource_fence_store: Option<String>,
    /// The path to a secure root-owned policy file.
    pub policy: String,
    /// Secure startup profile of the provider; by default, there are no providers.
    pub provider_profile: Option<String>,
    /// Limit of connection operations (not more than 4 without confirmation).
    pub ops_per_connection: u8,
    /// The limit of observers on the operation (no more than 32).
    pub observers_per_operation: u8,
    pub queued_per_authenticator: u8,
    /// Greeting period, seconds (no more than 5).
    pub hello_timeout_s: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socket: DEFAULT_SOCKET.to_string(),
            store: DEFAULT_STORE.to_string(),
            resource_fence_store: None,
            policy: DEFAULT_POLICY.to_string(),
            provider_profile: None,
            ops_per_connection: 4,
            observers_per_operation: OBSERVERS_MAX as u8,
            queued_per_authenticator: 16,
            hello_timeout_s: 5,
        }
    }
}

impl Config {
    /// He's parsing the settings.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let mut config = Self::default();
        let mut seen = std::collections::BTreeSet::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=').ok_or(ConfigError::Malformed {
                field: "строка",
            })?;
            let key = key.trim();
            let value = value.trim();
            if !seen.insert(key) {
                return Err(ConfigError::Malformed {
                    field: "повтор поля",
                });
            }
            match key {
                "socket" => config.socket = socket_path(value)?,
                "store" => config.store = absolute(value, "store")?,
                "resource_fence_store" => {
                    config.resource_fence_store = Some(absolute(value, "resource_fence_store")?)
                }
                "policy" => config.policy = absolute(value, "policy")?,
                "provider_profile" => {
                    config.provider_profile = Some(absolute(value, "provider_profile")?)
                }
                "ops_per_connection" => {
                    config.ops_per_connection = bounded(value, "ops_per_connection", 16)? as u8
                }
                "observers_per_operation" => {
                    config.observers_per_operation =
                        bounded(value, "observers_per_operation", OBSERVERS_HARD_MAX as u64)? as u8
                }
                "queued_per_authenticator" => {
                    config.queued_per_authenticator =
                        bounded(value, "queued_per_authenticator", 64)? as u8
                }
                "hello_timeout_s" => {
                    config.hello_timeout_s = bounded(value, "hello_timeout_s", 5)? as u8
                }
                other => return Err(ConfigError::UnknownField(other.to_string())),
            }
        }
        config.validate()?;
        Ok(config)
    }

    /// Checking limits.
    ///
    /// ⚠️ Zero is unacceptable: "zero connection operations" is not rigor, it's a broken
    /// service.
    pub fn validate(&self) -> Result<(), ConfigError> {
        socket_path(&self.socket)?;
        absolute(&self.store, "store")?;
        if let Some(path) = &self.resource_fence_store {
            absolute(path, "resource_fence_store")?;
            if path == &self.store {
                return Err(ConfigError::BadPath {
                    field: "resource_fence_store",
                });
            }
        }
        absolute(&self.policy, "policy")?;
        if let Some(path) = &self.provider_profile {
            absolute(path, "provider_profile")?;
        }
        if self.ops_per_connection == 0 || self.ops_per_connection > 4 {
            return Err(ConfigError::OutOfBounds {
                field: "ops_per_connection",
                value: u64::from(self.ops_per_connection),
                max: 4,
            });
        }
        if self.observers_per_operation == 0
            || usize::from(self.observers_per_operation) > OBSERVERS_HARD_MAX
        {
            return Err(ConfigError::OutOfBounds {
                field: "observers_per_operation",
                value: u64::from(self.observers_per_operation),
                max: OBSERVERS_HARD_MAX as u64,
            });
        }
        if self.queued_per_authenticator > 64 {
            return Err(ConfigError::OutOfBounds {
                field: "queued_per_authenticator",
                value: u64::from(self.queued_per_authenticator),
                max: 64,
            });
        }
        if self.hello_timeout_s == 0 || self.hello_timeout_s > 5 {
            return Err(ConfigError::OutOfBounds {
                field: "hello_timeout_s",
                value: u64::from(self.hello_timeout_s),
                max: 5,
            });
        }
        Ok(())
    }
}
