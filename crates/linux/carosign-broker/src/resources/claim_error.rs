use super::fd_role::FdRole;
use super::resource_kind::ResourceKind;
use carosign_core::ids::ResourceId;
use core::fmt;

/// What kind of application doesn't pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClaimError {
    /// The role doesn't fit the kind of host.
    RoleMismatch {
        role: FdRole,
        /// Carrier.
        kind: ResourceKind,
    },
    /// The resource is not issued in this leasing.
    NotInLease {
        resource: ResourceId,
    },
    /// Repeat the resource in one issue.
    Duplicate {
        resource: ResourceId,
    },
    /// It's not the era.
    StaleEpoch,
}

impl fmt::Display for ClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RoleMismatch { role, kind } => {
                write!(f, "роль {role:?} не подходит к {kind:?}")
            }
            Self::NotInLease { resource } => {
                write!(f, "ресурс {} не выдан в этом лизинге", hex(*resource))
            }
            Self::Duplicate { resource } => write!(f, "ресурс {} повторён", hex(*resource)),
            Self::StaleEpoch => f.write_str("эпоха не та"),
        }
    }
}

impl core::error::Error for ClaimError {}

fn hex(resource: ResourceId) -> String {
    resource.as_bytes()[..2]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
