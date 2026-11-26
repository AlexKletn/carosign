use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::{
    client::v1::{Authenticators, ListEnrollments, client_frame::Payload},
    common::v1::{EnrollmentSummary, Modality, SubjectSelector, subject_selector::Selector},
};
use carosign_protocol::limits::ENROLLMENTS_PER_RESPONSE_MAX;

use crate::domain::QueryError;

pub(super) fn read_enrollments(
    transport: &Transport,
    inventory: &Authenticators,
) -> Result<Vec<EnrollmentSummary>, QueryError> {
    let mut enrollments = Vec::new();
    for device in &inventory.entries {
        if !device
            .capabilities
            .as_ref()
            .is_some_and(|capabilities| capabilities.modality() == Modality::Fingerprint)
        {
            continue;
        }
        let response = transport.request(Payload::ListEnrollments(ListEnrollments {
            subject: Some(SubjectSelector {
                selector: Some(Selector::CurrentSubject(true)),
            }),
            authenticator_id: device.authenticator_id.clone(),
            authority_request_ref: Vec::new(),
        }))?;
        match response.payload {
            Some(Payload::Enrollments(items))
                if items.entries.len() <= ENROLLMENTS_PER_RESPONSE_MAX =>
            {
                enrollments.extend(items.entries);
            }
            Some(Payload::ProtocolError(error)) => return Err(error.into()),
            _ => return Err(QueryError::Unexpected),
        }
    }
    enrollments.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(enrollments)
}
