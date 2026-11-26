use std::sync::Arc;

use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::{client::v1::OperationEvent, common::v1::EnrollmentRef};

pub(crate) enum FingerprintUpdate {
    Accepted(Arc<Transport>, Vec<u8>),
    Progress(OperationEvent),
    Captured(Result<EnrollmentRef, &'static str>),
    Finished(Result<(), &'static str>),
    Renamed {
        reference: EnrollmentRef,
        expected: String,
        label: String,
        result: Result<(), &'static str>,
    },
}
