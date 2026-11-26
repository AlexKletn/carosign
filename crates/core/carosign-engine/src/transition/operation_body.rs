use super::MutationState;
use super::VerificationState;
use carosign_core::modality::OperationKind;
use carosign_core::operation::mutation::EnrollRequest;
use carosign_core::operation::mutation::RemoveRequest;
use core::fmt;

/// The state of the operation is the kind.
///
/// ⚠️ This is the only place where the type of operation is fixed: any sign of the species is
/// derived from here, there is no second copy (C-03).
#[derive(Clone, PartialEq, Eq)]
pub enum OperationBody {
    /// Testing. `VerificationState` can't carry `MutationState`.
    Verification(VerificationState),
    /// Registration together with the request (replacement - in `request.replace`).
    Enrollment {
        /// For asking.
        request: EnrollRequest,
        /// A state of change.
        state: MutationState,
    },
    /// Removal along with request (generation - in) `request.enrollment`).
    Removal {
        /// For asking.
        request: RemoveRequest,
        /// A state of change.
        state: MutationState,
    },
}

impl fmt::Debug for OperationBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Verification(_) => "Verification(..)",
            Self::Enrollment { .. } => "Enrollment { .. }",
            Self::Removal { .. } => "Removal { .. }",
        })
    }
}

impl OperationBody {
    /// The type of operation is **output **, not stored.
    pub fn kind(&self) -> OperationKind {
        match self {
            Self::Verification(_) => OperationKind::Verify,
            Self::Enrollment { .. } => OperationKind::Enroll,
            Self::Removal { .. } => OperationKind::Remove,
        }
    }

    /// The body of change, if it is a change.
    pub fn mutation(&self) -> Option<&MutationState> {
        match self {
            Self::Verification(_) => None,
            Self::Enrollment { state, .. } | Self::Removal { state, .. } => Some(state),
        }
    }

    /// Body changes for editing.
    pub fn mutation_mut(&mut self) -> Option<&mut MutationState> {
        match self {
            Self::Verification(_) => None,
            Self::Enrollment { state, .. } | Self::Removal { state, .. } => Some(state),
        }
    }

    /// Body check, if it's a check-up.
    pub fn verification(&self) -> Option<&VerificationState> {
        match self {
            Self::Verification(state) => Some(state),
            _ => None,
        }
    }
}
