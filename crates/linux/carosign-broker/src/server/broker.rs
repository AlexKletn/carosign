use super::{Call, CallResult, DispatchAuthorization};

use core::fmt;

use carosign_core::device::DeviceEpoch;

use crate::policy::{Policy, PolicyError};

/// A mediator.
pub struct Broker<'a> {
    policy: &'a Policy,
    armed: &'a dyn DispatchAuthorization,
    current_epoch: DeviceEpoch,
}

impl fmt::Debug for Broker<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Broker(..)")
    }
}

impl<'a> Broker<'a> {
    /// Gathering a middleman.
    pub fn new(
        policy: &'a Policy,
        armed: &'a dyn DispatchAuthorization,
        current_epoch: DeviceEpoch,
    ) -> Self {
        Self {
            policy,
            armed,
            current_epoch,
        }
    }

    /// It only checks the form of the challenge, the era and local politics.
    /// This result does not permit execution without independent verification of the live
    /// issue, owner, resource and origin of the request.
    ///
    /// ⚠️ The order of inspections is part of the contract: first the era (which is not ours
    /// at all), then the processor, then the permission to change.
    pub fn check(&self, call: &Call) -> CallResult {
        if call.epoch != self.current_epoch {
            return CallResult::Unauthorized;
        }
        let handler = match self
            .policy
            .handler(call.operation_ref, call.package, call.resource)
        {
            Ok(handler) => handler,
            Err(PolicyError::Unsupported) => return CallResult::Unsupported,
            Err(_) => return CallResult::Unauthorized,
        };

        if handler.may_mutate {
            let (Some(operation), Some(mutation), Some(command)) =
                (call.operation, call.mutation, call.command)
            else {
                return CallResult::Unauthorized;
            };
            if !self.armed.is_armed(operation, mutation, command) {
                return CallResult::Unauthorized;
            }
        } else if call.operation.is_none() || call.mutation.is_some() || call.command.is_some() {
            // An exception for a separately approved read-only probe is not provided in the
            // current registry; without it, circumvention is prohibited.
            return CallResult::Unauthorized;
        }

        CallResult::Handled
    }
}
