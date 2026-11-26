use super::RequestError;

use core::fmt;

use carosign_protocol::carosign::client::v1 as client;
use carosign_protocol::carosign::client::v1::client_frame::Payload;
use carosign_protocol::carosign::common::v1 as common;
use carosign_protocol::validation::{WireError, check_client_frame, check_frame_size};
use prost::Message;

/// Decoding does not authorize the subject selector, purpose hint, or authority reference.
#[derive(Clone, PartialEq)]
pub enum ClientRequest {
    Hello {
        major: u32,
        min_minor: u32,
        max_minor: u32,
    },
    ListAuthenticators,
    GetAuthenticator {
        authenticator: Vec<u8>,
    },
    ListEnrollments {
        authenticator: Vec<u8>,
        subject: common::SubjectSelector,
        authority_request_ref: Vec<u8>,
    },
    RenameEnrollment(client::RenameEnrollment),
    BeginVerification {
        idempotency_key: Vec<u8>,
        authenticator: Vec<u8>,
        subject: common::SubjectSelector,
        purpose_hint: common::VerificationPurpose,
        enrollment: Option<common::EnrollmentRef>,
        report_matched_enrollment: bool,
        authority_request_ref: Vec<u8>,
    },
    BeginEnrollment {
        idempotency_key: Vec<u8>,
        authenticator: Vec<u8>,
        label: String,
        subject: common::SubjectSelector,
        replace: Option<common::EnrollmentRef>,
        authority_request_ref: Vec<u8>,
    },
    BeginRemoval {
        idempotency_key: Vec<u8>,
        authenticator: Vec<u8>,
        enrollment: Vec<u8>,
        generation: u64,
        subject: common::SubjectSelector,
        authority_request_ref: Vec<u8>,
    },
    ObserveOperation {
        operation: Vec<u8>,
        /// Repeat events after this number.
        after_sequence: u64,
    },
    GetOperation {
        operation: Vec<u8>,
    },
    CancelOperation {
        operation: Vec<u8>,
    },
    GetHealth,
}

impl fmt::Debug for ClientRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Hello { .. } => "Hello(..)",
            Self::ListAuthenticators => "ListAuthenticators",
            Self::GetAuthenticator { .. } => "GetAuthenticator(..)",
            Self::ListEnrollments { .. } => "ListEnrollments(..)",
            Self::RenameEnrollment(_) => "RenameEnrollment(..)",
            Self::BeginVerification { .. } => "BeginVerification(..)",
            Self::BeginEnrollment { .. } => "BeginEnrollment(..)",
            Self::BeginRemoval { .. } => "BeginRemoval(..)",
            Self::ObserveOperation { .. } => "ObserveOperation(..)",
            Self::GetOperation { .. } => "GetOperation(..)",
            Self::CancelOperation { .. } => "CancelOperation(..)",
            Self::GetHealth => "GetHealth",
        })
    }
}

