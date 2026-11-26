use crate::generated::carosign::provider::v1 as provider;
use crate::limits;

use super::super::{WireError, require_id};

/// Checking the authenticator's binding.
///
/// ⚠️ Descriptors are the only place where v1 is allowed. `SCM_RIGHTS`.
/// Here only the declared part is checked; the descriptors themselves check the transport
/// (`carosign-linux-ipc`), comparing the number of slots accepted with the number of slots.
pub fn check_bind(bind: &provider::BindAuthenticator) -> Result<(), WireError> {
    if bind.fd_slots.len() > limits::FD_SLOTS_MAX {
        return Err(WireError::FieldTooLong {
            field: "fd_slots",
            actual: bind.fd_slots.len(),
            allowed: limits::FD_SLOTS_MAX,
        });
    }
    require_id(&bind.lease_id, "lease_id")?;
    require_id(&bind.profile_id, "profile_id")?;
    let Some(reference) = &bind.authenticator else {
        return Err(WireError::BadId {
            field: "authenticator",
        });
    };
    require_id(&reference.authenticator_id, "authenticator_id")?;
    if reference.device_epoch == 0 {
        return Err(WireError::BadId {
            field: "authenticator.device_epoch",
        });
    }

    // ⚠️ Indices must cover 0..N-1 ** exactly once **.
    let mut seen = vec![false; bind.fd_slots.len()];
    for slot in &bind.fd_slots {
        let index = slot.index as usize;
        if index >= seen.len() || seen[index] {
            return Err(WireError::FdMismatch {
                declared: bind.fd_slots.len(),
                received: index,
            });
        }
        seen[index] = true;
        require_id(&slot.resource_id, "resource_id")?;
        if slot.role == provider::FdRole::Unspecified as i32 {
            return Err(WireError::UnspecifiedEnum {
                field: "fd_slots.role",
            });
        }
        if provider::FdRole::from_i32(slot.role).is_none() {
            return Err(WireError::UnknownEnum {
                field: "fd_slots.role",
                value: slot.role,
            });
        }
    }
    Ok(())
}
