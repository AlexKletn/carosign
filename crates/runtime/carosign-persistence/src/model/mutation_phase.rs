use core::fmt;

///
/// ⚠️ The names are the same as document 10, but the phases of the machine (document 04) are
/// different. Here's the log: what do we know about the fate of the change on the disk.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum MutationPhase {
    /// Intention is written; nothing went to hardware.
    Prepared,
    /// The permission to send is written down; the hardware ** could change.
    DispatchArmed,
    /// The provider answered.
    ProviderObserved,
    /// The register is sealed.
    RegistryCommitted,
    /// It's closed.
    Completed,
    /// Could have happened; you need a reconciliation.
    NeedsReconciliation,
    /// The result NoChange It is recorded, but cleaning and completion have not yet been
    /// confirmed.
    NoChange,
}

impl fmt::Debug for MutationPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Prepared => "Prepared",
            Self::DispatchArmed => "DispatchArmed",
            Self::ProviderObserved => "ProviderObserved",
            Self::RegistryCommitted => "RegistryCommitted",
            Self::Completed => "Completed",
            Self::NeedsReconciliation => "NeedsReconciliation",
            Self::NoChange => "NoChange",
        })
    }
}

impl MutationPhase {
    /// Whether the phase is closed permanently.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed)
    }

    /// Whether sending a change command ** is allowed only ** from this phase.
    ///
    /// ⚠️ Only. `DispatchArmed`: `Prepared` (not yet), no repeats.
    pub fn allows_dispatch(self) -> bool {
        matches!(self, Self::DispatchArmed)
    }

    /// Could the command have reached the hardware?
    ///
    /// ⚠️ `DispatchArmed` Included: Between recording and sending, the cliff is no longer
    /// safe on the disc, and the recovery should consider the hardware as possibly altered.
    pub fn may_have_touched_hardware(self) -> bool {
        matches!(
            self,
            Self::DispatchArmed
                | Self::ProviderObserved
                | Self::RegistryCommitted
                | Self::NeedsReconciliation
                | Self::NoChange
        )
    }
}

#[cfg(test)]
#[path = "mutation_phase.tests.rs"]
mod tests;
