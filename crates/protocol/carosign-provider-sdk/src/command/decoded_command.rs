use super::{CommandError, EnrollmentAuthorization, EnrollmentChange};

use core::fmt;

use carosign_protocol::carosign::common::v1 as common;
use carosign_protocol::carosign::provider::v1 as provider;
use carosign_protocol::limits;
use carosign_protocol::validation::{WireError, bounded, require_id};

/// What the platform asks you to do.
#[derive(Clone, PartialEq)]
pub enum Command {
    /// Prepare a one-time call for registration without changing templates.
    PrepareEnrollmentChallenge {
        /// The user’s UID specified by the platform.
        uid: u32,
        operation: Vec<u8>,
        authenticator: common::AuthenticatorRef,
        /// The opaque subject of the platform.
        subject: common::SubjectRef,
        /// The number of the pre-prepared change.
        mutation: Vec<u8>,
    },
    StartVerification {
        /// The user’s UID specified by the platform.
        uid: u32,
        operation: Vec<u8>,
        authenticator: common::AuthenticatorRef,
        /// Subject (opaque to the worker).
        subject: common::SubjectRef,
        purpose: common::VerificationPurpose,
        /// A call reference (not the call itself).
        challenge_ref: Vec<u8>,
        recipient_ref: Vec<u8>,
        /// Allowed registrations with generation and storage method.
        permitted_enrollments: Vec<provider::EnrollmentBinding>,
    },
    StartEnrollment {
        /// The user’s UID specified by the platform.
        uid: u32,
        operation: Vec<u8>,
        authenticator: common::AuthenticatorRef,
        subject: common::SubjectRef,
        /// Change number (must be already on the disk at the platform).
        mutation: Vec<u8>,
        label: String,
        /// Creation or replacement with the previous link.
        change: EnrollmentChange,
        /// References to authorization when required by the provider.
        authorization: EnrollmentAuthorization,
    },
    /// Continuation of registration started by the trusted owner.
    ContinueEnrollment { enrollment: Box<Command> },
    StartRemoval {
        /// The user’s UID specified by the platform.
        uid: u32,
        operation: Vec<u8>,
        authenticator: common::AuthenticatorRef,
        subject: common::SubjectRef,
        mutation: Vec<u8>,
        /// Complete bundle of deleted registration.
        binding: provider::EnrollmentBinding,
    },
    /// Ask securely about the status of the change.
    QueryMutation {
        operation: Vec<u8>,
        authenticator: common::AuthenticatorRef,
        mutation: Vec<u8>,
        /// Restore link.
        recovery_ref: Vec<u8>,
    },
    /// Cancel the operation.
    RequestCancel {
        /// That we're canceling.
        target: Vec<u8>,
    },
    EnrollmentTransaction {
        operation: Vec<u8>,
        request: provider::EnrollmentTransactionRequest,
    },
    /// Aptitude survey.
    Describe {
        authenticator: common::AuthenticatorRef,
    },
    /// Get out.
    Quiesce {
        authenticator: common::AuthenticatorRef,
    },
    CloseAuthenticator {
        authenticator: common::AuthenticatorRef,
    },
    Shutdown,
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PrepareEnrollmentChallenge { .. } => "PrepareEnrollmentChallenge(..)",
            Self::StartVerification { .. } => "StartVerification(..)",
            Self::StartEnrollment { .. } => "StartEnrollment(..)",
            Self::ContinueEnrollment { .. } => "ContinueEnrollment(..)",
            Self::StartRemoval { .. } => "StartRemoval(..)",
            Self::QueryMutation { .. } => "QueryMutation(..)",
            Self::RequestCancel { .. } => "RequestCancel(..)",
            Self::EnrollmentTransaction { .. } => "EnrollmentTransaction(..)",
            Self::Describe { .. } => "Describe(..)",
            Self::Quiesce { .. } => "Quiesce(..)",
            Self::CloseAuthenticator { .. } => "CloseAuthenticator(..)",
            Self::Shutdown => "Shutdown",
        })
    }
}

