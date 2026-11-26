use super::MutationInput;
use super::MutationStepOutcome;
use crate::decision::TransitionError;
use crate::input::EffectSpec;
use crate::reconcile::ReconciliationVerdict;
use crate::transition::OperationPhase;
use crate::transition::OperationState;
use crate::transition::TerminalOutcome;

/// The next step of change.
///
/// ⚠️ The input that is not in the table
/// is`TransitionError::NotSpecified`](crate::decision::TransitionError::NotSpecified)The
/// table is short on purpose: everything that is invented outside of it must be visible.
pub fn step(
    state: &OperationState,
    input: MutationInput,
) -> Result<MutationStepOutcome, TransitionError> {
    use EffectSpec as E;
    use MutationInput as I;

    // Common for any phase: a break in storage and the loss of an worker.
    match input {
        I::StorageUnavailable => {
            // ⚠️ Uncertainty is recorded, you can't promise stability, the area is locked,
            // service is stopped.
            // If the metadata recording could have been completed without confirmation,
            // retain the need to read after the storage is returned.
            let effects = if state.common.phase == OperationPhase::Committing
                || state.common.phase == OperationPhase::Finalizing
                    && state
                        .mutation()
                        .is_some_and(|mutation| mutation.registry.is_some())
            {
                vec![E::QuarantineScope, E::RecoverMetadataCommit]
            } else {
                vec![E::QuarantineScope]
            };
            return Ok(MutationStepOutcome::new(
                OperationPhase::Reconciling,
                effects,
            ));
        }
        I::WorkerLost => {
            return if state.common.phase == OperationPhase::Reconciling
                && state
                    .common
                    .pending_effects
                    .iter()
                    .any(|effect| effect.spec == E::RecoverMetadataCommit)
            {
                // The unknown fixation has not yet been reread: a re-recording of the phase
                // could overwrite the confirmed outcome.
                Ok(MutationStepOutcome::new(
                    OperationPhase::Reconciling,
                    vec![E::QuarantineScope],
                ))
            } else if state.common.phase == OperationPhase::Finalizing
                && state
                    .mutation()
                    .is_some_and(|mutation| mutation.registry.is_some())
            {
                // Repeated notification should not obliterate the fixed outcome: hold the
                // requirement to reread the stable phase.
                Ok(MutationStepOutcome::new(
                    OperationPhase::Reconciling,
                    vec![E::QuarantineScope, E::RecoverMetadataCommit],
                ))
            } else if state.common.phase == OperationPhase::Committing {
                // The fixation could have passed before the loss of the worker.
                Ok(MutationStepOutcome::new(
                    OperationPhase::Reconciling,
                    vec![E::QuarantineScope, E::RecoverMetadataCommit],
                ))
            } else if state.common.phase.is_armed() {
                Ok(MutationStepOutcome::terminal(
                    OperationPhase::Reconciling,
                    vec![E::PersistNeedsReconciliation, E::QuarantineScope],
                    TerminalOutcome::NeedsReconciliation,
                ))
            } else {
                Ok(MutationStepOutcome::terminal(
                    OperationPhase::Finalizing,
                    vec![E::ReleaseResources],
                    TerminalOutcome::Failed,
                ))
            };
        }
        I::GrantRevoked => {
            return if state.common.phase == OperationPhase::Reconciling
                && state
                    .common
                    .pending_effects
                    .iter()
                    .any(|effect| effect.spec == E::RecoverMetadataCommit)
            {
                Ok(MutationStepOutcome::new(
                    OperationPhase::Reconciling,
                    vec![E::QuarantineScope],
                ))
            } else if state.common.phase == OperationPhase::Finalizing
                && state
                    .mutation()
                    .is_some_and(|mutation| mutation.registry.is_some())
            {
                // The recorded outcome is not overruled by the owner's recall. Cleaning is
                // already done; the right to deliver is verified separately.
                Ok(MutationStepOutcome::new(
                    OperationPhase::Finalizing,
                    Vec::new(),
                ))
            } else if state.common.phase == OperationPhase::Committing {
                Ok(MutationStepOutcome::new(
                    OperationPhase::Reconciling,
                    vec![
                        E::RequestCancel,
                        E::QuarantineScope,
                        E::RecoverMetadataCommit,
                    ],
                ))
            } else if matches!(
                state.common.phase,
                OperationPhase::WaitingLease | OperationPhase::Cancelling
            ) && state
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == E::AcquireResources)
            {
                // The request is still being fulfilled. The refusal or late issuance must
                // come before the record is recorded; the lease received must be removed.
                Ok(MutationStepOutcome::terminal(
                    OperationPhase::Cancelling,
                    Vec::new(),
                    TerminalOutcome::Cancelled,
                ))
            } else if state.common.phase.is_armed() {
                Ok(MutationStepOutcome::new(
                    OperationPhase::Reconciling,
                    vec![
                        E::RequestCancel,
                        E::PersistNeedsReconciliation,
                        E::QuarantineScope,
                    ],
                ))
            } else {
                Ok(MutationStepOutcome::terminal(
                    OperationPhase::Finalizing,
                    vec![E::ReleaseResources],
                    TerminalOutcome::Cancelled,
                ))
            };
        }
        _ => {}
    }

    match (state.common.phase, input) {
        // New + ManageGrantValid → Preparing.
        (OperationPhase::New, I::ManageGrantValid) => Ok(MutationStepOutcome::new(
            OperationPhase::Preparing,
            vec![E::PersistIntent],
        )),

        // Preparing + IntentDurable → WaitingLease.
        (OperationPhase::Preparing, I::IntentDurable) => Ok(MutationStepOutcome::new(
            OperationPhase::WaitingLease,
            vec![E::AcquireResources],
        )),
        (OperationPhase::Preparing, I::IntentFailed) => Ok(MutationStepOutcome::terminal(
            OperationPhase::Finalizing,
            vec![E::ReleaseResources],
            TerminalOutcome::Failed,
        )),

        // WaitingLease + LeaseGranted → Preflight.
        (OperationPhase::WaitingLease, I::LeaseGranted) => Ok(MutationStepOutcome::new(
            OperationPhase::ReadyToDispatch,
            vec![E::RevalidateGrant],
        )),
        (OperationPhase::WaitingLease, I::LeaseDenied) => Ok(MutationStepOutcome::terminal(
            OperationPhase::Finalizing,
            vec![E::RecordTerminal],
            TerminalOutcome::Failed,
        )),

        (OperationPhase::Cancelling, I::LeaseGranted) => Ok(MutationStepOutcome::new(
            OperationPhase::Finalizing,
            vec![E::ReleaseResources],
        )),
        (OperationPhase::Cancelling, I::LeaseDenied) => Ok(MutationStepOutcome::new(
            OperationPhase::Finalizing,
            vec![E::RecordTerminal],
        )),

        // Preflight + GrantValid Arming (sustained intent to send).
        (OperationPhase::ReadyToDispatch, I::GrantValid) => Ok(MutationStepOutcome::new(
            OperationPhase::Arming,
            vec![E::PersistDispatchArmed],
        )),

        // Arming + DispatchArmedDurable → InFlight (once, without repeating).
        (OperationPhase::Arming, I::DispatchArmedDurable) => Ok(MutationStepOutcome::new(
            OperationPhase::InFlight,
            vec![E::SendMutatingCommand],
        )),
        // Failure to fix may mean loss of confirmation of an already recorded
        // arm.Registration of the area and re-sending before reading the disk can not be
        // inferred from a single port failure.
        (OperationPhase::Arming, I::DispatchArmedFailed) => Ok(MutationStepOutcome::new(
            OperationPhase::Reconciling,
            vec![E::QuarantineScope],
        )),

        // InFlight + ProviderCompleted → ValidatingChange.
        (OperationPhase::InFlight, I::DispatchAccepted) => Ok(MutationStepOutcome::new(
            OperationPhase::InFlight,
            Vec::new(),
        )),
        (OperationPhase::InFlight, I::ProviderCompleted) => Ok(MutationStepOutcome::new(
            OperationPhase::ValidatingChange,
            vec![E::QueryProviderState],
        )),

        // The shipment has not been confirmed: the permission is withdrawn and... this is not
        // in the table – see README, B-02, “repeated prohibited”.
        (OperationPhase::InFlight, I::NotSent) => Ok(MutationStepOutcome::terminal(
            OperationPhase::Reconciling,
            vec![E::PersistNeedsReconciliation, E::QuarantineScope],
            TerminalOutcome::NeedsReconciliation,
        )),
        (OperationPhase::InFlight, I::OutcomeUnknown)
        | (OperationPhase::ValidatingChange, I::OutcomeUnknown) => Ok(MutationStepOutcome::new(
            OperationPhase::Reconciling,
            vec![E::PersistNeedsReconciliation, E::QuarantineScope],
        )),
        (OperationPhase::InFlight, I::ProviderMayHaveOccurred)
        | (OperationPhase::ValidatingChange, I::ProviderMayHaveOccurred) => {
            Ok(MutationStepOutcome::new(
                OperationPhase::Reconciling,
                vec![E::PersistNeedsReconciliation, E::QuarantineScope],
            ))
        }
        (OperationPhase::InFlight | OperationPhase::ValidatingChange, I::ProviderFailed) => {
            Ok(MutationStepOutcome::terminal(
                OperationPhase::Reconciling,
                vec![E::PersistNeedsReconciliation, E::QuarantineScope],
                TerminalOutcome::NeedsReconciliation,
            ))
        }

        (
            OperationPhase::InFlight | OperationPhase::ValidatingChange,
            I::TrustedEnrollmentRollback,
        ) if state.kind() == carosign_core::modality::OperationKind::Enroll => {
            let mut outcome = MutationStepOutcome::new(
                OperationPhase::Committing,
                vec![E::CommitNoChangeAndJournal],
            );
            outcome.registry = Some(crate::reconcile::RegistryAction::RecordNoChange);
            Ok(outcome)
        }
        (OperationPhase::ValidatingChange, I::TrustedProviderCompleted) => {
            let mut outcome = MutationStepOutcome::new(
                OperationPhase::Committing,
                vec![E::CommitDesiredRegistryAndJournal],
            );
            outcome.registry = Some(crate::reconcile::RegistryAction::ApplyDesiredState);
            Ok(outcome)
        }

        // ValidatingChange + ChangeConfirmed → Committing.
        (OperationPhase::ValidatingChange, I::NotSent) => Ok(MutationStepOutcome::terminal(
            OperationPhase::Reconciling,
            vec![E::PersistNeedsReconciliation, E::QuarantineScope],
            TerminalOutcome::NeedsReconciliation,
        )),
        // The worker's approval does not confirm the safety of the full initial state. Once
        // the shipment is authorized, an independent reconciliation is required.
        (OperationPhase::InFlight | OperationPhase::ValidatingChange, I::ProviderNoChange) => {
            Ok(MutationStepOutcome::terminal(
                OperationPhase::Reconciling,
                vec![E::PersistNeedsReconciliation, E::QuarantineScope],
                TerminalOutcome::NeedsReconciliation,
            ))
        }

        // Committing + MetadataDurable → Finalizing.
        (OperationPhase::Committing, I::IntentDurable) => Ok(MutationStepOutcome::new(
            OperationPhase::Finalizing,
            vec![E::ReleaseResources],
        )),
        // ⚠️ B-02: MetadataFailed After permission, the lock remains, the change could have
        // been on the hardware.
        (OperationPhase::Committing, I::IntentFailed) => Ok(MutationStepOutcome::new(
            OperationPhase::Reconciling,
            vec![E::QuarantineScope, E::RecoverMetadataCommit],
        )),

        (OperationPhase::Finalizing, I::OutcomeUnknown) => {
            let effects = if state
                .mutation()
                .is_some_and(|mutation| mutation.registry.is_some())
            {
                // An already confirmed record must be saved for restoration.
                vec![E::QuarantineScope, E::RecoverMetadataCommit]
            } else {
                vec![E::PersistNeedsReconciliation, E::QuarantineScope]
            };
            Ok(MutationStepOutcome::new(
                OperationPhase::Reconciling,
                effects,
            ))
        }

        // Reconciling + verdict (B-01).
        (
            OperationPhase::Reconciling | OperationPhase::ValidatingChange,
            I::Reconciled(verdict),
        ) => {
            if state
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == E::RecoverMetadataCommit)
            {
                // The hardware reconciliation itself doesn't tell you whether the registry record
                // passed before the confirmation was lost. First, you need to read the steady
                // phase; the re-recording could erase it.
                return Err(TransitionError::NotSpecified {
                    what: "ожидается восстановление неоднозначной записи метаданных",
                });
            }
            reconciliation_outcome(verdict)
        }

        // Finalizing + CleanupConfirmed The terminal and the recording of the result.
        (OperationPhase::Finalizing, I::IntentDurable) => {
            let outcome = match state.mutation().and_then(|mutation| mutation.registry) {
                Some(crate::reconcile::RegistryAction::ApplyDesiredState) => {
                    TerminalOutcome::MutationCommitted
                }
                Some(crate::reconcile::RegistryAction::RecordNoChange) => TerminalOutcome::NoChange,
                None => state.common.terminal.ok_or(TransitionError::NotSpecified {
                    what: "нет подтверждённого исхода изменения",
                })?,
            };
            Ok(MutationStepOutcome::terminal(
                OperationPhase::Terminal,
                vec![E::RecordTerminal],
                outcome,
            ))
        }

        _ => Err(TransitionError::NotSpecified {
            what: "unknown mutation transition",
        }),
    }
}

