use super::DispatchPlans;
use super::Execution;
use crate::ports::CommandPlan;
use crate::ports::DispatchOutcome;
use crate::ports::DurableMarker;
use crate::ports::MutationPort;
use crate::ports::ProviderExecutionPort;
use crate::ports::ResourceLease;
use carosign_engine::EffectReceipt;
use carosign_engine::EffectSpec;
use carosign_engine::PlannedEffect;
use carosign_provider_sdk::cancellation::CancelState;
use core::fmt;

/// Performer of the effects of one operation.
pub struct EffectRunner<'a> {
    provider: &'a dyn ProviderExecutionPort,
    mutation: &'a dyn MutationPort,
    plans: DispatchPlans,
    cancel: CancelState,
    operation: Option<carosign_core::ids::OperationId>,
    scoped_cancel: Option<(&'a ResourceLease, &'a dyn carosign_core::ids::IdSource)>,
    scoped_cancel_started: bool,
}

impl<'a> fmt::Debug for EffectRunner<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EffectRunner(..)")
    }
}

impl<'a> EffectRunner<'a> {
    /// It collects the performer over ports and plans.
    pub fn new(
        provider: &'a dyn ProviderExecutionPort,
        mutation: &'a dyn MutationPort,
        plans: DispatchPlans,
    ) -> Self {
        Self {
            provider,
            mutation,
            plans,
            cancel: CancelState::Idle,
            operation: None,
            scoped_cancel: None,
            scoped_cancel_started: false,
        }
    }

    /// Transmits the withheld issue and the source of the new ID for cancellation.
    /// Verification of the authority and relevance of extradition is carried out by its
    /// broker.
    pub fn with_scoped_cancel(
        mut self,
        lease: &'a ResourceLease,
        ids: &'a dyn carosign_core::ids::IdSource,
    ) -> Self {
        self.scoped_cancel = Some((lease, ids));
        self
    }

    /// Cancellation status in the performer.
    pub fn cancel_state(&self) -> CancelState {
        self.cancel
    }

    /// It does one thing.
    ///
    /// ⚠️ The plan itself does not give the right to change: you need a stable one-time
    /// ticket, which the current interface does not yet transmit.
    pub async fn execute(
        &mut self,
        effect: PlannedEffect,
        operation: carosign_core::ids::OperationId,
    ) -> Execution {
        use EffectSpec as E;

        match self.operation {
            Some(bound) if bound != operation => return Execution::NoDispatchPlan,
            None => self.operation = Some(operation),
            Some(_) => {}
        }

        match effect.spec {
            E::PersistIntent => self.durable(
                self.mutation.persist_intent(operation).await,
                EffectReceipt::IntentDurable,
            ),
            // The old port returns a copyable feature without a tether and a one-time ticket,
            // and you can't arm a shipment with it.
            E::PersistDispatchArmed => Execution::NoDispatchPlan,
            E::AcquireResources => Execution::Noop,
            E::RevalidateGrant | E::VerifyEvidence | E::RevalidateOwner => Execution::Noop,
            E::SendVerify => {
                if self.cancel != CancelState::Idle {
                    return Execution::NoDispatchPlan;
                }
                let Some(plan @ CommandPlan::Verify { .. }) = self.plans.verify.take() else {
                    return Execution::NoDispatchPlan;
                };
                match self.provider.dispatch(operation, plan).await {
                    DispatchOutcome::Accepted => Execution::Receipt(EffectReceipt::SendAccepted),
                    DispatchOutcome::NotSent => Execution::Receipt(EffectReceipt::DispatchNotSent),
                    DispatchOutcome::Unknown => {
                        Execution::Receipt(EffectReceipt::DispatchOutcomeUnknown)
                    }
                    DispatchOutcome::Failed => {
                        Execution::Receipt(EffectReceipt::DispatchOutcomeUnknown)
                    }
                }
            }
            E::SendMutatingCommand => {
                // ARCHITECTURE_BLOCKER: Without a ticket and its context, there is no
                // authorized shipping plan (document 34).
                Execution::NoDispatchPlan
            }
            // Without a proven ability to securely request a specific profile, the hardware
            // status remains unknown.
            E::QueryProviderState => Execution::Receipt(EffectReceipt::StateUnknown),
            // Rereading requires a verified log and registry image; before connecting such a
            // port, we do not declare the result of the reconciliation.
            E::RecoverMetadataCommit => Execution::Noop,
            // An operating port does not accept a verified separate outcome.
            E::CommitDesiredRegistryAndJournal | E::CommitNoChangeAndJournal => Execution::Noop,
            E::PersistNeedsReconciliation => self.durable(
                self.mutation.mark_uncertain(operation).await,
                EffectReceipt::ReconciliationPersisted,
            ),
            E::QuarantineScope => Execution::Noop,
            E::RequestCancel => {
                self.cancel = self.cancel.request();
                self.plans.verify = None;
                if let Some((lease, ids)) = self.scoped_cancel {
                    if self.scoped_cancel_started || lease.owner().operation != operation {
                        return Execution::Noop;
                    }
                    let Ok(command) = ids.new_command_id() else {
                        return Execution::Noop;
                    };
                    // Once a call has started, an unknown outcome does not allow a repeat.
                    self.scoped_cancel_started = true;
                    match self.provider.request_cancel_scoped(lease, command).await {
                        DispatchOutcome::Accepted => {
                            Execution::Receipt(EffectReceipt::CancelDispatched)
                        }
                        outcome => Execution::CancelUnconfirmed(outcome),
                    }
                } else {
                    if self.provider.request_cancel(operation).await {
                        Execution::Receipt(EffectReceipt::CancelDispatched)
                    } else {
                        Execution::Noop
                    }
                }
            }
            E::SuppressResult => Execution::Noop,
            E::ReleaseResources => Execution::Noop,
            E::RecordTerminal => self.durable(
                self.mutation.finalize(operation).await,
                EffectReceipt::MetadataDurable,
            ),
            E::DeliverOutcome => Execution::Noop,
            E::DestroyEvidence => Execution::Noop,
            E::PublishProgress => Execution::Noop,
        }
    }

    /// Call preparation plan, if any (document 31).
    pub fn prepare_challenge_plan(&self) -> Option<CommandPlan> {
        self.plans.prepare_challenge.clone()
    }

    fn durable(&self, marker: DurableMarker, receipt: EffectReceipt) -> Execution {
        if marker.durable {
            Execution::Receipt(receipt)
        } else {
            Execution::StorageUnavailable
        }
    }

    /// Confirmation for an effect that has just been performed.
    ///
    /// ⚠️ Observation belongs to the type of effect, not to the performer.
    pub fn receipt_for(&self, spec: EffectSpec, execution: &Execution) -> Option<EffectReceipt> {
        match execution {
            Execution::Receipt(_) if spec == EffectSpec::PersistDispatchArmed => None,
            Execution::Receipt(receipt) if receipt.confirms(spec) => Some(*receipt),
            Execution::Receipt(_) => None,
            Execution::StorageUnavailable
            | Execution::NoDispatchPlan
            | Execution::Noop
            | Execution::CancelUnconfirmed(_) => None,
        }
    }
}