impl Command {
    /// Operation number if the command is related to the operation.
    ///
    /// ⚠️ Cancellation returns, too `None`: the target is in a separate field.
    pub fn operation(&self) -> Option<&[u8]> {
        match self {
            Self::PrepareEnrollmentChallenge { operation, .. }
            | Self::StartVerification { operation, .. }
            | Self::StartEnrollment { operation, .. }
            | Self::StartRemoval { operation, .. }
            | Self::QueryMutation { operation, .. } => Some(operation),
            Self::ContinueEnrollment { enrollment } => enrollment.operation(),
            _ => None,
        }
    }

    /// Does the command change the steady state?
    ///
    /// ⚠️ Registration and deletion, yes; everything else is not. It's not a policy, it's a
    /// property of the command itself.
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            Self::StartEnrollment { .. }
                | Self::ContinueEnrollment { .. }
                | Self::StartRemoval { .. }
        )
    }
}

fn authenticator(
    value: &Option<common::AuthenticatorRef>,
    field: &'static str,
) -> Result<common::AuthenticatorRef, CommandError> {
    let reference = value.clone().ok_or(CommandError::Missing { field })?;
    require_id(&reference.authenticator_id, "authenticator_id")?;
    if reference.device_epoch == 0 {
        return Err(CommandError::Wire(WireError::BadId {
            field: "device_epoch",
        }));
    }
    Ok(reference)
}

fn subject(value: &Option<common::SubjectRef>) -> Result<common::SubjectRef, CommandError> {
    let reference = value
        .clone()
        .ok_or(CommandError::Missing { field: "subject" })?;
    require_id(&reference.authority_id, "subject.authority_id")?;
    require_id(&reference.subject_id, "subject.subject_id")?;
    Ok(reference)
}

fn enrollment(
    value: &Option<common::EnrollmentRef>,
) -> Result<common::EnrollmentRef, CommandError> {
    let reference = value.clone().ok_or(CommandError::Missing {
        field: "enrollment",
    })?;
    require_id(&reference.enrollment_id, "enrollment_id")?;
    require_id(&reference.authenticator_id, "authenticator_id")?;
    if reference.generation == 0 {
        return Err(CommandError::Wire(WireError::BadId {
            field: "generation",
        }));
    }
    Ok(reference)
}

fn binding_kind(
    binding: &provider::EnrollmentBinding,
) -> Result<provider::EnrollmentBindingKind, CommandError> {
    bounded(
        &binding.provider_opaque_ref,
        "binding.provider_opaque_ref",
        limits::OPAQUE_REF_MAX,
    )?;
    bounded_str(
        &binding.format_id,
        "binding.format_id",
        limits::FORMAT_ID_MAX,
    )?;
    match provider::EnrollmentBindingKind::from_i32(binding.kind) {
        Some(provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque) => {
            Ok(provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque)
        }
        Some(provider::EnrollmentBindingKind::EnrollmentBindingScopedVault)
            if binding.provider_opaque_ref.is_empty() =>
        {
            Ok(provider::EnrollmentBindingKind::EnrollmentBindingScopedVault)
        }
        Some(provider::EnrollmentBindingKind::EnrollmentBindingScopedVault) => {
            Err(CommandError::Wire(WireError::UnexpectedPayload {
                what: "непрозрачная ссылка для scoped vault",
            }))
        }
        _ => Err(CommandError::Enum {
            field: "enrollment_binding.kind",
        }),
    }
}