/// Reconciliation: Any successful person goes through `Committing` (B-01).
fn reconciliation_outcome(
    verdict: ReconciliationVerdict,
) -> Result<MutationStepOutcome, TransitionError> {
    match verdict {
        ReconciliationVerdict::DesiredStateConfirmed(_)
        | ReconciliationVerdict::PreStateConfirmed(_) => {
            let action = verdict
                .registry_action()
                .ok_or(TransitionError::NotSpecified {
                    what: "успешный итог сверки без действия с реестром",
                })?;
            let mut outcome = MutationStepOutcome::new(
                OperationPhase::Committing,
                vec![match action {
                    crate::reconcile::RegistryAction::ApplyDesiredState => {
                        EffectSpec::CommitDesiredRegistryAndJournal
                    }
                    crate::reconcile::RegistryAction::RecordNoChange => {
                        EffectSpec::CommitNoChangeAndJournal
                    }
                }],
            );
            outcome.registry = Some(action);
            Ok(outcome)
        }
        ReconciliationVerdict::Indeterminate(_) => Ok(MutationStepOutcome::terminal(
            OperationPhase::Reconciling,
            vec![
                EffectSpec::PersistNeedsReconciliation,
                EffectSpec::QuarantineScope,
            ],
            TerminalOutcome::NeedsReconciliation,
        )),
    }
}

#[path = "step.tests.rs"]
#[cfg(test)]
mod tests;
