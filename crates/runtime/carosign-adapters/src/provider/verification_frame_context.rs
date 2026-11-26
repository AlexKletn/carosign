use super::{ProviderFailure, binding_encoding::to_binding};
use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
use carosign_protocol::carosign::{common::v1 as common, provider::v1 as provider};

use carosign_core::ids::CommandId;
use carosign_core::ids::OperationId;
use carosign_core::ids::WorkerInstanceId;

/// Data to build a validation command. The context itself is not an authorization or
/// confirmation of an worker's trusted start-up.
#[derive(Clone, Debug)]
pub struct VerificationFrameContext {
    /// A copy of the selected worker.
    pub worker: WorkerInstanceId,
    /// The command number assigned by the platform.
    pub command: CommandId,
    pub operation: OperationId,
    /// Selected apparatus and the era of binding.
    pub authenticator: carosign_core::device::AuthenticatorRef,
    /// Subject in the field of provider; not authority.
    pub provider_subject: carosign_core::identity::SubjectRef,
    /// Confirmed by the UID platform of the subject of the operation.
    pub uid: u32,
    /// The goal chosen by the source of the policy.
    pub purpose: carosign_core::operation::context::VerificationPurpose,
    /// A call link.
    pub challenge: carosign_evidence::recipient::ChallengeRef,
    pub recipient: carosign_evidence::recipient::RecipientRef,
}

/// Builds a full frame of the check; the sequence number assigns a channel.
///
/// The construction does not permit sending: it is separately allowed by a trusted session, a
/// source of authority and an broker of resources.
pub fn verification_frame(
    context: &VerificationFrameContext,
    permitted: &carosign_runtime::operation::bindings::PermittedSet,
) -> Result<provider::ProviderFrame, ProviderFailure> {
    use carosign_core::operation::context::VerificationPurpose;
    if permitted.bindings().iter().any(|binding| {
        binding.enrollment.authenticator != context.authenticator.id
            || binding.kind == carosign_runtime::BindingKind::ScopedVault
    }) {
        return Err(ProviderFailure::Rejected);
    }
    let purpose = match context.purpose {
        VerificationPurpose::SessionLogin => common::VerificationPurpose::PurposeSessionLogin,
        VerificationPurpose::SessionUnlock => common::VerificationPurpose::PurposeSessionUnlock,
        VerificationPurpose::PrivilegedAction => {
            common::VerificationPurpose::PurposePrivilegedAction
        }
        VerificationPurpose::FactorSelfCheck => common::VerificationPurpose::PurposeSelfCheck,
        VerificationPurpose::KeyOperation => common::VerificationPurpose::PurposeKeyOperation,
    };
    Ok(provider::ProviderFrame {
        major: carosign_protocol::limits::MAJOR,
        minor: carosign_protocol::limits::MINOR_MAX,
        worker_instance_id: context.worker.as_bytes().to_vec(),
        frame_seq: 0,
        command_id: context.command.as_bytes().to_vec(),
        operation_id: context.operation.as_bytes().to_vec(),
        device_epoch: context.authenticator.device_epoch.value(),
        uid: Some(context.uid),
        payload: Some(Payload::StartVerification(provider::StartVerification {
            authenticator: Some(common::AuthenticatorRef {
                authenticator_id: context.authenticator.id.as_bytes().to_vec(),
                device_epoch: context.authenticator.device_epoch.value(),
            }),
            subject: Some(common::SubjectRef {
                authority_id: context.provider_subject.authority.as_bytes().to_vec(),
                subject_id: context.provider_subject.subject.as_bytes().to_vec(),
            }),
            purpose: purpose as i32,
            challenge_ref: context.challenge.id().to_vec(),
            recipient_ref: context.recipient.id().to_vec(),
            permitted_enrollments: permitted.bindings().iter().map(to_binding).collect(),
        })),
    })
}
