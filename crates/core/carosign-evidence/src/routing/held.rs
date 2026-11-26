use super::MaterialState;
use super::RoutingError;
use crate::material::EvidenceMaterialRef;

/// Material at delivery.
#[derive(Debug, PartialEq, Eq)]
pub struct Held {
    reference: EvidenceMaterialRef,
    state: MaterialState,
}

impl Held {
    /// Creates a local link state.
    ///
    /// Re-creation with the same reference is not detectable by this type; a unique delivery
    /// is provided by limited delivery.
    pub fn hold(reference: EvidenceMaterialRef) -> Self {
        Self {
            reference,
            state: MaterialState::Held,
        }
    }

    /// He's checking it out once.
    pub fn take(&mut self, operation: [u8; 16]) -> Result<EvidenceMaterialRef, RoutingError> {
        if self.state != MaterialState::Held {
            return Err(RoutingError::AlreadyConsumed);
        }
        if self.reference.operation() != operation {
            return Err(RoutingError::WrongOperation);
        }
        self.state = MaterialState::Consumed;
        Ok(self.reference.clone())
    }

    /// Revokes the link for the same operation.
    ///
    /// There are no bytes of material here: a link retraction alone does not prove the token
    /// is cleared in a limited storage.
    pub fn invalidate(&mut self, operation: [u8; 16]) -> Result<(), RoutingError> {
        if self.reference.operation() != operation {
            return Err(RoutingError::WrongOperation);
        }
        self.state = MaterialState::Destroyed;
        Ok(())
    }

    /// The state of the reference without access to its change from the outside.
    pub fn state(&self) -> MaterialState {
        self.state
    }
}
