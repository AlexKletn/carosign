use super::super::CommandPlan;
use super::super::MutationChange;
use carosign_core::ids::OperationId;

/// Full armament request; fields alone do not prove authority, valid issuance, or the
/// effect's ownership.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MutationArmRequest {
    /// The operation and the expected effect of sending.
    pub operation: OperationId,
    /// Confirmed by the UID platform of the subject of the operation.
    pub uid: u32,
    pub effect: carosign_engine::PlannedEffect,
    /// Command, Worker, and Epoch from a Plan for Sustainable Change.
    pub arm: carosign_persistence::journal::ArmMutation,
    /// Area of lockdown and related resource issuance.
    pub scope: [u8; carosign_persistence::limits::SCOPE_LOCK_KEY_LEN],
    pub lease: carosign_core::ids::LeaseId,
    /// A command and precise ligament for replacement or removal.
    pub plan: CommandPlan,
    /// An immutable snapshot of the entire area, which must coincide with the journal.
    pub logical_plan: carosign_core::operation::plan::MutationPlan,
    /// Registration tag from the original request; no deletion tag.
    pub label: Option<carosign_core::enrollment::EnrollmentLabel>,
}

impl MutationArmRequest {
    /// The label is checked with the live actor until steady arm and dispatch.
    pub fn matches_operation_label(&self, state: &carosign_engine::OperationState) -> bool {
        match (&state.body, &self.label) {
            (carosign_engine::OperationBody::Enrollment { request, .. }, Some(label)) => {
                &request.target == label
            }
            (carosign_engine::OperationBody::Removal { .. }, None) => true,
            _ => false,
        }
    }

    /// Only checks internal consistency, but does not confirm valid issuance, authority or
    /// compliance with the journal plan.
    pub fn is_consistent(&self) -> bool {
        use crate::operation::bindings::BindingKind;
        use carosign_core::operation::plan::{BindingKind as StoredKind, PlanTransition};
        if self.effect.spec != carosign_engine::EffectSpec::SendMutatingCommand {
            return false;
        }
        match (&self.plan, &self.label) {
            (
                CommandPlan::Mutate {
                    change: MutationChange::Create { .. } | MutationChange::Replace { .. },
                    ..
                },
                Some(_),
            )
            | (
                CommandPlan::Mutate {
                    change: MutationChange::Removal { .. },
                    ..
                },
                None,
            ) => {}
            _ => return false,
        }
        if self.logical_plan.label() != self.label.as_ref() {
            return false;
        }
        if self.scope[..32] != self.logical_plan.subject().to_canonical()
            || self.scope[32..] != *self.logical_plan.authenticator().as_bytes()
        {
            return false;
        }
        let matches_old = |old: &crate::operation::bindings::EnrollmentBinding| {
            self.logical_plan.pre_bindings().iter().any(|snapshot| {
                snapshot.enrollment == old.enrollment
                    && matches!(
                        (snapshot.kind, old.kind),
                        (StoredKind::ProviderOpaque, BindingKind::ProviderOpaque)
                            | (StoredKind::ScopedVault, BindingKind::ScopedVault)
                    )
                    && snapshot.provider_ref_digest
                        == carosign_persistence::mutation_plan::sha256(&old.provider_ref)
                    && snapshot.format_id == old.format_id.as_deref().unwrap_or("")
                    && snapshot.format_version == old.format_version
            })
        };
        match &self.plan {
            CommandPlan::Mutate {
                mutation,
                command,
                change,
                ..
            } if *mutation == self.arm.mutation && *command == self.arm.command => {
                match (change, self.logical_plan.transition()) {
                    (
                        MutationChange::Removal {
                            enrollment,
                            binding,
                        },
                        PlanTransition::Remove { old },
                    ) => *enrollment == old && binding.enrollment == old && matches_old(binding),
                    (
                        MutationChange::Replace { planned_new, old },
                        PlanTransition::Replace {
                            old: expected_old,
                            planned_new: expected_new,
                        },
                    ) => {
                        *planned_new == expected_new
                            && old.enrollment == expected_old
                            && matches_old(old)
                    }
                    (
                        MutationChange::Create { planned_new },
                        PlanTransition::Create {
                            planned_new: expected_new,
                        },
                    ) => *planned_new == expected_new,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Verifies the ticket with all the bindings it contains; separately checks the full
    /// plan, freshness of the permit and the issuance of resources.
    pub fn matches_ticket(
        &self,
        ticket: &carosign_persistence::journal::ArmedMutationTicket,
    ) -> bool {
        self.is_consistent()
            && ticket.operation() == self.operation
            && ticket.mutation() == self.arm.mutation
            && ticket.command() == self.arm.command
            && ticket.worker() == self.arm.worker
            && ticket.epoch() == self.arm.epoch
            && ticket.scope() == &self.scope
            && ticket.effect() == Some(self.effect)
    }
}
