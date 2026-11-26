#[cfg(test)]
use super::MutationStartError;

/// The frame of the request is not an authority: it can only be sent after verification by a
/// trusted broker, issuance and safe preparation.
pub(super) fn enrollment_challenge_frame(
    scope: &carosign_runtime::enrollment_authorization::EnrollmentAuthorizationScope,
) -> carosign_protocol::carosign::provider::v1::ProviderFrame {
    use carosign_protocol::carosign::{common::v1 as common, provider::v1 as provider};
    provider::ProviderFrame {
        major: carosign_protocol::limits::MAJOR,
        minor: carosign_protocol::limits::MINOR_MAX,
        worker_instance_id: scope.worker.as_bytes().to_vec(),
        frame_seq: 0,
        command_id: scope.challenge_command.as_bytes().to_vec(),
        operation_id: scope.operation.as_bytes().to_vec(),
        device_epoch: scope.authenticator.device_epoch.value(),
        uid: Some(scope.uid),
        payload: Some(
            provider::provider_frame::Payload::PrepareEnrollmentChallenge(
                provider::PrepareEnrollmentChallenge {
                    authenticator: Some(common::AuthenticatorRef {
                        authenticator_id: scope.authenticator.id.as_bytes().to_vec(),
                        device_epoch: scope.authenticator.device_epoch.value(),
                    }),
                    subject: Some(common::SubjectRef {
                        authority_id: scope.subject.authority.as_bytes().to_vec(),
                        subject_id: scope.subject.subject.as_bytes().to_vec(),
                    }),
                    mutation_id: scope.mutation.as_bytes().to_vec(),
                },
            ),
        ),
    }
}

/// The worker’s data remains under surveillance until an independent verification by the
/// broker.
#[cfg(test)]
pub(super) fn enrollment_challenge_observation(
    scope: &carosign_runtime::enrollment_authorization::EnrollmentAuthorizationScope,
    frame: carosign_protocol::carosign::provider::v1::ProviderFrame,
) -> Result<carosign_runtime::enrollment_authorization::ChallengeObservation, MutationStartError> {
    use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
    use carosign_runtime::enrollment_authorization::ChallengeObservation;
    if frame.worker_instance_id != scope.worker.as_bytes()
        || frame.operation_id != scope.operation.as_bytes()
        || frame.command_id != scope.challenge_command.as_bytes()
        || frame.device_epoch != scope.authenticator.device_epoch.value()
    {
        return Err(MutationStartError::Context);
    }
    let Some(Payload::EnrollmentChallengeReady(ready)) = frame.payload else {
        return Err(MutationStartError::Provider);
    };
    if ready.mutation_id != scope.mutation.as_bytes() {
        return Err(MutationStartError::Context);
    }
    let challenge_ref = ready
        .challenge_ref
        .try_into()
        .map_err(|_| MutationStartError::Context)?;
    // The upper limit is local policy, not trust in the term of the worker.
    ChallengeObservation::new(ready.challenge, challenge_ref, ready.validity_ms, 60_000)
        .map_err(|_| MutationStartError::Provider)
}
