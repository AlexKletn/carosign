use super::{FrameGuard, TransportError};

use carosign_protocol::carosign::provider::v1::{ProviderFrame, provider_frame::Payload};
use carosign_protocol::validation::{
    WireError, bounded_str, check_frame_size, check_provider_envelope,
};
use prost::Message;
use zeroize::Zeroize;

// Erase a known copy of the token in a disassembled frame at each output, including a failure
// to verify. The raw buffer belongs to the calling code.

/// Counting side of the wire: checks the frame number in one direction.
#[derive(Debug)]
pub struct Channel {
    next_out: u64,
    next_in: u64,
    closed: bool,
}

impl Default for Channel {
    fn default() -> Self {
        Self::new()
    }
}

impl Channel {
    /// New Channel: Numbers start with 1 (document 06).
    pub fn new() -> Self {
        Self {
            next_out: 1,
            next_in: 1,
            closed: false,
        }
    }

    /// The number of the next frame we send.
    pub fn next_out(&self) -> u64 {
        self.next_out
    }

    /// The number of the next frame we're waiting for.
    pub fn next_in(&self) -> u64 {
        self.next_in
    }

    /// Prepares the frame for sending: number, size check, envelope check.
    pub fn prepare(&mut self, frame: ProviderFrame) -> Result<Vec<u8>, TransportError> {
        let mut frame = FrameGuard(frame);
        if self.closed {
            return Err(TransportError::Closed);
        }
        let Some(next) = self.next_out.checked_add(1) else {
            self.closed = true;
            return Err(TransportError::Closed);
        };
        frame.0.frame_seq = self.next_out;
        check_provider_envelope(&frame.0)?;
        // ⚠️ On top of the envelope - check the messages themselves: exceeding the field
        // limit can not be sent, even if the frame still fits into 64 KB.
        check_payload_bounds(&frame.0)?;
        let mut bytes = frame.0.encode_to_vec();
        if check_frame_size(&bytes).is_err() {
            let size = bytes.len();
            bytes.zeroize();
            return Err(TransportError::TooLarge(size));
        }
        self.next_out = next;
        Ok(bytes)
    }

    /// Accepts the frame: size check, parsing, envelope and number of this** direction.
    pub fn accept(&mut self, bytes: &[u8]) -> Result<ProviderFrame, TransportError> {
        if self.closed {
            return Err(TransportError::Closed);
        }
        let result = self.accept_frame(bytes);
        if result.is_err() {
            self.closed = true;
        }
        result
    }

    fn accept_frame(&mut self, bytes: &[u8]) -> Result<ProviderFrame, TransportError> {
        check_frame_size(bytes).map_err(|_| TransportError::TooLarge(bytes.len()))?;
        check_oneofs(bytes)?;
        // The protection must own the frame already during parsing: a late protobuf error can
        // occur after the token has hit the frame field.
        let mut frame = FrameGuard(ProviderFrame::default());
        frame
            .0
            .merge(bytes)
            .map_err(|_| TransportError::Malformed)?;
        check_provider_envelope(&frame.0)?;
        if frame.0.frame_seq != self.next_in {
            return Err(TransportError::Sequence {
                expected: self.next_in,
                actual: frame.0.frame_seq,
            });
        }
        check_payload_bounds(&frame.0)?;
        let next = self.next_in.checked_add(1).ok_or(TransportError::Closed)?;
        self.next_in = next;
        Ok(frame.take())
    }
}

// Check the original bytes: after parsing, the prost retains only the last oneof value, so
// the ambiguity can no longer be detected.
fn check_oneofs(mut bytes: &[u8]) -> Result<(), TransportError> {
    use prost::encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field};
    let mut found_payload = false;
    let mut found_uid = false;
    while !bytes.is_empty() {
        let (tag, wire) = decode_key(&mut bytes).map_err(|_| TransportError::Malformed)?;
        if tag == 8 {
            if found_uid || wire != WireType::Varint {
                return Err(TransportError::Malformed);
            }
            found_uid = true;
        }
        if matches!(tag, 10..=16 | 20..=26 | 30..=38 | 40..=43 | 90) {
            if found_payload {
                return Err(TransportError::Malformed);
            }
            found_payload = true;
            if tag == 32 && wire == WireType::LengthDelimited {
                let len = decode_varint(&mut bytes).map_err(|_| TransportError::Malformed)?;
                let len = usize::try_from(len).map_err(|_| TransportError::Malformed)?;
                if len > bytes.len() {
                    return Err(TransportError::Malformed);
                }
                let (mut outcome, rest) = bytes.split_at(len);
                bytes = rest;
                let mut found_evidence = false;
                while !outcome.is_empty() {
                    let (tag, wire) =
                        decode_key(&mut outcome).map_err(|_| TransportError::Malformed)?;
                    if matches!(tag, 2 | 3) {
                        if found_evidence {
                            return Err(TransportError::Malformed);
                        }
                        found_evidence = true;
                    }
                    skip_field(wire, tag, &mut outcome, DecodeContext::default())
                        .map_err(|_| TransportError::Malformed)?;
                }
                continue;
            }
        }
        skip_field(wire, tag, &mut bytes, DecodeContext::default())
            .map_err(|_| TransportError::Malformed)?;
    }
    Ok(())
}

