use carosign_core::ids::{PackageId, ProfileId, ProviderId};

use super::PreparationError;

/// Identity of the first installation: they are not permission to publish.
/// Resilience is acquired only after a confirmed registry record.
pub(super) struct InitialIdentities {
    pub(super) package: PackageId,
    pub(super) provider: ProviderId,
    pub(super) profile: ProfileId,
}

pub(super) fn allocate_initial_identities(
    mut next: impl FnMut() -> Option<[u8; 16]>,
) -> Result<InitialIdentities, PreparationError> {
    // Repeating or rejecting a source prevents preparation: you can't substitute randomness
    // with a counter or time.
    let package = next().ok_or(PreparationError::IdUnavailable)?;
    let provider = next().ok_or(PreparationError::IdUnavailable)?;
    let profile = next().ok_or(PreparationError::IdUnavailable)?;
    if package == provider || package == profile || provider == profile {
        return Err(PreparationError::IdUnavailable);
    }
    Ok(InitialIdentities {
        package: PackageId::from_bytes(package).map_err(|_| PreparationError::IdUnavailable)?,
        provider: ProviderId::from_bytes(provider).map_err(|_| PreparationError::IdUnavailable)?,
        profile: ProfileId::from_bytes(profile).map_err(|_| PreparationError::IdUnavailable)?,
    })
}

#[cfg(test)]
#[path = "initial_identities.tests.rs"]
mod tests;