pub fn decode(bytes: &[u8]) -> Result<(client::ClientFrame, ClientRequest), RequestError> {
    check_frame_size(bytes)?;
    check_payload_assignment(bytes)?;
    let frame = client::ClientFrame::decode(bytes).map_err(|_| RequestError::Malformed)?;
    check_client_frame(&frame)?;

    let Some(payload) = &frame.payload else {
        return Err(RequestError::NotARequest);
    };

    let request = match payload {
        Payload::Hello(hello) => ClientRequest::Hello {
            major: hello.major,
            min_minor: hello.min_minor,
            max_minor: hello.max_minor,
        },
        Payload::ListAuthenticators(_) => ClientRequest::ListAuthenticators,
        Payload::GetAuthenticator(get) => ClientRequest::GetAuthenticator {
            authenticator: get.authenticator_id.clone(),
        },
        Payload::ListEnrollments(list) => ClientRequest::ListEnrollments {
            authenticator: list.authenticator_id.clone(),
            subject: list.subject.clone().ok_or(RequestError::Malformed)?,
            authority_request_ref: list.authority_request_ref.clone(),
        },
        Payload::RenameEnrollment(rename) => ClientRequest::RenameEnrollment(rename.clone()),
        Payload::BeginVerification(begin) => ClientRequest::BeginVerification {
            enrollment: begin.enrollment.clone(),
            report_matched_enrollment: begin.report_matched_enrollment,
            idempotency_key: begin.idempotency_key.clone(),
            authenticator: begin.authenticator_id.clone(),
            subject: begin.subject.clone().ok_or(RequestError::Malformed)?,
            purpose_hint: common::VerificationPurpose::from_i32(begin.purpose_hint)
                .ok_or(RequestError::Malformed)?,
            authority_request_ref: begin.authority_request_ref.clone(),
        },
        Payload::BeginEnrollment(begin) => ClientRequest::BeginEnrollment {
            idempotency_key: begin.idempotency_key.clone(),
            authenticator: begin.authenticator_id.clone(),
            label: begin.label.clone(),
            subject: begin.subject.clone().ok_or(RequestError::Malformed)?,
            replace: begin.replace.clone(),
            authority_request_ref: begin.authority_request_ref.clone(),
        },
        Payload::BeginRemoval(begin) => {
            let reference = begin.enrollment.clone().ok_or(WireError::BadId {
                field: "enrollment",
            })?;
            ClientRequest::BeginRemoval {
                idempotency_key: begin.idempotency_key.clone(),
                authenticator: reference.authenticator_id.clone(),
                enrollment: reference.enrollment_id.clone(),
                generation: reference.generation,
                subject: begin.subject.clone().ok_or(RequestError::Malformed)?,
                authority_request_ref: begin.authority_request_ref.clone(),
            }
        }
        Payload::ObserveOperation(observe) => ClientRequest::ObserveOperation {
            operation: observe.operation_id.clone(),
            after_sequence: observe.after_sequence,
        },
        Payload::GetOperation(get) => ClientRequest::GetOperation {
            operation: get.operation_id.clone(),
        },
        Payload::CancelOperation(cancel) => ClientRequest::CancelOperation {
            operation: cancel.operation_id.clone(),
        },
        Payload::GetHealth(_) => ClientRequest::GetHealth,
        _ => return Err(RequestError::NotARequest),
    };

    Ok((frame, request))
}

// prost retains the last oneof value; checking before parsing prevents an ambiguous frame
// from turning into a correct query.
fn check_payload_assignment(mut bytes: &[u8]) -> Result<(), RequestError> {
    use prost::encoding::{DecodeContext, WireType, decode_key, skip_field};
    let mut found = false;
    while !bytes.is_empty() {
        let (tag, wire_type) = decode_key(&mut bytes).map_err(|_| RequestError::Malformed)?;
        if matches!(tag, 10..=17 | 20..=33 | 90) {
            if found {
                return Err(RequestError::Malformed);
            }
            found = true;
            if matches!(tag, 16 | 20..=22) && wire_type == WireType::LengthDelimited {
                let payload = take_delimited(&mut bytes)?;
                check_subject_selector(payload, if tag == 16 { 1 } else { 2 })?;
                continue;
            }
        }
        skip_field(wire_type, tag, &mut bytes, DecodeContext::default())
            .map_err(|_| RequestError::Malformed)?;
    }
    Ok(())
}

fn take_delimited<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], RequestError> {
    let len = prost::encoding::decode_varint(bytes).map_err(|_| RequestError::Malformed)?;
    let len = usize::try_from(len).map_err(|_| RequestError::Malformed)?;
    if len > bytes.len() {
        return Err(RequestError::Malformed);
    }
    let (value, rest) = bytes.split_at(len);
    *bytes = rest;
    Ok(value)
}

fn check_subject_selector(mut bytes: &[u8], subject_tag: u32) -> Result<(), RequestError> {
    use prost::encoding::{DecodeContext, WireType, decode_key, skip_field};
    let mut found_subject = false;
    while !bytes.is_empty() {
        let (tag, wire_type) = decode_key(&mut bytes).map_err(|_| RequestError::Malformed)?;
        if tag == subject_tag && wire_type == WireType::LengthDelimited {
            if found_subject {
                return Err(RequestError::Malformed);
            }
            found_subject = true;
            let mut selector = take_delimited(&mut bytes)?;
            let mut found_choice = false;
            while !selector.is_empty() {
                let (tag, wire_type) =
                    decode_key(&mut selector).map_err(|_| RequestError::Malformed)?;
                if matches!(tag, 1..=3) {
                    if found_choice {
                        return Err(RequestError::Malformed);
                    }
                    found_choice = true;
                }
                skip_field(wire_type, tag, &mut selector, DecodeContext::default())
                    .map_err(|_| RequestError::Malformed)?;
            }
        } else {
            skip_field(wire_type, tag, &mut bytes, DecodeContext::default())
                .map_err(|_| RequestError::Malformed)?;
        }
    }
    Ok(())
}
