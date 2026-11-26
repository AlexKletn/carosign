use carosign_core::device::DeviceEpoch;
use carosign_core::ids::LeaseId;
use carosign_core::ids::ResourceId;

/// What the mediator responded to the request.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GrantOutcome {
    /// Issued: leasing, era and exactly issued set.
    Granted {
        lease: LeaseId,
        epoch: DeviceEpoch,
        /// What is issued (in canonical order).
        resources: Vec<ResourceId>,
    },
    Busy,
    /// Not allowed by politics.
    Denied,
    /// The broker is unavailable.
    Unavailable,
}
