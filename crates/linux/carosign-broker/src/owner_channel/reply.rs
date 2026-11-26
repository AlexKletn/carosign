use super::OwnerSession;
use std::os::fd::OwnedFd;

/// The result of one attempt: The FDs obtained belong to the broker; their source, type
/// and compliance with the established resources should be verified separately.
pub enum Reply {
    Prepared {
        channel: OwnerSession,
        descriptors: Vec<OwnedFd>,
    },
    Refused(carosign_protocol::carosign::common::v1::Error),
}