/// He's taking a shot at the command.
pub fn parse(frame: &provider::ProviderFrame) -> Result<Command, CommandError> {
    use provider::provider_frame::Payload as P;

    let Some(payload) = &frame.payload else {
        return Err(CommandError::Missing { field: "payload" });
    };

    let subject_command = matches!(
        payload,
        P::PrepareEnrollmentChallenge(_)
            | P::StartVerification(_)
            | P::StartEnrollment(_)
            | P::ContinueEnrollment(_)
            | P::StartRemoval(_)
    );
    let uid = if subject_command {
        if frame.minor < limits::MINOR_SUBJECT_UID {
            return Err(CommandError::Wire(WireError::UnsupportedVersion {
                major: frame.major,
                minor: frame.minor,
            }));
        }
        Some(frame.uid.ok_or(CommandError::Missing { field: "uid" })?)
    } else {
        if frame.uid.is_some() {
            return Err(CommandError::Wire(WireError::UnexpectedPayload {
                what: "uid не относится к команде",
            }));
        }
        None
    };

    match payload {
        P::PrepareEnrollmentChallenge(start) => {
            mutation_version(frame)?;
            let authenticator = authenticator(&start.authenticator, "authenticator")?;
            if frame.device_epoch != authenticator.device_epoch {
                return Err(CommandError::Wire(WireError::UnexpectedPayload {
                    what: "вызов регистрации относится к другой эпохе",
                }));
            }
            Ok(Command::PrepareEnrollmentChallenge {
                uid: uid.expect("проверен для команды субъекта"),
                operation: operation(frame)?,
                authenticator,
                subject: subject(&start.subject)?,
                mutation: mutation(&start.mutation_id)?,
            })
        }
        P::StartVerification(start) => {
            // ⚠️ Zero is an error, not a default.
            let purpose = common::VerificationPurpose::from_i32(start.purpose)
                .filter(|purpose| *purpose != common::VerificationPurpose::PurposeUnspecified)
                .ok_or(CommandError::Enum { field: "purpose" })?;
            require_id(&start.challenge_ref, "challenge_ref")?;
            require_id(&start.recipient_ref, "recipient_ref")?;
            let operation = operation(frame)?;
            let authenticator = authenticator(&start.authenticator, "authenticator")?;
            let subject = subject(&start.subject)?;
            if frame.device_epoch != authenticator.device_epoch {
                return Err(CommandError::Wire(WireError::UnexpectedPayload {
                    what: "эпоха команды не совпадает с устройством",
                }));
            }
            if start.permitted_enrollments.is_empty() {
                return Err(CommandError::Missing {
                    field: "permitted_enrollments",
                });
            }
            if start.permitted_enrollments.len() > limits::ENROLLMENTS_PER_RESPONSE_MAX {
                return Err(CommandError::Wire(WireError::FieldTooLong {
                    field: "permitted_enrollments",
                    actual: start.permitted_enrollments.len(),
                    allowed: limits::ENROLLMENTS_PER_RESPONSE_MAX,
                }));
            }
            let mut ids = std::collections::BTreeSet::new();
            for binding in &start.permitted_enrollments {
                binding_kind(binding)?;
                let reference = enrollment(&binding.enrollment)?;
                if reference.authenticator_id != authenticator.authenticator_id
                    || !ids.insert(reference.enrollment_id)
                {
                    return Err(CommandError::Wire(WireError::UnexpectedPayload {
                        what: "чужая или повторная регистрация в наборе",
                    }));
                }
            }
            Ok(Command::StartVerification {
                uid: uid.expect("проверен для команды субъекта"),
                operation,
                authenticator,
                subject,
                purpose,
                challenge_ref: start.challenge_ref.clone(),
                recipient_ref: start.recipient_ref.clone(),
                permitted_enrollments: start.permitted_enrollments.clone(),
            })
        }
        P::StartEnrollment(start) => {
            mutation_version(frame)?;
            bounded_str(&start.label, "label", limits::LABEL_MAX)?;
            let authorization = match (
                start.enrollment_authorization_ref.is_empty(),
                start.enrollment_challenge_digest.is_empty(),
            ) {
                (true, true) => EnrollmentAuthorization::None,
                (false, false) => {
                    require_id(
                        &start.enrollment_authorization_ref,
                        "enrollment_authorization_ref",
                    )?;
                    if start.enrollment_challenge_digest.len() != 32 {
                        return Err(CommandError::Wire(WireError::BadId {
                            field: "enrollment_challenge_digest",
                        }));
                    }
                    let mut digest = [0; 32];
                    digest.copy_from_slice(&start.enrollment_challenge_digest);
                    EnrollmentAuthorization::Challenged {
                        reference: start.enrollment_authorization_ref.clone(),
                        challenge_digest: digest,
                    }
                }
                _ => {
                    return Err(CommandError::Wire(WireError::UnexpectedPayload {
                        what: "неполная авторизация регистрации",
                    }));
                }
            };
            let replace = match &start.replace {
                Some(_) => Some(enrollment(&start.replace)?),
                None => None,
            };
            let planned = enrollment(&start.planned_enrollment)?;
            match (&replace, &start.replace_binding) {
                (None, None) => {}
                (Some(old), Some(binding)) => {
                    binding_kind(binding)?;
                    if binding.enrollment.as_ref() != Some(old)
                        || old.enrollment_id == planned.enrollment_id
                    {
                        return Err(CommandError::Wire(WireError::UnexpectedPayload {
                            what: "несогласованная замена регистрации",
                        }));
                    }
                }
                _ => {
                    return Err(CommandError::Wire(WireError::UnexpectedPayload {
                        what: "замена и её связка должны присутствовать вместе",
                    }));
                }
            }
            let authenticator = authenticator(&start.authenticator, "authenticator")?;
            if frame.device_epoch != authenticator.device_epoch
                || planned.authenticator_id != authenticator.authenticator_id
                || replace
                    .as_ref()
                    .is_some_and(|old| old.authenticator_id != authenticator.authenticator_id)
            {
                return Err(CommandError::Wire(WireError::UnexpectedPayload {
                    what: "регистрация относится к другому аутентификатору",
                }));
            }
            let change = match &start.replace_binding {
                None => EnrollmentChange::Create { planned },
                Some(binding) => EnrollmentChange::Replace {
                    planned,
                    old_binding: binding.clone(),
                },
            };
            Ok(Command::StartEnrollment {
                uid: uid.expect("проверен для команды субъекта"),
                operation: operation(frame)?,
                authenticator,
                subject: subject(&start.subject)?,
                mutation: mutation(&start.mutation_id)?,
                label: start.label.clone(),
                change,
                authorization,
            })
        }
        P::ContinueEnrollment(continuation) => {
            let start = continuation
                .enrollment
                .clone()
                .ok_or(CommandError::Missing {
                    field: "enrollment",
                })?;
            let mut nested = frame.clone();
            nested.payload = Some(P::StartEnrollment(start));
            let command = parse(&nested)?;
            if !matches!(
                &command,
                Command::StartEnrollment {
                    authorization: EnrollmentAuthorization::Challenged { .. },
                    ..
                }
            ) {
                return Err(CommandError::Wire(WireError::UnexpectedPayload {
                    what: "продолжение без авторизации",
                }));
            }
            Ok(Command::ContinueEnrollment {
                enrollment: Box::new(command),
            })
        }
        P::StartRemoval(start) => {
            mutation_version(frame)?;
            bounded(
                &start.provider_opaque_ref,
                "provider_opaque_ref",
                limits::OPAQUE_REF_MAX,
            )?;
            let target = enrollment(&start.enrollment)?;
            let binding = start
                .enrollment_binding
                .as_ref()
                .ok_or(CommandError::Missing {
                    field: "enrollment_binding",
                })?;
            if binding.enrollment.as_ref() != Some(&target) {
                return Err(CommandError::Wire(WireError::UnexpectedPayload {
                    what: "связка удаления не соответствует регистрации",
                }));
            }
            match binding_kind(binding)? {
                provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque => {
                    if !start.provider_opaque_ref.is_empty()
                        && start.provider_opaque_ref != binding.provider_opaque_ref
                    {
                        return Err(CommandError::Wire(WireError::UnexpectedPayload {
                            what: "ссылки удаления различаются",
                        }));
                    }
                }
                provider::EnrollmentBindingKind::EnrollmentBindingScopedVault => {
                    if !start.provider_opaque_ref.is_empty() {
                        return Err(CommandError::Wire(WireError::UnexpectedPayload {
                            what: "непрозрачная ссылка для scoped vault",
                        }));
                    }
                }
                provider::EnrollmentBindingKind::EnrollmentBindingUnspecified => {
                    return Err(CommandError::Enum {
                        field: "enrollment_binding.kind",
                    });
                }
            }
            let authenticator = authenticator(&start.authenticator, "authenticator")?;
            if frame.device_epoch != authenticator.device_epoch
                || target.authenticator_id != authenticator.authenticator_id
            {
                return Err(CommandError::Wire(WireError::UnexpectedPayload {
                    what: "удаляемая регистрация относится к другому аутентификатору",
                }));
            }
            Ok(Command::StartRemoval {
                uid: uid.expect("проверен для команды субъекта"),
                operation: operation(frame)?,
                authenticator,
                subject: subject(&start.subject)?,
                mutation: mutation(&start.mutation_id)?,
                binding: binding.clone(),
            })
        }
        P::QueryMutation(query) => {
            bounded(&query.recovery_ref, "recovery_ref", limits::OPAQUE_REF_MAX)?;
            Ok(Command::QueryMutation {
                operation: operation(frame)?,
                authenticator: authenticator(&query.authenticator, "authenticator")?,
                mutation: mutation(&query.mutation_id)?,
                recovery_ref: query.recovery_ref.clone(),
            })
        }
        P::RequestCancel(cancel) => {
            require_id(&cancel.target_operation_id, "target_operation_id")?;
            if operation(frame)? != cancel.target_operation_id {
                return Err(CommandError::Wire(WireError::UnexpectedPayload {
                    what: "цель отмены не совпадает с операцией кадра",
                }));
            }
            Ok(Command::RequestCancel {
                target: cancel.target_operation_id.clone(),
            })
        }
        P::EnrollmentTransactionRequest(request) => {
            carosign_protocol::validation::check_provider_envelope(frame)?;
            Ok(Command::EnrollmentTransaction {
                operation: operation(frame)?,
                request: request.clone(),
            })
        }
        P::Describe(describe) => Ok(Command::Describe {
            authenticator: authenticator(&describe.authenticator, "authenticator")?,
        }),
        P::Quiesce(quiesce) => Ok(Command::Quiesce {
            authenticator: authenticator(&quiesce.authenticator, "authenticator")?,
        }),
        P::CloseAuthenticator(close) => Ok(Command::CloseAuthenticator {
            authenticator: authenticator(&close.authenticator, "authenticator")?,
        }),
        P::Shutdown(_) => Ok(Command::Shutdown),
        _ => Err(CommandError::Unsupported {
            what: "не команда"
        }),
    }
}

fn operation(frame: &provider::ProviderFrame) -> Result<Vec<u8>, CommandError> {
    require_id(&frame.operation_id, "operation_id")?;
    Ok(frame.operation_id.clone())
}

fn mutation(bytes: &[u8]) -> Result<Vec<u8>, CommandError> {
    require_id(bytes, "mutation_id")?;
    Ok(bytes.to_vec())
}

fn mutation_version(frame: &provider::ProviderFrame) -> Result<(), CommandError> {
    carosign_protocol::validation::check_version(frame.major, frame.minor)?;
    if !(limits::MINOR_BASELINE..=limits::MINOR_MAX).contains(&frame.minor) {
        return Err(CommandError::Wire(WireError::UnsupportedVersion {
            major: frame.major,
            minor: frame.minor,
        }));
    }
    Ok(())
}

fn bounded_str(value: &str, field: &'static str, allowed: usize) -> Result<(), WireError> {
    bounded(value.as_bytes(), field, allowed)
}

#[path = "decoded_command.tests.rs"]
#[cfg(test)]
mod tests;
