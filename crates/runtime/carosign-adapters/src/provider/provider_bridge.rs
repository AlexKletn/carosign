use super::{
    ProviderChannel,
    binding_encoding::{enrollment_ref, to_binding},
    continuation_frame, mutation_frame,
};

use core::fmt;

use carosign_core::ids::OperationId;
use carosign_protocol::carosign::common::v1 as common;
use carosign_protocol::carosign::provider::v1 as provider;
use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
use carosign_runtime::ports::{
    CommandPlan, DispatchOutcome, EnrollmentAuthorization, MutationChange, ProviderExecutionPort,
};

/// Port adapter.
pub struct ProviderBridge<C: ProviderChannel> {
    channel: C,
}

impl<C: ProviderChannel> fmt::Debug for ProviderBridge<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProviderBridge(..)")
    }
}

impl<C: ProviderChannel> ProviderBridge<C> {
    /// Continues the change already initiated by the trusted owner on the same one-time
    /// ticket, called only after confirmed Execute.
    pub fn continue_mutation(
        &self,
        ticket: carosign_persistence::journal::ArmedMutationTicket,
        context: carosign_runtime::ports::MutationArmRequest,
        boundary: &mut dyn FnMut(&mut dyn FnMut() -> bool) -> bool,
    ) -> carosign_runtime::ports::DispatchOutcome {
        if !context.matches_ticket(&ticket) {
            return carosign_runtime::ports::DispatchOutcome::NotSent;
        }
        let Ok(frame) = continuation_frame(&context) else {
            return carosign_runtime::ports::DispatchOutcome::NotSent;
        };
        self.channel
            .send_continuation(frame, ticket, context, boundary)
    }
    /// Adapter over the channel.
    pub fn new(channel: C, _instance: [u8; 16], _epoch: u64) -> Self {
        Self { channel }
    }

    /// Returns the channel belonging to the adapter after completion of work.
    pub fn into_channel(self) -> C {
        self.channel
    }

