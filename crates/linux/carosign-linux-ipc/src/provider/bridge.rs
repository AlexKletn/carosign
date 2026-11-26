//! Bridge: worker frame → engine observation.
//!
//! ⚠️ The bridge doesn't rule the fortune, it doesn't decide the fate of the operation: it
//! translates and gives. Everything else is an actor.

use carosign_engine::DomainInput;
use carosign_protocol::carosign::provider::v1 as provider;

use crate::mapping;

/// Turns the frame into observation.
pub fn observe(
    frame: &provider::ProviderFrame,
) -> Result<Option<DomainInput>, crate::codec::RequestError> {
    let event = carosign_provider_sdk::events::parse(frame);
    match event {
        Ok(event) => mapping::provider::to_domain(event).map(Some),
        Err(carosign_provider_sdk::events::EventError::NotAnEvent) => Ok(None),
        Err(carosign_provider_sdk::events::EventError::Wire(error)) => {
            Err(crate::codec::RequestError::Wire(error))
        }
        Err(
            carosign_provider_sdk::events::EventError::Enum { .. }
            | carosign_provider_sdk::events::EventError::Percent(_),
        ) => Err(crate::codec::RequestError::Malformed),
    }
}

#[path = "bridge.tests.rs"]
#[cfg(test)]
mod tests;
