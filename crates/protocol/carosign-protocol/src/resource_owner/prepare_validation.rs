use crate::carosign::{
    broker_relay::v1 as relay, provider::v1::FdRole, resource_owner::v1 as owner,
};

use super::{Error, Outcome, quiescence::id};

/// Checks the form of the request; powers and fencing are checked separately.
pub fn validate_prepare(request: &owner::Prepare) -> Result<(), Error> {
    let scope = request.scope.as_ref().ok_or(Error::InvalidRequest)?;
    let who = scope.owner.as_ref().ok_or(Error::InvalidRequest)?;
    if !id(&request.request_id)
        || !id(&request.boot_id)
        || !id(&scope.lease_id)
        || !id(&who.operation_id)
        || !id(&who.worker_instance)
        || !id(&who.authenticator_id)
        || who.device_epoch == 0
        || scope.claims.is_empty()
        || scope.claims.len() > 8
        || request.slots.len() > 8
    {
        return Err(Error::InvalidRequest);
    }
    for (index, claim) in scope.claims.iter().enumerate() {
        if !id(&claim.resource_id)
            || claim.mode != relay::AccessMode::Exclusive as i32
            || scope.claims[..index]
                .iter()
                .any(|other| other.resource_id == claim.resource_id)
        {
            return Err(Error::InvalidRequest);
        }
    }
    for (index, slot) in request.slots.iter().enumerate() {
        if !id(&slot.resource_id)
            || !(slot.role == FdRole::Device as i32 || slot.role == FdRole::Config as i32)
            || !scope
                .claims
                .iter()
                .any(|claim| claim.resource_id == slot.resource_id)
            || request.slots[..index]
                .iter()
                .any(|other| other.resource_id == slot.resource_id)
        {
            return Err(Error::InvalidRequest);
        }
    }
    Ok(())
}

/// Verifies the answer with the original request and the number of descriptors already
/// received.
/// In case of any error, the transport must close all received descriptors and channel.
pub fn validate_response(
    request: &owner::Prepare,
    response: &owner::Response,
    fd_count: usize,
) -> Result<Outcome, Error> {
    validate_prepare(request)?;
    if response.request_id != request.request_id || response.boot_id != request.boot_id {
        return Err(Error::MismatchedResponse);
    }
    match &response.result {
        Some(owner::response::Result::Prepared(prepared)) => {
            if prepared.scope != request.scope || prepared.slots != request.slots {
                return Err(Error::MismatchedResponse);
            }
            if fd_count != request.slots.len() {
                return Err(Error::InvalidDescriptors);
            }
            Ok(Outcome::Prepared)
        }
        Some(owner::response::Result::Refused(error)) => {
            if fd_count != 0 {
                return Err(Error::InvalidDescriptors);
            }
            if error.code == 0
                || crate::carosign::common::v1::ErrorCode::from_i32(error.code).is_none()
                || error.safe_message.len() > 256
            {
                return Err(Error::MismatchedResponse);
            }
            Ok(Outcome::Refused)
        }
        None => Err(Error::MismatchedResponse),
    }
}
