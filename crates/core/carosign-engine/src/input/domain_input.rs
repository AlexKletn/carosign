use super::EffectReceipt;
use super::PlannedEffect;
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::EnrollmentRef;
use carosign_core::ids::CommandId;
use carosign_core::ids::EvidenceId;
use carosign_core::ids::OperationId;

/// That the engine gets in.
///
/// ⚠️ Each entrance has already been checked by operation number, era and worker.
/// The engine **rejects ** an inconsistent input, rather than adapting to it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DomainInput {
    /// Authorities confirmed for the operation.
    Authorized,
    /// Linking the prepared dispatch of the inspection to the worker and the command.
    /// The source and authority are verified by the session owner prior to this entry.
    BindVerificationDispatch {
        /// Expecting send-off effect.
        effect: PlannedEffect,
        /// Extradition to which the worker is associated.
        lease: carosign_core::ids::LeaseId,
        /// The worker and the age of the tied device.
        worker: crate::transition::WorkerRef,
        /// Identifier of the assigned command platform.
        command: CommandId,
    },
    /// A single binding of a prepared modifying shipment. This input does not confirm a
    /// stable record and does not allow a call to the provider.
    BindMutationDispatch {
        /// A pending record of sustainable weapons.
        effect: PlannedEffect,
        /// Withholding of resources.
        lease: carosign_core::ids::LeaseId,
        /// The worker and the age of the device.
        worker: crate::transition::WorkerRef,
        /// Identifier of the future changing command.
        command: CommandId,
    },
    /// The powers have been withdrawn.
    OwnerRevoked,
    /// Deadline's here.
    DeadlineElapsed,
    /// The owner passed out.
    OwnerDetached,
    /// Request for cancellation.
    CancelRequested,
    /// The worker's missing.
    WorkerLost,
    /// The era of the device has changed.
    DeviceEpochChanged(DeviceEpoch),
    /// Confirmation of effect from the queue.
    Receipt {
        effect: PlannedEffect,
        /// Surveillance.
        receipt: EffectReceipt,
    },
    /// Sending the **check* command is accepted.
    ///
    /// ⚠️ A separate species, not `Receipt { SendAccepted }`There is no stable barrier to
    /// verification. `DispatchArmed`It must not be allowed to be shipped in a change machine
    /// under any circumstances (C-03).
    VerificationSent,
    /// Verification: Confirmed that command **not* passed.
    VerificationNotSent,
    /// Check: It is not known whether the command has been able to start the execution.
    VerificationOutcomeUnknown,
    /// Provider matched; reference to proof.
    ProviderMatched(EvidenceId),
    /// The provider didn't match.
    ProviderNotMatched,
    /// I couldn't take it off.
    ProviderNoUsableSample,
    ProviderCancelled,
    /// The provider has completed the change and **repeated the planned registration**.
    ///
    /// ⚠️ Document 34: upon completion of the registration, the provider must return the
    /// **exactly* registration that was planned. The absence or discrepancy of the echo is a
    /// violation of the protocol, not "nothing changed."
    ProviderCompleted {
        /// Who he's confirming.
        echoed: EnrollmentRef,
    },
    /// The Provider has completed the change of **no** scheduled registration in the
    /// response.
    ///
    /// ⚠️ Separate entry, not “completed without details”: according to document 34, this is
    /// a violation of the protocol, and it must sound its own word.
    ProviderCompletedWithoutEcho,
    /// The provider claims "nothing has changed."
    ProviderNoChange,
    TrustedEnrollmentRollback,
    /// The provider claims "could have happened."
    ProviderMayHaveOccurred,
    /// The provider said no.
    ProviderFailed,
    /// Outcome of independent reconciliation (decision B-01).
    Reconciliation(crate::reconcile::ReconciliationVerdict),
    /// Warehouse not available: "Reconciliation required" is not confirmed (decision B-02).
    StorageUnavailable,
    /// The course from the provider (can be glued, the phase does not affect).
    Progress,
    /// Service: Operation number to verify that the entrance is addressed to us.
    _Operation(OperationId),
}

impl DomainInput {
    /// Whether the entrance refers to sending a verification command.
    pub fn is_verification_send_receipt(&self) -> bool {
        matches!(
            self,
            Self::VerificationSent
                | Self::VerificationNotSent
                | Self::VerificationOutcomeUnknown
                | Self::Receipt {
                    receipt: EffectReceipt::SendAccepted
                        | EffectReceipt::DispatchNotSent
                        | EffectReceipt::DispatchOutcomeUnknown,
                    ..
                }
        )
    }

    /// Whether the entrance is a changing shipment.
    pub fn is_mutation_send_receipt(&self) -> bool {
        matches!(
            self,
            Self::Receipt {
                receipt: EffectReceipt::MutationAccepted
                    | EffectReceipt::MutationNotSent
                    | EffectReceipt::MutationOutcomeUnknown,
                ..
            }
        )
    }
}
