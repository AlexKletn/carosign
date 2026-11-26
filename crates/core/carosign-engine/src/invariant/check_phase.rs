use crate::{decision::TransitionError, transition::OperationPhase};

/// The phase is the same machine as the type of operation (C-03).
pub fn check_phase(
    kind: carosign_core::modality::OperationKind,
    phase: OperationPhase,
) -> Result<(), TransitionError> {
    use crate::transition::PhaseFamily;
    use carosign_core::modality::OperationKind as K;

    let expected = match kind {
        K::Verify => PhaseFamily::Verification,
        K::Enroll | K::Remove => PhaseFamily::Mutation,
    };
    // ⚠️ The general phases are suitable for both machines; the machine-specific phases are
    // suitable only for their own.
    if phase.family() == PhaseFamily::Shared {
        return Ok(());
    }
    if phase.family() != expected {
        return Err(TransitionError::InvalidInputForOperation {
            kind,
            belongs_to: if expected == PhaseFamily::Mutation {
                carosign_core::modality::OperationKind::Verify
            } else {
                carosign_core::modality::OperationKind::Enroll
            },
        });
    }
    Ok(())
}
