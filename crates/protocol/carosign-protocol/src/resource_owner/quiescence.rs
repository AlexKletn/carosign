use super::{Error, validate_prepare};

use crate::carosign::resource_owner::v1 as owner;

/// Limit unique surveys of a single issue; the workforce uses 5s/10ms.
pub const QUIESCENCE_REQUEST_LIMIT: usize = 512;

/// No intermediate answer confirms physical peace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Quiescence {
    Draining,
    HardwareUnconfirmed,
    Unknown,
    /// Withdrawal of FD issuance; absence of other FDs and general rest are not proven.
    LeaseRevoked,
    /// Statement of the trusted owner on the withdrawal of exclusive hardware issuance.
    ExclusiveLeaseRevoked,
    /// The hardware operation has been completed and the exclusive issue has been withdrawn.
    ExclusiveQuiesced,
}

pub(super) fn id(value: &[u8]) -> bool {
    value.len() == 16 && value.iter().any(|byte| *byte != 0)
}

/// The request for inspection refers to the full extradition previously prepared.
pub fn validate_quiescence_request(
    prepare: &owner::Prepare,
    request: &owner::InspectQuiescence,
) -> Result<(), Error> {
    validate_prepare(prepare)?;
    if !id(&request.request_id)
        || request.request_id == prepare.request_id
        || request.boot_id != prepare.boot_id
        || request.scope != prepare.scope
    {
        return Err(Error::InvalidRequest);
    }
    Ok(())
}

/// All answers are observations and do not remove the durable fence.
pub fn validate_quiescence_observation(
    prepare: &owner::Prepare,
    request: &owner::InspectQuiescence,
    response: &owner::QuiescenceObservation,
    fd_count: usize,
) -> Result<Quiescence, Error> {
    validate_quiescence_request(prepare, request)?;
    if fd_count != 0 {
        return Err(Error::InvalidDescriptors);
    }
    if response.request_id != request.request_id
        || response.boot_id != request.boot_id
        || response.scope != request.scope
    {
        return Err(Error::MismatchedResponse);
    }
    match response.stage {
        1 => Ok(Quiescence::Draining),
        2 => Ok(Quiescence::HardwareUnconfirmed),
        3 => Ok(Quiescence::Unknown),
        4 => Ok(Quiescence::LeaseRevoked),
        5 => Ok(Quiescence::ExclusiveLeaseRevoked),
        6 => Ok(Quiescence::ExclusiveQuiesced),
        _ => Err(Error::MismatchedResponse),
    }
}