/// What's in the frame.
pub fn payload_name(frame: &ProviderFrame) -> &'static str {
    match &frame.payload {
        Some(Payload::WorkerHello(_)) => "WorkerHello",
        Some(Payload::Initialize(_)) => "Initialize",
        Some(Payload::Initialized(_)) => "Initialized",
        Some(Payload::BindAuthenticator(_)) => "BindAuthenticator",
        Some(Payload::AuthenticatorReady(_)) => "AuthenticatorReady",
        Some(Payload::Describe(_)) => "Describe",
        Some(Payload::Described(_)) => "Described",
        Some(Payload::PrebindDescribe(_)) => "PrebindDescribe",
        Some(Payload::PrebindDescription(_)) => "PrebindDescription",
        Some(Payload::StartVerification(_)) => "StartVerification",
        Some(Payload::StartEnrollment(_)) => "StartEnrollment",
        Some(Payload::ContinueEnrollment(_)) => "ContinueEnrollment",
        Some(Payload::StartRemoval(_)) => "StartRemoval",
        Some(Payload::QueryMutation(_)) => "QueryMutation",
        Some(Payload::RequestCancel(_)) => "RequestCancel",
        Some(Payload::EnrollmentTransactionRequest(_)) => "EnrollmentTransactionRequest",
        Some(Payload::EnrollmentTransactionResult(_)) => "EnrollmentTransactionResult",
        Some(Payload::CommandAck(_)) => "CommandAck",
        Some(Payload::ProviderProgress(_)) => "ProviderProgress",
        Some(Payload::VerificationOutcome(_)) => "VerificationOutcome",
        Some(Payload::MutationOutcome(_)) => "MutationOutcome",
        Some(Payload::QueryMutationResult(_)) => "QueryMutationResult",
        Some(Payload::CancelReceipt(_)) => "CancelReceipt",
        Some(Payload::Quiesce(_)) => "Quiesce",
        Some(Payload::Quiesced(_)) => "Quiesced",
        Some(Payload::CloseAuthenticator(_)) => "CloseAuthenticator",
        Some(Payload::Shutdown(_)) => "Shutdown",
        Some(Payload::PrepareEnrollmentChallenge(_)) => "PrepareEnrollmentChallenge",
        Some(Payload::EnrollmentChallengeReady(_)) => "EnrollmentChallengeReady",
        Some(Payload::ProviderFault(_)) => "ProviderFault",
        None => "пусто",
    }
}