    /// Scene for the plan.
    ///
    /// ⚠️ Modifying sending carries the change number; verification is **set of authorized
    /// registrations**.
    pub fn frame(
        instance: [u8; 16],
        epoch: u64,
        operation: OperationId,
        plan: &CommandPlan,
    ) -> provider::ProviderFrame {
        let payload = match plan {
            CommandPlan::Verify { permitted } => {
                Payload::StartVerification(provider::StartVerification {
                    permitted_enrollments: permitted
                        .bindings()
                        .iter()
                        .map(|binding| provider::EnrollmentBinding {
                            enrollment: Some(carosign_protocol::carosign::common::v1::EnrollmentRef {
                                enrollment_id: binding.enrollment.id.as_bytes().to_vec(),
                                authenticator_id: binding
                                    .enrollment
                                    .authenticator
                                    .as_bytes()
                                    .to_vec(),
                                generation: binding.enrollment.generation.value(),
                            }),
                            kind: match binding.kind {
                                carosign_runtime::BindingKind::ProviderOpaque => {
                                    provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque
                                        as i32
                                }
                                carosign_runtime::BindingKind::ScopedVault => {
                                    provider::EnrollmentBindingKind::EnrollmentBindingScopedVault
                                        as i32
                                }
                            },
                            provider_opaque_ref: binding.provider_ref.clone(),
                            format_id: binding.format_id.clone().unwrap_or_default(),
                            format_version: binding.format_version,
                        })
                        .collect(),
                    ..provider::StartVerification::default()
                })
            }
            CommandPlan::Mutate {
                mutation,
                command: _,
                authorization,
                change,
            } => {
                // ⚠️ Authorization mode determines what lies in the body: either nothing, or
                // reference and digest. There is no intermediate.
                let (reference, digest) = match authorization {
                    EnrollmentAuthorization::None => (Vec::new(), Vec::new()),
                    EnrollmentAuthorization::Reference {
                        reference,
                        challenge_digest,
                    } => (reference.to_vec(), challenge_digest.to_vec()),
                };
                match change {
                    MutationChange::Create { planned_new } => {
                        // ⚠️ Document 34: The new registration `planned_enrollment` and s**t
                        // `replace`, s**t `replace_binding`.
                        Payload::StartEnrollment(provider::StartEnrollment {
                            mutation_id: mutation.as_bytes().to_vec(),
                            enrollment_authorization_ref: reference,
                            enrollment_challenge_digest: digest,
                            replace: None,
                            replace_binding: None,
                            planned_enrollment: Some(enrollment_ref(*planned_new)),
                            ..provider::StartEnrollment::default()
                        })
                    }
                    MutationChange::Replace { planned_new, old } => {
                        // ⚠️ Replacement carries the same old link in two fields: either both
                        // or none. `planned_enrollment`.
                        Payload::StartEnrollment(provider::StartEnrollment {
                            mutation_id: mutation.as_bytes().to_vec(),
                            enrollment_authorization_ref: reference,
                            enrollment_challenge_digest: digest,
                            replace: Some(enrollment_ref(old.enrollment)),
                            replace_binding: Some(to_binding(old)),
                            planned_enrollment: Some(enrollment_ref(*planned_new)),
                            ..provider::StartEnrollment::default()
                        })
                    }
                    // ⚠️ D-07: Deletion carries a **link**, not just a link: a single link
                    // cannot distinguish a record from its storage type.
                    MutationChange::Removal {
                        enrollment,
                        binding,
                    } => Payload::StartRemoval(provider::StartRemoval {
                        mutation_id: mutation.as_bytes().to_vec(),
                        enrollment: Some(common::EnrollmentRef {
                            enrollment_id: enrollment.id.as_bytes().to_vec(),
                            authenticator_id: enrollment.authenticator.as_bytes().to_vec(),
                            generation: enrollment.generation.value(),
                        }),
                        provider_opaque_ref: binding.provider_ref.clone(),
                        enrollment_binding: Some(to_binding(binding)),
                        ..provider::StartRemoval::default()
                    }),
                }
            }
            CommandPlan::PrepareEnrollmentChallenge { mutation } => {
                Payload::PrepareEnrollmentChallenge(provider::PrepareEnrollmentChallenge {
                    mutation_id: mutation.as_bytes().to_vec(),
                    ..provider::PrepareEnrollmentChallenge::default()
                })
            }
            CommandPlan::Query => Payload::QueryMutation(provider::QueryMutation::default()),
        };
        provider::ProviderFrame {
            major: carosign_protocol::limits::MAJOR,
            minor: carosign_protocol::limits::MINOR_MAX,
            worker_instance_id: instance.to_vec(),
            frame_seq: 0,
            command_id: match plan {
                CommandPlan::Mutate { command, .. } => command.as_bytes().to_vec(),
                _ => Vec::new(),
            },
            operation_id: operation.as_bytes().to_vec(),
            device_epoch: epoch,
            uid: None,
            payload: Some(payload),
        }
    }
}

impl<C: ProviderChannel> ProviderExecutionPort for ProviderBridge<C> {
    fn dispatch(
        &self,
        _operation: OperationId,
        _command: CommandPlan,
    ) -> carosign_runtime::BoxFuture<'_, DispatchOutcome> {
        // No current plan carries a complete proven context; matching and preparing a call
        // also requires proven security.
        Box::pin(async { DispatchOutcome::NotSent })
    }

    fn dispatch_mutation(
        &self,
        ticket: carosign_persistence::journal::ArmedMutationTicket,
        context: carosign_runtime::ports::MutationArmRequest,
    ) -> carosign_runtime::BoxFuture<'_, DispatchOutcome> {
        if !context.matches_ticket(&ticket) {
            return Box::pin(async { DispatchOutcome::NotSent });
        }
        let Ok(frame) = mutation_frame(&context) else {
            return Box::pin(async { DispatchOutcome::NotSent });
        };
        let result = self.channel.send_mutation(frame, ticket, context);
        Box::pin(async move { result })
    }

    fn request_cancel(&self, operation: OperationId) -> carosign_runtime::BoxFuture<'_, bool> {
        let accepted = self.channel.cancel(operation);
        Box::pin(async move { accepted })
    }

    fn request_cancel_scoped<'a>(
        &'a self,
        lease: &'a carosign_runtime::ports::ResourceLease,
        command: carosign_core::ids::CommandId,
    ) -> carosign_runtime::BoxFuture<'a, DispatchOutcome> {
        let accepted = self.channel.cancel_scoped(lease, command);
        Box::pin(async move { accepted })
    }
}

#[path = "provider_bridge.tests.rs"]
#[cfg(test)]
mod tests;

#[path = "provider_bridge.removal_tests.tests.rs"]
#[cfg(test)]
mod removal_tests;
