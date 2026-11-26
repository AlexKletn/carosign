use super::DeviceEvent;
use core::fmt;

/// Device status.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceState {
    /// Dismissed.
    Available,
    /// Reserved for surgery.
    Reserved,
    /// Working.
    Active,
    /// Calms down.
    Quiescing,
    /// Status unknown.
    Uncertain,
    /// It's recovering.
    Recovering,
    Unavailable,
}

impl fmt::Debug for DeviceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Available => "Available",
            Self::Reserved => "Reserved",
            Self::Active => "Active",
            Self::Quiescing => "Quiescing",
            Self::Uncertain => "Uncertain",
            Self::Recovering => "Recovering",
            Self::Unavailable => "Unavailable",
        })
    }
}

/// Next state.
///
/// ⚠️ From `Uncertain` You can only get out through `Recover`from `Recovering` in `Available`
/// **Only **only confirmed. `Quiesced`The other way. `None`.
pub fn next_state(state: DeviceState, event: DeviceEvent) -> Option<DeviceState> {
    use DeviceEvent as E;
    use DeviceState as S;
    match (state, event) {
        (S::Available, E::Reserved) => Some(S::Reserved),
        (S::Reserved, E::Started) => Some(S::Active),
        (S::Active, E::Quiesce) => Some(S::Quiescing),
        (S::Quiescing, E::Quiesced) => Some(S::Available),
        (_, E::Disconnected) => Some(S::Unavailable),
        // Connection does not prove the rest of the device; separate cleaning and testing of
        // the new era is required before re-issuing resources.
        (S::Unavailable, E::Reconnected) => Some(S::Uncertain),
        (S::Recovering, E::Quiesced) => Some(S::Available),
        (S::Unavailable, E::Fault) => Some(S::Unavailable),
        (_, E::Fault) => Some(S::Uncertain),
        (S::Uncertain, E::Recover) => Some(S::Recovering),
        _ => None,
    }
}

/// Can I start a new backup of the device?
///
/// This is a necessary condition, but not the issuance of a lease or the verification of its
/// owner, era or readiness of the worker.
pub fn can_reserve(state: DeviceState) -> bool {
    matches!(state, DeviceState::Available)
}

#[path = "device_state.tests.rs"]
#[cfg(test)]
mod tests;