/// Checks known limitations of incoming and outgoing message fields.
fn check_payload_bounds(frame: &ProviderFrame) -> Result<(), TransportError> {
    use carosign_protocol::limits;
    use carosign_protocol::validation::bounded;

    match &frame.payload {
        Some(Payload::WorkerHello(hello)) => {
            carosign_protocol::validation::check_worker_hello(hello)?;
            bounded_str(
                &hello.implementation_version,
                "implementation_version",
                limits::IMPLEMENTATION_VERSION_MAX,
            )?;
        }
        Some(Payload::BindAuthenticator(bind)) => {
            carosign_protocol::validation::check_bind(bind)?;
            if bind
                .authenticator
                .as_ref()
                .map(|reference| reference.device_epoch)
                != Some(frame.device_epoch)
            {
                return Err(TransportError::Invalid(WireError::UnexpectedPayload {
                    what: "эпоха привязки не совпадает с конвертом",
                }));
            }
        }
        Some(Payload::PrebindDescribe(request)) => {
            let authenticator = request
                .authenticator
                .as_ref()
                .ok_or(TransportError::Invalid(WireError::BadId {
                    field: "authenticator",
                }))?;
            carosign_protocol::validation::require_id(
                &authenticator.authenticator_id,
                "authenticator_id",
            )?;
            if authenticator.device_epoch == 0 || authenticator.device_epoch != frame.device_epoch {
                return Err(TransportError::Invalid(WireError::UnexpectedPayload {
                    what: "эпоха декларации не совпадает",
                }));
            }
        }
        Some(Payload::PrebindDescription(description)) => {
            let authenticator =
                description
                    .authenticator
                    .as_ref()
                    .ok_or(TransportError::Invalid(WireError::BadId {
                        field: "authenticator",
                    }))?;
            carosign_protocol::validation::require_id(
                &authenticator.authenticator_id,
                "authenticator_id",
            )?;
            if description.declaration_version != 1
                || authenticator.device_epoch == 0
                || authenticator.device_epoch != frame.device_epoch
                || description.capabilities.is_none()
            {
                return Err(TransportError::Invalid(WireError::UnexpectedPayload {
                    what: "неверная декларация возможностей",
                }));
            }
            let capability = description.capabilities.as_ref().expect("проверено выше");
            if capability.template_format_id.len() > limits::FORMAT_ID_MAX
                || capability.operations.len() > 32
                || capability.interaction_modes.len() > 32
            {
                return Err(TransportError::Invalid(WireError::UnexpectedPayload {
                    what: "декларация превышает предел",
                }));
            }
        }
        Some(
            Payload::CancelReceipt(_)
            | Payload::EnrollmentTransactionResult(_)
            | Payload::Quiesced(_)
            | Payload::CommandAck(_)
            | Payload::ProviderProgress(_)
            | Payload::Described(_)
            | Payload::ProviderFault(_),
        ) => {
            crate::events::parse(frame).map_err(|error| match error {
                crate::events::EventError::Wire(error) => TransportError::Invalid(error),
                _ => TransportError::Invalid(WireError::UnexpectedPayload {
                    what: "неверное событие провайдера",
                }),
            })?;
        }
        Some(Payload::Initialize(initialize)) => {
            bounded(&initialize.package_id, "package_id", limits::ID_LEN)?;
            bounded(&initialize.profile_id, "profile_id", limits::ID_LEN)?;
        }
        Some(Payload::StartEnrollment(start)) => {
            bounded_str(&start.label, "label", limits::LABEL_MAX)?;
        }
        Some(Payload::ContinueEnrollment(continuation)) => {
            let start = continuation
                .enrollment
                .as_ref()
                .ok_or(TransportError::Invalid(WireError::UnexpectedPayload {
                    what: "нет продолжения регистрации",
                }))?;
            bounded_str(&start.label, "label", limits::LABEL_MAX)?;
        }
        Some(Payload::StartRemoval(start)) => {
            bounded(
                &start.provider_opaque_ref,
                "provider_opaque_ref",
                limits::OPAQUE_REF_MAX,
            )?;
        }
        Some(Payload::QueryMutation(query)) => {
            bounded(&query.recovery_ref, "recovery_ref", limits::OPAQUE_REF_MAX)?;
        }
        Some(Payload::QueryMutationResult(result)) => {
            crate::events::check_query_result(result).map_err(|error| match error {
                crate::events::EventError::Wire(error) => TransportError::Invalid(error),
                _ => TransportError::Invalid(WireError::UnexpectedPayload {
                    what: "неверный ответ сверки",
                }),
            })?;
        }
        Some(Payload::PrepareEnrollmentChallenge(command)) => {
            carosign_protocol::validation::require_id(&command.mutation_id, "mutation_id")?;
            let authenticator = command
                .authenticator
                .as_ref()
                .ok_or(TransportError::Invalid(WireError::BadId {
                    field: "authenticator",
                }))?;
            carosign_protocol::validation::require_id(
                &authenticator.authenticator_id,
                "authenticator_id",
            )?;
            if authenticator.device_epoch == 0 || authenticator.device_epoch != frame.device_epoch {
                return Err(TransportError::Invalid(WireError::UnexpectedPayload {
                    what: "эпоха подготовки challenge не совпадает с конвертом",
                }));
            }
            let subject =
                command
                    .subject
                    .as_ref()
                    .ok_or(TransportError::Invalid(WireError::BadId {
                        field: "subject",
                    }))?;
            carosign_protocol::validation::require_id(&subject.authority_id, "authority_id")?;
            carosign_protocol::validation::require_id(&subject.subject_id, "subject_id")?;
        }
        Some(Payload::EnrollmentChallengeReady(ready)) => {
            carosign_protocol::validation::require_id(&ready.mutation_id, "mutation_id")?;
            carosign_protocol::validation::require_id(&ready.challenge_ref, "challenge_ref")?;
            if ready.challenge.is_empty() || ready.challenge.len() > 64 || ready.validity_ms == 0 {
                return Err(TransportError::Invalid(WireError::UnexpectedPayload {
                    what: "неверная форма регистрационного challenge",
                }));
            }
        }
        Some(Payload::MutationOutcome(outcome)) => {
            crate::events::check_mutation_outcome(outcome).map_err(|error| match error {
                crate::events::EventError::Wire(error) => TransportError::Invalid(error),
                _ => TransportError::Invalid(WireError::UnexpectedPayload {
                    what: "неверный исход изменения",
                }),
            })?;
        }
        Some(Payload::VerificationOutcome(outcome)) => {
            carosign_protocol::validation::check_verification_outcome(outcome)?;
            if let Some(error) = &outcome.error {
                crate::events::check_error(error).map_err(|error| match error {
                    crate::events::EventError::Wire(error) => TransportError::Invalid(error),
                    _ => TransportError::Invalid(WireError::UnexpectedPayload {
                        what: "неверная ошибка проверки",
                    }),
                })?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[path = "channel.tests.rs"]
#[cfg(test)]
mod tests;
