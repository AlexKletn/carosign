use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::{
    client::v1::{RenameEnrollment, client_frame::Payload},
    common::v1::{EnrollmentRef, ErrorCode},
};

use super::read_snapshot::CONTROL_SOCKET;

pub(super) fn rename(
    reference: &EnrollmentRef,
    expected: &str,
    label: &str,
) -> Result<(), &'static str> {
    let failure = "ui-fingerprint-rename-failed";
    let transport = Transport::connect_root(CONTROL_SOCKET).map_err(|_| failure)?;
    match transport
        .request(Payload::RenameEnrollment(RenameEnrollment {
            enrollment: Some(reference.clone()),
            expected_label: Some(expected.to_owned()),
            label: label.to_owned(),
        }))
        .map_err(|_| failure)?
        .payload
    {
        Some(Payload::EnrollmentRenamed(reply))
            if reply.enrollment.as_ref() == Some(reference) && reply.label == label =>
        {
            Ok(())
        }
        Some(Payload::ProtocolError(reply))
            if reply
                .error
                .as_ref()
                .is_some_and(|error| error.code() == ErrorCode::ErrorGenerationConflict) =>
        {
            Err("ui-fingerprint-rename-conflict")
        }
        _ => Err(failure),
    }
}
