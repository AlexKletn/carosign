use carosign_core::ids::LeaseId;

/// How did the extradition end?
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GrantResult {
    /// Issued.
    Granted {
        lease: LeaseId,
    },
    /// Occupied; the request is in line.
    Queued,
    /// Rejected immediately: incompatible set or filled queue.
    Denied,
}
