use super::{
    capabilities::capabilities,
    request_validation::{valid_id, valid_verification},
};
use carosign_protocol::carosign::common::v1 as common;
use carosign_protocol::carosign::provider::v1 as provider;
use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
use carosign_provider_sdk::wire::Channel;
use std::os::fd::RawFd;

use crate::enrollments::{MockEnrollments, load_mock_state, save_mock_state};
use crate::packet_channel::{receive, send, validate_channel};
use crate::provider_frame::frame;
use crate::scenario::Scenario;
#[cfg(feature = "ssh-mock")]
use crate::ssh_decision::wait_ssh_decision;

pub(crate) fn run(fd: RawFd, scenario: Scenario) -> Result<(), String> {
    validate_channel(fd)?;
    let mut channel = Channel::new();
    let prebind_feature = provider::Feature::PrebindDescriptionV1 as i32;
    let mut features = vec![provider::Feature::BasicOperations as i32, prebind_feature];
    let feedback_feature = provider::Feature::CaptureFeedbackV1 as i32;
    if matches!(scenario, Scenario::LinuxPersistent) {
        features.push(feedback_feature);
    }
    let transaction_feature = provider::Feature::EnrollmentTransactionV1 as i32;
    if matches!(
        scenario,
        Scenario::LinuxPendingRollback
            | Scenario::LinuxErrorRollback
            | Scenario::LinuxLostRollback
            | Scenario::LinuxCommittedRollback
    ) {
        features.push(transaction_feature);
    }
    let hello = frame(
        Vec::new(),
        0,
        Vec::new(),
        Vec::new(),
        Payload::WorkerHello(provider::WorkerHello {
            major: 1,
            min_minor: carosign_protocol::limits::MINOR_BASELINE,
            max_minor: carosign_protocol::limits::MINOR_MAX,
            features: features.clone(),
            implementation_version: "mock-test-only".into(),
        }),
    );
    send(fd, &channel.prepare(hello).map_err(|e| e.to_string())?)?;
    let (init, descriptors) = receive(fd)?.ok_or("closed before initialize")?;
    if !descriptors.is_empty() {
        return Err("descriptor before bind".into());
    }
    let init = channel.accept(&init).map_err(|e| e.to_string())?;
    let Some(Payload::Initialize(ref data)) = init.payload else {
        return Err("expected initialize".into());
    };
    if init.minor != carosign_protocol::limits::MINOR_BASELINE
        || init.device_epoch != 0
        || !init.command_id.is_empty()
        || !init.operation_id.is_empty()
        || data
            .enabled_features
            .iter()
            .any(|feature| !features.contains(feature))
        || data.selected_minor != carosign_protocol::limits::MINOR_BASELINE
        || data.worker_instance_id != init.worker_instance_id
        || data.worker_instance_id.iter().all(|b| *b == 0)
        || data.package_id.len() != 16
        || data.profile_id.len() != 16
        || data.package_id.iter().all(|b| *b == 0)
        || data.profile_id.iter().all(|b| *b == 0)
    {
        return Err("invalid initialize".into());
    }
    let feedback_enabled = data.enabled_features.contains(&feedback_feature);
    let prebind_enabled = data.enabled_features.contains(&prebind_feature);
    let rollback_enabled = data.enabled_features.contains(&transaction_feature);
    let mut pending_enrollment = None;
    let instance = data.worker_instance_id.clone();
    let profile = data.profile_id.clone();
    let initialized = frame(
        instance.clone(),
        0,
        Vec::new(),
        Vec::new(),
        Payload::Initialized(provider::Initialized {
            ready_for_binding: true,
        }),
    );
    send(
        fd,
        &channel.prepare(initialized).map_err(|e| e.to_string())?,
    )?;
    let mut bound: Option<common::AuthenticatorRef> = None;
    let mut commands = std::collections::BTreeSet::new();
    let mut completed_operations = std::collections::BTreeSet::new();
    let mut pending_verifications = std::collections::BTreeMap::<Vec<u8>, Vec<u8>>::new();
    let mut completed_mutations = std::collections::BTreeSet::new();
    #[cfg(feature = "ssh-mock")]
    let ssh_state = matches!(scenario, Scenario::SshMock);
    #[cfg(not(feature = "ssh-mock"))]
    let ssh_state = false;
    let state_path = if ssh_state {
        Some(std::path::PathBuf::from("/var/lib/carosign-mock/state"))
    } else if matches!(scenario, Scenario::LinuxPersistent) {
        let path = std::path::PathBuf::from(
            std::env::var_os("CAROSIGN_MOCK_STATE").ok_or("missing mock state path")?,
        );
        if !path.is_absolute() {
            return Err("mock state path must be absolute".into());
        }
        Some(path)
    } else {
        None
    };
    let mut enrollments = match &state_path {
        Some(path) => load_mock_state(path)?,
        None => MockEnrollments::new(),
    };
    // Explicit Test Profile: The record exists prior to verification, so the vehicle
    // inspection is not passed off as a registration test.
    if matches!(
        scenario,
        Scenario::SeededNoSample
            | Scenario::SeededMatchedToken
            | Scenario::SeededMatchedScope
            | Scenario::SeededPendingCancel
            | Scenario::LinuxNoSample
            | Scenario::LinuxMatchedScope
            | Scenario::LinuxPendingCancel
    ) {
        let subject = if matches!(
            scenario,
            Scenario::LinuxNoSample | Scenario::LinuxMatchedScope | Scenario::LinuxPendingCancel
        ) {
            let mut subject_id = [0u8; 16];
            subject_id[0] = 1;
            // Only the test fixture runs under the same UID as the client.
            subject_id[8..].copy_from_slice(&u64::from(unsafe { libc::getuid() }).to_be_bytes());
            common::SubjectRef {
                authority_id: b"linux-local-v001".to_vec(),
                subject_id: subject_id.to_vec(),
            }
        } else {
            common::SubjectRef {
                authority_id: vec![11; 16],
                subject_id: vec![12; 16],
            }
        };
        let enrollment = common::EnrollmentRef {
            enrollment_id: vec![15; 16],
            authenticator_id: vec![5; 16],
            generation: 1,
        };
        enrollments.insert(
            enrollment.enrollment_id.clone(),
            (subject, enrollment, vec![21; 16]),
        );
    }
    while let Some((bytes, descriptors)) = receive(fd)? {
        let request = channel.accept(&bytes).map_err(|e| e.to_string())?;
        if request.worker_instance_id != instance
            || request.minor != carosign_protocol::limits::MINOR_BASELINE
        {
            return Err("worker identity or version mismatch".into());
        }
        if matches!(request.payload, Some(Payload::Shutdown(_))) {
            return Ok(());
        }
        if !valid_id(&request.command_id)
            || commands.len() >= 4096
            || !commands.insert(request.command_id.clone())
        {
            return Err("invalid or repeated command id".into());
        }
        if let Some(Payload::PrebindDescribe(ref describe)) = request.payload {
            let authenticator = describe
                .authenticator
                .as_ref()
                .ok_or("missing authenticator")?;
            if !prebind_enabled
                || bound.is_some()
                || !descriptors.is_empty()
                || !request.operation_id.is_empty()
                || request.device_epoch == 0
                || request.device_epoch != authenticator.device_epoch
                || !valid_id(&authenticator.authenticator_id)
            {
                return Err("invalid prebind description request".into());
            }
            let reply = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id,
                Vec::new(),
                Payload::PrebindDescription(provider::PrebindDescription {
                    declaration_version: 1,
                    authenticator: Some(authenticator.clone()),
                    capabilities: Some(capabilities(scenario)),
                }),
            );
            send(fd, &channel.prepare(reply).map_err(|e| e.to_string())?)?;
            continue;
        }
        if let Some(Payload::BindAuthenticator(ref bind)) = request.payload {
            if bound.is_some()
                || bind.fd_slots.len() != descriptors.len()
                || bind.profile_id != profile
            {
                return Err("invalid binding".into());
            }
            if !descriptors.is_empty() {
                use std::io::Read;
                if bind.fd_slots.len() != 1
                    || bind.fd_slots[0].role != provider::FdRole::Config as i32
                {
                    return Err("invalid mock resource".into());
                }
                let mut text = String::new();
                std::fs::File::from(descriptors.into_iter().next().unwrap())
                    .read_to_string(&mut text)
                    .map_err(|e| e.to_string())?;
                if text != "test resource" {
                    return Err("unexpected mock resource".into());
                }
            }
            carosign_protocol::validation::check_bind(bind).map_err(|e| e.to_string())?;
            let authenticator = bind.authenticator.clone().ok_or("missing authenticator")?;
            if authenticator.device_epoch == 0 || request.device_epoch != authenticator.device_epoch
            {
                return Err("invalid binding epoch".into());
            }
            let ready = frame(
                instance.clone(),
                authenticator.device_epoch,
                request.command_id,
                request.operation_id,
                Payload::AuthenticatorReady(provider::AuthenticatorReady {
                    authenticator: Some(authenticator.clone()),
                    capabilities: Some(capabilities(scenario)),
                }),
            );
            send(fd, &channel.prepare(ready).map_err(|e| e.to_string())?)?;
            bound = Some(authenticator);
            continue;
        }
        if !descriptors.is_empty() {
            return Err("descriptor outside bind".into());
        }
        let authenticator = bound.as_ref().ok_or("not bound")?;
        if request.device_epoch != authenticator.device_epoch {
            return Err("stale device epoch".into());
        }
        if let Some(Payload::Describe(ref describe)) = request.payload {
            if describe.authenticator.as_ref() != Some(authenticator) {
                return Err("wrong authenticator".into());
            }
            let response = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id,
                request.operation_id,
                Payload::Described(provider::Described {
                    capabilities: Some(capabilities(scenario)),
                }),
            );
            send(fd, &channel.prepare(response).map_err(|e| e.to_string())?)?;
            continue;
        }
        if let Some(Payload::EnrollmentTransactionRequest(ref rollback)) = request.payload {
            let expected = pending_enrollment.take().ok_or("no pending enrollment")?;
            if !rollback_enabled
                || rollback.action
                    != provider::EnrollmentTransactionAction::EnrollmentTransactionRollback as i32
                || expected
                    != (
                        request.operation_id.clone(),
                        rollback.enrollment_command_id.clone(),
                        rollback.mutation_id.clone(),
                    )
            {
                return Err("wrong enrollment transaction".into());
            }
            if matches!(scenario, Scenario::LinuxLostRollback) {
                return Ok(());
            }
            let state = if matches!(scenario, Scenario::LinuxCommittedRollback) {
                provider::EnrollmentTransactionState::EnrollmentTransactionCommitted
            } else {
                provider::EnrollmentTransactionState::EnrollmentTransactionRolledBack
            };
            let response = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id,
                request.operation_id,
                Payload::EnrollmentTransactionResult(provider::EnrollmentTransactionResult {
                    mutation_id: rollback.mutation_id.clone(),
                    enrollment_command_id: rollback.enrollment_command_id.clone(),
                    state: state as i32,
                }),
            );
            send(fd, &channel.prepare(response).map_err(|e| e.to_string())?)?;
            continue;
        }
        if let Some(Payload::RequestCancel(ref cancel)) = request.payload {
            if !valid_id(&request.operation_id)
                || request.operation_id != cancel.target_operation_id
            {
                return Err("invalid cancellation target".into());
            }
            let pending_command = pending_verifications.remove(&cancel.target_operation_id);
            let status = if pending_command.is_some() {
                provider::CancelStatus::CancelRequested
            } else if completed_operations.contains(&cancel.target_operation_id) {
                provider::CancelStatus::CancelAlreadyTerminal
            } else {
                provider::CancelStatus::CancelUnsupported
            };
            let receipt = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id,
                request.operation_id,
                Payload::CancelReceipt(provider::CancelReceipt {
                    status: status as i32,
                }),
            );
            send(fd, &channel.prepare(receipt).map_err(|e| e.to_string())?)?;
            if let Some(command_id) = pending_command {
                let outcome = frame(
                    instance.clone(),
                    request.device_epoch,
                    command_id,
                    cancel.target_operation_id.clone(),
                    Payload::VerificationOutcome(provider::VerificationOutcome {
                        status: provider::VerificationStatus::VerifyCancelled as i32,
                        evidence: None,
                        evidence_format_id: String::new(),
                        error: None,
                        matched_enrollment: None,
                    }),
                );
                completed_operations.insert(outcome.operation_id.clone());
                send(fd, &channel.prepare(outcome).map_err(|e| e.to_string())?)?;
            }
            continue;
        }
        if matches!(
            request.payload,
            Some(
                Payload::StartVerification(_)
                    | Payload::StartEnrollment(_)
                    | Payload::StartRemoval(_)
            )
        ) && (completed_operations.contains(&request.operation_id)
            || pending_verifications.contains_key(&request.operation_id))
        {
            return Err("operation already completed".into());
        }
        if let Some(Payload::StartVerification(ref verify)) = request.payload {
            let carosign_provider_sdk::command::Command::StartVerification {
                subject,
                permitted_enrollments,
                ..
            } = carosign_provider_sdk::command::parse(&request)
                .map_err(|_| "invalid verification command")?
            else {
                return Err("invalid verification command".into());
            };
            if !valid_id(&request.command_id)
                || !valid_id(&request.operation_id)
                || !valid_verification(verify, authenticator)
            {
                return Err("invalid verification context".into());
            }
            for binding in &permitted_enrollments {
                let reference = binding.enrollment.as_ref().ok_or("missing enrollment")?;
                let existing = enrollments
                    .get(&reference.enrollment_id)
                    .ok_or("unknown enrollment")?;
                if existing.0 != subject
                    || &existing.1 != reference
                    || binding.provider_opaque_ref != existing.2
                    || !binding.format_id.is_empty()
                    || binding.format_version != 0
                {
                    return Err("invalid verification binding".into());
                }
            }
            let ack = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id.clone(),
                request.operation_id.clone(),
                Payload::CommandAck(provider::CommandAck {
                    accepted: true,
                    rejection: None,
                }),
            );
            send(fd, &channel.prepare(ack).map_err(|e| e.to_string())?)?;
            if feedback_enabled {
                let update = frame(
                    instance.clone(),
                    request.device_epoch,
                    request.command_id.clone(),
                    request.operation_id.clone(),
                    Payload::ProviderProgress(provider::ProviderProgress {
                        operation_sequence: 1,
                        kind: provider::ProgressKind::ProgressWorking as i32,
                        capture_state: Some(common::CaptureState {
                            target: common::CaptureTarget::Finger as i32,
                            present: Some(true),
                            needed: Some(false),
                        }),
                        ..Default::default()
                    }),
                );
                send(fd, &channel.prepare(update).map_err(|e| e.to_string())?)?;
            }
            if matches!(
                scenario,
                Scenario::PendingCancel
                    | Scenario::SeededPendingCancel
                    | Scenario::LinuxPendingCancel
            ) {
                pending_verifications.insert(request.operation_id, request.command_id);
                continue;
            }
            #[cfg(feature = "ssh-mock")]
            let ssh_decision = if matches!(scenario, Scenario::SshMock) {
                match wait_ssh_decision(fd)? {
                    Some(status) => Some(status),
                    None => {
                        pending_verifications.insert(request.operation_id, request.command_id);
                        continue;
                    }
                }
            } else {
                None
            };
            let (status, matched_enrollment) = match scenario {
                Scenario::NoSample
                | Scenario::SeededNoSample
                | Scenario::PendingCancel
                | Scenario::SeededPendingCancel
                | Scenario::LinuxNoSample
                | Scenario::LinuxPendingCancel
                | Scenario::LinuxPendingMutation
                | Scenario::LinuxPendingRollback
                | Scenario::LinuxErrorRollback
                | Scenario::LinuxLostRollback
                | Scenario::LinuxCommittedRollback => {
                    (provider::VerificationStatus::VerifyNoUsableSample, None)
                }
                Scenario::Matched | Scenario::MatchedToken | Scenario::SeededMatchedToken => (
                    provider::VerificationStatus::VerifyMatched,
                    permitted_enrollments[0].enrollment.clone(),
                ),
                Scenario::MatchedScope
                | Scenario::SeededMatchedScope
                | Scenario::LinuxMatchedScope => {
                    (provider::VerificationStatus::VerifyMatched, None)
                }
                Scenario::LinuxPersistent => (
                    provider::VerificationStatus::VerifyMatched,
                    permitted_enrollments[0].enrollment.clone(),
                ),
                #[cfg(feature = "ssh-mock")]
                Scenario::SshMock => (ssh_decision.ok_or("missing mock decision")?, None),
            };
            let outcome = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id,
                request.operation_id,
                Payload::VerificationOutcome(provider::VerificationOutcome {
                    status: status as i32,
                    evidence: matches!(
                        scenario,
                        Scenario::MatchedToken | Scenario::SeededMatchedToken
                    )
                    .then(|| provider::verification_outcome::Evidence::OpaqueToken(vec![42; 32])),
                    evidence_format_id: String::new(),
                    error: None,
                    matched_enrollment,
                }),
            );
            completed_operations.insert(outcome.operation_id.clone());
            send(fd, &channel.prepare(outcome).map_err(|e| e.to_string())?)?;
            continue;
        }
        if let Some(Payload::StartEnrollment(ref enroll)) = request.payload {
            let subject = enroll.subject.as_ref().ok_or("missing subject")?;
            let planned = enroll
                .planned_enrollment
                .as_ref()
                .ok_or("missing planned enrollment")?;
            if enroll.authenticator.as_ref() != Some(authenticator)
                || !valid_id(&request.operation_id)
                || !valid_id(&enroll.mutation_id)
                || completed_mutations.contains(&enroll.mutation_id)
                || !valid_id(&subject.authority_id)
                || !valid_id(&subject.subject_id)
                || !valid_id(&planned.enrollment_id)
                || planned.authenticator_id != authenticator.authenticator_id
                || planned.generation == 0
                || enroll.label.len() > carosign_protocol::limits::LABEL_MAX
                || !enroll.enrollment_authorization_ref.is_empty()
                || !enroll.enrollment_challenge_digest.is_empty()
                || enrollments.contains_key(&planned.enrollment_id)
            {
                return Err("invalid enrollment context".into());
            }
            match (&enroll.replace, &enroll.replace_binding) {
                (None, None) => {}
                (Some(old), Some(binding)) => {
                    let existing = enrollments
                        .get(&old.enrollment_id)
                        .ok_or("missing replacement target")?;
                    if old.enrollment_id == planned.enrollment_id
                        || &existing.0 != subject
                        || &existing.1 != old
                        || binding.enrollment.as_ref() != Some(old)
                        || binding.kind
                            != provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque
                                as i32
                        || binding.provider_opaque_ref != existing.2
                        || !binding.format_id.is_empty()
                        || binding.format_version != 0
                    {
                        return Err("invalid replacement binding".into());
                    }
                }
                _ => return Err("incomplete replacement binding".into()),
            }
            let ack = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id.clone(),
                request.operation_id.clone(),
                Payload::CommandAck(provider::CommandAck {
                    accepted: true,
                    rejection: None,
                }),
            );
            send(fd, &channel.prepare(ack).map_err(|e| e.to_string())?)?;
            if matches!(
                scenario,
                Scenario::LinuxPendingMutation
                    | Scenario::LinuxPendingRollback
                    | Scenario::LinuxErrorRollback
                    | Scenario::LinuxLostRollback
                    | Scenario::LinuxCommittedRollback
            ) {
                if rollback_enabled {
                    pending_enrollment = Some((
                        request.operation_id.clone(),
                        request.command_id.clone(),
                        enroll.mutation_id.clone(),
                    ));
                }
                if matches!(scenario, Scenario::LinuxErrorRollback) {
                    let result = frame(
                        instance.clone(),
                        request.device_epoch,
                        request.command_id,
                        request.operation_id,
                        Payload::MutationOutcome(provider::MutationOutcome {
                            mutation_id: enroll.mutation_id.clone(),
                            status: provider::MutationStatus::MutationMayHaveOccurred as i32,
                            ..Default::default()
                        }),
                    );
                    send(fd, &channel.prepare(result).map_err(|e| e.to_string())?)?;
                }
                continue;
            }
            if let Some(old) = &enroll.replace {
                enrollments.remove(&old.enrollment_id);
            }
            let opaque = planned.enrollment_id.clone();
            completed_mutations.insert(enroll.mutation_id.clone());
            enrollments.insert(
                planned.enrollment_id.clone(),
                (subject.clone(), planned.clone(), opaque.clone()),
            );
            if let Some(path) = &state_path {
                save_mock_state(path, &enrollments)?;
            }
            let result = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id,
                request.operation_id,
                Payload::MutationOutcome(provider::MutationOutcome {
                    mutation_id: enroll.mutation_id.clone(),
                    status: provider::MutationStatus::MutationProviderCompleted as i32,
                    enrollment: Some(planned.clone()),
                    provider_opaque_ref: opaque,
                    recovery_ref: Vec::new(),
                    error: None,
                }),
            );
            completed_operations.insert(result.operation_id.clone());
            send(fd, &channel.prepare(result).map_err(|e| e.to_string())?)?;
            continue;
        }
        if let Some(Payload::StartRemoval(ref removal)) = request.payload {
            let subject = removal.subject.as_ref().ok_or("missing subject")?;
            let target = removal.enrollment.as_ref().ok_or("missing enrollment")?;
            let binding = removal
                .enrollment_binding
                .as_ref()
                .ok_or("missing enrollment binding")?;
            let existing = enrollments
                .get(&target.enrollment_id)
                .ok_or("missing removal target")?;
            if removal.authenticator.as_ref() != Some(authenticator)
                || !valid_id(&request.operation_id)
                || !valid_id(&removal.mutation_id)
                || completed_mutations.contains(&removal.mutation_id)
                || &existing.0 != subject
                || &existing.1 != target
                || binding.enrollment.as_ref() != Some(target)
                || binding.kind
                    != provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32
                || binding.provider_opaque_ref != existing.2
                || (!removal.provider_opaque_ref.is_empty()
                    && removal.provider_opaque_ref != existing.2)
                || !binding.format_id.is_empty()
                || binding.format_version != 0
            {
                return Err("invalid removal binding".into());
            }
            let ack = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id.clone(),
                request.operation_id.clone(),
                Payload::CommandAck(provider::CommandAck {
                    accepted: true,
                    rejection: None,
                }),
            );
            send(fd, &channel.prepare(ack).map_err(|e| e.to_string())?)?;
            enrollments.remove(&target.enrollment_id);
            if let Some(path) = &state_path {
                save_mock_state(path, &enrollments)?;
            }
            completed_mutations.insert(removal.mutation_id.clone());
            let result = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id,
                request.operation_id,
                Payload::MutationOutcome(provider::MutationOutcome {
                    mutation_id: removal.mutation_id.clone(),
                    status: provider::MutationStatus::MutationProviderCompleted as i32,
                    enrollment: Some(target.clone()),
                    provider_opaque_ref: Vec::new(),
                    recovery_ref: Vec::new(),
                    error: None,
                }),
            );
            completed_operations.insert(result.operation_id.clone());
            send(fd, &channel.prepare(result).map_err(|e| e.to_string())?)?;
            continue;
        }
        if let Some(Payload::QueryMutation(ref query)) = request.payload {
            if query.authenticator.as_ref() != Some(authenticator)
                || !valid_id(&query.mutation_id)
                || !valid_id(&request.operation_id)
                || query.recovery_ref.len() > carosign_protocol::limits::OPAQUE_REF_MAX
            {
                return Err("invalid mutation query".into());
            }
            let ack = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id.clone(),
                request.operation_id.clone(),
                Payload::CommandAck(provider::CommandAck {
                    accepted: true,
                    rejection: None,
                }),
            );
            send(fd, &channel.prepare(ack).map_err(|e| e.to_string())?)?;
            let result = frame(
                instance.clone(),
                request.device_epoch,
                request.command_id,
                request.operation_id,
                Payload::QueryMutationResult(provider::QueryMutationResult {
                    mutation_id: query.mutation_id.clone(),
                    state: provider::QueryState::QueryUnsupported as i32,
                    enrollment: None,
                    provider_opaque_ref: Vec::new(),
                    error: None,
                }),
            );
            send(fd, &channel.prepare(result).map_err(|e| e.to_string())?)?;
            continue;
        }
        // Unsupported commands never lead to a change of state.
        let ack = frame(
            instance.clone(),
            request.device_epoch,
            request.command_id,
            request.operation_id,
            Payload::CommandAck(provider::CommandAck {
                accepted: false,
                rejection: Some(common::Error {
                    code: common::ErrorCode::ErrorUnsupported as i32,
                    safe_message: "mock command not implemented".into(),
                }),
            }),
        );
        send(fd, &channel.prepare(ack).map_err(|e| e.to_string())?)?;
    }
    Ok(())
}
